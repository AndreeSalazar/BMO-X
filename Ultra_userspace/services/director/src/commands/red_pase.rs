//! **EL GATE RED DESDE EL ESCRITORIO** -- el pase, la prueba en tiempo real y el
//! perfil de la red, sin ensenar lo que dice quien eres.
//!
//! [consumo] NADA      trabaja solo con un pase abierto o una prueba en marcha;
//!                     sin ninguno, cada cuarto de segundo es una comparacion
//!
//! ## Por que salio de `red.rs` (2026-09-13)
//!
//! `red.rs` es el INFORME: lee contadores. Esto ACTUA -- abre el pase, envia
//! tramas y lleva una prueba de varios segundos -- y ademas va a crecer con el
//! camino a Gemini (`docs/plan/PLAN_RED_TX.md`, seccion 4). Se corta por donde se
//! va a seguir moviendo, que es la regla del corte del 28-08.
//!
//! ## *** LA PRIVACIDAD, que es regla y no adorno
//!
//! Eddi: *"no quiero exponer donde vivo, no quiero ser expuesto"*.
//!
//! ```text
//!    la MAC entera      identifica ESTE equipo. Se ensena el fabricante (tres
//!                       bytes); `red mac completa` si de verdad hace falta
//!    la IP de la LAN    192.168.x.x no dice donde vive nadie: es la misma en
//!                       millones de casas. Se ensena, y no se escribe en disco
//!    la IP PUBLICA      ESA si dice donde. BMO-X no la conoce: no le pregunta a
//!                       nadie de fuera, y este fichero no la pide
//! ```
//!
//! [!] Una foto de la pantalla viaja lejos. Por eso lo que se ensena ya sale
//! recortado, y no hay que acordarse de taparlo.

use core::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};

use bmo_userland as bmo;

use crate::commands::red::{link, mac_hex};
use crate::commands::tabla::{label, section};
use crate::scene::output::{Output, INK_ERR, INK_GOOD, INK_PLAIN};

type NoEnvia = bmo::red::puerta::buzon::NoEnvia;

// ===================================================================
//  LO QUE SE OYE: el buzon, vaciado sin syscall
// ===================================================================

/// A que IP se pregunto por ARP (big-endian), `0` = a nadie.
static PREGUNTADA: AtomicU32 = AtomicU32::new(0);
/// La MAC que contesto por esa IP.
static RESPUESTA_MAC: AtomicU64 = AtomicU64::new(0);
static RESPUESTAS: AtomicU64 = AtomicU64::new(0);
static RECIBIDAS: AtomicU64 = AtomicU64::new(0);

/// **Quien habla ARP en este cable**, y cuantas veces.
///
/// ** La foto del 13-09 dijo `sin respuesta todavia` con 56 tramas en el buzon.
/// Un router pregunta por ARP todo el rato, asi que su IP ya venia ahi dentro:
/// si no es la que se pregunto, el fallo no es el cable, es la IP.
static VECINOS: [AtomicU32; 8] = [
    AtomicU32::new(0), AtomicU32::new(0), AtomicU32::new(0), AtomicU32::new(0),
    AtomicU32::new(0), AtomicU32::new(0), AtomicU32::new(0), AtomicU32::new(0),
];
static VECES: [AtomicU32; 8] = [
    AtomicU32::new(0), AtomicU32::new(0), AtomicU32::new(0), AtomicU32::new(0),
    AtomicU32::new(0), AtomicU32::new(0), AtomicU32::new(0), AtomicU32::new(0),
];

fn apuntar_vecino(ip: u32) {
    if ip == 0 {
        return;
    }
    for (k, v) in VECINOS.iter().enumerate() {
        let hay = v.load(Ordering::Relaxed);
        if hay == ip {
            VECES[k].fetch_add(1, Ordering::Relaxed);
            return;
        }
        if hay == 0 {
            v.store(ip, Ordering::Relaxed);
            VECES[k].store(1, Ordering::Relaxed);
            return;
        }
    }
}

