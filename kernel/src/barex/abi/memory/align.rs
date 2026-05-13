//! Alignment helpers del BMO ABI.
//!
//! Reemplaza el `__attribute__((aligned(N)))` y `_Alignas` de C con tipos
//! y funciones explícitas. Defaults pensados para Zen 3:
//!   - 64 B  = línea de cache L1/L2
//!   - 4 KB  = página estándar
//!   - 2 MB  = huge page
//!   - 64 B  = stack alignment del BMO ABI

use crate::barex::abi::primitives::bx_u64;

pub const CACHE_LINE_BYTES: bx_u64 = 64;
pub const PAGE_BYTES:       bx_u64 = 4096;
pub const HUGE_PAGE_BYTES:  bx_u64 = 2 * 1024 * 1024;

/// Redondea `value` hacia arriba al múltiplo más cercano de `align`.
/// `align` debe ser potencia de 2.
#[inline(always)]
pub const fn align_up(value: bx_u64, align: bx_u64) -> bx_u64 {
    (value + align - 1) & !(align - 1)
}

/// Redondea `value` hacia abajo al múltiplo más cercano de `align`.
/// `align` debe ser potencia de 2.
#[inline(always)]
pub const fn align_down(value: bx_u64, align: bx_u64) -> bx_u64 {
    value & !(align - 1)
}

#[inline(always)]
pub const fn is_aligned(value: bx_u64, align: bx_u64) -> bool {
    (value & (align - 1)) == 0
}

#[inline(always)]
pub const fn is_power_of_two(v: bx_u64) -> bool {
    v != 0 && (v & (v - 1)) == 0
}

/// Wrapper que fuerza alineación de cache line para evitar false sharing.
///
/// Ejemplo: contadores entre threads.
/// ```ignore
/// static COUNTER: BmoAligned<core::sync::atomic::AtomicU64>
///     = BmoAligned::new(AtomicU64::new(0));
/// ```
#[repr(align(64))]
#[derive(Debug, Clone, Copy)]
pub struct BmoAligned<T>(pub T);

impl<T> BmoAligned<T> {
    #[inline(always)]
    pub const fn new(v: T) -> Self { Self(v) }
}

/// Wrapper para alineación de página (mapeo MMIO, DMA buffers).
#[repr(align(4096))]
#[derive(Debug)]
pub struct BmoPageAligned<T>(pub T);

impl<T> BmoPageAligned<T> {
    #[inline(always)]
    pub const fn new(v: T) -> Self { Self(v) }
}
