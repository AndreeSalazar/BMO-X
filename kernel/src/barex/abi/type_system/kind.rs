//! `TypeKind` — clasificación raíz de cualquier tipo BMO.
//!
//! Un solo `enum` cubre lo que C/C++/Rust/Java/Python/Go necesitan describir.

use crate::barex::abi::primitives::bx_u8;

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TypeKind {
    /// `void` / `()` / `Unit` / `None`.
    Void          = 0,
    /// Enteros con signo (i8..i128).
    SignedInt     = 1,
    /// Enteros sin signo (u8..u128).
    UnsignedInt   = 2,
    /// IEEE-754 (f16, f32, f64, f128).
    Float         = 3,
    /// Booleano (1 byte, sólo 0 o 1 válido).
    Bool          = 4,
    /// Carácter Unicode escalar (4 bytes, válido en `[0..=0x10FFFF] - surrogates`).
    Char          = 5,
    /// Puntero crudo (`*mut T`, `*const T`, `T*`).
    Pointer       = 6,
    /// Referencia BMO (handle 64-bit con generación).
    Handle        = 7,
    /// Slice `(ptr, len)` 16 bytes.
    Slice         = 8,
    /// String UTF-8 owned o borrowed.
    String        = 9,
    /// Array de tamaño fijo conocido en compile-time.
    Array         = 10,
    /// Tupla heterogénea (structs anónimos).
    Tuple         = 11,
    /// Struct nominal con campos nombrados.
    Struct        = 12,
    /// Union (variantes superpuestas, layout compartido).
    Union         = 13,
    /// Enum etiquetado (suma de productos, tagged union).
    Enum          = 14,
    /// Función (puntero a código + signature).
    Function      = 15,
    /// Closure (puntero a código + entorno capturado).
    Closure       = 16,
    /// VTable de método dinámico (interface, trait object).
    Interface     = 17,
    /// Tipo opaco (size+align conocidos pero sin layout interno expuesto).
    Opaque        = 18,
    /// Tipo genérico aún no monomorfizado (template parameter).
    Generic       = 19,
    /// Tipo bottom (`!` de Rust, `Never`, NoReturn). Habitantes: 0.
    Never         = 20,
}

impl TypeKind {
    #[inline(always)]
    pub const fn raw(self) -> bx_u8 { self as bx_u8 }

    #[inline(always)]
    pub const fn is_primitive(self) -> bool {
        matches!(
            self,
            Self::Void | Self::SignedInt | Self::UnsignedInt
            | Self::Float | Self::Bool | Self::Char | Self::Never
        )
    }

    #[inline(always)]
    pub const fn is_aggregate(self) -> bool {
        matches!(
            self,
            Self::Array | Self::Tuple | Self::Struct | Self::Union | Self::Enum
        )
    }

    #[inline(always)]
    pub const fn is_callable(self) -> bool {
        matches!(self, Self::Function | Self::Closure)
    }
}
