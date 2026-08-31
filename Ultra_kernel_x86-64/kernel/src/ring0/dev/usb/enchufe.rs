//! **QUE PASA CUANDO ALGO SE ENCHUFA O SE DESENCHUFA**, con la maquina ya
//!
//! [carril]  AMARILLO  que pasa al enchufar en caliente
//! encendida. Y el barrido que lo descubre.
//!
//! ## Por que soy un fichero (L6b)
//!
//! Porque es la pregunta que `arranque.rs` no contesta: aquel enumera lo que
//! habia al encender, y esto **lo que llega despues**.
//!
//! ## *** Y AQUI ESTA LA LECCION QUE COSTO UN TECLADO MUDO
//!
//! Un endpoint de interrupcion **no avisa: contesta cuando le preguntan**. Y el
//! evento ES EL PERMISO para volver a encolar -- perder uno no pierde una
//! pulsacion, **para la bomba**, y el teclado se queda mudo hasta que alguien
//! reinicia.
//!
//! Por eso el barrido corre cada **500 ms** aunque no haya pasado nada: es la
//! red que recoge lo que un aviso perdido habria dejado caer. Un camino de
//! recuperacion que solo se ejecuta cuando ya es tarde no esta escrito, esta
//! redactado (ley 14).
//!
//! ** El reparto es MOVER TEXTO (L6d): ni una linea cambia de contenido.

use super::*;

/// Cuantos avisos de puerto se atienden por bombeo.
///
/// No es un numero de gusto: adoptar un aparato lleva esperas de verdad --hasta
/// seis reintentos de 50 ms-- y esto se recorre tambien **desde dentro de un
/// syscall**. Cuatro acota lo peor que le puede pasar al que pidio una tecla, y
/// lo que quede en la cola espera 4 ms: el hilo del bus late a 250 Hz.
const MAX_AVISOS_POR_BOMBEO: u8 = 4;

/// **Atiende los avisos de cambio de puerto: enchufes y desenchufes.**
///
/// * ESTO ERA UN `if let`, y eso era medio bug. `bmo_xhci` guardaba un solo aviso
/// y aqui se recogia uno por bombeo; ahora guarda una cola
/// ([`bmo_xhci::avisos`]) y hay que **insistir hasta el `None`**. Un `if let`
/// sobre una cola reintroduce por arriba justo el retraso que la cola quita por
/// abajo: el desenchufe se atenderia en una vuelta y el enchufe en la siguiente,
/// y entre las dos el sistema seguiria creyendo que tiene un teclado que ya no
/// esta.
///
/// * La enumeracion del arranque era una carrera de UN SOLO INTENTO. El
/// bucle recorre los puertos una vez y lo que no estuviera listo en ese
/// instante se perdia **hasta el siguiente reinicio** -- y un raton con
/// firmware RGB tarda en engancharse mas que un teclado.
///
/// De ahi el sintoma que no encajaba con nada: unas veces arrancaba el
/// teclado y otras el raton, nunca los dos, sin cambiar una linea entre
/// arranque y arranque. No era hardware intermitente: era quien llegaba a
/// tiempo.
///
/// Actuar aqui es seguro por dos cosas que ya estan puestas: este camino corre
/// con el CR3 del kernel (ver la cabecera de `poll_ascii`), y los informes del
/// aparato que YA bombea no se pierden mientras se enumera el nuevo porque el
/// aparcadero de `bmo_xhci` los guarda.
pub(crate) fn atender_avisos() {
    let mut atendidos = 0u8;
    while let Some((puerto, conectado)) = bmo_xhci::tomar_cambio_puerto() {
        if conectado {
            atender_enchufe(puerto);
        } else {
            atender_desenchufe(puerto);
        }
        atendidos += 1;
        if atendidos >= MAX_AVISOS_POR_BOMBEO {
            break;
        }
    }
}

