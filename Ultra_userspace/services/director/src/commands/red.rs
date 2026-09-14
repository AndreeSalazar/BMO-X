//! **EL INFORME DE LA RED**, y los dos ayudantes que solo el usa.
//!
//! [consumo] APARATO   `red::armar` deja el ANILLO DE RECEPCION armado en la
//!                     tarjeta: a partir de ahi el aparato recibe por su
//!                     cuenta, sin que nadie vuelva a pedirlo (L6h)
//!           [!] MEZCLA -- declara la PEOR (L6h). El informe de la red solo
//!                         lee contadores; `red rx` ARMA el anillo de
//!                         recepcion y lo deja armado. Misma costura que
//!                         `disco.rs`.
//!
//! ## Por que esto ya no vive en `reports.rs` (2026-08-28)
//!
//! Porque `reports.rs` cruzo las mil lineas de codigo y L6a lo paro. Y de los
//! nueve informes que habia dentro, **este es el que estaba creciendo**: es el
//! unico con un plan abierto detras --`RED_MAESTRO.md`, pasos 2 a 4-- asi que
//! cortar por aqui no es cortar por donde cabe, es cortar por donde va a seguir
//! moviendose. Los otros ocho llevan semanas quietos.
//!
//! ** Y el corte es limpio de verdad, no de milagro: `mac_hex` y `link` no los
//! llamaba nadie mas. Se van enteros con el, y `reports.rs` no pierde ninguna
//! funcion que otro use.
//!
//! [!] Lo que se queda fuera a proposito: `section` y `label` siguen en
//! `reports.rs`. Son el ESTILO de la rejilla, no el informe, y duplicarlas aqui
//! seria como se consiguen dos maneras distintas de pintar la misma fila.
//!
//! [!] Y esto no transmite. Son campos de informe: mirar la red no es un
//! privilegio, `CR.TE` sigue apagado en el kernel, y desde aqui no se pone un
//! byte en el cable.

use core::sync::atomic::{AtomicU32, AtomicU64, Ordering};

use bmo_userland as bmo;

use crate::commands::tabla::{label, section};
use crate::scene::output::Output;

