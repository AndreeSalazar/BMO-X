//! BMO Layer_Nano-Wake — aprieta el gatillo y entrega el control.
//!
//! Valida el contrato mínimo de arranque, pinta un cuadrado blanco como
//! señal de vida post-UEFI y salta inmediatamente al kernel Ring 0.
//! Zero allocations, zero strings, zero fonts, zero animaciones.

#![no_std]
#![no_main]

const WAKE_SIDE: u64 = 8;
const WHITE: u32 = 0xFFFF_FFFF;

#[unsafe(no_mangle)]
#[link_section = ".text._start"]
#[unsafe(naked)]
unsafe extern "C" fn _start() -> ! {
    core::arch::naked_asm!("cld", "call nano_wake_main", "2: hlt", "jmp 2b",);
}

fn draw_wake_square(b: &bmo_boot_protocol::BootInfo) {
    let width = b.fb_width as u64;
    let height = b.fb_height as u64;
    let stride = b.fb_stride as u64; // GOP stride is measured in pixels.

    if b.fb_addr == 0 || b.fb_size == 0 || width < WAKE_SIDE || height < WAKE_SIDE || stride < width
    {
        return;
    }

    let x = (width - WAKE_SIDE) / 2;
    let y = (height - WAKE_SIDE) / 2;
    let Some(last_row) = (y + WAKE_SIDE - 1).checked_mul(stride) else {
        return;
    };
    let Some(last_pixel) = last_row.checked_add(x + WAKE_SIDE - 1) else {
        return;
    };
    let Some(required_bytes) = (last_pixel + 1).checked_mul(4) else {
        return;
    };
    if required_bytes > b.fb_size {
        return;
    }

    let fb = b.fb_addr as *mut u32;
    for row in 0..WAKE_SIDE {
        let row_start = (y + row) * stride + x;
        for column in 0..WAKE_SIDE {
            unsafe {
                fb.add((row_start + column) as usize).write_volatile(WHITE);
            }
        }
    }

    // Make the wake marker visible before Ring 0 takes ownership of the display.
    unsafe {
        core::arch::asm!("sfence", options(nostack, preserves_flags));
    }
}

#[unsafe(no_mangle)]
extern "C" fn nano_wake_main(bi: *const bmo_boot_protocol::BootInfo) -> ! {
    if bi.is_null() {
        halt();
    }
    let b = unsafe { &*bi };
    if !b.is_valid() {
        halt();
    }

    draw_wake_square(b);

    let entry = b.services_entry;
    let Some(services_end) = b.services_base.checked_add(b.services_size) else {
        halt();
    };
    if b.services_base == 0
        || entry < b.services_base
        || entry >= services_end
        || b.stack_top == 0
        || b.stack_size < 4096
        || b.stack_top & 0xF != 0
    {
        halt();
    }

    // Discard Nano-Wake's call frame and give Ring 0 the pristine boot stack.
    unsafe {
        core::arch::asm!(
            "mov rsp, {stack}",
            "xor ebp, ebp",
            "jmp {e}",
            stack = in(reg) b.stack_top,
            e = in(reg) entry,
            in("rdi") bi,
            options(noreturn)
        );
    }
}

#[inline(never)]
fn halt() -> ! {
    loop {
        unsafe {
            core::arch::asm!("hlt");
        }
    }
}

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    halt()
}
