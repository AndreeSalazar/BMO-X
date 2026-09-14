//! **`red ping` y `red dns`: la pila propia sobre el buzon** (G3 y G4, 2026-09-14).
//!
//! [consumo] NADA      trabaja solo mientras hay un ping o una pregunta en marcha
//!
//! Con la IP que dio `red ip`, el escritorio monta un `bmo_pila::nodo::Nodo`:
//! contesta ARP y ping a SU IP, y con el se hace ping a otra maquina y se le
//! pregunta un nombre al DNS. Lo que se equivoca vive en `bmo-pila`, con banco;
//! aqui se lleva la hora y se pinta, igual que `red prueba` y `red ip`.
//!
//! ```text
//!    1  quien tiene el siguiente salto?   ARP con NUESTRA IP de origen
//!    2a ping: cuatro ecos, uno por segundo, con su tiempo
//!    2b dns:  una pregunta A por UDP al DNS de la concesion, dos intentos
//! ```
//!
//! ** El tiempo del ping se mide con el TSC al LEER la respuesta, y mientras hay
//! un ping en marcha el buzon se vacia cada vuelta del escritorio (`cada_vuelta`)
//! y no cada cuarto de segundo. Aun asi incluye el latido de 4 ms del kernel en
//! cada sentido: se dice en pantalla, porque un numero sin su resolucion es una
//! opinion.
//!
//! [!] PRIVACIDAD: las IP salen en pantalla y viven en memoria. Nada al disco.

use core::ptr::addr_of_mut;

use bmo_pila::dns;
use bmo_pila::ether::Mac;
use bmo_pila::ipv4::Ip;
use bmo_pila::nodo::{Hecho, Nodo};
use bmo_userland as bmo;

use crate::scene::output::{Output, INK_ERR, INK_GOOD, INK_PLAIN};

const QUIETO: u8 = 0;
const RESOLVIENDO: u8 = 1;
const PINGANDO: u8 = 2;
const PREGUNTANDO: u8 = 3;

const ECOS: u16 = 4;
const ID_PING: u16 = 0x424D;
const ESPERA_ECO_MS: u64 = 2_000;
const ESPERA_ARP_MS: u64 = 6_000;
const ESPERA_DNS_MS: u64 = 3_000;

struct Tarea {
    fase: u8,
    dns: bool,
    destino: Ip,
    salto: Ip,
    mac: Mac,
    inicio_ms: u64,
    ultimo_ms: u64,
    // ping
    enviados: u16,
    recibidos: u16,
    esperando: bool,
    enviado_ciclos: u64,
    llego_ciclos: u64,
    llego_seq: u16,
    min_us: u64,
    max_us: u64,
    suma_us: u64,
    // dns
    id: u16,
    puerto: u16,
    nombre: [u8; dns::NOMBRE_MAX + 1],
    largo_nombre: usize,
    intentos: u8,
    respuesta: [u8; 512],
    largo_respuesta: usize,
}

static mut TAREA: Tarea = Tarea {
    fase: QUIETO,
    dns: false,
    destino: [0; 4],
    salto: [0; 4],
    mac: [0; 6],
    inicio_ms: 0,
    ultimo_ms: 0,
    enviados: 0,
    recibidos: 0,
    esperando: false,
    enviado_ciclos: 0,
    llego_ciclos: 0,
    llego_seq: 0,
    min_us: u64::MAX,
    max_us: 0,
    suma_us: 0,
    id: 0,
    puerto: 0,
    nombre: [0; dns::NOMBRE_MAX + 1],
    largo_nombre: 0,
    intentos: 0,
    respuesta: [0; 512],
    largo_respuesta: 0,
};
static mut NODO: Option<Nodo> = None;

fn tarea() -> &'static mut Tarea {
    unsafe { &mut *addr_of_mut!(TAREA) }
}

fn nodo() -> &'static mut Option<Nodo> {
    unsafe { &mut *addr_of_mut!(NODO) }
}

fn ahora_ms() -> u64 {
    let hz = bmo::info(bmo::INFO_TSC_HZ);
    if hz < 1000 {
        return 0;
    }
    bmo::ciclos() / (hz / 1000)
}

fn ciclos_a_us(c: u64) -> u64 {
    let hz = bmo::info(bmo::INFO_TSC_HZ);
    if hz < 1_000_000 {
        return 0;
    }
    c / (hz / 1_000_000)
}

fn u32ip(ip: Ip) -> u32 {
    u32::from_be_bytes(ip)
}