/// **EL INFORME DE LA RED.** `red` entero, o una sola pregunta con argumento.
///
/// === Por que este informe se lee de abajo arriba ===
///
/// Porque asi se depura un cable. Las cuatro preguntas van en el orden en que
/// se caen, y cada una **solo tiene sentido si la anterior dijo si**:
///
/// ```text
///   hay TARJETA?      no -> el problema es el PCI o el BAR, y nada mas importa
///   hay ENLACE?       no -> es el cable, el switch o el otro extremo
///   estamos ESCUCHANDO?  no -> el receptor no esta armado. `net rx` en Ring 0
///   llegan TRAMAS?    no -> hay enlace, escuchamos, y nadie habla
/// ```
///
/// ** Un panel que contestara *"red: 0"* obligaria a adivinar cual de las cuatro
/// fallo. Ese es todo el motivo de que sean siete campos y no uno con banderas.
///
/// === Y el `PHYstatus` va CRUDO ===
///
/// El driver ya tomo esa decision y aqui se respeta: *"se guarda sin interpretar
/// ademas de interpretado -- el dia que un bit no cuadre, el byte entero es la
/// prueba y las funciones son la opinion"*. Un diagnostico que solo ensena la
/// opinion no ayuda el dia que la opinion falle.
///
/// [!] **Nada de esto transmite.** Son campos de informe: mirar la red no es un
/// privilegio. `CR.TE` sigue apagado en el kernel y no se pone un byte en el
/// cable desde aqui.
///
/// === Que es de AHORA y que es del ARRANQUE (2026-08-28) ===
///
/// ```text
///    PHYstatus crudo        del APARATO, leido al teclear la orden
///    perdidas (MPC)         del APARATO
///    enlace / megabits      del ARRANQUE, cacheado: lo pinta `splash`, que
///                           se repinta, y un panel a 60 Hz no puede tocar
///                           el BAR de una NIC 60 veces por segundo
///    cogidas / reparto      contadores del driver -- **solo suben cuando
///                           alguien sondea**, y quien sondea es `red rx`
/// ```
///
/// ** Hasta el 28-08 el crudo tambien venia cacheado, y este informe remitia a
/// *"la orden `net` del shell de Ring 0"* para releer -- **un sitio al que el
/// dueno no vuelve**. La prueba del paso 1 --*desenchufa el cable y mira si el
/// enlace se cae*-- no se podia hacer desde donde el trabaja.
#[inline(never)]
pub(crate) fn report_net(s: &mut Output, what: &[u8]) {
    let present = bmo::info(bmo::INFO_NET_PRESENTE) != 0;
    let mac = bmo::info(bmo::INFO_NET_MAC);
    let mbit = bmo::info(bmo::INFO_NET_MEGABITS);
    let phy = bmo::info(bmo::INFO_NET_PHY_CRUDO);
    let armed = bmo::info(bmo::INFO_NET_RX_ARMADO) != 0;
    let frames_rx = bmo::info(bmo::INFO_NET_RX_TRAMAS);

    // Una sola pregunta, para cuando ya sabes cual quieres.
    if what == b"mac" {
        label(s, b"MAC");
        if present { mac_hex(s, mac); } else { s.text(b"no hay tarjeta"); }
        s.byte(b'\n');
        return;
    }
    if what == b"link" {
        label(s, b"enlace");
        link(s, present, mbit);
        s.byte(b'\n');
        return;
    }
    if what == b"frames" {
        label(s, b"tramas");
        s.dec(frames_rx);
        if !armed { s.text(b"   (el receptor NO esta armado)"); }
        s.byte(b'\n');
        return;
    }
    // *** `red rx` -- ARMAR EL RECEPTOR, DESDE DONDE SE TRABAJA (2026-08-24)
    //
    // ** El syscall existia (`RED_OP_ARMAR`), el envoltorio de Ring 3 existia
    // (`bmo::red::armar`), y el panel decia **"net rx en Ring 0"** -- o sea que
    // mandaba al dueno a un sitio del que no se vuelve. Cuarta vez en la misma
    // sesion que algo se escribe donde el no puede alcanzarlo.
    //
    // *** Y era el ULTIMO ESLABON: sin esto no se puede armar el receptor, sin
    // receptor no llegan tramas, y sin tramas no hay ARP, ni IP, ni TCP. Toda
    // la escalera de red estaba bloqueada por una linea que no existia.
    if what == b"rx" {
        // ** Se cuenta el TOTAL antes y despues, y no lo que devuelve
        // `sondear()`: `RED_OP_ARMAR` ya sondea dentro y se lleva las tramas
        // nuevas, asi que el Ryzen ensenaba 0 con 16 cogidas (2026-09-13). La
        // misma correccion que `commands::system::net`.
        let antes = frames_rx;
        match bmo::red::armar() {
            bmo::red::Armado::Ok => {
                bmo::red::sondear();
                let total = bmo::info(bmo::INFO_NET_RX_TRAMAS);
                label(s, b"receptor");
                s.text(b"ARMADO");
                s.byte(b'\n');
                label(s, b"nuevas");
                s.dec(total.saturating_sub(antes));
                s.text(b" tramas desde la ultima mirada   (total ");
                s.dec(total);
                s.text(b")\n");
                label(s, b"malas");
                s.dec(bmo::info(bmo::INFO_NET_RX_MALAS));
                s.text(b" devueltas a la tarjeta (error, partida o enana)\n");
                if total == 0 {
                    // ** Cero EN TOTAL justo al armar es LO ESPERADO, y decirlo
                    // evita la tarde que se pierde buscando un bug en un driver
                    // que funciona. Es la leccion escrita del paso 1.
                    s.text(b"    ninguna todavia: vuelve a escribir `red rx` en unos segundos\n");
                } else if total == antes {
                    s.text(b"    nada nuevo desde la ultima mirada: la red esta callada ahora\n");
                }
            }
            // [!] Sin cable NO es un fallo del anillo, y por eso tiene su
            // propio motivo: no van a llegar tramas por correcto que sea todo
            // lo demas.
            bmo::red::Armado::SinEnlace => {
                s.text(b"    el enlace esta ABAJO: enchufa el cable antes de armar nada\n");
            }
            bmo::red::Armado::NoArma => {
                s.text(b"    el receptor no se pudo armar -- F11 dice por que\n");
            }
            bmo::red::Armado::SinTarjeta => {
                s.text(b"    no hay tarjeta que este kernel sepa leer\n");
            }
            bmo::red::Armado::Raro(v) => {
                s.text(b"    el kernel contesto algo que no conozco: ");
                s.dec(v);
                s.byte(b'\n');
            }
        }
        return;
    }

    if what == b"phy" {
        label(s, b"PHYstatus");
        s.text(b"0x");
        s.hex(phy, 2);
        s.text(b"   (crudo, sin interpretar)");
        s.byte(b'\n');
        return;
    }

    section(s, b"RED");

    // 1. Hay tarjeta? Si no, lo demas no significa nada y se dice.
    label(s, b"tarjeta");
    if !present {
        s.text(b"NINGUNA reconocida en el PCI");
        s.byte(b'\n');
        s.text(b"    (sin tarjeta, el resto del informe no significa nada)\n");
        return;
    }
    let vd = bmo::info(bmo::INFO_NET_VENDOR_DEVICE);
    s.text(b"vendor:device 0x");
    s.hex(vd, 8);
    // El unico nombre que se traduce, porque es el que hay en esta placa y
    // reconocerlo de un vistazo ahorra buscarlo.
    if vd == 0x10EC_8168 { s.text(b"   (Realtek RTL8168)"); }
    s.byte(b'\n');

    let pci = bmo::info(bmo::INFO_NET_PCI);
    label(s, b"en el bus");
    s.dec((pci >> 16) & 0xFF);
    s.byte(b':');
    s.dec((pci >> 8) & 0xFF);
    s.byte(b'.');
    s.dec(pci & 0xFF);
    s.byte(b'\n');

    label(s, b"MAC");
    mac_hex(s, mac);
    s.byte(b'\n');

    // 2. Hay enlace?
    label(s, b"enlace");
    link(s, present, mbit);
    s.text(b"   (del ARRANQUE)");
    s.byte(b'\n');

    label(s, b"PHYstatus");
    s.text(b"0x");
    s.hex(phy, 2);
    s.text(b"   (crudo, LEIDO AHORA: la prueba, no la opinion)\n");

    // *** LA CONTRADICCION, DICHA. Ver `bmo::red::PHY_ENLACE_ARRIBA`.
    //
    // ** Dos filas que salen de dos instantes distintos y nadie las compara son
    // dos filas que un dia se contradicen en silencio. La de arriba es la foto
    // del arranque; esta viene del aparato hace un microsegundo. Cuando no
    // cuadran, **manda el crudo** y hay que decirlo aqui, no en la cabeza de
    // quien mira.
    let vivo = phy & bmo::red::PHY_ENLACE_ARRIBA != 0;
    if mbit != 0 && !vivo {
        s.text(b"    [!] el enlace se CAYO despues de arrancar: el crudo manda\n");
    }
    if mbit == 0 && vivo {
        s.text(b"    [!] hay enlace AHORA que no habia al arrancar\n");
    }

    // 3. Estamos escuchando? 4. Llega algo?
    label(s, b"receptor");
    if armed { s.text(b"ARMADO"); } else { s.text(b"apagado   (escribe `red rx` para armarlo)"); }
    s.byte(b'\n');

    label(s, b"cogidas");
    s.dec(frames_rx);
    s.text(b" tramas, ");
    s.dec(bmo::info(bmo::INFO_NET_RX_BYTES));
    s.text(b" bytes");
    if armed && frames_rx == 0 {
        // *** ESTA LINEA DECIA "escuchando y nadie habla todavia" Y ERA FALSA.
        //
        // ** El contador solo sube cuando alguien MIRA el anillo, y el unico
        // que mira es `red rx` --`RED_OP_SONDEAR`--. `red` a secas lee la
        // casilla y no toca el anillo, asi que este cero **no puede subir por
        // mucho que hable la red**: no dice que nadie hable, dice que nadie ha
        // mirado. Son dos sistemas distintos y mandan a sitios distintos.
        //
        // [!] Es el mismo defecto que la pantalla azul del 26-08: un cero
        // presentado como un hecho. Ver `docs/metal/METAL_RED_PASO_1.md`.
        s.text(b"   (sin sondear: `red rx` es quien mira el anillo)");
    }
    s.byte(b'\n');

    // *** LA FILA QUE HACE QUE ESTO SEA UNA MEDIDA Y NO UN VOLCADO.
    //
    // ** Un contador propio solo puede contar lo que se COGIO. Lo que llego
    // y se tiro por no haber descriptor libre no lo sabe el software: lo
    // lleva el silicio, en `MPC`. Sin esta fila, "40 tramas" suena igual si
    // por detras se perdieron cuatro que cuatro mil -- y son dos sistemas
    // distintos: uno anda y el otro tiene el anillo pequeno.
    //
    // [!] El cero se dice CON SU NOMBRE. Una fila que desaparece cuando vale
    // cero deja al que mira sin saber si es que no se perdio nada o es que
    // nadie lo mide, y esas dos cosas piden trabajos distintos.
    if armed {
        let perdidas = bmo::info(bmo::INFO_NET_RX_PERDIDAS);
        label(s, b"perdidas");
        s.dec(perdidas);
        if perdidas == 0 {
            s.text(b"   (la tarjeta no tiro ninguna)");
        } else {
            s.text(b"   [!] llegaron y no habia descriptor libre");
        }
        s.byte(b'\n');

        // ** CUATRO CASILLAS Y NO UNA. En una red domestica en reposo lo que
        // llega es ARP y broadcast; si sale IPv4 sin que nadie haya pedido
        // nada, hay alguien hablando. Un solo contador no separa "el cable
        // esta vivo" de "esta red tiene vecinos".
        let t = bmo::info(bmo::INFO_NET_RX_TIPOS);
        label(s, b"reparto");
        s.text(b"ARP ");
        s.dec(t & 0xFFFF);
        s.text(b"  IPv4 ");
        s.dec((t >> 16) & 0xFFFF);
        s.text(b"  IPv6 ");
        s.dec((t >> 32) & 0xFFFF);
        s.text(b"  otros ");
        s.dec((t >> 48) & 0xFFFF);
        s.byte(b'\n');
        if frames_rx > 0 && (t >> 16) & 0xFFFF == 0 && (t >> 32) & 0xFFFF == 0 {
            s.text(b"    solo ARP/broadcast: el cable vive y nadie habla contigo\n");
        }
    }

    // ** Y como se transmite, dicho aqui y no en un README.
    if bmo::red::pase_abierto() {
        s.text(b"    transmitir: PASE ABIERTO -- `red pase` dice como va\n");
    } else {
        s.text(b"    transmitir: solo con pase -- `red abrir 60` y luego `red arp <ip del router>`\n");
    }
}

