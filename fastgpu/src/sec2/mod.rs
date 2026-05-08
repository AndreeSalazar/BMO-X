

use core::ptr::{read_volatile, write_volatile};
use crate::loader::{GpuBootState};
use crate::pci::GA106_DEVICE;
use crate::segments::{GA106_VRAM_SIZE, GA106_WPR2_SIZE};

// --- MMIO Offsets para Ampere (GA106) ---
// Base del Falcon SEC2
pub const SEC2_FALCON_BASE: u64 = 0x00084000;

// Registros de control del Falcon SEC2
pub const SEC2_FALCON_ENGCTL: u64     = SEC2_FALCON_BASE + 0x000;
pub const SEC2_FALCON_CPUCTL: u64     = SEC2_FALCON_BASE + 0x100;
pub const SEC2_FALCON_BOOTVEC: u64    = SEC2_FALCON_BASE + 0x104;
pub const SEC2_FALCON_DMACTL: u64     = SEC2_FALCON_BASE + 0x10C;
pub const SEC2_FALCON_IRQMSET: u64    = SEC2_FALCON_BASE + 0x010;
pub const SEC2_FALCON_IRQSTAT: u64    = SEC2_FALCON_BASE + 0x008;

// Mailboxes para la comunicación del Firmware (Handshake)
pub const SEC2_MAILBOX0: u64          = SEC2_FALCON_BASE + 0x040;
pub const SEC2_MAILBOX1: u64          = SEC2_FALCON_BASE + 0x044;

// Registro que confirma si el procesador interno entró en RISC-V Mode
pub const SEC2_RISC_MODE_REG: u64     = SEC2_FALCON_BASE + 0x0C0;
pub const RISC_MODE_ACTIVE_VAL: u32   = 0x00000011;

// Offsets del Memory Controller (FB/MC) para WPR2 (Workspace Protected Region 2)
// Configurar esto es crítico ANTES de arrancar SEC2 para evitar el lockdown 0xBADF5620
pub const FB_WPR2_BASE_LO: u64        = 0x00100CD4;
pub const FB_WPR2_BASE_HI: u64        = 0x00100CD8;
pub const FB_WPR2_SIZE: u64           = 0x00100CDC;
pub const FB_WPR2_CTRL: u64           = 0x00100CE0;

#[inline(always)]
unsafe fn mmio_read32(bar0: u64, offset: u64) -> u32 {
    read_volatile((bar0 + offset) as *const u32)
}

#[inline(always)]
unsafe fn mmio_write32(bar0: u64, offset: u64, value: u32) {
    write_volatile((bar0 + offset) as *mut u32, value)
}

/// Bucle de espera pasiva (Polling)
fn delay_loop(cycles: usize) {
    for _ in 0..cycles {
        core::hint::spin_loop();
    }
}

/// Función principal de Bootstrap del SEC2
pub unsafe fn bootstrap_sec2_falcon() -> bool {
    let bar0 = GA106_DEVICE.pci_bar0;
    
    if bar0 == 0 || GA106_DEVICE.state != GpuBootState::BarsMapped {
        // Fallo: Los BARs no están mapeados aún
        return false;
    }

    // =========================================================================
    // PASO 1: WPR2 Region Setup (El Firewall Antilockdown)
    // El fallo 0xBADF5620 ocurre porque el SEC2 intenta validar sus firmas en 
    // la memoria protegida. Si WPR2 no está habilitado, hace lockdown.
    // =========================================================================
    
    // WPR2 se ubica justo al final de la VRAM (12GB)
    let wpr2_physical_base = GA106_VRAM_SIZE;
    
    mmio_write32(bar0, FB_WPR2_BASE_LO, (wpr2_physical_base & 0xFFFFFFFF) as u32);
    mmio_write32(bar0, FB_WPR2_BASE_HI, (wpr2_physical_base >> 32) as u32);
    mmio_write32(bar0, FB_WPR2_SIZE, (GA106_WPR2_SIZE >> 12) as u32); // Usualmente en páginas de 4K
    
    // Habilitar la región WPR2 (Bit 0)
    let wpr2_ctrl = mmio_read32(bar0, FB_WPR2_CTRL);
    mmio_write32(bar0, FB_WPR2_CTRL, wpr2_ctrl | 0x1);

    // =========================================================================
    // PASO 2: EngCtl y DmaCtl (Preparar el Motor DMA)
    // =========================================================================
    
    // Limpiar interrupciones pendientes
    mmio_write32(bar0, SEC2_FALCON_IRQMSET, 0xFFFFFFFF);
    
    // Despertar el Engine de DMA del SEC2
    let engctl = mmio_read32(bar0, SEC2_FALCON_ENGCTL);
    mmio_write32(bar0, SEC2_FALCON_ENGCTL, engctl | 0x1); // ENABLE bit
    
    let dmactl = mmio_read32(bar0, SEC2_FALCON_DMACTL);
    mmio_write32(bar0, SEC2_FALCON_DMACTL, dmactl | 0x2); // REQUIRE_CTX bit

    // Actualizar estado a FalconReady
    GA106_DEVICE.state = GpuBootState::FalconReady;

    // =========================================================================
    // PASO 3: Arrancar el Falcon en modo RISC-V
    // =========================================================================
    
    // Escribir el Vector de Boot del Microcódigo
    mmio_write32(bar0, SEC2_FALCON_BOOTVEC, 0x00000000); 
    
    // Sacar a la CPU del reset (CPUCTL bit 1 = START)
    let cpuctl = mmio_read32(bar0, SEC2_FALCON_CPUCTL);
    mmio_write32(bar0, SEC2_FALCON_CPUCTL, cpuctl | 0x2);

    // =========================================================================
    // PASO 4: Polling Loop - Verificar activación de RISC-V
    // =========================================================================
    
    let mut riscv_active = false;
    for _ in 0..1000 {
        let mode = mmio_read32(bar0, SEC2_RISC_MODE_REG);
        if mode == RISC_MODE_ACTIVE_VAL {
            riscv_active = true;
            break;
        }
        delay_loop(100);
    }

    if !riscv_active {
        // Fallo crítico: El procesador no entró en modo RISC-V, posible lockdown.
        return false;
    }

    // =========================================================================
    // PASO 5: Mailbox Handshake
    // =========================================================================
    
    // Notificamos al SEC2 que estamos listos para recibir su payload en WPR2
    mmio_write32(bar0, SEC2_MAILBOX0, 0x00001337); // Magic handshake initiat
    mmio_write32(bar0, SEC2_MAILBOX1, 0x00000001); // SEC2_CMD_INIT

    // Esperar acuse de recibo del SEC2 (Mailbox0 cambia a OK = 0x0)
    for _ in 0..1000 {
        let mbox0 = mmio_read32(bar0, SEC2_MAILBOX0);
        if mbox0 == 0x0 {
            // SEC2 Aceptó los comandos y el WPR2 está validado
            GA106_DEVICE.state = GpuBootState::Sec2Ready;
            return true;
        }
        delay_loop(100);
    }

    false // Timeout en el Handshake
}
