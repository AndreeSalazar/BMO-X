//! `KIND_INPUT` -- el raton como capability.
//!
//! El kernel ya sabia donde esta el raton: `dev::usb` acumula los deltas del
//! HID desde que el xHCI lo enumero. Lo que no habia era forma de que ese dato
//! **saliera de Ring 0**, asi que el puntero era un numero que solo servia para
//! el diagnostico. Esto lo abre.
//!
//! ## Por que esto si es del kernel y el cursor no
//!
//! Leer el HID es tocar hardware: transferencias xHCI, endpoints, reintentos.
//! Eso vive en Ring 0 porque no hay otro sitio donde pueda vivir todavia.
//!
//! **Dibujar el cursor no.** El cursor es una decision de aspecto --forma,
//! color, si tiene sombra, si cambia sobre un borde-- y ninguna de esas
//! decisiones tiene nada que hacer dentro de un kernel. Aqui solo sale un par
//! de coordenadas y un mapa de botones; el compositor decide que pinta con eso.
//! La misma linea que separa `KIND_FRAMEBUFFER` de un `DRAW_RECT`.
//!
//! ## Exclusiva, como la pantalla
//!
//! Un solo proceso la tiene: el multiplexor de entrada. Cuando `services/input`
//! exista de verdad, sera el quien la reclame y quien reparta los eventos entre
//! el compositor y las aplicaciones. Hoy la reclama el compositor directamente,
//! que es lo honesto mientras no haya nadie mas a quien repartir.
//!
//! * Y como con la pantalla: hoy la reclama el primero que la pide. La
//! autoridad correcta es la bandera del BEF verificada por el gate.

use core::sync::atomic::{AtomicU32, AtomicU64, Ordering};

use crate::ring0::obj::cap;

const SIN_DUENO: u32 = u32::MAX;
static DUENO: AtomicU32 = AtomicU32::new(SIN_DUENO);
/// El handle concedido al dueno, para poder revocarlo si la SUELTA. Mismo
/// motivo que en `obj::fb`: `cap` no ofrece "revoca todo lo de este tipo", y
/// `revoke_all` se llevaria por delante su pantalla y su consola.
static HANDLE: AtomicU64 = AtomicU64::new(0);

/// Ya la tiene otro proceso.
pub const ERROR_OCUPADO: u32 = 16;

/// Estado del puntero, empaquetado: `(x << 32) | (y << 16) | botones`.
///
/// Una llamada por fotograma en vez de tres. `x` e `y` caben en 16 bits con
/// holgura --4096 px es mas pantalla de la que hay-- y los botones son una
/// mascara de bits.
pub const INPUT_OP_PUNTERO: u64 = 0x01;

/// Cuantos eventos HID se han visto. Sirve para distinguir "el raton no se
/// mueve" de "el raton no llega": si esto no sube, el problema esta en el USB,
/// no en el compositor.
pub const INPUT_OP_EVENTOS: u64 = 0x02;

/// Una tecla, si hay alguna esperando. **No bloquea.**
///
/// Devuelve `0` cuando no hay nada, y `0x100 | byte` cuando si. El bit 8 existe
/// porque el byte 0 es un byte legitimo y "no hay tecla" tenia que poder
/// distinguirse de el sin gastar el codigo de error del `Status` -- que aqui
/// significa "esta operacion no existe", que es otra cosa.
///
/// El byte es **Latin-1, no UTF-8**: un caracter es un byte, igual que en el
/// teclado, en el shell y en la fuente. Asi los cuatro hablan el mismo idioma
/// sin un decodificador de por medio.
///
/// ## Por que no bloquea
///
/// Un compositor tiene un bucle de fotograma y ya cede el turno al final de
/// cada vuelta. Bloquearlo en el teclado congelaria el cursor entre tecla y
/// tecla -- el raton dejaria de moverse mientras nadie escribe, que es
/// exactamente al reves de lo que uno quiere. `WAIT` esta para bloquearse; esto
/// esta para preguntar.
pub const INPUT_OP_TECLA: u64 = 0x03;

/// La mascara de modificadores AHORA MISMO, sin consumir nada.
///
/// Hace falta porque `INPUT_OP_TECLA` entrega un byte ya resuelto, y hay
/// combinaciones que **no producen caracter**: `Ctrl+Alt` a secas no es
/// ninguna letra. Sin esto, Ring 3 no puede tener atajos -- veria una `r`, no
/// un `Ctrl+Alt+R`.
///
/// No consume: es estado, no evento. Se puede leer una vez por fotograma sin
/// robarle teclas a nadie.
pub const INPUT_OP_MODIFICADORES: u64 = 0x04;

/// Las vueltas de rueda desde la ultima lectura. **Consume**: leerla la vacia.
///
/// Quien pregunta quiere saber cuanto se ha girado DESDE QUE MIRO. Un acumulado
/// desde el arranque obligaria a cada llamante a guardar el anterior y restar,
/// y el primero que lo olvidara tendria un scroll que se va solo.
pub const INPUT_OP_RUEDA: u64 = 0x05;

