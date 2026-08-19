//! **TRAER UN FICHERO DE FUERA** y dejarlo escrito en ESTRATOS.
//!
//! === Por que esto existe, y no es "mandar mas bytes" ===
//!
//! El techo de un fichero eran 96 bytes, y resulto ser DOS techos con el mismo
//! numero:
//!
//! ```text
//!   RESIDENTE_MAX = 96   lo que cabe DENTRO del nodo    <- lo tumbo `flujo`
//!   DATOS_MAX     = 96   el renglon del syscall         <- sigue en pie
//! ```
//!
//! El formato ya sabe partir un fichero en bloques desde el 19-08. Lo que sigue
//! sin poder es **cruzar la puerta**: el contenido viaja de ocho en ocho bytes
//! por un renglon que acumula 96, y meter 4 KiB por ahi serian 512 llamadas por
//! bloque. Esa puerta no esta hecha para eso, y ensancharla seria empeorar el
//! diseno para forzar un caso raro.
//!
//! * **Asi que el contenido NO cruza el anillo.** Ring 3 dice dos nombres --de
//! donde y a donde-- y el kernel lee la fuente el mismo. Es la misma forma que
//! ya tiene `ARCH_OP_LEER_EN`, que lleva los sectores del disco al bloque del
//! que pregunta sin pasar por ninguna mesa.
//!
//! Y de paso es lo que de verdad hace falta: los ficheros de esta maquina estan
//! todos en FAT32 y ESTRATOS esta casi vacio. Lo util no era escribir un texto
//! de 96 bytes mas largo -- era **poder meter lo que ya tienes**.
//!
//! === El reparto, y por que en dos llamadas ===
//!
//! [`coste`] mide y [`traer`] escribe. Estan separadas porque una transaccion
//! **reserva antes de escribir**, y para reservar hay que saber cuanto: el
//! tamano del origen decide cuantos bloques de datos, cuantos de indice y
//! cuantos niveles. Mezclarlas obligaria a pedir sitio a mitad de la escritura,
//! que es justo lo que la maquina de estados prohibe.

use bmo_estratos as es;
use bmo_estratos::escritura::nodo_de_fichero_grande;
use bmo_estratos::flujo::{plan_de, Arbol, Plan};
use bmo_estratos::objects::{BLOQUE, NODO_LEN};

use super::WriteError;

/// Los buffers de indice del constructor: uno por nivel.
///
/// [!] Estaticos y no en la pila. Son cuatro bloques de 4 KiB y la pila del
/// kernel son 64 KiB para todo -- montarlos en un marco se lleva un cuarto de
/// ella para una operacion que ademas llama a disco.
static mut INDICE: [[u8; BLOQUE]; es::objects::NIVELES_MAX] =
    [[0u8; BLOQUE]; es::objects::NIVELES_MAX];
/// Donde aterriza cada trozo leido del origen antes de escribirse.
static mut TROZO: [u8; BLOQUE] = [0u8; BLOQUE];

/// **Lo que va a costar traer `origen`**: `(bloques, plan, tamano)`.
///
/// El `+ 1` del nodo del fichero NO va aqui: lo suma quien reserva, junto con
/// los dos bloques por nivel de la ruta y el del estrato. Aqui se contesta solo
/// por el CONTENIDO, que es lo unico que esta funcion sabe.
pub(super) fn coste(origen: &str) -> Result<(u64, Plan, u32), WriteError> {
    let size = crate::ring0::fsys::fs::tamano(origen).map_err(|_| WriteError::RutaNoEsta)?;
    if size == 0 {
        // Un fichero vacio no tiene arbol que construir. Se dice, en vez de
        // pedirle un plan de cero bloques a algo que no sabe escribirlo.
        return Err(WriteError::NoCabe);
    }
    let plan = plan_de(size as u64).ok_or(WriteError::NoCabe)?;
    Ok((plan.total, plan, size))
}

/// **Lee `origen` y escribe su contenido a partir de `base`.**
///
/// Devuelve el nodo del fichero, ya listo para que lo cuelgue quien republica
/// el arbol. No toca entradas ni directorios: solo deja el contenido puesto y
/// la ficha que dice donde esta.
///
/// ** Va de bloque en bloque y **el fichero entero no esta nunca en memoria**.
/// Es la misma disciplina que el reflejo de `obj/file.rs`: un WAD de 4 MiB
/// cuesta lo mismo que un `.txt`, y el que la rompio una vez --trayendose el
/// fichero entero-- se llevo el aviso de que no cabia.
pub(super) fn traer(
    origen: &str,
    plan: Plan,
    size: u32,
    base: u64,
) -> Result<[u8; NODO_LEN], WriteError> {
    let (mut cur, _) =
        crate::ring0::fsys::fs::abrir_rangos(origen).map_err(|_| WriteError::RutaNoEsta)?;

    let indice = unsafe { &mut *core::ptr::addr_of_mut!(INDICE) };
    let trozo = unsafe { &mut *core::ptr::addr_of_mut!(TROZO) };
    let mut arbol = Arbol::nuevo(plan, base, &mut indice[..plan.niveles as usize])
        .map_err(|_| WriteError::NoCabe)?;

    let mut leidos = 0usize;
    while leidos < size as usize {
        let piden = (size as usize - leidos).min(BLOQUE);
        let n = crate::ring0::fsys::fs::leer_rango(&mut cur, leidos, size, &mut trozo[..piden]);
        if n == 0 {
            // El origen se corto a mitad. Se para: seguir escribiria un arbol
            // mas corto que su `size` y el fichero se leeria a medias sin que
            // nada fallara. Lo escrito hasta aqui no lo alcanza nadie -- el
            // commit no ha ocurrido.
            crate::ring0::cabina::warn("estratos", "el origen se corto al copiarlo", leidos as u64);
            return Err(WriteError::NoSeLeeLaRaiz);
        }
        let mut poner = |lba: u64, datos: &[u8]| super::escribir::poner(lba, datos).is_ok();
        arbol
            .empujar(&trozo[..n], &mut poner)
            .map_err(|_| WriteError::NoEscribio)?;
        leidos += n;
    }

    let mut poner = |lba: u64, datos: &[u8]| super::escribir::poner(lba, datos).is_ok();
    let raiz = arbol.cerrar(&mut poner).map_err(|_| WriteError::NoEscribio)?;
    nodo_de_fichero_grande(size as u64, plan.niveles, raiz).map_err(|_| WriteError::NoCabe)
}