fn cuantos_vecinos() -> usize {
    VECINOS.iter().filter(|v| v.load(Ordering::Relaxed) != 0).count()
}

/// Cuanto se parece a un router: acabar en `.1` o `.254` pesa mas que hablar mucho.
fn puntuacion(ip: u32, veces: u32) -> u64 {
    let base = match ip & 0xFF {
        1 => 2000,
        254 => 1000,
        _ => 0,
    };
    base + veces as u64
}

/// Los mejores candidatos a router, del mas probable al menos. Cuantos hay.
fn mejores(dst: &mut [u32; 4]) -> usize {
    let mut n = 0;
    while n < dst.len() {
        let mut mejor: Option<(u64, u32)> = None;
        for (k, v) in VECINOS.iter().enumerate() {
            let ip = v.load(Ordering::Relaxed);
            if ip == 0 || dst[..n].contains(&ip) {
                continue;
            }
            let p = puntuacion(ip, VECES[k].load(Ordering::Relaxed));
            if mejor.map_or(true, |(q, _)| p > q) {
                mejor = Some((p, ip));
            }
        }
        match mejor {
            Some((_, ip)) => {
                dst[n] = ip;
                n += 1;
            }
            None => break,
        }
    }
    n
}

/// **Vacia el buzon.** Sin syscall. Tambien lo llama `red_nodo::cada_vuelta`.
pub(crate) fn drenar() {
    if !bmo::red::pase_abierto() {
        return;
    }
    let mut t = [0u8; 1514];
    for _ in 0..16 {
        let Some(n) = bmo::red::recibir(&mut t) else { break };
        RECIBIDAS.fetch_add(1, Ordering::Relaxed);
        // Toda trama pasa tambien por `red ip`: el DHCP llega por el mismo buzon.
        crate::commands::red_ip::oir(&t[..n]);
        crate::commands::red_nodo::oir(&t[..n]);
        if n < 42 || t[12] != 0x08 || t[13] != 0x06 {
            continue;
        }
        let spa = u32::from_be_bytes([t[28], t[29], t[30], t[31]]);
        apuntar_vecino(spa);
        // Respuesta (oper 2) de la IP por la que se pregunto.
        if t[20] == 0 && t[21] == 2 && spa != 0 && spa == PREGUNTADA.load(Ordering::Relaxed) {
            let mut mac = 0u64;
            for &b in &t[22..28] {
                mac = (mac << 8) | b as u64;
            }
            RESPUESTA_MAC.store(mac, Ordering::Relaxed);
            RESPUESTAS.fetch_add(1, Ordering::Relaxed);
        }
    }
}

// ===================================================================
//  *** LA PRUEBA EN TIEMPO REAL: `red prueba`
// ===================================================================
//
// Eddi: *"otro que se automatice en test, en tiempo real"*. La secuencia que el
// 13-09 hubo que teclear a mano, y que fallo por elegir la IP a ciegas, la hace
// sola -- y en vez de adivinar el router, lo OYE.
//
// ```text
//    1/3  abre el pase (o usa el que hay)
//    2/3  escucha ARP de 3 a 10 s y elige candidatos: .1 y .254 primero
//    3/3  pregunta a cada uno, 2 s cada vez, y da UNO de tres veredictos:
//           PASA                    alguien contesto: salio al cable
//           FALLA en la TARJETA     el grifo la dejo y la tarjeta no la solto
//           SIN VEREDICTO           la tarjeta la envio y nadie contesto
// ```
//
// ** Avanza con los cuartos de segundo del bucle, no con un hilo: el escritorio
// ya late, y otro reloj seria otra cosa que puede ir desfasada.

const QUIETA: u32 = 0;
const ESCUCHANDO: u32 = 1;
const PREGUNTANDO: u32 = 2;
/// Cuartos de segundo escuchando, como poco (si ya hay vecinos) y como mucho.
const ESCUCHAR_MIN: u32 = 12;
const ESCUCHAR_MAX: u32 = 40;
/// Cuartos de segundo esperando cada respuesta.
const ESPERA_RESPUESTA: u32 = 8;

