//! **`red hola <ip>`: TCP de verdad contra un servidor real** (G5, 2026-09-18).
//!
//! [consumo] NADA      trabaja solo mientras hay un saludo en marcha
//!
//! La maquina de estados de TCP vive en `bmo-pila` (`tcp/mod.rs`) y esta
//! probada en el anfitrion con dos pilas y un cable de mentira. Esto es la
//! otra mitad: **la misma pila sobre el buzon de la tarjeta**, contra algo que
//! no escribimos nosotros. El servidor elegido es la ANTENA (`antena.py`,
//! puerto 7117), porque es el que NAVEGAR va a necesitar y porque habla en
//! lineas: se le manda `HOLA ANTENA/1` y contesta `HOLA ANTENA/1 <nombre>`.
//!
//! ```text
//!    RESOLVIENDO   ARP del salto (la antena, o el router)
//!    CONECTANDO    SYN -> SYN+ACK -> ACK, con los tiempos de la pila
//!    HABLANDO      HOLA ANTENA/1, y la linea de vuelta
//!    CERRANDO      FIN -> FIN+ACK -> ACK: el cierre limpio
//! ```
//!
//! ** El reloj lo pone el escritorio: el buzon se vacia cada vuelta
//! (`cada_vuelta`, 16 ms) y los plazos se miran cada cuarto de segundo
//! (`latir`). El RTO de la pila es 1 s, asi que un RTT de 16-32 ms no le
//! cuesta ni un reintento. Lo que no hace: mas de una conexion a la vez.
//! Es una sonda, no el ANTENISTA (N3): el ANTENISTA es un servicio en Rust
//! que hara exactamente esto por otros, y esta sonda es lo que lo prueba.

use core::ptr::addr_of_mut;

use bmo_pila::ether::Mac;
use bmo_pila::ipv4::{self, Ip};
use bmo_pila::nodo::CARGA;
use bmo_pila::tcp::{Cierre, Estado, Tcp};
use bmo_userland as bmo;

use crate::scene::output::{Output, INK_ERR, INK_GOOD, INK_PLAIN};

const QUIETO: u8 = 0;
const RESOLVIENDO: u8 = 1;
const CONECTANDO: u8 = 2;
const HABLANDO: u8 = 3;
const CERRANDO: u8 = 4;

/// El puerto de ANTENA/1.
pub(crate) const PUERTO_ANTENA: u16 = 7117;
const SALUDO: &[u8] = b"HOLA ANTENA/1\n";
const ESPERA_ARP_MS: u64 = 6_000;
const ESPERA_CONEXION_MS: u64 = 10_000;
const ESPERA_RESPUESTA_MS: u64 = 5_000;
const ESPERA_CIERRE_MS: u64 = 10_000;
const LINEA_MAX: usize = 256;

struct Saludo {
    fase: u8,
    destino: Ip,
    salto: Ip,
    mac: Mac,
    mi_ip: Ip,
    asa: usize,
    inicio_ms: u64,
    fase_ms: u64,
    dejadas: u64,
    recibidas: u64,
    linea: [u8; LINEA_MAX],
    largo: usize,
    completa: bool,
}

const fn quieto() -> Saludo {
    Saludo {
        fase: QUIETO,
        destino: [0; 4],
        salto: [0; 4],
        mac: [0; 6],
        mi_ip: [0; 4],
        asa: 0,
        inicio_ms: 0,
        fase_ms: 0,
        dejadas: 0,
        recibidas: 0,
        linea: [0; LINEA_MAX],
        largo: 0,
        completa: false,
    }
}

static mut SALUDO_EN_MARCHA: Saludo = quieto();
/// La pila TCP: ocho conexiones con sus buferes (~130 KB). Vive aqui y no
/// en el nodo, porque el nodo es de ping y dns tambien y no sabe de asas.
///
/// [!] `MaybeUninit` y no `Option<Tcp>`: un `None` de `Option<Tcp>` no son
/// ceros (el tag va en un hueco de la struct), y un `static` que no es todo
/// ceros va a `.data` -- **130 KB dentro del `.bex`**, que es lo que paso
/// la primera vez (`d.bex` +179 KB). Sin inicializar va a `.bss`, que no
/// pesa en el fichero, y `TCP_LISTA` dice si ya se construyo.
static mut TCP: core::mem::MaybeUninit<Tcp> = core::mem::MaybeUninit::uninit();
static mut TCP_LISTA: bool = false;

