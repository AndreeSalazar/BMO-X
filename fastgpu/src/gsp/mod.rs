use crate::loader::{GpuBootState};
use crate::pci::GA106_DEVICE;
use crate::abi::gsp_abi::GspBootParams;
use crate::segments::{GA106_VRAM_SIZE, GA106_WPR2_SIZE};
use core::ptr::{read_volatile, write_volatile};

// --- MMIO Offsets para el GSP Falcon (Ampere GA106) ---
pub const GSP_FALCON_BASE: u64      = 0x00110000;
pub const GSP_FALCON_CPUCTL: u64    = GSP_FALCON_BASE + 0x100;
pub const GSP_FALCON_BOOTVEC: u64   = GSP_FALCON_BASE + 0x104;
pub const GSP_RISCV_BR_ADDR: u64    = GSP_FALCON_BASE + 0x180; // Crítico: Offset 0x1180 real
pub const GSP_MAILBOX0: u64         = GSP_FALCON_BASE + 0x040;
pub const GSP_MAILBOX1: u64         = GSP_FALCON_BASE + 0x044;

pub const GSP_INIT_DONE: u32        = 0x00001001;

#[inline(always)]
unsafe fn mmio_read32(bar0: u64, offset: u64) -> u32 {
    read_volatile((bar0 + offset) as *const u32)
}

#[inline(always)]
unsafe fn mmio_write32(bar0: u64, offset: u64, value: u32) {
    write_volatile((bar0 + offset) as *mut u32, value)
}

/// Carga del firmware del GPU System Processor (GSP) para Ampere
pub unsafe fn load_gsp_firmware(gsp_fw_addr: u64, gsp_fw_size: u64) -> Result<(), &'static str> {
    let bar0 = GA106_DEVICE.pci_bar0;
    
    if GA106_DEVICE.state != GpuBootState::Sec2Ready {
        return Err("Prerequisite SEC2 not ready for GSP Load");
    }

    if gsp_fw_addr == 0 || gsp_fw_size == 0 {
        return Err("GSP firmware blob not provided or empty");
    }

    // 1. Configurar región WPR2 en VRAM
    let wpr2_offset = GA106_VRAM_SIZE - GA106_WPR2_SIZE;
    
    if GA106_DEVICE.vram_ptr == 0 {
        return Err("VRAM not mapped for direct firmware load");
    }

    // 2. Inyectar GspBootParams al inicio de WPR2
    let params_vaddr = (GA106_DEVICE.vram_ptr + wpr2_offset) as *mut GspBootParams;
    let entry_point_offset = 0x15C71B; // kernelServerEntry extraído por SigDead
    
    let params = GspBootParams::new(
        wpr2_offset, 
        GA106_WPR2_SIZE,
        wpr2_offset + params_vaddr.read_volatile().signature as u64 // Placeholder, recalculado abajo
    );
    
    // Configurar correctamente el entry point absoluto dentro de VRAM
    let mut params = GspBootParams::new(wpr2_offset, GA106_WPR2_SIZE, wpr2_offset + entry_point_offset);
    // Añadimos offsets de logs estándar (extraídos de nvlddmkm.sys)
    params.log_init_size = 0x40000;
    params.log_rm_size = 0x100000;
    
    params_vaddr.write_volatile(params);

    // 3. Copiar el firmware justo después de los parámetros
    let fw_vaddr = (GA106_DEVICE.vram_ptr + wpr2_offset + 0x1000) as *mut u8; // 4KB offset para params + padding
    let src = gsp_fw_addr as *const u8;
    for i in 0..gsp_fw_size as usize {
        fw_vaddr.add(i).write_volatile(*src.add(i));
    }

    // 4. Configurar registros de arranque del Falcon GSP
    // RISCV_BR_ADDR debe apuntar a la estructura de parámetros o al entry point directo
    // En Ampere, suele apuntar al manifest que luego carga el ELF.
    mmio_write32(bar0, GSP_RISCV_BR_ADDR, (wpr2_offset & 0xFFFFFFFF) as u32);
    
    // Pasar el puntero a los parámetros por Mailbox1
    mmio_write32(bar0, GSP_MAILBOX1, (wpr2_offset & 0xFFFFFFFF) as u32);
    
    // 5. Iniciar la CPU del GSP
    mmio_write32(bar0, GSP_FALCON_BOOTVEC, 0x0);
    let cpuctl = mmio_read32(bar0, GSP_FALCON_CPUCTL);
    mmio_write32(bar0, GSP_FALCON_CPUCTL, cpuctl | 0x2); // START bit

    // 6. Polling loop para esperar GSP_INIT_DONE (0x1001)
    let mut success = false;
    for _ in 0..5000 {
        let status = mmio_read32(bar0, GSP_MAILBOX0);
        if status == GSP_INIT_DONE {
            success = true;
            break;
        }
        for _ in 0..1000 { core::hint::spin_loop(); }
    }
    
    if !success {
        return Err("Timeout esperando GSP_INIT_DONE (Handshake Failed)");
    }
    
    // Avanzar el estado
    GA106_DEVICE.state = GpuBootState::GspReady;
    Ok(())
}