/// **Lo comun a ping y dns**: hay IP, hay pase, hay nodo, y a quien se le
/// pregunta por ARP. `None` si no se puede, ya dicho en pantalla.
fn preparar(s: &mut Output, destino: Ip) -> Option<Ip> {
    if tarea().fase != QUIETO {
        s.text(b"  ya hay un ping o una pregunta en marcha: sale aqui abajo segun avanza\n");
        return None;
    }
    let Some(k) = crate::commands::red_ip::concesion() else {
        s.text(b"  no hay IP propia todavia: `red ip` primero\n");
        return None;
    };
    if destino == k.ip {
        s.text(b"  esa IP es la de esta maquina\n");
        return None;
    }
    let _ = bmo::red::armar();
    if !bmo::red::pase_abierto() {
        if let Err(e) = bmo::red::abrir(60_000, 50) {
            s.with_ink(INK_ERR);
            s.text(b"  NO se pudo abrir el pase");
            if let bmo::red::NoAbre::Negado(no) = e {
                s.text(b": ");
                s.text(no.texto().as_bytes());
            }
            s.byte(b'\n');
            s.with_ink(INK_PLAIN);
            return None;
        }
    }
    let mac = bmo::info(bmo::INFO_NET_MAC);
    let mut m = [0u8; 6];
    for (i, b) in m.iter_mut().enumerate() {
        *b = (mac >> ((5 - i) * 8)) as u8;
    }
    let rehacer = nodo().as_ref().map_or(true, |n| n.ip != k.ip);
    if rehacer {
        *nodo() = Some(Nodo::nuevo(m, k.ip));
    }
    let mascara = u32ip(k.mascara);
    let mismo = mascara != 0 && (u32ip(destino) & mascara) == (u32ip(k.ip) & mascara);
    Some(if mismo { destino } else { k.router })
}

/// **`red ping <ip>`.**
pub(crate) fn ping(s: &mut Output, resto: &[u8]) {
    let Some(ip) = crate::commands::red_pase::ipv4(resto) else {
        s.text(b"  uso: red ping 192.168.0.1\n");
        return;
    };
    let destino = ip.to_be_bytes();
    let Some(salto) = preparar(s, destino) else { return };
    let t = tarea();
    *t = Tarea { fase: RESOLVIENDO, dns: false, destino, salto, inicio_ms: ahora_ms(), ..quieta() };
    s.text(b"  [ping] quien tiene ");
    ip_texto(s, salto);
    s.text(b"? (ARP con nuestra IP)\n");
}

/// **`red dns <nombre>`.**
pub(crate) fn dns(s: &mut Output, resto: &[u8]) {
    let mut prueba = [0u8; 300];
    if resto.is_empty() || dns::preguntar(&mut prueba, 0, resto).is_err() {
        s.text(b"  uso: red dns geminiprotocol.net   (letras, cifras, guiones y puntos)\n");
        return;
    }
    let Some(k) = crate::commands::red_ip::concesion() else {
        s.text(b"  no hay IP propia todavia: `red ip` primero\n");
        return;
    };
    if k.dns == [0; 4] {
        s.text(b"  la concesion de DHCP no trajo DNS\n");
        return;
    }
    let Some(salto) = preparar(s, k.dns) else { return };
    let semilla = bmo::ciclos();
    let t = tarea();
    *t = Tarea {
        fase: RESOLVIENDO,
        dns: true,
        destino: k.dns,
        salto,
        inicio_ms: ahora_ms(),
        id: semilla as u16,
        puerto: 49_152 + ((semilla >> 16) as u16 % 16_000),
        largo_nombre: resto.len().min(dns::NOMBRE_MAX),
        ..quieta()
    };
    t.nombre[..t.largo_nombre].copy_from_slice(&resto[..t.largo_nombre]);
    s.text(b"  [dns] pregunto a ");
    ip_texto(s, k.dns);
    s.text(b" por ");
    s.text(&resto[..t.largo_nombre]);
    s.text(b"\n");
}

const fn quieta() -> Tarea {
    Tarea {
        fase: QUIETO,
        dns: false,
        destino: [0; 4],
        salto: [0; 4],
        mac: [0; 6],
        inicio_ms: 0,
        ultimo_ms: 0,
        enviados: 0,
        recibidos: 0,
        esperando: false,
        enviado_ciclos: 0,
        llego_ciclos: 0,
        llego_seq: 0,
        min_us: u64::MAX,
        max_us: 0,
        suma_us: 0,
        id: 0,
        puerto: 0,
        nombre: [0; dns::NOMBRE_MAX + 1],
        largo_nombre: 0,
        intentos: 0,
        respuesta: [0; 512],
        largo_respuesta: 0,
    }
}

/// **Con un ping en marcha, el buzon se mira cada vuelta.** Lo llama el bucle.
pub(crate) fn cada_vuelta() {
    let t = tarea();
    if t.fase == PINGANDO && t.esperando {
        crate::commands::red_pase::drenar();
    }
}

