//! **`red hola <ip>` y `red pagina <ip> <url>`: TCP de verdad contra la
//! antena** (G5 y N3a, 2026-09-18).
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
//!
//! ** Y `red pagina <ip> <url>` es el ANTENISTA DE BOLSILLO (N3a): la misma
//! conexion, y despues del saludo pide `PAGINA <url>`; la respuesta pasa por
//! `bmo_antena::Conversacion` (el protocolo) y por su `lamina::Lector` (el
//! juez), linea a linea, y si la lamina es entera y valida va al bloque del
//! ANTENISTA (`antenista.rs`, N3: se OFRECE a NAVEGAR al lanzarla, sin el
//! disco en medio) y ademas a `datos/pagina.lam`, que es lo que NAVEGAR
//! pinta si arranca sin oferta. La cadena entera es antena -> TCP -> juez ->
//! bloque -> prestamo -> NAVEGAR, y el disco es el camino de reserva.

use core::ptr::addr_of_mut;

use bmo_antena::{Conversacion, Fase, Lineas, Pedido, Respuesta};
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
const ESPERA_ARP_MS: u64 = 6_000;
const ESPERA_CONEXION_MS: u64 = 10_000;
const ESPERA_RESPUESTA_MS: u64 = 5_000;
/// Una pagina la antena la NAVEGA: 2,5 s medidos en el HONOR sin imagenes,
/// 7 con. Veinte segundos es el doble del peor caso visto.
const ESPERA_PAGINA_MS: u64 = 20_000;
const ESPERA_CIERRE_MS: u64 = 10_000;
const LINEA_MAX: usize = 256;
/// Lo que cabe de lamina: la de Wikipedia mide 90 KB (L3c).
const LAMINA_MAX: usize = 256 * 1024;
/// Donde queda lo que trajo la antena, y de donde NAVEGAR lo pinta. 8.3.
const RUTA_LAMINA: &[u8] = b"datos/pagina.lam";

/// Que se le pide a la antena despues del saludo.
const GUION_HOLA: u8 = 0;
const GUION_PAGINA: u8 = 1;

struct Saludo {
    fase: u8,
    guion: u8,
    destino: Ip,
    salto: Ip,
    mac: Mac,
    mi_ip: Ip,
    asa: usize,
    inicio_ms: u64,
    fase_ms: u64,
    dejadas: u64,
    recibidas: u64,
    /// La linea del saludo (`red hola`), para ensenarla.
    linea: [u8; LINEA_MAX],
    largo: usize,
    completa: bool,
    // -- red pagina --
    url: [u8; bmo_antena::URL_MAX],
    largo_url: usize,
    conv: Conversacion,
    lineas: Lineas,
    /// Bytes de lamina guardados en `LAMINA` (las lineas tal cual llegaron).
    lam_largo: usize,
    lam_ancho: u32,
    lam_alto: u32,
    lam_elementos: u32,
    lam_lineas: u32,
    /// La antena dijo NO, con este motivo.
    no: [u8; 64],
    largo_no: usize,
    /// La antena se salio del protocolo, o la lamina no paso el juez.
    rechazo: Option<bmo_antena::Rechazo>,
}

const fn quieto() -> Saludo {
    Saludo {
        fase: QUIETO,
        guion: GUION_HOLA,
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
        url: [0; bmo_antena::URL_MAX],
        largo_url: 0,
        conv: Conversacion::nueva(),
        lineas: Lineas::nueva(),
        lam_largo: 0,
        lam_ancho: 0,
        lam_alto: 0,
        lam_elementos: 0,
        lam_lineas: 0,
        no: [0; 64],
        largo_no: 0,
        rechazo: None,
    }
}

static mut SALUDO_EN_MARCHA: Saludo = quieto();
/// La lamina tal cual llega, linea a linea. Ceros: `.bss`, no pesa.
static mut LAMINA: [u8; LAMINA_MAX] = [0; LAMINA_MAX];
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
    empezar(s, ip.to_be_bytes(), GUION_HOLA, &[]);
}