// ===================================================================
//  *** EL GATE RED DESDE EL ESCRITORIO (E3, 2026-09-13)
// ===================================================================
//
// ** La prueba de E3 es la que no se puede fingir: preguntar por ARP al router
// y ver SU respuesta, dirigida a nuestra MAC. El router solo contesta si la
// trama llego al cable.

/// A que IP se pregunto por ARP (big-endian), `0` = a nadie.
static PREGUNTADA: AtomicU32 = AtomicU32::new(0);
/// La MAC que contesto por esa IP.
static RESPUESTA_MAC: AtomicU64 = AtomicU64::new(0);
static RESPUESTAS: AtomicU64 = AtomicU64::new(0);
static RECIBIDAS: AtomicU64 = AtomicU64::new(0);
/// **Quien habla ARP en este cable**: las cuatro primeras IP de origen distintas.
///
/// ** La foto del 13-09 dijo `sin respuesta todavia` con 56 tramas en el buzon.
/// Un router pregunta por ARP todo el rato, asi que su IP ya estaba ahi dentro:
/// si no es la que se pregunto, el fallo no es el cable, es la IP.
static VECINOS: [AtomicU32; 4] = [AtomicU32::new(0), AtomicU32::new(0), AtomicU32::new(0), AtomicU32::new(0)];