static FASE: AtomicU32 = AtomicU32::new(QUIETA);
static CUARTOS: AtomicU32 = AtomicU32::new(0);
static TURNO: AtomicU32 = AtomicU32::new(0);
static N_CAND: AtomicU32 = AtomicU32::new(0);
static CANDIDATOS: [AtomicU32; 4] = [AtomicU32::new(0), AtomicU32::new(0), AtomicU32::new(0), AtomicU32::new(0)];

/// **Un cuarto de segundo.** Lo llama el bucle del escritorio con el receptor armado.
pub(crate) fn latir(s: &mut Output) {
    drenar();
    crate::commands::red_ip::latir(s);
    crate::commands::red_nodo::latir(s);
    let fase = FASE.load(Ordering::Relaxed);
    if fase == QUIETA {
        return;
    }
    if !bmo::red::pase_abierto() {
        s.with_ink(INK_ERR);
        s.text(b"  [prueba] el pase se cerro antes de terminar");
        motivo_del_cierre(s);
        s.byte(b'\n');
        s.with_ink(INK_PLAIN);
        FASE.store(QUIETA, Ordering::Relaxed);
        return;
    }
    let c = CUARTOS.fetch_add(1, Ordering::Relaxed) + 1;

    if fase == ESCUCHANDO {
        let n = cuantos_vecinos();
        if (n > 0 && c >= ESCUCHAR_MIN) || c >= ESCUCHAR_MAX {
            elegir_candidatos(s);
            TURNO.store(0, Ordering::Relaxed);
            CUARTOS.store(0, Ordering::Relaxed);
            FASE.store(PREGUNTANDO, Ordering::Relaxed);
            preguntar_turno(s);
        } else if c % 8 == 0 {
            s.text(b"  [prueba] escuchando ARP... ");
            s.dec(n as u64);
            s.text(b" vecino(s)\n");
        }
        return;
    }

    // PREGUNTANDO
    if RESPUESTAS.load(Ordering::Relaxed) > 0 {
        s.with_ink(INK_GOOD);
        s.text(b"  [prueba] PASA: ");
        ip_texto(s, PREGUNTADA.load(Ordering::Relaxed));
        s.text(b" contesto desde ");
        mac_privada(s, RESPUESTA_MAC.load(Ordering::Relaxed));
        s.text(b"\n  la trama salio al cable y el router la oyo. E3 hecho.\n");
        s.with_ink(INK_PLAIN);
        terminar();
        return;
    }
    if c < ESPERA_RESPUESTA {
        return;
    }
    let (dadas, devueltas) = bmo::red::vuelos();
    if dadas > devueltas {
        s.with_ink(INK_ERR);
        s.text(b"  [prueba] FALLA en la TARJETA: se le dieron ");
        s.dec(dadas);
        s.text(b" y devolvio enviadas ");
        s.dec(devueltas);
        s.text(b"\n  el grifo la dejo salir y el transmisor no la solto: es el anillo de salida, no la IP.\n");
        s.with_ink(INK_PLAIN);
        terminar();
        return;
    }
    s.text(b"  [prueba] ");
    ip_texto(s, PREGUNTADA.load(Ordering::Relaxed));
    s.text(b" no contesto en 2 s (la tarjeta SI la envio)\n");
    let t = TURNO.fetch_add(1, Ordering::Relaxed) + 1;
    if t < N_CAND.load(Ordering::Relaxed) {
        CUARTOS.store(0, Ordering::Relaxed);
        preguntar_turno(s);
    } else {
        s.with_ink(INK_ERR);
        s.text(b"  [prueba] SIN VEREDICTO: la tarjeta envia y nadie contesto a la sonda ARP.\n");
        s.text(b"  la sonda va con origen 0.0.0.0 y hay routers que no la contestan;\n");
        s.text(b"  preguntar con IP propia llega con DHCP (G2 del camino a Gemini).\n");
        s.with_ink(INK_PLAIN);
        terminar();
    }
}

