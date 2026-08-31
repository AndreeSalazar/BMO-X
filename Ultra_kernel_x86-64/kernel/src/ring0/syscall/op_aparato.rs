//! **TOMAR Y SOLTAR UN APARATO EXCLUSIVO**: la entrada, la pantalla y el audio.
//!
//! [carril]  ROJO      tomar y soltar la pantalla, la entrada y el audio
//!
//! ## Por que estas siete van juntas (L6b)
//!
//! Porque son la puerta de los LIDERES, y eso ya tiene documento propio:
//! `docs/identidad/LIDERES.md` -- *"el kernel concede un aparato exclusivo a UN
//! proceso; ese proceso no lo usa para si: lo REPARTE"*.
//!
//! ```text
//!    kernel --KIND_FRAMEBUFFER--> gui.bex   --superficies--> programas
//!    kernel --KIND_AUDIO-------> audio.bex  --voces-------> programas
//! ```
//!
//! ** Las dos siguen la misma forma y no por gusto: **es la misma forma.** Un
//! aparato que solo puede tener un dueno, y un trabajo de reparto que el kernel
//! no debe hacer. Tener `tomar` y `soltar` de los tres en una pagina es lo que
//! deja comprobar que los tres se comportan igual.
//!
//! *** Y el `soltar` importa tanto como el `tomar`: una ventana que se cierra
//! sin soltar el audio deja MUDO a todo lo demas. Ese fue el dia --2026-08-09--
//! en que `KIND_AUDIO` dejo de ser una idea.
//!
//! ## [!] Esto NO es un reparto puro de L6d, y se dice
//!
//! El cuerpo de cada brazo se movio tal cual; el brazo paso a ser una llamada.

use super::*;

//// La pantalla. El espacio de direcciones en el que se mapea es el que
//// esta cargado AHORA: durante un SYSCALL desde Ring 3, CR3 sigue
//// siendo el del llamante -- el cambio de CR3 solo ocurre en un cambio
//// de contexto, y aqui todavia no ha habido ninguno.
pub(super) fn input_claim(arg0: u64, arg1: u64) -> BmoStatus {
        let _ = arg0;
        match crate::ring0::obj::input::claim(scheduler::current_pid()) {
            Ok(handle) => BmoStatus::ok_value(handle),
            Err(code) => BmoStatus::err(code),
        }
}

pub(super) fn framebuffer_claim(arg0: u64, arg1: u64) -> BmoStatus {
        let _ = arg0;
        match crate::ring0::obj::fb::claim(
            scheduler::current_pid(),
            crate::ring0::mm::vmm::read_cr3(),
        ) {
            Ok(handle) => BmoStatus::ok_value(handle),
            Err(code) => BmoStatus::err(code),
        }
}

//// * SOLTAR la pantalla sin morirse. La pareja que le faltaba a
//// `FRAMEBUFFER_CLAIM`: hasta hoy la unica forma de dejar de ser dueno
//// era terminar, asi que el escritorio no podia prestarla ni queriendo y
//// `ray.bex` se llevaba un "la pantalla ya tiene dueno".
///
//// El `CR3` es el del llamante, igual que al reclamar -- y aqui importa
//// mas, porque es de donde hay que DESMAPEAR: el proceso sigue vivo y
//// dejarle las paginas seria dejarle escribir en una pantalla que ya no
//// es suya.
pub(super) fn entrada_soltar(arg0: u64, arg1: u64) -> BmoStatus {
        let _ = arg0;
        match crate::ring0::obj::input::release(scheduler::current_pid()) {
            Ok(()) => BmoStatus::ok_value(0),
            Err(code) => BmoStatus::err(code),
        }
}

pub(super) fn pantalla_soltar(arg0: u64, arg1: u64) -> BmoStatus {
        let _ = arg0;
        match crate::ring0::obj::fb::release(
            scheduler::current_pid(),
            crate::ring0::mm::vmm::read_cr3(),
        ) {
            Ok(()) => BmoStatus::ok_value(0),
            Err(code) => BmoStatus::err(code),
        }
}

