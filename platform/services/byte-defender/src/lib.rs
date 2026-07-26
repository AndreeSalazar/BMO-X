//! ByteDefender: guardia de seguridad de BMO — pre-ejecución y runtime.
//!
//! # ⚠ NO ESTÁ CABLEADO. NO PROTEGE NADA HOY. NO LO CONECTES.
//!
//! Nadie depende de este crate (solo aparece en `members` del workspace), y
//! así debe seguir hasta que se rehaga. Esta advertencia existe porque un
//! componente de seguridad roto es PEOR que su ausencia: uno ausente te deja
//! alerta, uno que dice "verificado" sin verificar te quita la alerta y no da
//! nada a cambio. Es teatro, y en un sistema cuya bandera es cero-confianza
//! en el código, es justo la pieza que no puede mentir.
//!
//! ## Lo que se encontró al auditarlo (2026-07-25)
//!
//! **Hay TRES parsers del formato BEF1 en el repo y ninguno se parece a otro.**
//! El bueno es `kernel/src/ring0/bex.rs` — el que admite los programas Ring 3
//! en hardware real. `scanner.rs` y `verifier.rs` traen cada uno el suyo, y se
//! contradicen ENTRE ELLOS, lo que prueba que ninguno se probó jamás contra un
//! BEF de verdad:
//!
//! | Campo             | `bex.rs` (real)      | `scanner.rs`        | `verifier.rs`        |
//! |-------------------|----------------------|---------------------|----------------------|
//! | `flags`           | offset 8, u32        | —                   | lo lee como count    |
//! | `arch`            | offset 12, u8        | lo lee como count   | lo lee como size     |
//! | `abi_major/minor` | offset 16-17         | lo lee como imports | —                    |
//! | `section_count`   | **offset 40**, u32   | offset 12           | offset 8             |
//! | Entrada de sección| 48 bytes, flags en +4| 32 bytes, flags +8  | 16 bytes             |
//!
//! Consecuencias medidas:
//!
//! - **`verifier::verify_header` rechaza TODOS los binarios válidos.** Lee
//!   `total_size` del offset 12, donde vive `arch = 0x01`, y luego comprueba
//!   `total_size < 48` → siempre falso. Conectado al arranque, `hola_C.bex` y
//!   `hola_COBOL.bex` serían "corruptos".
//! - **`scanner::has_wx_section` es ruido**: busca los flags con el offset y
//!   el paso equivocados; lo que reporta como sección W+X son bytes al azar.
//! - **La policy no existe**: `check_syscall` devuelve `Allow` para todo y
//!   `has_all_caps` devuelve `true` para todo. Pero el orquestador arranca con
//!   `pre_exec: true, runtime_guard: true` — se anuncia encendido.
//! - **Usa FNV-1a para integridad.** No es criptográfico: fabricar un binario
//!   distinto con el mismo hash es trivial. Un hash forjable no protege de
//!   nadie; solo hace creer que sí. (BLAKE3 ya está en `bmo-abi`.)
//!
//! ## En qué debe convertirse
//!
//! No en un verificador nuevo — BMO ya tiene el que funciona:
//!
//! 1. **Delegar el parseo**: `bex::inspect` en Ring 0 y `forge/bmo-verify` en
//!    el toolchain. Un formato, un parser. Si el BEF gana un campo, cambia en
//!    un sitio y no en tres que se separan en silencio.
//! 2. **Quedarse con lo suyo**, que no existe en ningún otro lado: la POLÍTICA
//!    (qué capabilities puede pedir un programa y quién se las concede), la
//!    cuarentena y el informe que CABINA pinta.
//! 3. **El guardia de runtime vive en el dispatch de syscalls**, que es
//!    territorio del kernel, no de un crate de Ring 3.
//! 4. **BLAKE3**, el mismo hash que `bmo-verify` y que ESTRATOS.
//!
//! ## Cuándo
//!
//! Cuando ESTRATOS pida la firma. Ver `platform/services/timeback/ESTRATOS.md`
//! §7: `abrir(nodo, EJECUTAR)` comprueba el atributo `:firma` y, si no cuadra,
//! no entrega handle ejecutable. Ese es el agujero que ByteDefender rellena —
//! y construir la pieza antes de saber la forma del agujero es cómo nacen las
//! tres versiones que no encajan.
//!
//! ## Regla de oro (esta se mantiene)
//!
//! - ByteDefender **no pinta UI**. Solo analiza y reporta.
//! - CABINA muestra sus informes.
//! - TimeBack puede crear un punto de retorno antes de ejecutar.

#![no_std]

extern crate alloc;

pub mod bytedefender;
pub mod policy;
pub mod scanner;
pub mod verifier;
pub mod capability;
pub mod report;
pub mod quarantine;

#[cfg(test)]
mod tests;

pub use bytedefender::*;
pub use capability::*;
pub use report::*;
pub use scanner::*;
pub use verifier::*;
