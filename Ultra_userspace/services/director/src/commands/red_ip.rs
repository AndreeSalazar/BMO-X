//! **`red ip`: pedir IP propia por DHCP, en tiempo real** (G2, 2026-09-14).
//!
//! [consumo] NADA      trabaja solo mientras hay una peticion en marcha
//!
//! Lo que se puede equivocar vive en `bmo-pila` (`dhcp.rs`), con banco contra
//! servidores de mentira. Aqui se abre el pase, se lleva la hora y se pinta lo
//! que va pasando, igual que `red prueba`.
//!
//! [!] PRIVACIDAD (`docs/plan/PLAN_RED_TX.md`, seccion 5): la IP concedida se
//! ensena en pantalla y vive en la memoria de este proceso. No se escribe en
//! disco ni en el repositorio, y se olvida al apagar.

use core::ptr::addr_of_mut;

use bmo_pila::dhcp;
use bmo_pila::Rechazo;
use bmo_userland as bmo;

use crate::scene::output::{Output, INK_ERR, INK_GOOD, INK_PLAIN};

static mut CLIENTE: Option<dhcp::Cliente> = None;
/// La fase que ya se pinto, para decir cada paso UNA vez.
static mut PINTADA: u8 = 0;
/// Lo ultimo que llego A NUESTRO PUERTO y no se creyo: el porque de un fallo.
static mut RECHAZO: Option<Rechazo> = None;
/// **La ultima concesion CONFIRMADA**, con su hora. Sobrevive a pedir otra:
/// el 14-09 un segundo `red ip` dejo a la maquina sin la IP que ya tenia, y
/// `red dns` contesto "no hay IP propia" con una concesion de dos horas viva.
static mut ULTIMA: Option<(dhcp::Concesion, u64)> = None;
/// Preguntas que salieron en esta peticion, para decirlo si nadie contesta.
static mut ENVIADAS: u32 = 0;

fn cliente() -> &'static mut Option<dhcp::Cliente> {
    unsafe { &mut *addr_of_mut!(CLIENTE) }
}

fn ahora_ms() -> u64 {
    let hz = bmo::info(bmo::INFO_TSC_HZ);
    if hz < 1000 {
        return 0;
    }
    bmo::ciclos() / (hz / 1000)
}

fn fase(e: dhcp::Estado) -> u8 {
    match e {
        dhcp::Estado::Quieto => 0,
        dhcp::Estado::Descubriendo => 1,
        dhcp::Estado::Pidiendo(_) => 2,
        dhcp::Estado::Concedida { .. } => 3,
        dhcp::Estado::Fallo(_) => 4,
    }
}

/// La IP concedida, si la hay.
pub(crate) fn concesion() -> Option<dhcp::Concesion> {
    let (k, desde) = unsafe { ULTIMA }?;
    let vida_ms = k.segundos as u64 * 1000;
    (ahora_ms().saturating_sub(desde) < vida_ms).then_some(k)
}

/// **`red ip`.**
pub(crate) fn empezar(s: &mut Output, resto: &[u8]) {
    // ** Con una concesion viva no se vuelve a preguntar: un cliente DHCP no
    // pide lo que ya tiene. `red ip nueva` lo fuerza, y aun asi la vieja sigue
    // valiendo hasta que llegue la nueva.
    if resto != b"nueva" {
        if let (Some(k), Some((_, desde))) = (concesion(), unsafe { ULTIMA }) {
            let vida_ms = k.segundos as u64 * 1000;
            let quedan = vida_ms.saturating_sub(ahora_ms().saturating_sub(desde)) / 1000;
            s.text(b"  ya hay IP propia: ");
            ip(s, k.ip);
            s.text(b"  (quedan ");
            s.dec(quedan);
            s.text(b" s). `red ip nueva` pide otra\n");
            return;
        }
    }
    if cliente().as_ref().is_some_and(|c| c.en_marcha()) {
        s.text(b"  ya hay una peticion DHCP en marcha: sale aqui abajo segun avanza\n");
        return;
    }
    let _ = bmo::red::armar();
    if !bmo::red::pase_abierto() {
        match bmo::red::abrir(60_000, 50) {
            Ok(_) => {}
            Err(bmo::red::NoAbre::Negado(no)) => {
                s.with_ink(INK_ERR);
                s.text(b"  [ip] NO se pudo abrir el pase: ");
                s.text(no.texto().as_bytes());
                s.byte(b'\n');
                s.with_ink(INK_PLAIN);
                return;
            }
            Err(bmo::red::NoAbre::Otro { code, flags }) => {
                s.text(b"  [ip] el kernel contesto algo que no conozco: ");
                s.dec(code as u64);
                s.byte(b'/');
                s.dec(flags as u64);
                s.byte(b'\n');
                return;
            }
        }
    }
    let mac = bmo::info(bmo::INFO_NET_MAC);
    let mut m = [0u8; 6];
    for (k, b) in m.iter_mut().enumerate() {
        *b = (mac >> ((5 - k) * 8)) as u8;
    }
    // El `xid` solo tiene que no repetirse entre dos peticiones seguidas.
    let xid = (bmo::ciclos() as u32) ^ 0x424D_4F58;
    let mut c = dhcp::Cliente::nuevo(m, xid);
    c.empezar();
    *cliente() = Some(c);
    unsafe {
        PINTADA = 1;
        RECHAZO = None;
        ENVIADAS = 0;
    }
    s.text(b"  [ip 1/3] pase ABIERTO. Pregunto por DHCP quien reparte IPs (DISCOVER)...\n");
}

