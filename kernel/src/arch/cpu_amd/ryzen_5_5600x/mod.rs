//! Ryzen 5 5600X (Zen 3) CPU initialization
//!
//! Enables full Zen 3 processing power by configuring:
//! - CR0/CR4 for FPU/SSE/AVX/OSXSAVE
//! - XCR0 for x87+SSE+AVX extended states
//! - FPU initial state (FNINIT, MXCSR)
//! - MTRRs for memory type configuration
//! - PAT for framebuffer optimization
//! - Performance counters (fixed counter 0: instructions retired)

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
        cr0 |= 1 << 5;    // set NE (Numeric Error — #MF exception, not FPU error pin)
        cr0 |= 1 << 16;   // set WP (Write Protect — Ring 0 cannot write read-only pages)
        cr0 &= !(1 << 3); // clear TS (Task Switched — will be used for lazy FPU switching)
        core::arch::asm!("mov cr0, {}", in(reg) cr0);
        
        // 2. Configurar CR4 para activar SSE, excepciones XMM, OSXSAVE
        let mut cr4: u64;
        core::arch::asm!("mov {}, cr4", out(reg) cr4);
        
        if features.has_sse {
            cr4 |= 1 << 9;  // set OSFXSR (FXSAVE/FXRSTOR support)
        }
        if features.has_sse2 {
            cr4 |= 1 << 10; // set OSXMMEXCPT (Unmasked Exception support)
        }
        if features.has_avx {
            cr4 |= 1 << 18; // set OSXSAVE (XSAVE/XRSTOR/XGETBV/XSETBV support)
            crate::drivers::serial::serial_write("[Ryzen-5600X] Soporte AVX/FMA3: OK\n");
        }
        if features.has_fs_gs_base {
            cr4 |= 1 << 13; // set FSGSBASE (RDFSBASE/WRFSBASE/RDGSBASE/WRGSBASE)
            crate::drivers::serial::serial_write("[Ryzen-5600X] FSGSBASE: OK\n");
        }
        if features.has_smep {
            cr4 |= 1 << 20; // set SMEP (Supervisor Mode Execution Prevention)
        }
        if features.has_smap {
            cr4 |= 1 << 21; // set SMAP (Supervisor Mode Access Prevention)
        }
        if features.has_umip {
            cr4 |= 1 << 11; // set UMIP (User Mode Instruction Prevention)
        }
        
        core::arch::asm!("mov cr4, {}", in(reg) cr4);

        // 3. Configurar XCR0 para habilitar x87, SSE, AVX, y管理 de estados extendidos
        if features.has_avx {
            let mut xcr0: u64 = 0;
            xcr0 |= 1 << 0; // x87 state
            xcr0 |= 1 << 1; // SSE state (XMM registers)
            xcr0 |= 1 << 2; // AVX state (upper YMM halves)
            
            // If XSAVEOPT is supported, enable optimization hints
            if features.has_xsaveopt {
                crate::drivers::serial::serial_write("[Ryzen-5600X] XSAVEOPT: OK\n");
            }
            
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

        // 4. Initialize FPU with clean state
        crate::arch::fpu::init_fpu();

        // 5. Configure MTRRs for optimal memory types
        // VRAM base/size will be set later when GOP framebuffer is discovered
        crate::arch::cpu::init_mtrrs(0, 0);

        // 6. Configure PAT
        crate::arch::cpu::init_pat();

        // 7. Enable performance counters
        crate::arch::cpu::init_perf_counters();

        // 8. Enable FSGSBASE for fast thread-local storage access
        if features.has_fs_gs_base {
            let mut cr4: u64;
            core::arch::asm!("mov {}, cr4", out(reg) cr4);
            cr4 |= 1 << 13; // FSGSBASE
            core::arch::asm!("mov cr4, {}", in(reg) cr4);
        }
    }

    // Print full CPU info
    crate::arch::cpu::print_cpu_info(features);

    // Enable lazy FPU switching — CR0.TS will be set on context switch
    // First FPU/SSE/AVX instruction triggers #NM, handler clears TS
    crate::arch::fpu::enable_lazy_fpu();
    crate::drivers::serial::serial_write("[Ryzen-5600X] Lazy FPU switching enabled (CR0.TS)\n");

    crate::drivers::serial::serial_write("[Ryzen-5600X] CPU Ryzen 5 5600X configurado a maximo rendimiento de hardware.\n");
}