fn apuntar_vecino(ip: u32) {
    if ip == 0 {
        return;
    }
    for v in &VECINOS {
        let hay = v.load(Ordering::Relaxed);
        if hay == ip {
            return;
        }
        if hay == 0 {
            v.store(ip, Ordering::Relaxed);
            return;
        }
    }
}

fn ip_texto(s: &mut Output, ip: u32) {
    for k in 0..4 {
        s.dec(((ip >> ((3 - k) * 8)) & 0xFF) as u64);
        if k < 3 {
            s.byte(b'.');
        }
    }
}

/// **Vacia el buzon.** Lo llama el bucle cuatro veces por segundo. Sin syscall.
pub(crate) fn drenar() {
    if !bmo::red::pase_abierto() {
        return;
    }
    let mut t = [0u8; 1514];
    for _ in 0..16 {
        let Some(n) = bmo::red::recibir(&mut t) else { break };
        RECIBIDAS.fetch_add(1, Ordering::Relaxed);
        if n >= 42 && t[12] == 0x08 && t[13] == 0x06 {
            apuntar_vecino(u32::from_be_bytes([t[28], t[29], t[30], t[31]]));
        }
        // ARP (0x0806), respuesta (oper 2), y de la IP por la que se pregunto.
        if n >= 42 && t[12] == 0x08 && t[13] == 0x06 && t[20] == 0 && t[21] == 2 {
            let spa = u32::from_be_bytes([t[28], t[29], t[30], t[31]]);
            if spa != 0 && spa == PREGUNTADA.load(Ordering::Relaxed) {
                let mut mac = 0u64;
                for &b in &t[22..28] {
                    mac = (mac << 8) | b as u64;
                }
                RESPUESTA_MAC.store(mac, Ordering::Relaxed);
                RESPUESTAS.fetch_add(1, Ordering::Relaxed);
            }
        }
    }
}

