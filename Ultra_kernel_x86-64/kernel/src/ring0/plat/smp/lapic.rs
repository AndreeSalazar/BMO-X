//! **El LAPIC del BSP**: mandar IPIs y esperar.
//!
//! [carril]  ROJO      mandar IPIs y esperar
//!
//! Solo lo usa el que despierta. El AP recien llegado no toca esto -- ver
//! `tramp::apic_id`, que lo resuelve por CPUID justo por eso.

use crate::ring0::mm::HIGH_MEM_BASE;

/// Donde esta el LAPIC, **por el physmap**.
///
/// Su MMIO vive en `0xFEE0_0000` y el identity map del kernel solo cubre
/// `0..32 MiB`: tocarlo por su direccion fisica seria un #PF. Se llega por la
/// mitad alta, igual que hace `timer.rs`.
fn base() -> u64 {
    let lo: u32;
    let hi: u32;
    unsafe {
        core::arch::asm!("rdmsr", in("ecx") 0x1Bu32, out("eax") lo, out("edx") hi,
                         options(nomem, nostack));
    }
    HIGH_MEM_BASE + ((((hi as u64) << 32) | lo as u64) & 0x000F_FFFF_FFFF_F000)
}

/// Manda una IPI y espera a que salga.
///
/// El orden importa: **primero el ICR alto (el destino) y el bajo AL FINAL**,
/// porque escribir el bajo es lo que dispara el envio. Al reves se manda la
/// orden al destino anterior.
pub unsafe fn ipi(destino: u32, orden: u32) {
    let b = base();
    unsafe {
        core::ptr::write_volatile((b + 0x310) as *mut u32, destino << 24);
        core::ptr::write_volatile((b + 0x300) as *mut u32, orden);
        // Bit 12 = Delivery Status. Con tope: un LAPIC que no contesta no puede
        // quedarse con el que pregunta.
        let mut vueltas = 0u32;
        while core::ptr::read_volatile((b + 0x300) as *const u32) & (1 << 12) != 0
            && vueltas < 100_000
        {
            vueltas += 1;
            core::hint::spin_loop();
        }
    }
}

/// INIT: pone al AP en su estado de arranque.
pub const INIT: u32 = 0x0000_4500;
/// SIPI con vector `0x08` -> el AP empieza en `0x8000`.
pub const SIPI: u32 = 0x0000_4608;

/// Espera por TSC. Si no esta calibrado, unas vueltas y a otra cosa: aqui el
/// objetivo es dar aire al hardware, no medir.
pub fn esperar_us(us: u64) {
    let hz = crate::ring0::task::scheduler::tsc_freq();
    if hz == 0 {
        for _ in 0..200_000 {
            core::hint::spin_loop();
        }
        return;
    }
    let hasta = crate::ring0::task::scheduler::rdtsc() + (hz / 1_000_000) * us;
    while crate::ring0::task::scheduler::rdtsc() < hasta {
        core::hint::spin_loop();
    }
}
