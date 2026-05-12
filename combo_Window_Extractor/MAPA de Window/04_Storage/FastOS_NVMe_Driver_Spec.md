# FastOS NVMe & AHCI Drivers Specification
**Capa:** Storage
**Prioridad:** ALTA
**Depende de:** `El_Cerebro_Hardware_PCIe_APIC.md`, `FastOS_Memory_Manager_Spec.md`, `FastOS_Scheduler_Spec.md`
**Inspiración:** Linux NVMe/AHCI Drivers, Windows StorPort.

---

## FASE 1: ADN Extraído (¿Qué hace Windows/Linux aquí?)
Sistemas antiguos cargan la pila de almacenamiento asumiendo que el disco puede ser rotacional (HDD), implementando algoritmos de *Elevator* (SCAN, C-SCAN) para minimizar el movimiento del cabezal físico de lectura, añadiendo enorme sobrecarga a discos SSD modernos.
- **Qué conservamos:** El modelo de descubrimiento PCI/PCIe y el uso exhaustivo de acceso a memoria directa (DMA).
- **Qué tiramos:** IDE, PATA, discos mecánicos (HDD), colas de I/O complejas para reordenamiento, *polling* bloqueante. FastOS asume **100% Estado Sólido** (SSD).

---

## FASE 2: Diseño BMO Nativo

Diseñamos una abstracción limpia `BlockDevice`. El VFS (Virtual File System) no sabrá si está hablando con un disco NVMe de última generación por PCIe o un SSD SATA por AHCI. Todo ocurre de forma asíncrona apoyándose en el Scheduler.

### La Abstracción Principal (Rust Trait)

```rust
// bmo_storage/block.rs

pub enum BlockError {
    DeviceOffline,
    DmaError,
    Timeout,
    InvalidLba,
}

/// Abstracción Universal para discos de estado sólido en BMO
pub trait BlockDevice: Send + Sync {
    /// Lee asíncronamente bloques del disco a la memoria
    fn read_blocks(&self, lba: u64, count: u32, buf: &mut [u8]) -> Result<(), BlockError>;
    
    /// Escribe asíncronamente bloques de memoria al disco
    fn write_blocks(&self, lba: u64, count: u32, buf: &[u8]) -> Result<(), BlockError>;
    
    fn block_size(&self) -> u32; // Típicamente 512 o 4096
    fn total_blocks(&self) -> u64;
}
```

### El Driver NVMe (PCIe M.2 / U.2)
Descubierto vía ECAM/MCFG buscando `Class Code 0x01` (Mass Storage) y `Subclass 0x08` (Non-Volatile Memory).

```rust
pub struct NvmeDriver {
    bar0_mmio: u64, // Puntero a registros de hardware mapeados en memoria
    admin_queue: NvmeQueuePair,
    io_queues: Vec<NvmeQueuePair>, // Una por cada CPU Core del Ryzen 5600X
}

impl BlockDevice for NvmeDriver {
    fn read_blocks(&self, lba: u64, count: u32, buf: &mut [u8]) -> Result<(), BlockError> {
        let core_id = get_current_core_id();
        let queue = &self.io_queues[core_id as usize];
        
        // 1. Obtener la memoria física (DMA) usando DOC-01
        let phys_addr = virtual_to_physical(buf.as_ptr() as u64);
        
        // 2. Insertar comando NVMe Read en la Submission Queue
        let cmd_id = queue.submit_read(lba, count, phys_addr);
        
        // 3. Timbrar el Doorbell MMIO para despertar al controlador del disco
        queue.ring_doorbell();
        
        // 4. DORMIR EL HILO. No hacemos polling. El hardware enviará un MSI-X.
        sleep_until_irq(IrqType::Nvme(cmd_id));
        
        Ok(())
    }
    // write_blocks es simétrico...
}
```

### El Driver AHCI (SATA SSD)
Descubierto vía PCI `Class Code 0x01` y `Subclass 0x06` (SATA).
```rust
pub struct AhciDriver {
    abar: u64, // BAR5 Memory Registers
    ports: Vec<AhciPort>,
}

impl BlockDevice for AhciDriver {
    fn read_blocks(&self, lba: u64, count: u32, buf: &mut [u8]) -> Result<(), BlockError> {
        let port = &self.ports[0]; // Simplificado al puerto activo
        let phys_addr = virtual_to_physical(buf.as_ptr() as u64);
        
        // Configurar la Physical Region Descriptor Table (PRDT) para el DMA
        port.build_prdt(phys_addr, count * self.block_size());
        
        // Configurar el Frame Information Structure (FIS) de comando SATA
        port.build_fis(lba, count, SataCommand::ReadFpdmaQueued);
        
        // Emitir el comando
        port.issue_command();
        
        // Dormir hasta que la IRQ AHCI (Legacy INTx o MSI) despierte el hilo
        sleep_until_irq(IrqType::Ahci(port.id));
        
        Ok(())
    }
}
```

---

## FASE 3: Implementación (El Flujo de IRQ)

Dado que **no hacemos polling**, este es el flujo del Scheduler cuando el disco responde:

1. El Hilo envía la petición DMA y llama a `sleep_until_irq()`. (El Scheduler lo pone en `ThreadState::Blocked`).
2. El disco SSD completa la lectura a la RAM directamente (DMA).
3. El SSD dispara una interrupción MSI-X al CPU.
4. El Kernel atrapa la IRQ, lee la *Completion Queue* del NVMe.
5. El Kernel marca el Hilo bloqueado como `ThreadState::Ready` y lo mete en la `RunQueue` (DOC-03).
6. El Hilo se despierta y su buffer `&mut [u8]` ya tiene los datos mágicamente.

---

## FASE 4: Integración con el Stack FastOS

- **Conexión con `FastOS_Memory_Manager_Spec.md`:** Los buffers pasados a `read_blocks` deben ser fijados en memoria (*pinned*) o convertidos a direcciones físicas para que el DMA del hardware no los corrompa ni intente acceder a memoria paginada a disco (que FastOS no tiene).
- **Conexión con `FastOS_Scheduler_Spec.md`:** La dependencia vital para que `sleep_until_irq` funcione y no congelemos el PC.
- **Conexión Futura (`FastOS_VFS_Spec.md`):** Ambos drivers exponen el trait `BlockDevice`, sobre el cual el sistema de archivos montará sus particiones lógicas.

---

## Conclusión
Al rechazar explícitamente el hardware rotacional (HDD), eliminamos miles de líneas de código relacionadas con *I/O schedulers*. Nuestro modelo es puro DMA Asíncrono manejado por interrupciones MSI-X. Esto exprime el ancho de banda del PCIe Gen4 para los M.2 NVMe y maximiza los ciclos del CPU Ryzen.
