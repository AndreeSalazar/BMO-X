//! **EL PORTERO: el libro de quien LLEGO y que se le contesto.**
//!
//! [carril]  VERDE     solo apunta y cuenta. No decide nada ni toca el bus
//! [consumo] NADA      corre cuando alguien lo pide
//!
//! [cuesta]  NADA -- doce fichas en RAM y una comparacion por llegada. No lee
//!           MMIO, no pide memoria y no cambia ni un veredicto: los veredictos
//!           ya estaban tomados en `bmo_uhid`, y hasta hoy se tiraban.
//!
//! [riesgo]  SILENCIO -- si esto se llena o se equivoca dedupando, un aparato
//!           nuevo deja de anunciarse y vuelve el estado de antes: enchufas algo
//!           y no hay forma de saber que era. No pierde datos de nadie; pierde
//!           la unica respuesta a *"que acabo de enchufar"*.
//!
//! # *** POR QUE ESTE FICHERO EXISTE
//!
//! El dueno lo pidio el 2026-09-07 con la imagen entera dentro de la frase:
//!
//! > *"es como un guardian con que busca nombres y papeles, y si no sale le
//! > avisa al kernel y ya"*
//!
//! Y lo que se encontro al ir a escribirlo es que **el guardian ya miraba los
//! papeles**. `bmo_uhid` lee clase, subclase y protocolo de CADA interfaz, y se
//! obliga a decirlos con su motivo escrito en el propio codigo:
//!
//! > *"Toda interfaz se DICE antes de juzgarla. Sin esto, un aparato descartado
//! > y un aparato ausente se ven exactamente igual"*
//!
//! ** Pero los decia al LOG. Y un log se va con el scroll, asi que esa frase
//! valia mientras alguien estuviera mirando el serial **en ese instante**, y no
//! despues -- que es justo cuando se pregunta: *"enchufe algo y no paso nada,
//! que era?"*.
//!
//! > Los papeles se leian y se tiraban. El veredicto se tomaba y se olvidaba.
//!
//! # El reparto, y por que el veredicto no se decide aqui
//!
//! ```text
//!    bmo-uhid    DECIDE       conoce las clases, sabe que es un teclado
//!    el HAL      TRANSPORTA   `papeles()`: bmo-xhci no sabe que es un teclado
//!    ESTE        APUNTA       y lo dice UNA vez
//! ```
//!
//! Es el mismo corte que ya usa el barrido --`barrido::decidir` decide, el
//! kernel obedece-- y por la misma razon: **la decision se prueba sin encender
//! la maquina**, y apuntar no puede cambiarla.
//!
//! [!] Asi que esto NO aprueba ni rechaza nada. Un portero que ademas decidiera
//! seria una segunda politica de adopcion viviendo al lado de la primera, y el
//! dia que discreparan ganaria la que corriera antes.
//!
//! # Por que se dice UNA vez, y quien lo garantiza
//!
//! El barrido vuelve a mirar los puertos cada 500 ms, y en cada pasada los
//! mismos papeles producen el mismo veredicto. Anunciarlo cada vez serian dos
//! renglones por segundo por aparato: CABINA tiene sitio para 82, asi que en
//! menos de un minuto la linea que explica la causa estaria fuera.
//!
//! ** El libro ES el antirrebote: una ficha que ya esta no se vuelve a decir. Y
//! si el veredicto CAMBIA --el mismo aparato que antes se rechazaba y ahora
//! entra-- esa es una ficha distinta, y **si** se dice. Que es exactamente lo
//! que hay que saber.

use bmo_uhid as uhid;

/// Cuantas llegadas se recuerdan.
///
/// Doce, por lo mismo que las ocho de la morgue: caben en el sitio y un caso se
/// resuelve con las ultimas. Un teclado compuesto trae dos o tres interfaces y
/// un raton otra, asi que doce son tres o cuatro aparatos con sus papeles --
/// mas de lo que hay enchufado en esta maquina.
const FICHAS: usize = 12;

/// Los papeles de una llegada y lo que se le contesto.
#[derive(Clone, Copy, PartialEq)]
struct Ficha {
    /// El NOMBRE: `idVendor`/`idProduct`. Cero = no se pudo leer.
    ///
    /// Clase y subclase dicen QUE es --un HID de arranque--; solo esto dice
    /// CUAL es. Es lo que Windows ensena como `USB\VID_046D&PID_C077`, y sin
    /// ello un aparato rechazado no se puede ni buscar.
    vid: u16,
    pid: u16,
    puerto: u8,
    /// `0xFF` = no llego a haber interfaz que mirar (fallo antes).
    iface: u8,
    clase: u8,
    subclase: u8,
    proto: u8,
    veredicto: u8,
}

const VACIA: Ficha =
    Ficha { vid: 0, pid: 0, puerto: 0, iface: 0, clase: 0, subclase: 0, proto: 0, veredicto: 0 };

static mut LIBRO: [Ficha; FICHAS] = [VACIA; FICHAS];
/// Cuantas fichas hay escritas. Se detiene en `FICHAS`: ver [`apunta`].
static mut ESCRITAS: usize = 0;
/// Llegadas que no cupieron. Un cero aqui dice que el libro esta entero.
static mut SIN_SITIO: u64 = 0;
static mut ADMITIDOS: u64 = 0;
static mut RECHAZADOS: u64 = 0;