fn empezar_prueba(s: &mut Output) {
    if FASE.load(Ordering::Relaxed) != QUIETA {
        s.text(b"  ya hay una prueba en marcha: sale aqui abajo segun avanza\n");
        return;
    }
    let _ = bmo::red::armar();
    if !bmo::red::pase_abierto() {
        match bmo::red::abrir(60_000, 50) {
            Ok(_) => {}
            Err(bmo::red::NoAbre::Negado(no)) => {
                s.with_ink(INK_ERR);
                s.text(b"  [prueba] NO se pudo abrir el pase: ");
                s.text(no.texto().as_bytes());
                s.byte(b'\n');
                s.with_ink(INK_PLAIN);
                return;
            }
            Err(bmo::red::NoAbre::Otro { code, flags }) => {
                s.text(b"  [prueba] el kernel contesto algo que no conozco: ");
                s.dec(code as u64);
                s.byte(b'/');
                s.dec(flags as u64);
                s.byte(b'\n');
                return;
            }
        }
    }
    PREGUNTADA.store(0, Ordering::Relaxed);
    RESPUESTAS.store(0, Ordering::Relaxed);
    CUARTOS.store(0, Ordering::Relaxed);
    TURNO.store(0, Ordering::Relaxed);
    N_CAND.store(0, Ordering::Relaxed);
    FASE.store(ESCUCHANDO, Ordering::Relaxed);
    s.text(b"  [prueba 1/3] pase ABIERTO. Desde aqui va SOLA: escucho quien habla ARP,\n");
    s.text(b"  elijo el router, le pregunto y doy el veredicto. Hasta 20 segundos.\n");
}

fn elegir_candidatos(s: &mut Output) {
    let mut c = [0u32; 4];
    let mut n = mejores(&mut c);
    s.text(b"  [prueba 2/3] ");
    if n == 0 {
        c[..3].copy_from_slice(&[0xC0A8_0101, 0xC0A8_0001, 0x0A00_0001]);
        n = 3;
        s.text(b"nadie hablo ARP en 10 s; pruebo las puertas de enlace tipicas: ");
    } else {
        s.text(b"candidatos a router, oidos en el cable: ");
    }
    for k in 0..n {
        CANDIDATOS[k].store(c[k], Ordering::Relaxed);
        ip_texto(s, c[k]);
        if k + 1 < n {
            s.text(b", ");
        }
    }
    s.byte(b'\n');
    N_CAND.store(n as u32, Ordering::Relaxed);
}

fn preguntar_turno(s: &mut Output) {
    let t = (TURNO.load(Ordering::Relaxed) as usize).min(CANDIDATOS.len() - 1);
    let ip = CANDIDATOS[t].load(Ordering::Relaxed);
    s.text(b"  [prueba 3/3] pregunto por ARP a ");
    ip_texto(s, ip);
    s.byte(b'\n');
    if let Err(e) = enviar_arp(ip) {
        s.with_ink(INK_ERR);
        s.text(b"  [prueba] la pregunta no entro en el buzon: ");
        s.text(no_envia(e));
        s.byte(b'\n');
        s.with_ink(INK_PLAIN);
        terminar();
    }
}

fn terminar() {
    bmo::red::cerrar();
    FASE.store(QUIETA, Ordering::Relaxed);
}

// ===================================================================
//  LAS ORDENES
// ===================================================================