/// **Una trama del buzon.** La llama `red_pase::drenar` con TODAS.
pub(crate) fn oir(trama: &[u8]) {
    let t = tarea();
    if t.fase == QUIETO {
        return;
    }
    let Some(n) = nodo().as_mut() else { return };
    let mut salida = [0u8; 1514];
    match n.atender(trama, ahora_ms(), &mut salida) {
        // Alguien pregunta por NUESTRA IP (el router, antes de contestar): se le
        // contesta, o la respuesta al ping no sabria a donde ir.
        Ok(Hecho::Contestar(largo)) => {
            let _ = bmo::red::enviar(&salida[..largo]);
        }
        Ok(Hecho::Eco { origen, id, secuencia }) => {
            if t.fase == PINGANDO && t.esperando && origen == t.destino && id == ID_PING && secuencia == t.enviados {
                t.llego_ciclos = bmo::ciclos();
                t.llego_seq = secuencia;
            }
        }
        Ok(Hecho::Udp { origen, datagrama }) => {
            if t.fase == PREGUNTANDO
                && origen == t.destino
                && datagrama.origen == dns::PUERTO
                && datagrama.destino == t.puerto
                && t.largo_respuesta == 0
            {
                let l = datagrama.datos.len().min(t.respuesta.len());
                t.respuesta[..l].copy_from_slice(&datagrama.datos[..l]);
                t.largo_respuesta = l;
            }
        }
        _ => {}
    }
}

/// **Un cuarto de segundo.**
pub(crate) fn latir(s: &mut Output) {
    let t = tarea();
    if t.fase == QUIETO {
        return;
    }
    if !bmo::red::pase_abierto() {
        s.with_ink(INK_ERR);
        s.text(b"  [red] el pase se cerro antes de terminar\n");
        s.with_ink(INK_PLAIN);
        t.fase = QUIETO;
        return;
    }
    let Some(n) = nodo().as_mut() else {
        t.fase = QUIETO;
        return;
    };
    let ahora = ahora_ms();
    let mut buf = [0u8; 1514];
    let quien: &[u8] = if t.dns { b"  [dns] " } else { b"  [ping] " };

    match t.fase {
        RESOLVIENDO => {
            if let Some(mac) = n.arp.buscar(t.salto, ahora) {
                t.mac = mac;
                s.text(quien);
                ip_texto(s, t.salto);
                s.text(b" esta en ");
                crate::commands::red_pase::mac_privada(s, mac_u64(mac));
                s.byte(b'\n');
                t.fase = if t.dns { PREGUNTANDO } else { PINGANDO };
                t.ultimo_ms = 0;
                return;
            }
            if let Ok(Some(largo)) = n.preguntar(t.salto, ahora, &mut buf) {
                let _ = bmo::red::enviar(&buf[..largo]);
            }
            if ahora.saturating_sub(t.inicio_ms) >= ESPERA_ARP_MS {
                s.with_ink(INK_ERR);
                s.text(quien);
                s.text(b"nadie contesto por ARP en 6 s\n");
                s.with_ink(INK_PLAIN);
                terminar();
            }
        }
        PINGANDO => {
            if t.esperando && t.llego_ciclos != 0 {
                let us = ciclos_a_us(t.llego_ciclos.saturating_sub(t.enviado_ciclos));
                t.recibidos += 1;
                t.min_us = t.min_us.min(us);
                t.max_us = t.max_us.max(us);
                t.suma_us += us;
                t.esperando = false;
                s.with_ink(INK_GOOD);
                s.text(b"  [ping] respuesta de ");
                ip_texto(s, t.destino);
                s.text(b": seq ");
                s.dec(t.llego_seq as u64);
                s.text(b"  tiempo ");
                ms(s, us);
                s.text(b" ms\n");
                s.with_ink(INK_PLAIN);
            }
            if t.esperando && ahora.saturating_sub(t.ultimo_ms) >= ESPERA_ECO_MS {
                s.text(b"  [ping] seq ");
                s.dec(t.enviados as u64);
                s.text(b": sin respuesta en 2 s\n");
                t.esperando = false;
            }
            if !t.esperando && (t.ultimo_ms == 0 || ahora.saturating_sub(t.ultimo_ms) >= 1_000) {
                if t.enviados == ECOS {
                    resumen_ping(s, t);
                    terminar();
                    return;
                }
                let seq = t.enviados + 1;
                match n.eco(&mut buf, t.mac, t.destino, ID_PING, seq, b"BMO-X ping, sin prisa") {
                    Ok(largo) if bmo::red::enviar(&buf[..largo]).is_ok() => {
                        t.enviados = seq;
                        t.enviado_ciclos = bmo::ciclos();
                        t.llego_ciclos = 0;
                        t.esperando = true;
                        t.ultimo_ms = ahora;
                    }
                    _ => {
                        s.text(b"  [ping] el eco no entro en el buzon\n");
                        terminar();
                    }
                }
            }
        }
        PREGUNTANDO => {
            if t.largo_respuesta != 0 {
                let r = dns::leer(&t.respuesta[..t.largo_respuesta], t.id, &t.nombre[..t.largo_nombre]);
                pintar_dns(s, t, r);
                terminar();
                return;
            }
            if t.ultimo_ms == 0 || ahora.saturating_sub(t.ultimo_ms) >= ESPERA_DNS_MS {
                if t.intentos >= 2 {
                    s.with_ink(INK_ERR);
                    s.text(b"  [dns] el servidor no contesto en 6 s\n");
                    s.with_ink(INK_PLAIN);
                    terminar();
                    return;
                }
                let mut msg = [0u8; 300];
                let enviado = dns::preguntar(&mut msg, t.id, &t.nombre[..t.largo_nombre])
                    .and_then(|m| n.udp(&mut buf, t.mac, t.destino, t.puerto, dns::PUERTO, &msg[..m]))
                    .ok()
                    .is_some_and(|largo| bmo::red::enviar(&buf[..largo]).is_ok());
                if !enviado {
                    s.text(b"  [dns] la pregunta no entro en el buzon\n");
                    terminar();
                    return;
                }
                t.intentos += 1;
                t.ultimo_ms = ahora;
            }
        }
        _ => {}
    }
}

