//! **LA SALUD DEL BUS, COMO ESTADO.** La sexta exigencia (E6) de
//!
//! [carril]  VERDE     la salud del bus, como estado que se consulta
//! `docs/componente/EL_TECLADO_EXIGE.md`.
//!
//! === Por que este fichero existe, dicho de una vez ===
//!
//! Las cinco primeras exigencias del teclado estan cumplidas **y cada una tiene
//! su contador**. Y aun asi, el dueno del 2026-08-17 solo veia *"el teclado no
//! responde"*: los contadores viven en funciones de kernel, los avisos se dicen
//! UNA vez, y al shell de Ring 0 --donde se leen-- no se vuelve.
//!
//! > ** R-USB6: UNA AVERIA VIVA ES UN ESTADO, NO UN EVENTO.
//! >
//! > Un `fault()` informa a quien ya estaba mirando. Una averia que **sigue
//! > ocurriendo** necesita un indicador encendido mientras dure, y en el sitio
//! > donde vive el dueno.
//!
//! Esto es la mitad de kernel de esa regla: **dos numeros** que se leen con
//! `OP_INFO` desde Ring 3 y que dicen, sin abrir nada, cual de las seis
//! exigencias esta fallando AHORA. La otra mitad --la luz-- la pinta el
//! escritorio.
//!
//! === Por que se hace una FOTO en el bombeo y no se lee al preguntar ===
//!
//! Porque leer el estado de un endpoint **toca memoria del controlador**
//! (`ep_state` recorre el Device Context, `usbsts` es MMIO), y el MMIO del xHCI
//! solo esta mapeado en el PML4 del kernel. Un `OP_INFO` llega con el CR3 del
//! programa que pregunta: leerlo ahi seria un `#PF` o, peor, basura.
//!
//! Asi que quien mira es [`refrescar`], llamado desde `pump_bus` --que ya carga
//! el PML4 del kernel-- 250 veces por segundo. Preguntar solo lee `static`s.
//!
//! [!] **Y eso deja una trampa que hay que cerrar de frente**: si el hilo del
//! bus muere, la foto se queda quieta y el ultimo valor bueno seguiria
//! contestando *"todo bien"* para siempre. Por eso [`estado`] no devuelve solo
//! los bits: devuelve **la edad del latido** al lado, calculada en el momento de
//! preguntar. Un informe de salud que no puede caducar es un informe que miente
//! el dia que mas importa.

use super::bus;

// -- ** LOS BITS. Cada uno es un HECHO, no un veredicto ----------------------
//
// El kernel contesta hechos y Ring 3 decide el color. Es la misma frontera de
// `INFO_CPU_EXT_*`: una linea ya pintada aqui obligaria a todo cliente a la
// palabra y al criterio que eligiera el kernel.
//
// [!] Estas constantes viven TRES veces --aqui, en `bmo-abi` y en el
// userland--, como toda la tabla de `OP_INFO`, y por el mismo motivo: el kernel
// no depende del ABI. Y como toda la tabla, las barre el guardian de
// `build.ps1`: una que se escriba en dos de los tres sitios **falla el build**.