/// **`red pagina <ip> <url>`**: el antenista de bolsillo (N3a).
pub(crate) fn pagina(s: &mut Output, resto: &[u8]) {
    let (ip_txt, url) = match resto.iter().position(|&b| b == b' ') {
        Some(i) => (&resto[..i], resto[i + 1..].trim_ascii()),
        None => (resto, &b""[..]),
    };
    let Some(ip) = crate::commands::red_pase::ipv4(ip_txt) else {
        s.text(b"  uso: red pagina 192.168.0.103 https://example.com\n");
        return;
    };
    if !bmo_antena::url_valida(url) {
        s.text(b"  la url tiene que empezar por http:// o https://, sin espacios, y medir menos de 200\n");
        return;
    }
    empezar(s, ip.to_be_bytes(), GUION_PAGINA, url);
}

fn empezar(s: &mut Output, destino: Ip, guion: u8, url: &[u8]) {
    if activa() {
        s.text(b"  ya hay un saludo en marcha: sale aqui abajo segun avanza\n");
        return;
    }
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
    t.guion = guion;
    t.url[..url.len()].copy_from_slice(url);
    t.largo_url = url.len();
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
    // Lo que haya llegado de datos se recoge en el acto, linea a linea, y
    // cada linea pasa por el protocolo (`Conversacion`) -- que es quien sabe
    // si lo que dice la antena es lo que toca decir ahora.
    if t.fase == HABLANDO && !t.completa {
        let mut buf = [0u8; 256];
        let mut linea = [0u8; LINEA_MAX + 2];
        while let Ok(n) = p.recibir(t.asa, &mut buf) {
            if n == 0 {
                break;
            }
            for &b in &buf[..n] {
                if t.completa {
                    break;
                }
                let paso = match t.lineas.empujar(b) {
                    None => continue,
                    Some(Err(r)) => {
                        t.rechazo = Some(r);
                        t.completa = true;
                        break;
                    }
                    Some(Ok(l)) => {
                        let k = l.len().min(linea.len());
                        linea[..k].copy_from_slice(&l[..k]);
                        k
                    }
                };
                oir_linea(t, p, &linea[..paso]);
            }
        }
    }
}

/// Manda un pedido por la conexion, y la conversacion se entera.
fn pedir(t: &mut Saludo, p: &mut Tcp, pedido: &Pedido) {
    let _ = t.conv.pedir(pedido);
    let mut out = [0u8; bmo_antena::URL_MAX + 16];
    if let Ok(n) = bmo_antena::escribir(&mut out, pedido) {
        let _ = p.enviar(t.asa, &out[..n]);
    }
}

/// **Una linea de la antena.** El protocolo decide que es; aqui se decide
/// que se hace con ella segun el guion.
fn oir_linea(t: &mut Saludo, p: &mut Tcp, linea: &[u8]) {
    match t.conv.oir(linea) {
        Ok(Respuesta::Hola { nombre }) => {
            let k = nombre.len().min(LINEA_MAX);
            t.linea[..k].copy_from_slice(&nombre[..k]);
            t.largo = k;
            if t.guion == GUION_PAGINA {
                let mut url = [0u8; bmo_antena::URL_MAX];
                url[..t.largo_url].copy_from_slice(&t.url[..t.largo_url]);
                let largo = t.largo_url;
                pedir(t, p, &Pedido::Pagina(&url[..largo]));
            } else {
                t.completa = true;
            }
        }
        Ok(Respuesta::Lamina(cab)) => {
            t.lam_ancho = cab.ancho;
            t.lam_alto = cab.alto;
            t.lam_elementos = cab.elementos;
            guardar_linea(t, linea);
            // Una lamina de cero elementos ya esta entera.
            if matches!(t.conv.fase(), Fase::Charla) {
                t.completa = true;
            }
        }
        Ok(Respuesta::Elemento(_)) => {
            guardar_linea(t, linea);
            if matches!(t.conv.fase(), Fase::Charla) {
                t.completa = true;
            }
        }
        Ok(Respuesta::No { motivo }) => {
            let k = motivo.len().min(t.no.len());
            t.no[..k].copy_from_slice(&motivo[..k]);
            t.largo_no = k;
            t.completa = true;
        }
        Ok(_) => {}
        Err(r) => {
            t.rechazo = Some(r);
            t.completa = true;
        }
    }
}