/// `red abrir|arp|pase|cerrar|prueba|perfil|opciones`. `false` si no es ninguna.
pub(crate) fn orden(s: &mut Output, what: &[u8]) -> bool {
    let (orden, resto) = partir(what);
    match orden {
        b"abrir" => abrir(s, resto),
        b"arp" => arp(s, resto),
        b"pase" => pase(s),
        b"prueba" => empezar_prueba(s),
        b"perfil" => perfil(s),
        b"ip" => crate::commands::red_ip::empezar(s),
        b"ping" => crate::commands::red_nodo::ping(s, resto),
        b"dns" => crate::commands::red_nodo::dns(s, resto),
        b"ver" => ver(s, false),
        b"tapar" => ver(s, true),
        b"opciones" | b"ayuda" | b"?" => opciones(s),
        b"cerrar" => {
            FASE.store(QUIETA, Ordering::Relaxed);
            bmo::red::cerrar();
            s.text(b"  pase CERRADO: grifo cerrado, y el buzon ya es una lapida\n");
        }
        _ => return false,
    }
    true
}

/// **TODAS LAS OPCIONES DE `red`**, en una pantalla.
pub(crate) fn opciones(s: &mut Output) {
    section(s, b"RED -- LAS OPCIONES");
    let filas: [(&[u8], &[u8]); 14] = [
        (b"red ping <ip>", b"cuatro ecos con la pila propia, con su tiempo (pide `red ip`)"),
        (b"red dns <nombre>", b"pregunta la IPv4 de un nombre al DNS del router (pide `red ip`)"),
        (b"red", b"el informe: tarjeta, enlace, receptor y lo que llega"),
        (b"red perfil", b"lo que la maquina sabe de su red, recortado"),
        (b"red rx", b"arma el receptor y cuenta lo nuevo"),
        (b"red prueba", b"SOLA y en tiempo real: abre, oye, elige router, pregunta, veredicto"),
        (b"red ip", b"pide IP propia por DHCP, en tiempo real (vive en memoria)"),
        (b"red ver|tapar", b"la MAC sin censura en pantalla, o tapada otra vez"),
        (b"red abrir [s]", b"GATE RED: pide el pase (60 s si no dices)"),
        (b"red arp <ip>", b"pregunta por ARP quien tiene esa IP"),
        (b"red pase", b"como va el pase, la tarjeta y quien habla ARP"),
        (b"red cerrar", b"cierra el pase: el buzon pasa a la lapida"),
        (b"red mac", b"el fabricante de la MAC (`red mac completa`: entera)"),
        (b"red phy|link", b"una sola fila"),
    ];
    for (orden, que) in filas {
        label(s, orden);
        s.text(que);
        s.byte(b'\n');
    }
}

fn abrir(s: &mut Output, resto: &[u8]) {
    let segundos = numero(resto).unwrap_or(60);
    // El receptor primero: el pase lo exige, y armar es idempotente.
    let _ = bmo::red::armar();
    match bmo::red::abrir(segundos.saturating_mul(1000), 1000) {
        Ok(_) => {
            s.text(b"  pase ABIERTO por ");
            s.dec(segundos.min(600));
            s.text(b" s y 1000 tramas. Pagado UNA vez: desde aqui, cero syscalls por trama\n");
            s.text(b"  y el radar mira cada 4 ms. Prueba: `red arp <ip>`, o `red prueba` para todo solo\n");
        }
        Err(bmo::red::NoAbre::Negado(no)) => {
            s.text(b"  NO: ");
            s.text(no.texto().as_bytes());
            s.byte(b'\n');
        }
        Err(bmo::red::NoAbre::Otro { code, flags }) => {
            s.text(b"  el kernel contesto algo que no conozco: codigo ");
            s.dec(code as u64);
            s.text(b", banderas ");
            s.dec(flags as u64);
            s.byte(b'\n');
        }
    }
}

/// **La pregunta ARP**: origen 0.0.0.0, una SONDA (RFC 5227). No hace falta tener
/// IP para preguntar, y no se le ensucia la tabla a nadie con una inventada.
fn enviar_arp(ip: u32) -> Result<(), NoEnvia> {
    let m = t_mac(bmo::info(bmo::INFO_NET_MAC));
    let mut t = [0u8; 42];
    t[0..6].fill(0xFF);
    t[6..12].copy_from_slice(&m);
    t[12] = 0x08;
    t[13] = 0x06;
    // Ethernet, IPv4, 6 y 4 bytes, PREGUNTA.
    t[14..22].copy_from_slice(&[0, 1, 0x08, 0x00, 6, 4, 0, 1]);
    t[22..28].copy_from_slice(&m);
    // 28..32 origen 0.0.0.0 y 32..38 destino desconocido: ya son ceros.
    t[38..42].copy_from_slice(&ip.to_be_bytes());
    PREGUNTADA.store(ip, Ordering::Relaxed);
    RESPUESTAS.store(0, Ordering::Relaxed);
    RESPUESTA_MAC.store(0, Ordering::Relaxed);
    bmo::red::enviar(&t)
}

