#![no_std]

extern crate alloc;

use core::alloc::{GlobalAlloc, Layout};
use core::sync::atomic::{AtomicUsize, Ordering};

/// Simple bump allocator for UEFI boot services.
/// This allocator uses a static buffer and never frees memory.
/// It's only used during boot before ExitBootServices.
struct BumpAllocator {
    buffer: [u8; 65536], // 64 KB should be enough for boot
    offset: AtomicUsize,
}

impl BumpAllocator {
    const fn new() -> Self {
        Self {
            buffer: [0; 65536],
            offset: AtomicUsize::new(0),
        }
    }
}

unsafe impl GlobalAlloc for BumpAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let size = layout.size();
        let align = layout.align();

        loop {
            let current = self.offset.load(Ordering::Relaxed);
            let aligned = (current + align - 1) & !(align - 1);
            let new_offset = aligned + size;

            if new_offset > self.buffer.len() {
                return core::ptr::null_mut();
            }

            if self.offset.compare_exchange_weak(
                current,
                new_offset,
                Ordering::Relaxed,
                Ordering::Relaxed,
            ).is_ok() {
                return self.buffer.as_ptr().add(aligned) as *mut u8;
            }
        }
    }

    unsafe fn dealloc(&self, _ptr: *mut u8, _layout: Layout) {
        // Bump allocator never frees
    }
}

#[global_allocator]
static ALLOCATOR: BumpAllocator = BumpAllocator::new();

pub mod layers;
pub mod serial;

pub use layers::layer0_enter::layer0_efi_main;
pub use layers::layer1_getmem::l1_entry;
pub use layers::layer2_getgop::l2_entry;
pub use layers::layer3_load::l3_entry;
pub use layers::layer4_exit::l4_entry;