/// `(admitidos, rechazados, llegadas que no cupieron)`.
pub fn stats() -> (u64, u64, u64) {
    unsafe { (ADMITIDOS, RECHAZADOS, SIN_SITIO) }
}

/// **Un aparato llego, estos son sus papeles y esto se le contesto.**
///
/// Lo llama `KernelXhciHal::papeles`, que es la unica implementacion del HAL.
/// Corre dentro de `pump_bus`, o sea con el PML4 del kernel puesto -- pero esto
/// no toca MMIO, asi que no depende de ello.
#[allow(clippy::too_many_arguments)]
pub(super) fn apunta(
    vid: u16,
    pid: u16,
    puerto: u8,
    iface: u8,
    clase: u8,
    subclase: u8,
    proto: u8,
    veredicto: u8,
) {
    let f = Ficha { vid, pid, puerto, iface, clase, subclase, proto, veredicto };
    unsafe {
        let libro = &mut *core::ptr::addr_of_mut!(LIBRO);
        // Ya lo dijimos? Entonces callar. Ver la cabecera.
        for i in 0..ESCRITAS {
            if libro[i] == f {
                return;
            }
        }
        // ** NO SE DA LA VUELTA AL LLENARSE, y es a proposito. Sobrescribir la
        // ficha mas vieja haria que su aparato volviera a anunciarse en el
        // siguiente barrido, y a partir de ahi el libro seria un generador de
        // renglones en vez de un antirrebote. Lleno = se cuenta y se calla.
        if ESCRITAS >= FICHAS {
            SIN_SITIO = SIN_SITIO.wrapping_add(1);
            return;
        }
        libro[ESCRITAS] = f;
        ESCRITAS += 1;
        if admitido(veredicto) {
            ADMITIDOS = ADMITIDOS.wrapping_add(1);
        } else {
            RECHAZADOS = RECHAZADOS.wrapping_add(1);
        }
    }
    // ** Y AHORA SE DICE, fuera del `unsafe`: CABINA no necesita el libro.
    //
    // El numero lleva la ficha entera para que el renglon se pueda leer sin
    // volver a enumerar nada. CABINA lo pinta en hexadecimal (`Fmt::Raw`), asi
    // que sale con esta forma y se lee de izquierda a derecha:
    //
    //    vid(16) | pid(16) | puerto | clase | subclase | proto
    //    046D      C077      02       03      01         01
    //    \_ el nombre, igual que lo dice Windows _/  \_ que ES _/
    //
    // ** El VEREDICTO no va en el numero: va en el TEXTO, que es donde se lee
    // sin decodificar nada. Y `iface` se queda fuera porque, cuando hay que
    // elegir, el puerto es el que se puede tocar con la mano.
    let papeles = ((vid as u64) << 48)
        | ((pid as u64) << 32)
        | ((puerto as u64) << 24)
        | ((clase as u64) << 16)
        | ((subclase as u64) << 8)
        | proto as u64;
    if admitido(veredicto) {
        crate::ring0::cabina::info("portero", motivo(veredicto), papeles);
    } else {
        crate::ring0::cabina::warn("portero", motivo(veredicto), papeles);
    }
}

/// Entro o no. Las dos unicas respuestas que significan *"esta funcionando"*.
fn admitido(v: u8) -> bool {
    v == uhid::VEREDICTO_TECLADO || v == uhid::VEREDICTO_RATON
}

/// El veredicto EN PALABRAS.
///
/// Se traduce aqui y no se manda el numero pelado porque quien lee esto lo lee
/// con la maquina rota: un codigo que hay que ir a buscar a otro fichero es un
/// codigo que no se busca.
fn motivo(v: u8) -> &'static str {
    match v {
        uhid::VEREDICTO_TECLADO => "LLEGO y entra como TECLADO",
        uhid::VEREDICTO_RATON => "LLEGO y entra como RATON",
        uhid::VEREDICTO_NO_ES_HID => "llego algo que NO ES HID: no hay codigo para su clase",
        uhid::VEREDICTO_HID_SIN_BOOT => "llego un HID SIN subclase BOOT: hoy no se adopta",
        uhid::VEREDICTO_YA_CUBIERTO => "llego y VALIA, pero su puesto ya estaba ocupado",
        uhid::VEREDICTO_SIN_ENDPOINT => "llego un HID sin endpoint de interrupcion de entrada",
        uhid::VEREDICTO_SIN_PREPARAR => "su endpoint NO se pudo preparar en el controlador",
        uhid::VEREDICTO_RATON_NO_ENTRO => "era un RATON y no se pudo instalar",
        uhid::VEREDICTO_SIN_DIRECCION => "un puerto con algo dentro NO se pudo direccionar",
        uhid::VEREDICTO_SIN_DESCRIPTORES => "direccionado, pero sus descriptores no se leyeron",
        // Un veredicto que este kernel no conoce es un `bmo_uhid` mas nuevo que
        // el codigo que lo lee. Se dice asi en vez de inventarle un nombre.
        _ => "llego algo con un veredicto que este kernel no sabe nombrar",
    }
}
