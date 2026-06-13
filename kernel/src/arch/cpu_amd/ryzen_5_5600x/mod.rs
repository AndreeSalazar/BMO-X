//! Ryzen 5 5600X (Zen 3) CPU initialization
//!
//! Enables full Zen 3 processing power (SSE, AVX, AVX2, FMA3) by configuring
//! CPU Control Registers (CR0, CR4) and Extended Control Registers (XCR0).

use crate::arch::cpu::CpuFeatures;

/// Configures CR0, CR4, and XCR0 registers to enable hardware vector extensions.
pub fn init(features: &CpuFeatures) {
    crate::drivers::serial::serial_write("[Ryzen-5600X] Inicializando optimizaciones Ryzen 5 5600X (Zen 3)...\n");

    unsafe {
        // 1. Configurar CR0 para activar Coprocesador Matemático y deshabilitar emulación por software
        let mut cr0: u64;
        core::arch::asm!("mov {}, cr0", out(reg) cr0);
        cr0 |= 1 << 1;    // set MP (Monitor Coprocessor)
        cr0 &= !(1 << 2); // clear EM (Emulation)
        core::arch::asm!("mov cr0, {}", in(reg) cr0);
        
        // 2. Configurar CR4 para activar SSE y excepciones XMM
        let mut cr4: u64;
        core::arch::asm!("mov {}, cr4", out(reg) cr4);
        
        if features.has_sse {
            cr4 |= 1 << 9;  // set OSFXSR (FXSAVE/FXRSTOR support)
        }
        if features.has_sse2 {
            cr4 |= 1 << 10; // set OSXMMEXCPT (Unmasked Exception support)
        }
        
        // 3. Habilitar soporte OSXSAVE para XSAVE y registro de estados extendidos (AVX)
        if features.has_avx {
            cr4 |= 1 << 18; // set OSXSAVE
            crate::drivers::serial::serial_write("[Ryzen-5600X] Soporte AVX/FMA3: OK\n");
        }
        
        core::arch::asm!("mov cr4, {}", in(reg) cr4);

        // 4. Configurar XCR0 para habilitar x87, SSE y AVX
        if features.has_avx {
            let mut xcr0: u64 = 0;
            xcr0 |= 1 << 0; // x87 state
            xcr0 |= 1 << 1; // SSE state
            xcr0 |= 1 << 2; // AVX state
            
            let eax = (xcr0 & 0xFFFFFFFF) as u32;
            let edx = (xcr0 >> 32) as u32;
            core::arch::asm!(
                "xsetbv",
                in("ecx") 0,
                in("eax") eax,
                in("edx") edx
            );
            crate::drivers::serial::serial_write("[Ryzen-5600X] XSAVE/XCR0 configurado (x87 + SSE + AVX activados).\n");
        }
    }

    crate::drivers::serial::serial_write("[Ryzen-5600X] CPU Ryzen 5 5600X configurado a maximo rendimiento de hardware.\n");
}