fn no_envia(e: NoEnvia) -> &'static [u8] {
    match e {
        NoEnvia::Revocado => b"no hay pase abierto: `red abrir 60` primero",
        NoEnvia::Llena => b"el buzon esta lleno: vuelve a intentarlo en un momento",
        NoEnvia::Larga => b"la trama es demasiado larga",
    }
}

fn arp(s: &mut Output, resto: &[u8]) {
    let Some(ip) = ipv4(resto) else {
        s.text(b"  uso: red arp 192.168.1.1   (o `red prueba`, que elige la IP oyendo el cable)\n");
        return;
    };
    match enviar_arp(ip) {
        Ok(()) => {
            s.text(b"  pregunta ARP dejada en el buzon: sale en el siguiente latido.\n");
            s.text(b"  `red pase` en un segundo: si el router contesta, la trama LLEGO al cable\n");
        }
        Err(e) => {
            s.text(b"  ");
            s.text(no_envia(e));
            s.byte(b'\n');
        }
    }
}

fn motivo_del_cierre(s: &mut Output) {
    let motivo = ((bmo::red::estado() >> 56) & 0x7F) as u32;
    if let Some(m) = bmo::red::puerta::radar::Motivo::desde_codigo(motivo) {
        s.text(b" -- ");
        s.text(m.texto().as_bytes());
    }
}

fn pase(s: &mut Output) {
    let e = bmo::red::estado();
    label(s, b"pase");
    if e >> 63 != 0 {
        s.text(b"ABIERTO");
    } else {
        s.text(b"cerrado");
        motivo_del_cierre(s);
    }
    s.byte(b'\n');
    label(s, b"salieron");
    s.dec(e & 0xFF_FFFF);
    s.text(b"   negadas ");
    s.dec((e >> 24) & 0xFF_FFFF);
    let ultimo = (e >> 48) & 0xFF;
    if ultimo != 0 {
        s.text(b"   (ultimo no: ");
        s.dec(ultimo);
        s.text(b")");
    }
    s.byte(b'\n');
    // *** LA PREGUNTA DE E3: la tarjeta la SOLTO? Salir del grifo no es salir
    // al cable; volver de la tarjeta, si.
    let (dadas, devueltas) = bmo::red::vuelos();
    label(s, b"tarjeta");
    s.dec(devueltas);
    s.text(b" de ");
    s.dec(dadas);
    s.text(b" devueltas ENVIADAS");
    if dadas > devueltas {
        s.text(b"\n    [!] la tarjeta NO ha soltado alguna: no salio al cable (transmisor)\n");
    } else if dadas > 0 {
        s.text(b"\n    la tarjeta las envio: salieron al cable\n");
    } else {
        s.byte(b'\n');
    }
    label(s, b"buzon");
    s.dec(RECIBIDAS.load(Ordering::Relaxed));
    s.text(b" tramas recogidas sin syscall\n");
    if cuantos_vecinos() > 0 {
        label(s, b"hablan ARP");
        for (k, v) in VECINOS.iter().enumerate() {
            let ip = v.load(Ordering::Relaxed);
            if ip != 0 {
                ip_texto(s, ip);
                s.byte(b'(');
                s.dec(VECES[k].load(Ordering::Relaxed) as u64);
                s.text(b")  ");
            }
        }
        s.text(b"\n    el router suele ser la que acaba en .1 o .254 -- `red prueba` elige sola\n");
    }
    let ip = PREGUNTADA.load(Ordering::Relaxed);
    if ip != 0 {
        label(s, b"ARP");
        ip_texto(s, ip);
        if RESPUESTAS.load(Ordering::Relaxed) > 0 {
            s.text(b" CONTESTO desde ");
            mac_privada(s, RESPUESTA_MAC.load(Ordering::Relaxed));
            s.text(b"\n    *** la trama salio al cable y el router la oyo: E3 hecho\n");
        } else {
            s.text(b" sin respuesta todavia\n");
        }
    }
    if FASE.load(Ordering::Relaxed) != QUIETA {
        s.text(b"    (hay una `red prueba` en marcha)\n");
    }
}