/// Una linea de la lamina, tal cual, al bufer (con su salto de linea).
fn guardar_linea(t: &mut Saludo, linea: &[u8]) {
    let lam = unsafe { &mut *addr_of_mut!(LAMINA) };
    if t.lam_largo + linea.len() + 1 > LAMINA_MAX {
        t.rechazo = Some(bmo_antena::Rechazo::Largo);
        t.completa = true;
        return;
    }
    lam[t.lam_largo..t.lam_largo + linea.len()].copy_from_slice(linea);
    lam[t.lam_largo + linea.len()] = b'\n';
    t.lam_largo += linea.len() + 1;
    t.lam_lineas += 1;
}

/// **La lamina, al disco**: `datos/pagina.lam`. Devuelve los bytes escritos.
fn guardar_lamina(t: &Saludo) -> usize {
    let lam = unsafe { &*core::ptr::addr_of!(LAMINA) };
    let Ok(f) = bmo::Archivo::create(RUTA_LAMINA) else { return 0 };
    let n = f.write(&lam[..t.lam_largo]);
    if !f.close() {
        return 0;
    }
    n
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
                    pedir(t, p, &Pedido::Hola);
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
                contar_lo_que_paso(s, t);
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
            let espera = if t.guion == GUION_PAGINA { ESPERA_PAGINA_MS } else { ESPERA_RESPUESTA_MS };
            if ahora.saturating_sub(t.fase_ms) >= espera {
                s.with_ink(INK_ERR);
                s.text(if t.guion == GUION_PAGINA {
                    b"  [pagina] conectada, pero la antena no acabo la lamina en 20 s\n"
                } else {
                    b"  [hola] conectada, pero sin respuesta al HOLA en 5 s\n"
                });
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

/// Lo que la antena contesto, dicho segun el guion.
fn contar_lo_que_paso(s: &mut Output, t: &Saludo) {
    if let Some(r) = t.rechazo {
        s.with_ink(INK_ERR);
        s.text(b"  [antena] se salio del protocolo, o la lamina no paso el juez: ");
        s.text(r.texto().as_bytes());
        s.text(b"\n  [antena] linea ");
        s.dec(t.lam_lineas as u64 + 1);
        s.text(b" de la lamina; no se guarda nada\n");
        s.with_ink(INK_PLAIN);
        return;
    }
    if t.largo_no > 0 {
        s.with_ink(INK_ERR);
        s.text(b"  [antena] dice NO: ");
        s.text(&t.no[..t.largo_no]);
        s.byte(b'\n');
        s.with_ink(INK_PLAIN);
        return;
    }
    s.with_ink(INK_GOOD);
    s.text(b"  [antena] HOLA ANTENA/1 ");
    s.text(&t.linea[..t.largo]);
    s.byte(b'\n');
    if t.guion == GUION_PAGINA {
        s.text(b"  [antena] LAMINA ");
        s.dec(t.lam_ancho as u64);
        s.byte(b'x');
        s.dec(t.lam_alto as u64);
        s.text(b", ");
        s.dec(t.lam_elementos as u64);
        s.text(b" elementos, ");
        s.dec(t.lam_largo as u64);
        s.text(b" bytes, juzgada entera\n");
        // ** N3: primero al bloque del ANTENISTA (se ofrece a NAVEGAR al
        // lanzarla, sin disco), y despues al disco, que es lo que queda para
        // una NAVEGAR lanzada sin oferta.
        let lam = unsafe { &*core::ptr::addr_of!(LAMINA) };
        if crate::commands::antenista::guardar(&lam[..t.lam_largo]) {
            s.text(b"  [antena] en el bloque del ANTENISTA: `run apps/navegar.ibx` la recibe OFRECIDA\n");
        } else {
            s.with_ink(INK_ERR);
            s.text(b"  [antena] sin bloque para el ANTENISTA (tope de peticiones?): queda el disco\n");
            s.with_ink(INK_PLAIN);
        }
        let n = guardar_lamina(t);
        if n == t.lam_largo {
            s.text(b"  [antena] y guardada en datos/pagina.lam\n");
        } else {
            s.with_ink(INK_ERR);
            s.text(b"  [antena] NO se pudo guardar en datos/pagina.lam (escritos ");
            s.dec(n as u64);
            s.text(b")\n");
        }
    }
    s.with_ink(INK_PLAIN);
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
