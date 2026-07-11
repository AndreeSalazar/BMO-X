//! BMO Layer_Nano-Wake — abre los ojos y nada más.
//!
//! `nano_wake_main` recibe el framebuffer de UEFI, pinta una señal de
//! vida breve (el ojo de BMO abriéndose), y salta al kernel real.
//! Zero allocations, zero strings, zero fonts, zero math de más.
//! Binario actual: ~785 bytes — cabe 40 veces en L1 cache.

#![no_std]
#![no_main]
#![allow(static_mut_refs)]

const BG: u32       = 0xFF0A0F1D;  // Azul profundo — el fondo antes de despertar
const CYAN: u32     = 0xFF00E5FF;  // Primera luz — el color de la conciencia
const LOGO_OUT: u32 = 0xFF4F46E5;  // Anillo exterior del ojo
const LOGO_MID: u32 = 0xFF312E81;  // Anillo medio del ojo

static mut FB: u64 = 0;  // Dirección del framebuffer
static mut W: u32  = 0;  // Ancho en píxeles
static mut H: u32  = 0;  // Alto en píxeles
static mut S: u32  = 0;  // Stride en bytes

/// Pinta el fondo completo. Cada píxel es un latido.
fn clear_screen(color: u32) {
    let fb = unsafe { FB } as *mut u64;
    let stride = unsafe { S } as usize;
    let height = unsafe { H } as usize;
    let dword = ((color as u64) << 32) | color as u64;
    let count = (stride * height) / 2;
    unsafe {
        core::arch::asm!(
            "rep stosq",
            inout("rdi") fb => _,
            inout("rcx") count => _,
            in("rax") dword,
            options(nostack)
        );
    }
}

/// Línea horizontal — un destello de luz en una fila.
fn hline(x: u32, y: u32, w: u32, c: u32) {
    let fb = unsafe { FB } as *mut u32;
    let stride = unsafe { S } as usize;
    let base = y as usize * stride + x as usize;
    let mut i = 0u32;
    while i < w {
        unsafe { fb.add(base + i as usize).write_volatile(c); }
        i += 1;
    }
}

/// Rectángulo sólido — un plano de luz.
fn fill_small(x: u32, y: u32, w: u32, h: u32, c: u32) {
    let mut row = 0u32;
    while row < h {
        hline(x, y + row, w, c);
        row += 1;
    }
}

/// Contorno rectangular — el borde del ojo que se abre.
fn outline(x: u32, y: u32, w: u32, h: u32, t: u32, c: u32) {
    fill_small(x, y, w, t, c);
    fill_small(x, y + h - t, w, t, c);
    fill_small(x, y, t, h, c);
    fill_small(x + w - t, y, t, h, c);
}

#[unsafe(no_mangle)]
#[link_section = ".text._start"]
#[unsafe(naked)]
unsafe extern "C" fn _start() -> ! {
    core::arch::naked_asm!(
        "mov r12, rdi",
        "lea rax, [rip + __bss_start]",
        "lea rcx, [rip + __bss_end]",
        "sub rcx, rax",
        "jz 2f",
        "mov rdi, rax",
        "xor eax, eax",
        "mov rdx, rcx",
        "shr rcx, 3",
        "jz 1f",
        "rep stosq",
        "1: and rdx, 7",
        "mov rcx, rdx",
        "jz 2f",
        "rep stosb",
        "2: mov rdi, r12",
        "call nano_wake_main",
        "3: hlt",
        "jmp 3b",
    );
}

#[inline(always)]
fn rdtsc() -> u64 {
    let lo: u32; let hi: u32;
    unsafe { core::arch::asm!("rdtsc", out("eax") lo, out("edx") hi, options(nostack, nomem)); }
    ((hi as u64) << 32) | lo as u64
}

#[inline(always)]
fn tsc_wait(cycles: u64) {
    let t = rdtsc();
    while rdtsc() - t < cycles { core::hint::spin_loop(); }
}

#[unsafe(no_mangle)]
extern "C" fn nano_wake_main(bi: *const bmo_boot_protocol::BootInfo) -> ! {
    if bi.is_null() { halt(); }
    let b = unsafe { &*bi };
    if b.magic != bmo_boot_protocol::BOOT_MAGIC { halt(); }

    unsafe { FB = b.fb_addr; W = b.fb_width; H = b.fb_height; S = b.fb_stride; }

    let (w, h) = unsafe { (W, H) };
    let cx = w / 2;
    let cy = h / 2;

    // Abrir los ojos: pinta el fondo, luego la pupila que se expande en anillos.
    clear_screen(BG);

    // Pupila — el primer destello de conciencia
    fill_small(cx - 5, cy - 65, 10, 10, CYAN);
    tsc_wait(30_000_000);

    // Iris — el ojo empieza a abrirse
    outline(cx - 16, cy - 76, 32, 32, 1, CYAN);
    tsc_wait(30_000_000);

    // Anillo medio — la mirada se afirma
    outline(cx - 24, cy - 84, 48, 48, 3, LOGO_MID);
    tsc_wait(30_000_000);

    // Anillo exterior — el ojo está completamente abierto. BMO ya ve.
    outline(cx - 32, cy - 92, 64, 64, 2, LOGO_OUT);
    tsc_wait(30_000_000);

    // Despertar completo: saltar al kernel real
    let entry = b.services_entry;
    if entry == 0 { halt(); }
    unsafe {
        core::arch::asm!(
            "jmp {e}",
            e = in(reg) entry,
            in("rdi") bi,
            options(noreturn)
        );
    }
}

#[inline(never)]
fn halt() -> ! { loop { unsafe { core::arch::asm!("hlt"); } } }

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! { halt() }
