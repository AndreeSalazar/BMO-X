//! **EL INFORME DE LA RED**, y los dos ayudantes que solo el usa.
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

use bmo_userland as bmo;

use crate::commands::reports::{label, section};
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
        match bmo::red::armar() {
            bmo::red::Armado::Ok => {
                let n = bmo::red::sondear();
                label(s, b"receptor");
                s.text(b"ARMADO");
                s.byte(b'\n');
                label(s, b"esta vuelta");
                s.dec(n);
                s.text(b" tramas");
                if n == 0 {
                    // ** Cero en la primera vuelta es LO ESPERADO, y decirlo
                    // evita la tarde que se pierde buscando un bug en un driver
                    // que funciona. Es la leccion escrita del paso 1.
                    s.text(b"   (normal: el anillo se acaba de armar)");
                }
                s.byte(b'\n');
                s.text(b"    vuelve a escribir `red rx` en unos segundos\n");
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
    if armed { s.text(b"ARMADO"); } else { s.text(b"apagado   (net rx en Ring 0)"); }
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
        // presentado como un hecho. Ver `docs/metal/PRUEBA_RED_PASO_1.md`.
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

    // ** Y lo que NO hace, dicho aqui y no en un README.
    s.text(b"    transmitir: CERRADO a proposito (CR.TE apagado)\n");
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