/// `red abrir`, `red arp`, `red pase` y `red cerrar`. `false` si no es ninguna.
pub(crate) fn orden_pase(s: &mut Output, what: &[u8]) -> bool {
    let (orden, resto) = partir(what);
    match orden {
        b"abrir" => abrir(s, resto),
        b"arp" => arp(s, resto),
        b"pase" => pase(s),
        b"cerrar" => {
            bmo::red::cerrar();
            s.text(b"  pase CERRADO: grifo cerrado, y el buzon ya es una lapida\n");
        }
        _ => return false,
    }
    true
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
            s.text(b"  y el radar mira cada 4 ms. Prueba: `red arp <ip del router>`\n");
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

fn arp(s: &mut Output, resto: &[u8]) {
    let Some(ip) = ipv4(resto) else {
        s.text(b"  uso: red arp 192.168.1.1\n");
        return;
    };
    let mac = bmo::info(bmo::INFO_NET_MAC);
    let mut t = [0u8; 42];
    t[0..6].fill(0xFF);
    for k in 0..6 {
        t[6 + k] = (mac >> ((5 - k) * 8)) as u8;
    }
    t[12] = 0x08;
    t[13] = 0x06;
    // Ethernet, IPv4, 6 y 4 bytes, PREGUNTA.
    t[14..22].copy_from_slice(&[0, 1, 0x08, 0x00, 6, 4, 0, 1]);
    let (sha, resto_t) = t[22..].split_at_mut(6);
    sha.copy_from_slice(&t_mac(mac));
    // ** Origen 0.0.0.0: una SONDA ARP (RFC 5227). No hace falta tener IP para
    // preguntar, y no se le ensucia la tabla a nadie con una IP inventada.
    resto_t[0..4].fill(0);
    resto_t[4..10].fill(0);
    resto_t[10..14].copy_from_slice(&ip.to_be_bytes());
    PREGUNTADA.store(ip, Ordering::Relaxed);
    RESPUESTAS.store(0, Ordering::Relaxed);
    RESPUESTA_MAC.store(0, Ordering::Relaxed);
    match bmo::red::enviar(&t) {
        Ok(()) => {
            s.text(b"  pregunta ARP dejada en el buzon: sale en el siguiente latido.\n");
            s.text(b"  `red pase` en un segundo: si el router contesta, la trama LLEGO al cable\n");
        }
        Err(bmo::red::puerta::buzon::NoEnvia::Revocado) => {
            s.text(b"  no hay pase abierto: `red abrir 60` primero\n");
        }
        Err(bmo::red::puerta::buzon::NoEnvia::Llena) => {
            s.text(b"  el buzon esta lleno: vuelve a intentarlo en un momento\n");
        }
        Err(bmo::red::puerta::buzon::NoEnvia::Larga) => {
            s.text(b"  la trama es demasiado larga\n");
        }
    }
}

fn pase(s: &mut Output) {
    let e = bmo::red::estado();
    let abierto = e >> 63 != 0;
    label(s, b"pase");
    if abierto {
        s.text(b"ABIERTO");
    } else {
        let motivo = ((e >> 56) & 0x7F) as u32;
        s.text(b"cerrado");
        if let Some(m) = bmo::red::puerta::radar::Motivo::desde_codigo(motivo) {
            s.text(b" -- ");
            s.text(m.texto().as_bytes());
        }
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
    if VECINOS[0].load(Ordering::Relaxed) != 0 {
        label(s, b"hablan ARP");
        for v in &VECINOS {
            let ip = v.load(Ordering::Relaxed);
            if ip != 0 {
                ip_texto(s, ip);
                s.text(b"  ");
            }
        }
        s.text(b"\n    el router suele ser la que acaba en .1 o .254 de esas\n");
    }
    let ip = PREGUNTADA.load(Ordering::Relaxed);
    if ip != 0 {
        label(s, b"ARP");
        for k in 0..4 {
            s.dec(((ip >> ((3 - k) * 8)) & 0xFF) as u64);
            if k < 3 {
                s.byte(b'.');
            }
        }
        if RESPUESTAS.load(Ordering::Relaxed) > 0 {
            s.text(b" CONTESTO desde ");
            mac_hex(s, RESPUESTA_MAC.load(Ordering::Relaxed));
            s.text(b"\n    *** la trama salio al cable y el router la oyo: E3 hecho\n");
        } else {
            s.text(b" sin respuesta todavia\n");
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

fn ipv4(b: &[u8]) -> Option<u32> {
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

/// Los seis bytes con dos puntos, del mas significativo al menos.
fn mac_hex(s: &mut Output, mac: u64) {
    let mut i = 6;
    while i > 0 {
        i -= 1;
        s.hex((mac >> (i * 8)) & 0xFF, 2);
        if i > 0 { s.byte(b'-'); }
    }
}

/// `ARRIBA, 100 Mbit` o `ABAJO`. El cero de megabits **es** la respuesta.
fn link(s: &mut Output, present: bool, mbit: u64) {
    if !present {
        s.text(b"no hay tarjeta");
    } else if mbit == 0 {
        s.text(b"ABAJO   (sin cable, o el otro extremo apagado)");
    } else {
        s.text(b"ARRIBA, ");
        s.dec(mbit);
        s.text(b" Mbit");
    }
}
