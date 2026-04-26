//! GMMU (GPU Memory Management Unit) Base
//!
//! Fase 1: Estructuras base para administrar la memoria virtual de la GPU.
//! La RTX 3060 (Ampere) usa un sistema de paginación de 4 niveles similar a x86_64,
//! permitiendo que la tarjeta gráfica lea la memoria RAM del sistema (DMA) 
//! y maneje su propia VRAM.

use crate::console::Console;

// --- Constantes GMMU (NVIDIA Ampere) ---
pub const GMMU_PAGE_SIZE_4K: u64  = 4096;
pub const GMMU_PAGE_SIZE_64K: u64 = 65536;

// Bits de los Page Table Entries (PTE) de NVIDIA
pub const PTE_VALID: u64       = 1 << 0; // La página existe
pub const PTE_PRIVILEGE: u64   = 1 << 1; // Solo el kernel de la GPU puede acceder
pub const PTE_READ_ONLY: u64   = 1 << 2; // Memoria de solo lectura
pub const PTE_SYSTEM_RAM: u64  = 1 << 4; // 1 = RAM de la PC (SysRAM), 0 = VRAM de la GPU

/// Representa el Page Directory (PDE) base de la GPU (Similar al CR3 en la CPU).
/// En Ampere, este directorio apunta a otras tablas para formar direcciones virtuales de 47 bits.
#[repr(C, align(4096))]
pub struct GpuPageDirectory {
    pub entries: [u64; 512],
}

/// Representa una tabla de páginas (PTE) que mapea direcciones físicas.
#[repr(C, align(4096))]
pub struct GpuPageTable {
    pub entries: [u64; 512],
}

pub struct GmmuManager<'a> {
    bar0: &'a nv_hal::MmioRegion,
    // Punteros físicos y virtuales del directorio raíz
    pd_phys: u64,
    pd_virt: *mut GpuPageDirectory,
}

impl<'a> GmmuManager<'a> {
    /// Crea el administrador GMMU y asigna la memoria para el directorio de páginas (PD).
    pub fn new(bar0: &'a nv_hal::MmioRegion) -> Option<Self> {
        // Pedimos 1 página (4KB) contigua en la RAM del sistema para el Directorio Raíz.
        let phys_addr = unsafe { crate::arch::page_alloc::alloc_pages_contiguous(1)? };
        
        // Limpiamos la memoria con ceros
        let virt_ptr = phys_addr as *mut GpuPageDirectory;
        unsafe {
            core::ptr::write_bytes(virt_ptr as *mut u8, 0, 4096);
        }

        Some(Self {
            bar0,
            pd_phys: phys_addr,
            pd_virt: virt_ptr,
        })
    }

    /// Inicializa el subsistema de memoria gráfica.
    pub fn init(&mut self, con: &mut Console) {
        con.print_colored("=== Fase 1: GMMU Init (Ampere) ===\n", crate::fb::colors::ACCENT_CYAN);
        
        con.print("  [GMMU] Page Directory alojado en phys RAM: 0x");
        con.print_hex32((self.pd_phys >> 32) as u32);
        con.print_hex32(self.pd_phys as u32);
        con.newline();

        // TODO: En los próximos pasos, mapearemos el buffer RPC (SysRAM) 
        // y la memoria VRAM creando los PTEs y escribiendo el pd_phys en 
        // los registros de control de memoria (NV_PFIFO / NV_BIF).

        con.print_colored("=== GMMU Base Lista ===\n", crate::fb::colors::TEXT_SUCCESS);
    }

    /// (Futuro) Mapear una dirección física (SysRAM o VRAM) a una virtual en la GPU.
    pub fn map_page(&mut self, gpu_virt: u64, phys_addr: u64, is_sysram: bool) {
        // Aquí programaremos los PTEs para conectar la GPU con la RAM.
        // Se implementará al conectar con el RPC.
    }
}