// ===================================================================
//  *** EL PERFIL DE LA RED: lo que la maquina sabe, recortado
// ===================================================================
//
// ** Eddi pidio que el kernel "lea el perfil de la red y le diga que IP es". La
// mitad de eso no puede ir al kernel, y no por gusto: la ley de
// `RED_MAESTRO.md` es que **el kernel no sabe lo que es una IP**. Asi que el
// reparto es el de siempre:
//
// ```text
//    del kernel   la tarjeta, la MAC, el enlace, el receptor, los vuelos
//    de aqui      el router oido, la subred supuesta y la IP propia
// ```
//
// Y `PERFIL/RED.txt` describe el HARDWARE, que es lo que se puede publicar. Lo
// que dice quien eres se aprende al arrancar y no sale de la maquina.

fn perfil(s: &mut Output) {
    section(s, b"PERFIL DE LA RED");
    let presente = bmo::info(bmo::INFO_NET_PRESENTE) != 0;
    label(s, b"tarjeta");
    if !presente {
        s.text(b"ninguna reconocida en el PCI\n");
        return;
    }
    let vd = bmo::info(bmo::INFO_NET_VENDOR_DEVICE);
    s.text(b"0x");
    s.hex(vd, 8);
    if vd == 0x10EC_8168 {
        s.text(b"   Realtek RTL8111/8168, en la placa (PERFIL/RED.txt)");
    }
    s.byte(b'\n');
    label(s, b"MAC");
    mac_privada(s, bmo::info(bmo::INFO_NET_MAC));
    s.text(b"   el fabricante; el resto no se ensena\n");
    label(s, b"enlace");
    link(s, presente, bmo::info(bmo::INFO_NET_MEGABITS));
    s.byte(b'\n');
    label(s, b"receptor");
    if bmo::info(bmo::INFO_NET_RX_ARMADO) != 0 {
        s.text(b"ARMADO\n");
    } else {
        s.text(b"apagado   (`red rx`)\n");
    }
    let (dadas, devueltas) = bmo::red::vuelos();
    label(s, b"transmisor");
    if bmo::red::pase_abierto() {
        s.text(b"PASE ABIERTO");
    } else if dadas > 0 {
        s.text(b"armado, grifo cerrado");
    } else {
        s.text(b"se arma con el primer pase");
    }
    s.text(b"   (");
    s.dec(devueltas);
    s.byte(b'/');
    s.dec(dadas);
    s.text(b" devueltas enviadas)\n");

    let mut c = [0u32; 4];
    let router = (mejores(&mut c) > 0).then_some(c[0]);
    label(s, b"router");
    match router {
        Some(ip) => {
            ip_texto(s, ip);
            s.text(b"   supuesto: el que mas se parece, oido en ARP\n");
        }
        None => s.text(b"sin oir todavia -- `red prueba` lo busca\n"),
    }
    label(s, b"subred");
    match router {
        Some(ip) => {
            ip_texto(s, ip & 0xFFFF_FF00);
            s.text(b"/24   supuesta: la del router\n");
        }
        None => s.text(b"-\n"),
    }
    label(s, b"IP propia");
    match crate::commands::red_ip::concesion() {
        Some(k) => {
            ip_texto(s, u32::from_be_bytes(k.ip));
            s.text(b"   por DHCP, de ");
            ip_texto(s, u32::from_be_bytes(k.servidor));
            s.text(b" (en memoria: no se guarda)\n");
        }
        None => s.text(b"ninguna todavia -- `red ip` la pide por DHCP\n"),
    }
    s.text(b"    ** nada de esto se escribe en disco ni sale de la maquina. La IP PUBLICA --\n");
    s.text(b"    la que si dice donde vives-- BMO-X no la conoce: no pregunta a nadie de fuera.\n");
}