pub(super) fn audio_claim(arg0: u64, arg1: u64) -> BmoStatus {
        let _ = arg0;
        match crate::ring0::obj::audio::claim(scheduler::current_pid()) {
            Ok(handle) => BmoStatus::ok_value(handle),
            Err(code) => BmoStatus::err(code),
        }
}

pub(super) fn audio_release(arg0: u64, arg1: u64) -> BmoStatus {
        let _ = arg0;
        match crate::ring0::obj::audio::release(scheduler::current_pid()) {
            Ok(()) => BmoStatus::ok_value(0),
            Err(code) => BmoStatus::err(code),
        }
}

//// * Despertar nucleos DESDE Ring 3. Es la unica operacion de esta tabla
//// que cambia el estado del hardware en vez de contestar una pregunta, y
//// por eso conviene decir por que se acepta: no concede nada al llamante
//// --los APs quedan parados y sin tocar el kernel-- y el resultado es un
//// numero. Ver `plat/smp` y `docs/maestro/SMP_MAESTRO.md`.
///
//// El aviso por nucleo se traga aqui: cruzar el borde de Ring 3 once
//// veces para pintar una linea costaria mas que el propio bring-up. Lo
//// que si queda es CABINA, que ya recibe el relato entero desde dentro.
//// `arg0` = cuantos despertar (0 = solo censar, `u32::MAX` = todos).
//// `arg1` = el modo: 0 despertar - 1 PARAR - 2 la prueba de reparto.
//// Devuelve 1 si encontro un aparato de reproduccion. Los NUMEROS van a
//// CABINA: son ocho y por la puerta cabe uno.
pub(super) fn audio_censo(arg0: u64, arg1: u64) -> BmoStatus {
        let hubo = unsafe { crate::ring0::dev::usb::audio::censar() };
        BmoStatus::ok_value(hubo as u64)
}

//// * TOMAR LA VENTANA DE UN APARATO (S1 del suelo de Ring 3).
////
//// `arg0` = cual, de la lista cerrada de `obj::mmio`. **No es una direccion**, y
//// esa es la decision entera: ver la cabecera de `obj/mmio.rs`.
////
//// El `CR3` es el del llamante --igual que al reclamar la pantalla-- porque es
//// ahi donde hay que mapear: durante un SYSCALL desde Ring 3 el cambio de CR3
//// todavia no ha ocurrido.
pub(super) fn aparato_tomar(arg0: u64, arg1: u64) -> BmoStatus {
        let _ = arg1;
        match crate::ring0::obj::mmio::claim(
            scheduler::current_pid(),
            crate::ring0::mm::vmm::read_cr3(),
            arg0,
        ) {
            Ok(handle) => BmoStatus::ok_value(handle),
            Err(code) => BmoStatus::err(code),
        }
}

//// * TOMAR EL LATIDO (S3 del suelo de Ring 3).
////
//// No lleva argumentos y no es exclusivo: el reloj no se gasta. Cien procesos
//// pueden esperarlo a la vez y los cien despiertan.
pub(super) fn latido_tomar(arg0: u64, arg1: u64) -> BmoStatus {
        let _ = (arg0, arg1);
        match crate::ring0::obj::latido::claim(scheduler::current_pid()) {
            Ok(handle) => BmoStatus::ok_value(handle),
            Err(code) => BmoStatus::err(code),
        }
}

pub(super) fn aparato_soltar(arg0: u64, arg1: u64) -> BmoStatus {
        let _ = (arg0, arg1);
        match crate::ring0::obj::mmio::release(
            scheduler::current_pid(),
            crate::ring0::mm::vmm::read_cr3(),
        ) {
            Ok(()) => BmoStatus::ok_value(0),
            Err(code) => BmoStatus::err(code),
        }
}


