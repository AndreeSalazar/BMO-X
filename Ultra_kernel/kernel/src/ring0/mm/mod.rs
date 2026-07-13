//! Memory subsystem — minimal Ring 0 base.
//!
//! In Ultra_kernel's Ring 0 base we don't yet have a full paging or
//! allocator stack. What we do have:
//!
//! - A parsed copy of the UEFI memory map (filled by the UEFI chain).
//! - A bitmap-based physical frame allocator over the usable ranges.
//! - Stubs for VMM, heap, and buddy that will be implemented in
//!   later phases.
//!
//! The `mm` module exists so that the rest of the kernel can call
//! `mm::init(ctx)` once and then use the frame allocator in a
//! `mm::phys::alloc_frame()` style.

pub mod types;
pub mod phys;
pub mod frame_alloc;
pub mod vmm_stub;
pub mod heap_stub;
pub mod buddy_stub;
pub mod slab;
pub mod vdso;

/// Initialize the memory subsystem from a `BootContext`.
/// Called once at kernel entry.
pub fn init(_ctx: &boot_context::BootContext) {
    // The full paging/vmm/heap setup is intentionally left for
    // future phases. For the Ring 0 base, we just log that we saw
    // the memory map.
    crate::ring0::dev::console::serial_write("[mm] init (stub: full paging/heap deferred)\n");
}