/// Enchufaron algo en `puerto` (1-based, tal cual lo manda el xHC).
fn atender_enchufe(puerto: u8) {
    // `port_reset` y compania trabajan en indice 0-based; el Port ID del
    // evento es 1-based. Restar aqui y no en el driver: el que traduce es el
    // que conoce las dos convenciones.
    let idx = puerto.saturating_sub(1);
    let adoptado = unsafe {
        let hid = &mut *core::ptr::addr_of_mut!(HID);
        hid.adoptar_puerto(idx)
    };
    if adoptado {
        crate::ring0::cabina::info("usb", "puerto: ENCHUFADO y adoptado", puerto as u64);
        unsafe { refrescar_presencia() };

        // ** ADOPTADO NO ES LO MISMO QUE VIVO, y el metal del 12-08 lo
        // enseno: la adopcion salio bien --esta linea de arriba-- y el
        // teclado seguia sin escribir.
        //
        // Entre las dos cosas hay UN paso mas: encolar la transferencia
        // de interrupcion. `bmo_uhid::arrancar_bombas` ya detecta que no
        // pudo y lo dice... por `hal().log()`, que va al panel del
        // arranque -- **tapado por el compositor**. O sea que el unico
        // testigo de "enumero y quedo mudo" se pintaba donde nadie lo ve.
        //
        // Aqui se pregunta el hecho y se apunta en CABINA. Un `k-` es un
        // teclado enumerado con el endpoint en Running y **sin nadie que
        // le pida nada**, que es exactamente el sintoma que se sufrio.
        let (bomba_k, bomba_r, _, _) = panel::reparto_stats();
        let bombas = ((bomba_k as u64) << 8) | bomba_r as u64;
        crate::ring0::cabina::bits("usb", "  ...y su bomba encolada k:r", bombas);
        if !bomba_k {
            // Se dice aparte y como AVISO porque es LA causa de que un
            // teclado adoptado no escriba, y merece color propio.
            crate::ring0::cabina::warn(
                "usb",
                "  ...pero el TECLADO quedo MUDO: sin transferencia encolada",
                puerto as u64,
            );
        }
        return;
    }

    // -- ** NO ADOPTAR TIENE TRES MOTIVOS, Y SALIAN LOS TRES IGUAL ---------
    //
    // `puerto: ENCHUFADO, nada que adoptar` era tecnicamente cierto y contaba
    // la historia equivocada. El dueno volvia a enchufar el teclado, salia esta
    // linea, y la verdad era *"sigo creyendo que lo tengo"*. Se le puso al lado
    // el `creo tener teclado:raton` para que la mentira se pudiera ver -- y se
    // vio: `=257`, o sea `0x101`, o sea "tengo los dos", con el dueno mirando un
    // teclado que no escribia.
    //
    // Pero ver la mentira no es saber por que. `adoptar_puerto` devuelve `false`
    // por tres razones que no se parecen en nada:
    //
    //   1. **completo()**: creo tenerlo todo. Se va sin tocar el bus. Si esto
    //      sale con un aparato que no funciona delante, hay un FANTASMA -- y es
    //      lo que el barrido repara.
    //   2. **el puerto esta cerrado**: tomado, o con los tres intentos gastados.
    //   3. **enumere y no habia nada mio**: un pendrive, unos auriculares. Este
    //      es el unico de los tres que es normal.
    //
    // Se preguntan aqui, en este orden, porque es el orden en que decide el
    // driver. Un motivo por linea: un diagnostico que junta tres causas debajo
    // de una frase no es un diagnostico, es una foto que hay que interpretar.
    let (k, m, cerrado, intentos) = unsafe {
        let hid = &*core::ptr::addr_of!(HID);
        (
            hid.has_kbd(),
            hid.has_mouse(),
            !hid.puertos().se_puede_intentar(idx),
            hid.puertos().intentos(idx),
        )
    };
    let estado = ((k as u64) << 8) | m as u64;
    if k && m {
        crate::ring0::cabina::info("usb", "puerto: ENCHUFADO, ya creo tenerlo todo", puerto as u64);
        crate::ring0::cabina::bits("usb", "  ...creo tener teclado:raton", estado);
    } else if cerrado {
        crate::ring0::cabina::warn("usb", "puerto: ENCHUFADO pero CERRADO por intentos", puerto as u64);
        crate::ring0::cabina::id("usb", "  ...intentos gastados en el", intentos as u64);
    } else {
        crate::ring0::cabina::info("usb", "puerto: ENCHUFADO, enumere y no era mio", puerto as u64);
        crate::ring0::cabina::bits("usb", "  ...creo tener teclado:raton", estado);
    }
}

/// Desenchufaron algo de `puerto` (1-based).
fn atender_desenchufe(puerto: u8) {
    // * Desenchufar LIBERA el puerto y le devuelve los intentos. Sin
    // esto, enchufar y desenchufar tres veces dejaria un puerto
    // inservible hasta el siguiente reinicio: los intentos son para
    // "este aparato tarda", no para "este puerto esta prohibido".
    //
    // ** Y DESDE EL 08-12, SUELTA TAMBIEN EL APARATO. La mitad que
    // faltaba: sin ella el teclado desenchufado seguia contando como
    // presente y no volvia jamas. Ver `bmo_uhid::soltar_puerto`.
    let solto = unsafe {
        let hid = &mut *core::ptr::addr_of_mut!(HID);
        hid.soltar_puerto(puerto.saturating_sub(1))
    };
    crate::ring0::cabina::warn("usb", "puerto: algo se DESENCHUFO", puerto as u64);
    if solto {
        // Two different pieces of news, and they used to be one. A
        // device leaving is what has to be REPAIRED; an empty port
        // changing state is noise.
        crate::ring0::cabina::warn("usb", "  ...y ERA UN APARATO MIO: lo suelto", puerto as u64);
        unsafe { refrescar_presencia() };
    }
}

