# FastOS Memory Manager Specification
**Capa:** Kernel (Ring 0)
**Prioridad:** CRÍTICA
**Depende de:** Ninguno (Módulo Fundacional de Ring 0)
**Inspiración:** Windows `Mm*`, Linux `mm/`, seL4 (Memory Capabilities).

---

## FASE 1: ADN Extraído (¿Qué hace Windows/Linux aquí?)
Tradicionalmente, sistemas operativos como Windows implementan gestores de memoria increíblemente complejos para lidiar con arquitecturas de 32 bits heredadas (PAE), paginación hacia discos duros mecánicos lentos (Pagefiles), y subsistemas como `Wow64`.
- **Estructuras Clave (Windows):** `PFN_DATABASE` (Page Frame Number), `MDL` (Memory Descriptor List para DMA), `VAD` (Virtual Address Descriptors).
- **El Problema:** La gestión de la memoria virtual en Windows está fuertemente atada al sistema de archivos NTFS para la paginación a disco y depende de múltiples *spinlocks* que causan cuellos de botella masivos en sistemas Multi-Core.
- **Lo que conservamos:** El concepto estricto de Paginación de 4 Niveles de x86-64 (PML4) y la separación de direcciones espaciales (Upper Half vs Lower Half).
- **Lo que tiramos:** Todo el sistema de Paginación a disco (Pagefile). FastOS asume entornos donde la memoria física y la VRAM dictan la ejecución. Si te quedas sin memoria RAM, se aplica *OOM Killer* (Out-Of-Memory Killer), no se hacen escrituras bloqueantes a disco.

---

## FASE 2: Diseño BMO Nativo

Diseñamos un gestor de memoria puro para x86-64 escrito nativamente en Rust, aprovechando su modelo de *Ownership* para prevenir *Use-After-Free* o filtraciones (*memory leaks*).

### 1. El Allocator Físico (Buddy System)
Gestión de marcos físicos (Physical Frames) de 4KB.
```rust
/// Un Marco Físico de Memoria RAM (4096 bytes exactos en x86-64)
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct PhysFrame {
    pub number: u64, // PFN (Page Frame Number) = Physical_Address / 4096
}

/// Allocator basado en Buddy System para prevenir fragmentación
pub struct BuddyAllocator {
    // Array de listas enlazadas para bloques de tamaños de potencia de 2 (Order 0 = 4KB, Order 9 = 2MB)
    pub free_lists: [Option<PhysFrame>; MAX_ORDER], 
}
```

### 2. Paginación de 4 Niveles (Page Tables x86-64)
El hardware x86-64 exige que el registro `CR3` apunte al `PML4`.
```rust
use bitflags::bitflags;

bitflags! {
    /// Flags nativos de Paginación en Intel/AMD x86-64
    pub struct PageTableFlags: u64 {
        const PRESENT =         1 << 0;
        const WRITABLE =        1 << 1;
        const USER_ACCESSIBLE = 1 << 2; // Crucial: Si 0, Ring 3 colapsa intentando leer
        const WRITE_THROUGH =   1 << 3;
        const CACHE_DISABLE =   1 << 4; // Obligatorio activarlo para la VRAM de la RTX 3060
        const ACCESSED =        1 << 5;
        const DIRTY =           1 << 6;
        const HUGE_PAGE =       1 << 7; // Usado en PDE para páginas de 2MB
        const NO_EXECUTE =      1 << 63; // Seguridad: Prevención de ejecución de datos
    }
}

/// Una entrada genérica en cualquier nivel de la tabla (PML4E, PDPE, PDE, PTE)
#[repr(transparent)]
pub struct PageTableEntry(u64);

/// La Tabla de Páginas (Alineada exactamente a 4096 bytes)
#[repr(C, align(4096))]
pub struct PageTable {
    pub entries: [PageTableEntry; 512],
}
```

### 3. Separación de Memoria (ASLR y Half-Spaces)
- **Kernel Space (Upper Half):** `0xFFFF_8000_0000_0000` hasta `0xFFFF_FFFF_FFFF_FFFF`. Mapeo idéntico físico (Physical Identity Map) constante en todos los procesos.
- **User Space (Lower Half):** `0x0000_0000_0000_0000` hasta `0x0000_7FFF_FFFF_FFFF`. Donde el `BEF Loader` inyectará el ejecutable de forma aleatoria (ASLR).