fn resumen_ping(s: &mut Output, t: &Tarea) {
    s.text(b"  [ping] ");
    s.dec(t.enviados as u64);
    s.text(b" enviados, ");
    s.dec(t.recibidos as u64);
    s.text(b" recibidos");
    if t.recibidos > 0 {
        s.text(b"   min ");
        ms(s, t.min_us);
        s.text(b" / media ");
        ms(s, t.suma_us / t.recibidos as u64);
        s.text(b" / max ");
        ms(s, t.max_us);
        s.text(b" ms\n");
        s.text(b"  (incluye el latido de 4 ms del kernel en cada sentido)\n");
        s.with_ink(INK_GOOD);
        s.text(b"  G3 hecho: la pila propia hace ping y le contestan.\n");
        s.with_ink(INK_PLAIN);
    } else {
        s.byte(b'\n');
    }
}

fn pintar_dns(s: &mut Output, t: &Tarea, r: Result<dns::Respuesta, bmo_pila::Rechazo>) {
    match r {
        Ok(dns::Respuesta::Ips { ips, cuantas, ttl }) => {
            s.with_ink(INK_GOOD);
            s.text(b"  [dns] ");
            s.text(&t.nombre[..t.largo_nombre]);
            s.text(b" es ");
            for (k, ip) in ips.iter().take(cuantas).enumerate() {
                ip_texto(s, *ip);
                if k + 1 < cuantas {
                    s.text(b", ");
                }
            }
            s.text(b"   (vale ");
            s.dec(ttl as u64);
            s.text(b" s)\n  G4 hecho: un nombre ya es una IP.\n");
            s.with_ink(INK_PLAIN);
        }
        Ok(dns::Respuesta::SinIpv4) => s.text(b"  [dns] el nombre existe y no tiene IPv4\n"),
        Ok(dns::Respuesta::NoExiste) => s.text(b"  [dns] ese nombre NO existe\n"),
        Ok(dns::Respuesta::Fallo(c)) => {
            s.text(b"  [dns] el servidor contesto con error ");
            s.dec(c as u64);
            s.byte(b'\n');
        }
        Err(e) => {
            s.with_ink(INK_ERR);
            s.text(b"  [dns] la respuesta no se creyo: ");
            s.text(e.texto().as_bytes());
            s.byte(b'\n');
            s.with_ink(INK_PLAIN);
        }
    }
}

fn terminar() {
    tarea().fase = QUIETO;
    bmo::red::cerrar();
}

fn mac_u64(m: Mac) -> u64 {
    m.iter().fold(0u64, |a, b| (a << 8) | *b as u64)
}

fn ip_texto(s: &mut Output, ip: Ip) {
    for (k, b) in ip.iter().enumerate() {
        s.dec(*b as u64);
        if k < 3 {
            s.byte(b'.');
        }
    }
}

/// Microsegundos como milisegundos con tres decimales.
fn ms(s: &mut Output, us: u64) {
    s.dec(us / 1000);
    s.byte(b'.');
    let r = us % 1000;
    s.byte(b'0' + (r / 100) as u8);
    s.byte(b'0' + (r / 10 % 10) as u8);
    s.byte(b'0' + (r % 10) as u8);
}
