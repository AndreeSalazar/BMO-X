//! `BmoHandle` -- handle opaco 64-bit con generacion.
//!
//! Layout:
//! ```text
//!   bit 63        : tag        (0 = recurso, 1 = canal/cola)
//!   bits 62..56   : kind       (7 bits -- 128 tipos)
//!   bits 55..40   : generation (16 bits -- invalida UAF)
//!   bits 39..0    : index      (40 bits -- 1 trillon de slots)
//! ```
//!
//! -- EL SEMAFORO (L6g) y las dos preguntas de antes (L6e, L6f) --------
//!
//! Que cuesta que falle, por que falla ESTA pieza, y que arrastro si la
//! toco. La ley esta en `META-KERNEL_HARD.md`.
//!
//! [carril]  ROJO         empaqueta tag, kind y GENERACION en 64 bits a base
//!                        de desplazamientos
//! [cuesta]  PUERTA       un desplazamiento mal deja pasar un handle revocado
//!                        como si fuera valido
//! [riesgo]  SILENCIO     la generacion es lo unico que caduca un handle. Si
//!                        se lee del bit que no es, un handle muerto vuelve a
//!                        valer y NADA falla al hacerlo

use super::kind::HandleKind;
use crate::bmo_abi::primitives::{bx_u16, bx_u64, bx_u8};

// -- ** EL FORMATO, CON NOMBRE ------------------------------------------
//
// Estos seis numeros estaban escritos a pelo dentro de los metodos de abajo
// **y otra vez dentro de `ring0/obj/cap.rs`**, que ni siquiera lo disimulaba:
// su comentario dice *"mirror of bmo-abi handle/kind.rs"*. El mismo formato en
// dos ficheros que no se hablaban.
//
// ** ESA ES LA FORMA EXACTA DEL `#GP(0x18)` DEL 16-08: `SYSRET_SELECTOR_BASE` y
// `USER_SS` eran el mismo numero en dos sitios sin contrato, y costo un
// arranque. Aqui habria costado peor: un desplazamiento mal puesto compila,
// devuelve "handle invalido", y parece un permiso denegado.
//
// Con nombre, el guardian de `build.ps1` puede exigir que los dos lados digan
// lo mismo -- igual que ya hace con las 49 operaciones y los 63 campos de
// `OP_INFO`. Un formato es un CONTRATO; escribirlo dos veces es tener dos.
pub const HANDLE_TAG_SHIFT: u64 = 63;
pub const HANDLE_KIND_SHIFT: u64 = 56;
pub const HANDLE_KIND_MASK: u64 = 0x7F;
pub const HANDLE_GEN_SHIFT: u64 = 40;
pub const HANDLE_GEN_MASK: u64 = 0xFFFF;
pub const HANDLE_INDEX_MASK: u64 = 0x000000FF_FFFFFFFF;

#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BmoHandle(pub bx_u64);

impl BmoHandle {
    pub const NULL: Self = Self(0);

    /// Handle invalido (distinto de NULL para detectar errores).
    /// Generacion = 0xFFFF, index = 0xFFF... -- nunca se asigna un handle real.
    pub const INVALID: Self = Self(0x0000_FFFF_FFFF_FFFF);

    #[inline(always)]
    pub const fn new(kind: HandleKind, generation: bx_u16, index: bx_u64) -> Self {
        let tag = (kind.tag() as bx_u64) << HANDLE_TAG_SHIFT;
        let kind_bits = ((kind.code() as bx_u64) & HANDLE_KIND_MASK) << HANDLE_KIND_SHIFT;
        let gen_bits = ((generation as bx_u64) & HANDLE_GEN_MASK) << HANDLE_GEN_SHIFT;
        let idx_bits = index & HANDLE_INDEX_MASK;
        Self(tag | kind_bits | gen_bits | idx_bits)
    }

    #[inline(always)]
    pub const fn is_null(self) -> bool {
        self.0 == 0
    }

    #[inline(always)]
    pub const fn is_resource(self) -> bool {
        (self.0 >> HANDLE_TAG_SHIFT) == 0
    }

    #[inline(always)]
    pub const fn is_active(self) -> bool {
        (self.0 >> HANDLE_TAG_SHIFT) == 1
    }

    #[inline(always)]
    pub const fn kind_code(self) -> bx_u8 {
        ((self.0 >> HANDLE_KIND_SHIFT) & HANDLE_KIND_MASK) as bx_u8
    }

    /// Decodifica el `HandleKind`. `None` si el codigo es desconocido.
    #[inline(always)]
    pub const fn kind(self) -> Option<HandleKind> {
        HandleKind::from_code(self.kind_code())
    }

    #[inline(always)]
    pub const fn generation(self) -> bx_u16 {
        ((self.0 >> HANDLE_GEN_SHIFT) & HANDLE_GEN_MASK) as bx_u16
    }

    #[inline(always)]
    pub const fn index(self) -> bx_u64 {
        self.0 & HANDLE_INDEX_MASK
    }

    /// Verifica que el handle es del kind esperado. Util para asserts en hot paths.
    #[inline(always)]
    pub fn is_kind(self, expected: HandleKind) -> bool {
        self.kind_code() == expected.code()
    }
}