/// **Una trama del buzon.** La llama `red_pase::drenar` con TODAS las que recoge.
pub(crate) fn oir(trama: &[u8]) {
    let Some(c) = cliente().as_mut() else { return };
    if !c.en_marcha() {
        return;
    }
    // ** Solo se apunta el rechazo de lo que venia A NUESTRO PUERTO (UDP 67 -> 68).
    // El ARP, el mDNS o el IGMP de la LAN tambien se rechazan aqui, y apuntarlos
    // taparia el unico porque que importa.
    let a_nuestro_puerto = trama.len() >= 42
        && trama[12..14] == [0x08, 0x00]
        && trama[23] == 17
        && trama[34..38] == [0, 67, 0, 68];
    if let Err(r) = c.oir(trama, ahora_ms()) {
        if a_nuestro_puerto {
            unsafe { RECHAZO = Some(r) };
        }
    }
}

/// **Un cuarto de segundo.** Envia lo que toque y pinta cada paso una vez.
pub(crate) fn latir(s: &mut Output) {
    let Some(c) = cliente().as_mut() else { return };
    let mut t = [0u8; 400];
    match c.latir(ahora_ms(), &mut t) {
        Ok(Some(n)) => {
            if bmo::red::enviar(&t[..n]).is_ok() {
                unsafe { ENVIADAS += 1 };
            } else {
                s.with_ink(INK_ERR);
                s.text(b"  [ip] la pregunta no entro en el buzon: el pase ya no esta abierto\n");
                s.with_ink(INK_PLAIN);
                *cliente() = None;
                return;
            }
        }
        Ok(None) => {}
        Err(r) => {
            s.text(b"  [ip] no se pudo armar la pregunta: ");
            s.text(r.texto().as_bytes());
            s.byte(b'\n');
        }
    }
    let estado = c.estado();
    let f = fase(estado);
    if f == unsafe { PINTADA } {
        return;
    }
    unsafe { PINTADA = f };
    match estado {
        dhcp::Estado::Pidiendo(o) => {
            s.text(b"  [ip 2/3] OFERTA de ");
            ip(s, o.servidor);
            s.text(b": ");
            ip(s, o.ip);
            s.text(b". La pido (REQUEST)...\n");
        }
        dhcp::Estado::Concedida { concesion: k, desde_ms } => {
            unsafe { ULTIMA = Some((k, desde_ms)) };
            s.with_ink(INK_GOOD);
            s.text(b"  [ip 3/3] CONCEDIDA: ");
            ip(s, k.ip);
            s.text(b" durante ");
            s.dec(k.segundos as u64);
            s.text(b" s\n");
            s.with_ink(INK_PLAIN);
            s.text(b"  router ");
            ip(s, k.router);
            s.text(b"   mascara ");
            ip(s, k.mascara);
            s.text(b"   dns ");
            ip(s, k.dns);
            s.text(b"\n  G2 hecho: BMO-X tiene IP propia. Vive en memoria y no se escribe en ningun sitio.\n");
            bmo::red::cerrar();
        }
        dhcp::Estado::Fallo(fallo) => {
            s.with_ink(INK_ERR);
            match fallo {
                dhcp::Fallo::Negada => s.text(b"  [ip] el servidor NEGO la IP (NAK)\n"),
                dhcp::Fallo::NadieContesta => {
                    s.text(b"  [ip] nadie contesto en 12 s (salieron ");
                    s.dec(unsafe { ENVIADAS } as u64);
                    s.text(b" preguntas). ");
                    match unsafe { RECHAZO } {
                        Some(r) => {
                            s.text(b"Lo que llego a nuestro puerto se rechazo: ");
                            s.text(r.texto().as_bytes());
                            s.byte(b'\n');
                        }
                        None => s.text(b"No llego ni una respuesta a nuestro puerto 68.\n"),
                    }
                }
            }
            s.with_ink(INK_PLAIN);
            bmo::red::cerrar();
        }
        _ => {}
    }
}

fn ip(s: &mut Output, v: [u8; 4]) {
    for (k, b) in v.iter().enumerate() {
        s.dec(*b as u64);
        if k < 3 {
            s.byte(b'.');
        }
    }
}