/// Hay controlador xHCI enumerado. Sin esto, todo lo demas es cero y no
/// significa "roto": significa que no hay bus que mirar.
pub const USB_SALUD_XHCI: u64 = 1 << 0;
/// El teclado esta adoptado (enumerado y con sus endpoints configurados).
pub const USB_SALUD_KBD: u64 = 1 << 1;
/// El teclado tiene una transferencia ENCOLADA. Es la diferencia entre
/// *"enumero"* y *"esta escuchando"*: un endpoint sin TRB encolado esta
/// perfecto, en `Running`, y mudo para siempre.
pub const USB_SALUD_KBD_BOMBA: u64 = 1 << 2;
/// Su endpoint de interrupcion esta en `Running` **segun el hardware** (Device
/// Context, no nuestras suposiciones). Apagado = `Halted`/`Stopped`/`Error`, y
/// entonces el xHC ignora el timbre: no se reintenta, se resucita (R-USB7).
pub const USB_SALUD_KBD_CORRE: u64 = 1 << 3;
/// Lo mismo para el raton. Va al lado a proposito: **la asimetria teclado/raton
/// es medio diagnostico** -- lo que le pasa a uno y no al otro no puede ser del
/// hilo, del CR3 ni de la enumeracion.
pub const USB_SALUD_RATON: u64 = 1 << 4;
pub const USB_SALUD_RATON_BOMBA: u64 = 1 << 5;
pub const USB_SALUD_RATON_CORRE: u64 = 1 << 6;
/// `USBSTS` dice que el controlador esta muerto: HSE (bit 2, error de sistema
/// -- tipicamente un DMA a memoria que no puede tocar) o HCE (bit 12). Si esto
/// esta encendido, todo lo demas de esta palabra es ruido.
pub const USB_SALUD_XHC_AVERIADO: u64 = 1 << 7;

/// Donde empieza la edad del ultimo latido del hilo del bus, en milisegundos.
pub const USB_SALUD_EDAD_SHIFT: u64 = 16;
/// Mascara de esa edad: 16 bits.
pub const USB_SALUD_EDAD_MASK: u64 = 0xFFFF;
/// Edad saturada = *"hace mucho, o no se puede saber"*. Las dos cosas piden la
/// misma reaccion --dejar de fiarse de los bits-- asi que comparten valor en vez
/// de inventar un bit para distinguirlas.
pub const USB_SALUD_EDAD_VIEJA: u64 = 0xFFFF;

/// La foto del ultimo bombeo. Solo bits; la edad no se guarda porque **envejece
/// sola** y guardarla seria guardar una mentira que crece.
static mut FOTO: u64 = 0;

/// **Lo que la foto ANTERIOR decia del controlador**, y nada mas.
///
/// Existe para distinguir dos cosas que el bit no distingue: *"sigue
/// averiado"* de *"ACABA de averiarse"*. Sin esto solo caben dos malas
/// opciones -- callarse (lo que habia) o gritar 250 veces por segundo.
static mut AVERIADO_ANTES: bool = false;