// ===================================================================
//  AYUDANTES
// ===================================================================

/// **La MAC recortada**: el fabricante y `xx` en el resto. Tres bytes dicen
/// "Micro-Star" o "TP-Link"; seis dicen "este equipo".
pub(crate) fn mac_privada(s: &mut Output, mac: u64) {
    if !TAPAR.load(Ordering::Relaxed) {
        mac_hex(s, mac);
        return;
    }
    for i in 0..3 {
        s.hex((mac >> ((5 - i) * 8)) & 0xFF, 2);
        s.byte(b'-');
    }
    s.text(b"xx-xx-xx");
}

/// **Tapada o no.** Nace tapada en cada arranque: quitar la censura es para
/// esta sesion, no se guarda (Eddi, 2026-09-14: *"que tengan sin censura con
/// click"*; la salida no tiene clic todavia, asi que por ahora es una orden).
static TAPAR: AtomicBool = AtomicBool::new(true);

fn ver(s: &mut Output, tapar: bool) {
    TAPAR.store(tapar, Ordering::Relaxed);
    if tapar {
        s.text(b"  MAC TAPADA otra vez: solo el fabricante\n");
    } else {
        s.with_ink(INK_ERR);
        s.text(b"  MAC SIN CENSURA hasta `red tapar` o reiniciar. [!] no la ensenes en fotos\n");
        s.with_ink(INK_PLAIN);
    }
}

/// La MAC ENTERA, solo cuando se pide por su nombre.
pub(crate) fn mac_completa(s: &mut Output, mac: u64) {
    mac_hex(s, mac);
    s.text(b"   [!] identifica ESTE equipo: no la ensenes en fotos");
}

fn ip_texto(s: &mut Output, ip: u32) {
    for k in 0..4 {
        s.dec(((ip >> ((3 - k) * 8)) & 0xFF) as u64);
        if k < 3 {
            s.byte(b'.');
        }
    }
}

fn t_mac(mac: u64) -> [u8; 6] {
    let mut m = [0u8; 6];
    for (k, b) in m.iter_mut().enumerate() {
        *b = (mac >> ((5 - k) * 8)) as u8;
    }
    m
}

fn recortar(b: &[u8]) -> &[u8] {
    let i = b.iter().position(|&c| c != b' ').unwrap_or(b.len());
    let f = b.iter().rposition(|&c| c != b' ').map_or(i, |p| p + 1);
    &b[i..f.max(i)]
}

fn partir(b: &[u8]) -> (&[u8], &[u8]) {
    let b = recortar(b);
    match b.iter().position(|&c| c == b' ') {
        Some(i) => (&b[..i], recortar(&b[i + 1..])),
        None => (b, &[]),
    }
}

fn numero(b: &[u8]) -> Option<u64> {
    if b.is_empty() {
        return None;
    }
    let mut n = 0u64;
    for &c in b {
        if !c.is_ascii_digit() {
            return None;
        }
        n = n.checked_mul(10)?.checked_add((c - b'0') as u64)?;
    }
    Some(n)
}

pub(crate) fn ipv4(b: &[u8]) -> Option<u32> {
    let mut ip = 0u32;
    let mut partes = 0;
    for trozo in recortar(b).split(|&c| c == b'.') {
        let v = numero(trozo)?;
        if v > 255 || partes == 4 {
            return None;
        }
        ip = (ip << 8) | v as u32;
        partes += 1;
    }
    (partes == 4).then_some(ip)
}
