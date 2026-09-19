//! **LA CONVENCION DE LLAMADA DE BMO-X** -- una sola, x86-64, y la que los
//! emisores EJECUTAN, no la que alguien escribio antes que ellos.
//!
//! ```text
//!    argumentos escalares   rdi, rsi, rdx, rcx, r8, r9      (ARGUMENTOS)
//!    del septimo en adelante   la pila, empujados de derecha a izquierda;
//!                              el que LLAMA los quita al volver
//!    agregados y flotantes  la pila (BMO C), por ranuras de 8
//!    funciones variadicas   TODO por la pila (el `va_arg` de BMO C es `*ap++`)
//!    resultado              rax                              (RETORNO)
//!    preservados            rbx, rbp, r12, r13, r14, r15    (PRESERVADOS)
//!    zona roja              NINGUNA                          (RED_ZONE_BYTES = 0)
//!    pila al hacer `call`   alineada a 8 garantizado         (STACK_ALIGN_BYTES)
//! ```
//!
//! # [!] 2026-09-19: lo que decia antes, y por que era peor que no decir nada
//!
//! Este fichero declaraba SIETE registros (con `r10`), retorno en `rax:rdx`,
//! zona roja de 256 y pila alineada a 64. Ningun emisor hizo nunca nada de eso,
//! y un test (`abi_layout.rs`) blindaba el siete. A la vez `inti.toml` decia
//! CINCO (le faltaba el cuarto: `rcx` figuraba solo como "trabajo") y C e INTI
//! llevaban cada uno su lista de seis escrita a mano. Tres fuentes, tres
//! respuestas, y la unica verdadera era la que no estaba en el contrato.
//!
//! ** Ahora los dos emisores IMPORTAN [`ARGUMENTOS`] en vez de copiarlo: que
//! diverjan no es algo que un test tenga que cazar, es algo que no compila.
//!
//! # Lo que NO se promete, y por que
//!
//! - **16 bytes al hacer `call`.** INTI redondea su marco a 16, pero un numero
//!   impar de argumentos por la pila lo deja en 8; BMO C alinea sus locales a 8
//!   y empuja temporales entre medias. Lo garantizado es 8. Mientras ningun
//!   emisor guarde en la pila con una instruccion alineada (`movaps`), no hace
//!   falta mas; el dia que se quiera, se sube AQUI y se exige a los dos.
//! - **Zona roja.** El reenvio de BMO C (19-09) ya va sin marco, pero no
//!   escribe por debajo de `rsp`. Usarla es rapido en Ring 3 y una corrupcion
//!   cada pocos miles de arranques donde una interrupcion comparte la pila.
//! - **Retorno en `rax:rdx`.** Solo la PUERTA devuelve dos registros (codigo y
//!   valor, [`X86_64_SYSCALL_RETURN_REGISTERS`]); una funcion devuelve uno.

use crate::bmo_abi::primitives::{bx_u32, bx_u8};

/// Los registros de argumento, POR SU NUMERO de x86-64, en orden: rdi, rsi,
/// rdx, rcx, r8, r9. **Lo leen los emisores** (C: `decidir/llamada.rs`, INTI:
/// `emisor-x86_64/src/lib.rs`).
pub const ARGUMENTOS: [u8; 6] = [7, 6, 2, 1, 8, 9];
/// Los mismos, por su nombre.
pub const ARGUMENTOS_NOMBRE: [&str; 6] = ["rdi", "rsi", "rdx", "rcx", "r8", "r9"];
/// Cuantos argumentos van en registro.
pub const GPR_ARG_COUNT: usize = ARGUMENTOS.len();
/// Por donde vuelve el resultado: `rax`.
pub const RETORNO: u8 = 0;
/// Los que sobreviven a una llamada: rbx, rbp, r12..r15. El que los usa, los
/// devuelve como estaban.
pub const PRESERVADOS: [u8; 6] = [3, 5, 12, 13, 14, 15];

/// La puerta del kernel NO es una llamada: `syscall` destruye `rcx` (el `rip`
/// de vuelta) y `r11` (las banderas), asi que el cuarto argumento va en `r10`.
pub const SYSCALL_GPR_ARG_COUNT: usize = 6;
pub const X86_64_SYSCALL_ARG_REGISTERS: &[&str] =
    &["rdi", "rsi", "rdx", "r10", "r8", "r9"];
/// La puerta si devuelve dos: el CODIGO en `rax` y el VALOR en `rdx`.
pub const X86_64_SYSCALL_RETURN_REGISTERS: &[&str] = &["rax", "rdx"];

/// Zona roja: NINGUNA. Ver la cabecera del fichero.
pub const RED_ZONE_BYTES: u32 = 0;

/// Alineacion de la pila GARANTIZADA al hacer `call`. Ver la cabecera.
pub const STACK_ALIGN_BYTES: u32 = 8;

/// La convencion. Una, porque BMO-X es una maquina: el `SystemVAmd64` que
/// habia aqui era "para shims de compatibilidad ELF" que no existen.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CallingConvention {
    BmoX86_64 = 0,
    // ** 1 y 2 fueron ARM64 y RISC-V y 3 System V: no se reutilizan.
}

impl CallingConvention {
    /// Cuantos argumentos van en registro.
    pub const fn gpr_arg_count(self) -> bx_u8 {
        GPR_ARG_COUNT as bx_u8
    }

    /// Alineacion garantizada al hacer `call`.
    pub const fn stack_align(self) -> bx_u32 {
        STACK_ALIGN_BYTES
    }

    pub fn name(self) -> &'static str {
        "bmo-x86_64"
    }

    pub const NATIVE: CallingConvention = CallingConvention::BmoX86_64;
}

/// Scalar types that can be passed in a single GPR.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScalarKind {
    Void = 0,
    I8 = 1,
    I16 = 2,
    I32 = 3,
    I64 = 4,
    U8 = 5,
    U16 = 6,
    U32 = 7,
    U64 = 8,
    F32 = 9,
    F64 = 10,
    Pointer = 11,
    Bool = 12,
}

impl ScalarKind {
    pub const fn size(self) -> bx_u8 {
        match self {
            Self::Void => 0,
            Self::I8 | Self::U8 | Self::Bool => 1,
            Self::I16 | Self::U16 => 2,
            Self::I32 | Self::U32 | Self::F32 => 4,
            Self::I64 | Self::U64 | Self::F64 | Self::Pointer => 8,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Void => "void",
            Self::I8 => "i8",
            Self::I16 => "i16",
            Self::I32 => "i32",
            Self::I64 => "i64",
            Self::U8 => "u8",
            Self::U16 => "u16",
            Self::U32 => "u32",
            Self::U64 => "u64",
            Self::F32 => "f32",
            Self::F64 => "f64",
            Self::Pointer => "ptr",
            Self::Bool => "bool",
        }
    }
}