fn saludo() -> &'static mut Saludo {
    unsafe { &mut *addr_of_mut!(SALUDO_EN_MARCHA) }
}

fn tcp() -> Option<&'static mut Tcp> {
    unsafe {
        if !TCP_LISTA {
            return None;
        }
        Some(&mut *(*addr_of_mut!(TCP)).as_mut_ptr())
    }
}

fn tcp_nueva(secreto: [u8; 32]) {
    unsafe {
        (*addr_of_mut!(TCP)).write(Tcp::nueva(secreto));
        TCP_LISTA = true;
    }
}

/// Hay un saludo en marcha? Lo preguntan `red_nodo::oir` y `cada_vuelta`.
pub(crate) fn activa() -> bool {
    saludo().fase != QUIETO
}

/// **`red hola <ip>`.**
pub(crate) fn hola(s: &mut Output, resto: &[u8]) {
    let Some(ip) = crate::commands::red_pase::ipv4(resto) else {
        s.text(b"  uso: red hola 192.168.0.103   (la antena, puerto 7117)\n");
        return;
    };
    if activa() {
        s.text(b"  ya hay un saludo en marcha: sale aqui abajo segun avanza\n");
        return;
    }
    let destino = ip.to_be_bytes();
    let Some(salto) = crate::commands::red_nodo::preparar(s, destino) else { return };
    let Some(k) = crate::commands::red_ip::concesion() else { return };
    // El secreto del ISN (RFC 6528): el reloj y la MAC, que nadie de fuera
    // ve. Con el mismo secreto, el mismo numero; sin el, imposible de
    // adivinar -- y cambia en cada saludo.
    let mut secreto = [0u8; 32];
    let c = bmo::ciclos().to_le_bytes();
    let m = bmo::info(bmo::INFO_NET_MAC).to_le_bytes();
    for i in 0..32 {
        secreto[i] = c[i % 8] ^ m[i % 8].rotate_left((i % 8) as u32) ^ (i as u8).wrapping_mul(29);
    }
    tcp_nueva(secreto);
    let t = saludo();
    *t = quieto();
    t.fase = RESOLVIENDO;
    t.destino = destino;
    t.salto = salto;
    t.mi_ip = k.ip;
    t.inicio_ms = ahora();
    t.fase_ms = t.inicio_ms;
    s.text(b"  [hola] quien tiene ");
    ip_texto(s, salto);
    s.text(b"? (ARP)\n");
}

fn ahora() -> u64 {
    crate::commands::red_nodo::ahora_ms()
}

/// **Un segmento TCP que llego** (lo trae `red_nodo::oir` desde el nodo).
pub(crate) fn segmento(origen: Ip, destino: Ip, bytes: &[u8]) {
    let t = saludo();
    if t.fase == QUIETO || origen != t.destino {
        return;
    }
    let Some(p) = tcp() else { return };
    t.recibidas += 1;
    let _ = p.entrada(origen, destino, bytes, ahora());
    // Lo que haya llegado de datos se recoge en el acto, hasta el salto de
    // linea: la antena contesta UNA linea.
    if t.fase == HABLANDO && !t.completa {
        let mut buf = [0u8; 128];
        while let Ok(n) = p.recibir(t.asa, &mut buf) {
            if n == 0 {
                break;
            }
            for &b in &buf[..n] {
                if b == b'\n' {
                    t.completa = true;
                } else if !t.completa && t.largo < LINEA_MAX {
                    t.linea[t.largo] = b;
                    t.largo += 1;
                }
            }
        }
    }
}