/// **Mira el bus y guarda lo que ve.** Se llama desde `pump_bus`, con el PML4
/// del kernel ya cargado -- ver la cabecera del modulo.
///
/// Cuesta dos lecturas de Device Context y una de MMIO por vuelta. A 250 Hz eso
/// es ruido al lado de lo que ya hace el bombeo, y es lo que compra que
/// preguntar desde Ring 3 no cueste ni un mapeo.
pub(super) fn refrescar() {
    let mut b = 0u64;
    unsafe {
        if !super::PRESENT {
            // Sin controlador no hay nada que mirar, y **el cero se escribe**:
            // dejar la foto anterior seria contestar con el estado de un bus
            // que ya no esta.
            FOTO = 0;
            // Sin controlador no hay averia que recordar: si un dia vuelve a
            // haber bus, su primera muerte tiene que poder decirse otra vez.
            AVERIADO_ANTES = false;
            return;
        }
        b |= USB_SALUD_XHCI;

        let hid = &*core::ptr::addr_of!(super::HID);
        let (kbd_bombea, raton_bombea) = hid.bombeando();

        if super::KBD_RDY {
            b |= USB_SALUD_KBD;
            if kbd_bombea {
                b |= USB_SALUD_KBD_BOMBA;
            }
            if bmo_xhci::ep_state(super::KBD_SLOT, hid.kbd_dci()) == 1 {
                b |= USB_SALUD_KBD_CORRE;
            }
        }
        if super::MOUSE_RDY {
            b |= USB_SALUD_RATON;
            if raton_bombea {
                b |= USB_SALUD_RATON_BOMBA;
            }
            if bmo_xhci::ep_state(super::MOUSE_SLOT, hid.mouse_dci()) == 1 {
                b |= USB_SALUD_RATON_CORRE;
            }
        }

        let sts = bmo_xhci::usbsts();
        if sts & (1 << 2) != 0 || sts & (1 << 12) != 0 {
            b |= USB_SALUD_XHC_AVERIADO;
        }
        FOTO = b;

        // == *** Y AQUI EL KERNEL LO DICE EN VOZ ALTA (2026-09-07) ===========
        //
        // ** El bit se encendia y **no lo leia nadie de Ring 0**. Sus dos
        // unicos lectores estan los dos al otro lado:
        //
        // ```text
        //    scene/testigo.rs    la luz     <- la pinta el ESCRITORIO
        //    commands/reports.rs la orden `usb` <- hay que TECLEARLA
        // ```
        //
        // *** Y las dos cosas que hacen falta para verlo --un escritorio vivo y
        // un teclado que escriba-- son exactamente las dos que ya no estan
        // cuando el controlador se muere. El unico aviso de que el teclado ha
        // muerto solo se podia leer con el teclado.
        //
        // > Un estado cuyo unico lector muere con la averia que anuncia no es
        // > un instrumento: es un mensaje dentro de la casa que se quema.
        //
        // La cabecera de este fichero ya cerro la trampa hermana --si el hilo
        // del bus muere, la foto miente-- con la edad del latido. Esta es la
        // otra mitad, y CABINA es el sitio: la pinta el kernel, sobrevive a la
        // purga y sale en la azul.
        //
        // [!] Por FLANCO y no por nivel. Esto corre 250 veces por segundo: un
        // aviso por nivel llenaria el anillo de eventos con la misma linea y se
        // llevaria por delante la que explica la causa, tres renglones mas
        // arriba. Se dice cuando CAMBIA, y se rearma para que una segunda
        // muerte tambien se diga.
        let averiado = b & USB_SALUD_XHC_AVERIADO != 0;
        if averiado && !AVERIADO_ANTES {
            crate::ring0::cabina::fault(
                "xhci",
                "el controlador SE MURIO en marcha (USBSTS HSE/HCE)",
                sts as u64,
            );
            // == *** Y AHORA SE PIDE LA PANTALLA (2026-09-08) ================
            //
            // El 07-09 esto solo lo decia en CABINA, y CABINA hay que IR A
            // MIRARLA -- con F11, o sea con el teclado. El mismo teclado que
            // acaba de morirse.
            //
            // ** El dueno lo topo al dia siguiente: *"entre pero veia el
            // puntero y teclado que no respondia"*. Reinicio, y no se llevo ni
            // un dato. La averia se anuncio en un sitio al que solo se llega
            // con lo que la averia acaba de romper.
            //
            // > Un instrumento que solo se lee tecleando no existe el dia que
            // > no hay teclado.
            //
            // *** El mecanismo YA ESTABA ENTERO --`core/emergencia.rs` toma la
            // pantalla, suelta la entrada y SE EXPLICA-- y hasta hoy solo lo
            // disparaba una corrupcion de memoria. Aqui se le da su segundo
            // motivo. Quien lo recoge es el hilo del bus, en esta misma vuelta:
            // `pump_bus` corre antes que `emergencia::atender`.
            //
            // [!] Y quitarle la pantalla al escritorio se puede JUSTIFICAR aqui
            // y no en cualquier sitio: sin teclado ni raton, lo que hay debajo
            // ya no se puede usar. **No se interrumpe a nadie: se le da al
            // dueno lo unico que todavia funciona.**
            //
            // [!] Una vez. `declarar` guarda el PRIMER motivo y `atender`
            // consume la bandera, y esta rama es un FLANCO -- si el
            // controlador parpadeara, no se robaria la pantalla en bucle.
            crate::ring0::core::emergencia::declarar(
                "el controlador USB murio: no hay teclado ni raton",
                sts as u64,
            );
        } else if !averiado && AVERIADO_ANTES {
            // ** Y la vuelta tambien se dice. Sin esta rama, una averia que se
            // cura deja el ultimo renglon diciendo "muerto" para siempre, y el
            // que lee la bitacora despues no puede saber si duro un instante o
            // toda la sesion.
            crate::ring0::cabina::info(
                "xhci",
                "el controlador VOLVIO: USBSTS ya no dice HSE/HCE",
                sts as u64,
            );
        }
        AVERIADO_ANTES = averiado;
    }
}

