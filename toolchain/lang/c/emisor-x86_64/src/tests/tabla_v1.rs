//! LA TABLA V1 SE FUE -- y lo que la usaba ya no compila
//!
//! Hasta el 2026-09-19 `use "bmo/proc"` prestaba 109 nombres (`bmo_exit` 0x181,
//! `bmo_mem_alloc` 0x190...) que el kernel no despacha: todos contestan
//! `rax = ERROR_UNSUPPORTED` (10), y 10 no es cero. El `malloc` de
//! `stdlib/heap` lo tomaba por una direccion y escribia en `0xA`. Y un test de
//! este banco EXIGIA que `bmo_exit(42)` emitiera `mov eax, 0x181`.
//!
//! Estas filas son la otra cara: lo que antes compilaba hacia una puerta muerta
//! ahora se para al compilar, con el nombre de lo que falta.

use super::*;
use std::path::PathBuf;

fn base() -> Vec<PathBuf> {
    vec![PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../base")]
}

#[test]
fn un_use_sin_modulo_no_compila() {
    let src = r#"use "bmo/proc"; int main() { return 0; }"#;
    let e = compile_source_to_bef_with_modules(src, base()).unwrap_err();
    assert!(e.message.contains("bmo/proc"), "el error tiene que nombrar el modulo: {e:?}");
}

#[test]
fn un_nombre_v1_no_compila() {
    // Sin `use`, y sin definicion: es una llamada a una funcion que no existe.
    let src = r#"int main() { bmo_exit(42); return 0; }"#;
    assert!(compile_source_to_bef(src).is_err(), "bmo_exit ya no es una puerta: no puede compilar");
}