/// **Lo que la pila quiera mandar, al cable.** Cada vuelta del escritorio
/// mientras hay saludo: asi el ACK del SYN+ACK sale en la vuelta siguiente y
/// no un cuarto de segundo despues.
pub(crate) fn cada_vuelta() {
    let t = saludo();
    if t.fase == QUIETO {
        return;
    }
    crate::commands::red_pase::drenar();
    bombear(t);
}

fn bombear(t: &mut Saludo) {
    if t.fase == RESOLVIENDO {
        return;
    }
    let Some(p) = tcp() else { return };
    let Some(n) = crate::commands::red_nodo::nodo().as_mut() else { return };
    let ahora = ahora();
    let mut buf = [0u8; 1514];
    // Como mucho ocho por vuelta: el grifo da 50 por segundo.
    for _ in 0..8 {
        let Some((_, su_ip, largo)) = p.salida(ahora, &mut buf[CARGA..]) else { break };
        let Ok(total) = n.envolver(&mut buf, t.mac, su_ip, ipv4::TCP, largo) else { break };
        if bmo::red::enviar(&buf[..total]).is_ok() {
            t.dejadas += 1;
        }
    }
}

/// **Un cuarto de segundo.**
pub(crate) fn latir(s: &mut Output) {
    let t = saludo();
    if t.fase == QUIETO {
        return;
    }
    if !bmo::red::pase_abierto() {
        s.with_ink(INK_ERR);
        s.text(b"  [hola] el pase se cerro antes de terminar\n");
        s.with_ink(INK_PLAIN);
        terminar();
        return;
    }
    let Some(n) = crate::commands::red_nodo::nodo().as_mut() else {
        terminar();
        return;
    };
    let ahora = ahora();
    match t.fase {
        RESOLVIENDO => {
            if let Some(mac) = n.arp.buscar(t.salto, ahora) {
                t.mac = mac;
                let Some(p) = tcp() else { terminar(); return };
                match p.conectar(t.mi_ip, t.destino, PUERTO_ANTENA, ahora) {
                    Ok(asa) => {
                        t.asa = asa;
                        t.fase = CONECTANDO;
                        t.fase_ms = ahora;
                        s.text(b"  [hola] SYN a ");
                        ip_texto(s, t.destino);
                        s.text(b":7117\n");
                        bombear(t);
                    }
                    Err(r) => {
                        s.with_ink(INK_ERR);
                        s.text(b"  [hola] la pila no abre la conexion: ");
                        s.text(r.texto().as_bytes());
                        s.byte(b'\n');
                        s.with_ink(INK_PLAIN);
                        terminar();
                    }
                }
                return;
            }
            let mut buf = [0u8; 1514];
            if let Ok(Some(largo)) = n.preguntar(t.salto, ahora, &mut buf) {
                if bmo::red::enviar(&buf[..largo]).is_ok() {
                    t.dejadas += 1;
                }
            }
            if ahora.saturating_sub(t.inicio_ms) >= ESPERA_ARP_MS {
                s.with_ink(INK_ERR);
                s.text(b"  [hola] nadie contesto por ARP en 6 s\n");
                s.with_ink(INK_PLAIN);
                terminar();
            }
        }
        CONECTANDO => {
            let Some(p) = tcp() else { terminar(); return };
            match p.estado(t.asa) {
                Estado::Establecida => {
                    s.with_ink(INK_GOOD);
                    s.text(b"  [hola] CONECTADA en ");
                    s.dec(ahora.saturating_sub(t.fase_ms));
                    s.text(b" ms: los tres pasos\n");
                    s.with_ink(INK_PLAIN);
                    let _ = p.enviar(t.asa, SALUDO);
                    t.fase = HABLANDO;
                    t.fase_ms = ahora;
                    bombear(t);
                }
                Estado::Cerrada(c) => {
                    s.with_ink(INK_ERR);
                    s.text(b"  [hola] NO conecto: ");
                    s.text(cierre_texto(c));
                    s.byte(b'\n');
                    s.with_ink(INK_PLAIN);
                    resumen(s, t);
                    terminar();
                }
                _ => {
                    if ahora.saturating_sub(t.fase_ms) >= ESPERA_CONEXION_MS {
                        s.with_ink(INK_ERR);
                        s.text(b"  [hola] sin SYN+ACK en 10 s: nadie escucha en 7117, o no llega\n");
                        s.with_ink(INK_PLAIN);
                        let _ = p.abortar(t.asa);
                        bombear(t);
                        resumen(s, t);
                        terminar();
                    }
                }
            }
        }
        HABLANDO => {
            let Some(p) = tcp() else { terminar(); return };
            if t.completa {
                s.with_ink(INK_GOOD);
                s.text(b"  [hola] la antena dice: ");
                s.text(&t.linea[..t.largo]);
                s.byte(b'\n');
                s.with_ink(INK_PLAIN);
                let _ = p.cerrar(t.asa);
                t.fase = CERRANDO;
                t.fase_ms = ahora;
                bombear(t);
                return;
            }
            if let Estado::Cerrada(c) = p.estado(t.asa) {
                s.with_ink(INK_ERR);
                s.text(b"  [hola] la conexion se cerro sin contestar: ");
                s.text(cierre_texto(c));
                s.byte(b'\n');
                s.with_ink(INK_PLAIN);
                resumen(s, t);
                terminar();
                return;
            }
            if ahora.saturating_sub(t.fase_ms) >= ESPERA_RESPUESTA_MS {
                s.with_ink(INK_ERR);
                s.text(b"  [hola] conectada, pero sin respuesta al HOLA en 5 s\n");
                s.with_ink(INK_PLAIN);
                let _ = p.abortar(t.asa);
                bombear(t);
                resumen(s, t);
                terminar();
            }
        }
        CERRANDO => {
            let Some(p) = tcp() else { terminar(); return };
            match p.estado(t.asa) {
                // TIME-WAIT ya es el cierre hecho: los dos FIN cruzaron. Los
                // 60 s de espera son de la pila, no del dueno.
                Estado::TiempoEspera | Estado::Cerrada(Cierre::Normal) => {
                    s.with_ink(INK_GOOD);
                    s.text(b"  [hola] CIERRE LIMPIO en ");
                    s.dec(ahora.saturating_sub(t.fase_ms));
                    s.text(b" ms\n");
                    s.with_ink(INK_PLAIN);
                    resumen(s, t);
                    terminar();
                }
                Estado::Cerrada(c) => {
                    s.text(b"  [hola] cerrada: ");
                    s.text(cierre_texto(c));
                    s.byte(b'\n');
                    resumen(s, t);
                    terminar();
                }
                _ => {
                    if ahora.saturating_sub(t.fase_ms) >= ESPERA_CIERRE_MS {
                        s.with_ink(INK_ERR);
                        s.text(b"  [hola] el otro lado no cerro en 10 s: se aborta\n");
                        s.with_ink(INK_PLAIN);
                        let _ = p.abortar(t.asa);
                        bombear(t);
                        resumen(s, t);
                        terminar();
                    }
                }
            }
        }
        _ => terminar(),
    }
}

fn resumen(s: &mut Output, t: &Saludo) {
    s.text(b"  [hola] tramas: dejadas en el buzon ");
    s.dec(t.dejadas);
    s.text(b", segmentos de vuelta ");
    s.dec(t.recibidas);
    s.text(b", en ");
    s.dec(ahora().saturating_sub(t.inicio_ms));
    s.text(b" ms\n");
}

fn cierre_texto(c: Cierre) -> &'static [u8] {
    match c {
        Cierre::Normal => b"normal",
        Cierre::Reset => b"el otro lado mando RST (nadie escucha, o no quiere)",
        Cierre::Abortada => b"abortada aqui",
        Cierre::Agotada => b"se agotaron los reintentos",
    }
}

fn terminar() {
    let t = saludo();
    if let Some(p) = tcp() {
        let _ = p.soltar(t.asa);
    }
    t.fase = QUIETO;
    bmo::red::cerrar();
}

fn ip_texto(s: &mut Output, ip: Ip) {
    for (k, b) in ip.iter().enumerate() {
        s.dec(*b as u64);
        if k < 3 {
            s.byte(b'.');
        }
    }
}
