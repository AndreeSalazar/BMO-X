use core::alloc::{GlobalAlloc, Layout};
use core::ptr::null_mut;
use core::sync::atomic::{AtomicUsize, Ordering};

const HEAP_SIZE: usize = 2 * 1024 * 1024; // 2 MB heap
static mut HEAP_SPACE: [u8; HEAP_SIZE] = [0; HEAP_SIZE];
static HEAP_NEXT: AtomicUsize = AtomicUsize::new(0);

struct SimpleAllocator;

unsafe impl GlobalAlloc for SimpleAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let size = layout.size();
        let align = layout.align();

        let mut current = HEAP_NEXT.load(Ordering::Relaxed);
        loop {
            let start = (current + align - 1) & !(align - 1);
            let end = start + size;

            if end > HEAP_SIZE {
                return null_mut();
            }

            match HEAP_NEXT.compare_exchange_weak(current, end, Ordering::Relaxed, Ordering::Relaxed) {
                Ok(_) => return &mut HEAP_SPACE[start] as *mut u8,
                Err(actual) => current = actual,
            }
        }
    }

    unsafe fn dealloc(&self, _ptr: *mut u8, _layout: Layout) {
        // Bump allocator — no deallocation.
    }
}

#[global_allocator]
static ALLOCATOR: SimpleAllocator = SimpleAllocator;

#[no_mangle]
pub extern "C" fn __rust_no_alloc_shim_is_unstable() {}
