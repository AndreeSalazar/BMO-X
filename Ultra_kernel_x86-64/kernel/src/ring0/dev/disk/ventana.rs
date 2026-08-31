//! **Las ventanas de escritura: donde SE PUEDE escribir.** El estado; la
//!
//! [carril]  ROJO      donde SE PUEDE escribir: la valla que protege el NVMe de Windows
//! decision vive en `bmo_block::ventana`.
//!
//! Paso 2 de `docs/plan/PLAN_ALMACENAMIENTO.md`. De las siete preguntas que
//! respondia `dev/disk/mod.rs`, esta es la segunda que **no toca hardware**:
//!
//! ```text
//!   puedo escribir AQUI?   ->  la ventana   ** CERO HARDWARE: es POLITICA
//! ```
//!
//! Vivia mezclada con los registros del HBA y no tiene nada que ver con ellos:
//! es una decision sobre rangos de LBA que se toma antes de que ningun
//! dispositivo se entere.
//!
//! # Por que la DECISION no esta aqui
//!
//! Se escribio aqui primero, con sus pruebas, y **las pruebas no corrian**:
//! `bmo-kernel` es un binario `no_std` para `x86_64-unknown-none` con asm
//! desnudo, y `cargo test -p bmo-kernel` ni compila. Un `#[cfg(test)]` en este
//! fichero es codigo que parece una prueba y no se ejecuta jamas.
//!
//! Asi que la funcion pura se fue al CONTRATO (`bmo_block::ventana::decidir`),
//! que ademas es donde el plan ya decia que debia estar: *"un dispositivo que
//! solo acepta escrituras dentro de un rango de LBA declarado merece vivir en
//! el contrato, no como caso especial del pegamento"*. Alli son siete casillas
//! en 0 segundos.
//!
//! **Lo que se queda aqui es lo unico que no puede irse: el ESTADO.**
//!
//! # Por que DOS ventanas y no una ensanchada
//!
//! ESTRATOS vive en otra particion que la FAT32 de los `.bex`. La tentacion al
//! llegar su primera escritura fue ensanchar la ventana existente.
//!
//! ** Ensanchar un guardian es quitarlo. Dos ventanas con nombre propio dejan
//! la ESP --donde vive el `BOOTX64.EFI` con el que arranca BMO, y en una
//! maquina con Windows tambien el cargador del boss-- fuera de las dos. Una
//! ventana ensanchada que las cubriera a ambas se comeria el hueco de en
//! medio, y ese hueco es justo lo que se protegia. Hay una casilla que lo
//! comprueba.

/// ** LA VENTANA DE ESTRATOS.
///
/// Se registra **solo** cuando se cumplen las dos condiciones: el volumen
/// queda montado *y* el gate de identidad dijo que nacio en este disco. Un
/// volumen clonado no registra ventana y por tanto **no puede escribir**,
/// aunque el disco este armado.
static mut VENTANA_ES: Option<(u64, u64)> = None;

/// Registra la ventana de ESTRATOS. La llama `fsys::estratos::mount`.
pub fn armar_ventana_estratos(primer_lba: u64, ultimo_lba: u64) {
    unsafe { VENTANA_ES = Some((primer_lba, ultimo_lba)) };
    crate::ring0::cabina::info("estratos", "ventana de escritura armada en", primer_lba);
}

/// Quita la ventana. Se llama al EMPEZAR a montar: si el montaje falla o la
/// identidad no cuadra, **no queda la ventana de la vez anterior**.
pub fn desarmar_ventana_estratos() {
    unsafe { VENTANA_ES = None };
}

pub fn ventana_estratos() -> Option<(u64, u64)> {
    unsafe { VENTANA_ES }
}