/// **Los bits de la foto MAS la edad del latido**, en milisegundos, en los bits
/// 16..31.
///
/// La edad se calcula aqui y no se guarda: es lo unico de esta palabra que
/// cambia sin que nadie la refresque, y es justo lo que delata que el que
/// refresca ha muerto. Ver la cabecera del modulo.
pub fn estado() -> u64 {
    let bits = unsafe { FOTO };
    (bits & 0xFFFF) | (edad_latido_ms() << USB_SALUD_EDAD_SHIFT)
}

/// Cuantos milisegundos hace que el hilo del bus dio su ultima vuelta.
///
/// [`USB_SALUD_EDAD_VIEJA`] cuando no ha latido nunca o cuando no hay TSC
/// medido con el que contar el tiempo. **No se inventa un cero**: un cero aqui
/// significa *"acaba de latir"*, que es la afirmacion mas fuerte de todo este
/// fichero y la que no se puede regalar.
fn edad_latido_ms() -> u64 {
    use crate::ring0::task::scheduler;
    let ultimo = bus::ultimo_latido();
    if ultimo == 0 {
        return USB_SALUD_EDAD_VIEJA;
    }
    let hz = scheduler::tsc_freq();
    let por_ms = hz / 1000;
    if por_ms == 0 {
        return USB_SALUD_EDAD_VIEJA;
    }
    let ahora = scheduler::rdtsc();
    // `wrapping_sub` y no resta: si el reloj se leyera al reves --dos nucleos
    // con TSC no sincronizado-- una resta normal daria un numero enorme y la
    // luz gritaria sin motivo. Al revirar, la saturacion de abajo lo convierte
    // en "viejo", que es la reaccion prudente.
    let edad = ahora.wrapping_sub(ultimo) / por_ms;
    edad.min(USB_SALUD_EDAD_VIEJA - 1)
}

/// **Los cuatro contadores que TIENEN QUE SER CERO**, empaquetados de 16 en 16
/// bits -- igual que `INFO_CPU_EXT_AVERIAS`, y por el mismo motivo: separarlos
/// permitiria leer uno y no el otro, que es como se dice *"todo bien"* mirando
/// la mitad.
///
/// ```text
///    0..15   eventos PERDIDOS del aparcadero      E2  -> endpoint mudo
///   16..31   RECUPERACIONES_FALLIDAS              E3  -> se intento y no salio
///   32..47   RECUPERACIONES                       E3  -> hay errores de bus
///   48..63   barridos que REPARARON algo          E5  -> se pierden avisos
/// ```
///
/// Los dos primeros son averia; los dos ultimos son **desgaste**: el sistema se
/// esta reparando solo, funciona, y aun asi cada uno de ellos es medio segundo
/// en que el teclado no respondia. Por eso viajan: quien pinta decide si eso es
/// rojo o ambar, pero no puede decir que no lo sabia.
///
/// Todos saturan a `0xFFFF`. Un contador que da la vuelta a los 65.536 volveria
/// a cero y **apagaria la luz**, que es exactamente el fallo que este fichero
/// existe para no repetir.
pub fn averias() -> u64 {
    let (_, perdidos, _) = bmo_xhci::evt_park_stats();
    let (recuperaciones, fallidas) = bmo_xhci::recuperaciones();
    let (_, barridos_utiles) = super::barrido_stats();
    sat16(perdidos as u64)
        | (sat16(fallidas as u64) << 16)
        | (sat16(recuperaciones as u64) << 32)
        | (sat16(barridos_utiles) << 48)
}

fn sat16(v: u64) -> u64 {
    if v > 0xFFFF {
        0xFFFF
    } else {
        v
    }
}