/// La tiene un proceso de Ring 3?
///
/// Lo pregunta el shell de Ring 0 antes de leer el teclado. Sin esto los dos
/// drenan la MISMA cola y se reparten las letras al azar: escribes "run" en la
/// caja y al shell le llega la "u". Cedido es cedido, tambien para el que la
/// cedio.
pub fn cedido() -> bool {
    DUENO.load(Ordering::SeqCst) != SIN_DUENO
}

pub fn reclamar(pid: u32) -> Result<u64, u32> {
    if DUENO
        .compare_exchange(SIN_DUENO, pid, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        return Err(ERROR_OCUPADO);
    }
    match cap::grant(pid, cap::KIND_INPUT, cap::RIGHT_READ, 0) {
        Some(h) => {
            HANDLE.store(h, Ordering::SeqCst);
            crate::ring0::cabina::info("input", "raton cedido a Ring 3", pid as u64);
            Ok(h)
        }
        None => {
            DUENO.store(SIN_DUENO, Ordering::SeqCst);
            Err(cap::ERROR_PERMISSION_DENIED)
        }
    }
}

/// * SOLTAR LA ENTRADA sin morirse. Pareja de [`reclamar`].
///
/// # Por que hizo falta, y es una leccion
///
/// El 2026-08-07 se anadio `presta <ruta>` al escritorio para que un programa
/// grafico pudiera tomar la pantalla. Funciono: `ray.bex` pinto cielo y suelo en
/// el Ryzen. Y **se quedo colgado para siempre**, porque el escritorio le presta
/// la pantalla y se queda la ENTRADA -- asi que el raycaster no podia leer su
/// propio ESC para salir, y el escritorio esperaba a que muriese.
///
/// La maquina quedaba sin teclado y sin forma de volver. Eddi: *"no me deja
/// controlar ni puedo llamar el comando con control+alt ni alt+tab ni nada"*.
///
/// **Ceder la pantalla sin ceder la entrada no es prestar: es dejar a un
/// programa pintando en una habitacion cerrada.** Las dos capabilities van
/// juntas o no van.
///
/// # Mas simple que la de la pantalla, y por que
///
/// Aqui no hay nada que desmapear: la entrada se lee por `INVOKE` sobre un
/// handle, no por memoria compartida. Asi que basta revocar el handle y soltar
/// la propiedad -- y no hace falta el `aspace` del llamante.
pub fn soltar(pid: u32) -> Result<(), u32> {
    if DUENO.load(Ordering::SeqCst) != pid {
        // No es suya. Se dice, en vez de contestar OK a quien no la tenia.
        return Err(ERROR_OCUPADO);
    }
    let h = HANDLE.swap(0, Ordering::SeqCst);
    if h != 0 {
        cap::revoke(pid, h);
    }
    DUENO.store(SIN_DUENO, Ordering::SeqCst);
    crate::ring0::cabina::info("input", "entrada SOLTADA por su dueno", pid as u64);
    Ok(())
}

/// Lo llama `cap::revoke_all`: si el dueno muere, la entrada vuelve al kernel.
pub fn proceso_muerto(pid: u32) {
    if DUENO
        .compare_exchange(pid, SIN_DUENO, Ordering::SeqCst, Ordering::SeqCst)
        .is_ok()
    {
        // El handle muere con el proceso, pero el static no: dejarlo puesto
        // haria que un `soltar` posterior revocase el handle de un muerto.
        HANDLE.store(0, Ordering::SeqCst);
    }
}

/// Despacho de las operaciones. `None` = operacion que no existe.
pub fn operacion(operacion: u64) -> Option<u64> {
    let (x, y, botones, eventos) = crate::ring0::dev::usb::puntero();
    match operacion {
        INPUT_OP_PUNTERO => {
            // Se recorta al panel AQUI, que es donde se sabe de que tamano es.
            // Un acumulador de deltas sin tope se va a valores absurdos con
            // dos pasadas de raton y el compositor tendria que recortarlo
            // igual, solo que sin saber contra que.
            let (ancho, alto) = unsafe {
                (
                    crate::info::FB_WIDTH.max(1) - 1,
                    crate::info::FB_HEIGHT.max(1) - 1,
                )
            };
            let cx = x.clamp(0, ancho as i32) as u64;
            let cy = y.clamp(0, alto as i32) as u64;
            Some((cx << 32) | (cy << 16) | botones as u64)
        }
        INPUT_OP_EVENTOS => Some(eventos as u64),
        // Misma fuente que alimentaba al shell (`poll_ascii` drena el puente
        // USB HID y, si no hay, el i8042). No se duplica la cola: se cambia
        // quien la vacia.
        INPUT_OP_TECLA => match crate::ring0::dev::usb::poll_ascii() {
            Some(b) => Some(0x100 | b as u64),
            None => Some(0),
        },
        INPUT_OP_MODIFICADORES => Some(crate::ring0::dev::usb::modificadores() as u64),
        // Se devuelve como i32 en complemento a dos dentro del u64: girar hacia
        // atras es negativo, y el llamante lo recupera con un `as i32`.
        INPUT_OP_RUEDA => Some(crate::ring0::dev::usb::rueda() as i64 as u64),
        _ => None,
    }
}