---

## FASE 3: Implementación (Pseudocódigo Rust)

### TLB Flush e Invalidación de Cache
Cuando el Kernel altera una tabla de páginas, la CPU sigue teniendo el camino viejo en caché (TLB).
```rust
/// Usando Inline Assembly seguro en Rust para el hardware
pub unsafe fn flush_tlb(virtual_address: u64) {
    core::arch::asm!("invlpg [{}]", in(reg) virtual_address);
}

/// Invalida absolutamente toda la TLB (Peligroso para rendimiento, usado en Context Switches pesados)
pub unsafe fn reload_cr3(pml4_phys_address: u64) {
    core::arch::asm!("mov cr3, {}", in(reg) pml4_phys_address);
}
```

### La Implementación de la Syscall Crítica: `sys_mmap`
Esta es la función interna del Kernel que responde a la Syscall `0x10` especificada en `FastOS_Syscall_Table_Spec.md`.
```rust
/// Retorna la dirección virtual mapeada al proceso
pub fn sys_mmap_internal(
    process: &mut BmoProcess, 
    requested_size: usize, 
    mem_type: u64
) -> Result<u64, BmoError> {
    
    let mut flags = PageTableFlags::PRESENT | PageTableFlags::USER_ACCESSIBLE | PageTableFlags::WRITABLE;
    
    // El "ADN Físico" de FastOS: Si el usuario pidió VRAM para el NVIDIA GSP
    if mem_type == 0x1 { // MemType::GpuVRAM
        // Activamos Uncacheable/Write-Combined. Si no lo hacemos, las lecturas PCIe asíncronas fallan
        flags |= PageTableFlags::CACHE_DISABLE; 
    } else {
        // Ejecución solo permitida en RAM normal (No queremos ejecutar código desde la GPU por seguridad)
        flags |= PageTableFlags::NO_EXECUTE; 
    }

    // 1. Encontrar un hueco virtual vacío en el Lower Half del proceso (ASLR)
    let v_addr = process.virtual_allocator.find_free_region(requested_size)?;
    
    // 2. Pedir frames físicos al Buddy System
    // Si la request es >= 2MB, pedimos Huge Pages para mejorar drásticamente el rendimiento del DMA
    let is_huge = requested_size >= 2_097_152;
    if is_huge { flags |= PageTableFlags::HUGE_PAGE; }
    
    let phys_frames = kernel::BUDDY_ALLOCATOR.lock().allocate(requested_size)?;
    
    // 3. Escribir los registros en la Page Table activa (CR3 del proceso)
    map_pages(process.pml4_table, v_addr, phys_frames, flags)?;
    
    Ok(v_addr.as_u64())
}
```

---

## FASE 4: Integración con el Stack FastOS

El Memory Manager es el pegamento de todo el Ring 0. 

- **Conexión con `FastOS_Syscall_Table_Spec.md`:** 
  Este documento implementa la lógica pesada detrás de la syscall `0x10` (`sys_mmap`).
- **Conexión con `BEF_Executable_Format_Spec.md`:** 
  Cuando el Kernel necesita lanzar una nueva aplicación, invoca la función `map_sections()` del BEF Loader (ver Fase 3 del BEF Spec). Esta función llamará al Virtual Allocator del Memory Manager para separar en RAM la sección `CODE` (con flag `PRESENT` + `USER_ACCESSIBLE` y SIN `NO_EXECUTE`) de la sección `DATA` (donde sí hay `NO_EXECUTE`).
- **Conexión con `BmoProcessEnv`:**
  Este módulo decide el `image_base`, el `stack_base` y el `stack_limit` inyectados en la estructura `BmoProcessEnv` para que la aplicación BMO sepa dónde vive en el espacio Lower Half.

---

## Conclusión

**Qué aprendimos y mejoramos vs Windows:**
Al arrancar la basura heredada de la paginación a disco y dependencias como PAE/Wow64, el gestor de memoria de BMO es extremadamente rápido y rígido. Hemos instaurado como prioridad fundamental en Ring 0 la separación nativa a nivel de TLB entre "RAM Normal" y "VRAM", lo que permitirá un envío de comandos GSP (`MSG_INIT` a NVIDIA) a máxima velocidad vía memoria `Uncacheable`. 
Todo esto se apoya en el hardware de Intel/AMD (4-level Paging) mapeado nativamente mediante constructos seguros de Rust.
