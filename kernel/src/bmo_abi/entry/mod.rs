//! `bmo_abi::entry` — Punto de entrada de un programa BMO ABI.
//!
//! Define la convención de cómo un programa BMO ABI recibe el control
//! cuando el loader de BEF lo mapea en memoria.
//!
//! ## Convención
//!
//! Todo binario BMO ABI **debe** exportar el símbolo `_bmo_start`. El
//! loader salta a esa dirección después de:
//!
//! 1. Mapear el binario en memoria.
//! 2. Aplicar relocalaciones.
//! 3. Configurar el stack pointer (`RSP` apunta a una pila de 1 MB).
//! 4. Limpiar `.bss`.
//! 5. Inicializar TLS (puntero en `FS:0`).
//!
//! ## Firma
//!
//! ```ignore
//! #[no_mangle]
//! pub extern "sysv64" fn _bmo_start(argc: u64, argv: *const *const u8) -> ! {
//!     // llamar al main del usuario
//!     let rc = user_main(argc, argv);
//!     bmo_exit(rc as i32);
//! }
//! ```
//!
//! **El programa nunca retorna de `_bmo_start`**: debe terminar con
//! `bmo_exit` o `bmo_proc_exit`.

#![allow(dead_code)]

// ─── Entry signature ──────────────────────────────────────────────

/// Tipo de la función `_bmo_start`.
///
/// **Importante**: el programa NO debe retornar. Si lo hace, el
/// kernel lo considerará un crash y el proceso morirá con
/// `BmoErrorCode::InvalidState`.
pub type BmoEntryFn = unsafe extern "sysv64" fn(argc: u64, argv: *const *const u8) -> !;

// ─── Stack layout ─────────────────────────────────────────────────

/// Tamaño por defecto del stack inicial (1 MB).
pub const BMO_STACK_DEFAULT_SIZE: u64 = 1 * 1024 * 1024;

/// Tamaño mínimo del stack (64 KB).
pub const BMO_STACK_MIN_SIZE: u64 = 64 * 1024;

/// Tamaño máximo del stack (16 MB).
pub const BMO_STACK_MAX_SIZE: u64 = 16 * 1024 * 1024;

/// Alineación del stack pointer (16 bytes para SysV).
pub const BMO_STACK_ALIGN: u64 = 16;

// ─── Argumentos (argc/argv/envp) ─────────────────────────────────

/// Estructura de `argv` / `envp` pasadas al programa.
///
/// `argv` y `envp` son arrays de `*const u8` (punteros a strings
/// UTF-8 null-terminated), terminados con un puntero nulo.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct BmoArgs {
    pub argc: u64,
    pub argv: *const *const u8,
    pub envp: *const *const u8,
}

impl BmoArgs {
    /// `true` si el programa no recibió argumentos.
    pub fn is_empty(&self) -> bool {
        self.argc == 0 || self.argv.is_null()
    }

    /// Itera sobre los argumentos como `&str` (sin copia).
    ///
    /// **Unsafe**: el llamador debe garantizar que los punteros son
    /// válidos hasta el final del programa.
    pub unsafe fn iter(&self) -> BmoArgsIter {
        BmoArgsIter { ptr: self.argv, end: self.argc }
    }
}

/// Iterador sobre `argv`.
pub struct BmoArgsIter {
    ptr: *const *const u8,
    end: u64,
}

impl Iterator for BmoArgsIter {
    type Item = *const u8;
    fn next(&mut self) -> Option<Self::Item> {
        if self.end == 0 { return None; }
        self.end -= 1;
        unsafe {
            let p = *self.ptr;
            self.ptr = self.ptr.add(1);
            Some(p)
        }
    }
}