/// Cada cuanto se comparan los puertos de verdad con lo que el driver cree.
///
/// Medio segundo. Barrer cuesta una lectura de `PORTSC` por puerto --unos pocos
/// accesos a MMIO-- asi que podria ir mas rapido; el motivo de que no vaya es
/// otro. El barrido es la RED, no el camino: los avisos siguen atendiendose en
/// el mismo bombeo en que llegan, en 4 ms. Medio segundo es lo que tarda en
/// repararse algo de lo que nadie se entero, y para una mano humana enchufando
/// un cable eso es instantaneo.
const BARRIDO_PERIODO_MS: u64 = 500;

/// TSC del ultimo barrido. En cero al arrancar, y eso hace que el primer bombeo
/// barra -- que es justo cuando mas falta hace: el firmware acaba de soltar el
/// bus y `init()` solo lo recorrio una vez.
static mut BARRIDO_ULTIMO: u64 = 0;
/// Cuantos barridos se han hecho y cuantos repararon algo. Para el panel: si el
/// primero sube y el segundo no, el bus esta sano.
static mut BARRIDOS: u64 = 0;
static mut BARRIDOS_UTILES: u64 = 0;

/// `(barridos hechos, barridos que repararon algo)`.
pub fn barrido_stats() -> (u64, u64) {
    unsafe { (BARRIDOS, BARRIDOS_UTILES) }
}

/// **La red: comparar con los puertos de verdad cada medio segundo.**
///
/// Todo lo demas en este camino depende de haberse enterado de un aviso. Esto no
/// depende de nada, y por eso es lo unico que cumple lo que se pidio: *"mi Kernel
/// tiene que tener siempre abierto las puertas"*. Un aviso que se pierda --por un
/// desborde de la cola, por un CSC que el firmware limpio antes que nosotros, por
/// un puerto que cambio mientras la maquina estaba en la BIOS-- deja de ser una
/// puerta cerrada hasta el reinicio y pasa a ser medio segundo de retraso.
///
/// La decision de que hacer con cada puerto NO esta aqui: vive en
/// `bmo_uhid::barrido`, que se prueba sin encender la maquina. Un barrido
/// automatico que se equivoque resetea el puerto del teclado que esta
/// escribiendo, y eso no se puede dejar a que salga bien en el metal.
pub(crate) fn barrer_si_toca() {
    use crate::ring0::task::scheduler;
    let hz = scheduler::tsc_freq();
    if hz == 0 {
        // Sin TSC medido no hay forma de saber cuanto ha pasado. Barrer en cada
        // bombeo seria 250 barridos por segundo; no barrer es lo que habia antes.
        return;
    }
    let ahora = scheduler::rdtsc();
    unsafe {
        if ahora.wrapping_sub(BARRIDO_ULTIMO) < hz / 1000 * BARRIDO_PERIODO_MS {
            return;
        }
        BARRIDO_ULTIMO = ahora;
        BARRIDOS = BARRIDOS.wrapping_add(1);
    }

    let r = unsafe {
        let hid = &mut *core::ptr::addr_of_mut!(HID);
        hid.barrer()
    };
    if !r.hubo_algo() {
        // Un barrido que no repara nada se calla. Una linea que sale dos veces
        // por segundo es una linea que se deja de leer, y CABINA tiene 82 sitios.
        return;
    }
    unsafe {
        BARRIDOS_UTILES = BARRIDOS_UTILES.wrapping_add(1);
        refrescar_presencia();
    }
    if r.soltados != 0 {
        // El fantasma: yo creia tener un aparato en un puerto que esta vacio.
        // O sea que su desconexion no me llego. Es la causa exacta del
        // `nada que adoptar` que no se podia explicar.
        crate::ring0::cabina::warn("usb", "BARRIDO: habia un FANTASMA, lo solte", r.soltados as u64);
    }
    if r.adoptados != 0 {
        crate::ring0::cabina::info("usb", "BARRIDO: adopte lo que un aviso perdido dejo fuera", r.adoptados as u64);
    }
    if r.reabiertos != 0 {
        crate::ring0::cabina::info("usb", "BARRIDO: puertos vacios reabiertos", r.reabiertos as u64);
    }
    if r.fallidos != 0 {
        crate::ring0::cabina::info("usb", "BARRIDO: intentos que no dieron nada", r.fallidos as u64);
    }
}
