# FastOS Native FS Format Specification (BMOFS)
**Capa:** Storage
**Prioridad:** MEDIA
**Depende de:** `FastOS_VFS_Spec.md`, `FastOS_Memory_Manager_Spec.md`
**Inspiración:** Estructura básica de `ext4`, Copy-on-Write (CoW) de `APFS`.

---

## FASE 1: ADN Extraído (¿Qué hace Windows/Linux aquí?)
El sistema FAT es propenso a corrupción, NTFS es extremadamente complejo y pesado con su `$MFT` masivo y atributos, y ext4 hereda conceptos rotacionales (como colocar los inodes cerca de los datos para que la aguja física del disco mecánico no salte demasiado).
- **Qué conservamos:** La estabilidad del bloque de 4KB (para alinear perfectamente con el Memory Manager y el VFS) y la robustez del `Superblock` + `Inode Table`.
- **Qué tiramos:** El Journaling en la Versión 1 (es redundante en discos SSD súper seguros, se delegará a V2 con CoW). Cilindros, cabezales rotacionales y tiempos de búsqueda (seek penalties). Asumimos latencia casi 0 (NVMe).

---

## FASE 2: Diseño BMO Nativo

BMOFS (Bare Metal Orchestrator File System) está optimizado puramente para unidades Flash NAND. La alineación y el acceso se basan exclusivamente en LBA (Logical Block Addressing) y bloques estáticos de 4KB.

### Estructuras Físicas en Disco (Rust)

```rust
// bmofs/format.rs

pub const BMOFS_MAGIC: u32 = 0x424D4F00; // "BMO\0"
pub const BLOCK_SIZE: u32 = 4096;

/// El Superbloque (Reside en el Bloque 0 del disco o partición)
#[repr(C, packed)]
pub struct BmoSuperblock {
    pub magic: u32,
    pub version: u32,             // 1 para v1 (Sin Journaling)
    pub total_blocks: u64,
    pub block_size: u32,          // Siempre 4096
    
    // El mapa de bits (Bitmap) donde cada bit dice si el bloque 4KB está libre o en uso
    pub block_bitmap_lba: u64,    
    
    // La ubicación de la tabla de Inodes
    pub inode_table_lba: u64,
    pub inode_count: u32,
    
    // Inode de la carpeta raíz '/'
    pub root_directory_inode: u32, 
}

/// Representa un Archivo o Directorio en el disco
#[repr(C, packed)]
pub struct BmoInode {
    pub flags: u32,           // 0x1 = File, 0x2 = Directory
    pub size_bytes: u64,
    
    // Punteros directos a los bloques de datos físicos en el disco NVMe
    pub direct_blocks: [u64; 12], 
    
    // Si el archivo pesa más de 48KB, usamos indirección (puntero a un bloque lleno de punteros)
    pub indirect_block: u64,
}

/// Una entrada dentro de un directorio
#[repr(C, packed)]
pub struct BmoDirectoryEntry {
    pub inode_id: u32,
    pub name_len: u16,
    pub name: [u8; 122], // Hasta 122 chars. Total struct size = 128 bytes.
}
```

---

## FASE 3: Implementación (El Flujo del FsDriver)

Para que el VFS pueda montar esto, BMOFS implementa el `FsDriver` (Ver DOC-06).

```rust
pub struct BmoFs {
    // La interfaz hacia el hardware físico (DOC-05 NVMe/AHCI)
    pub disk: Arc<dyn BlockDevice>, 
    pub superblock: BmoSuperblock,
}

impl FsDriver for BmoFs {
    /// Resuelve el nombre del archivo leyendo los bloques de la carpeta
    fn lookup(&self, parent_inode_id: u64, name: &str) -> Result<u64, VfsError> {
        let parent_inode = self.read_inode_from_disk(parent_inode_id)?;
        
        // Lee los datos crudos del directorio usando BlockDevice
        let mut buffer = vec![0u8; parent_inode.size_bytes as usize];
        self.read_inode_data(&parent_inode, &mut buffer)?;
        
        // Parsea los BmoDirectoryEntry
        for chunk in buffer.chunks_exact(128) {
            let entry: &BmoDirectoryEntry = unsafe { &*(chunk.as_ptr() as *const _) };
            
            let entry_name = core::str::from_utf8(&entry.name[..entry.name_len as usize])
                .unwrap_or("");
                
            if entry_name == name {
                return Ok(entry.inode_id as u64);
            }
        }
        
        Err(VfsError::NotFound)
    }

    /// Implementación de lectura usando los punteros directos del Inode
    fn read_file(&self, node_id: u64, _offset: u64, buf: &mut [u8]) -> Result<usize, VfsError> {
        let inode = self.read_inode_from_disk(node_id)?;
        
        // 1. Calcular qué bloques 4KB necesita leer el usuario
        // 2. Ejecutar self.disk.read_blocks(inode.direct_blocks[0], ...)
        // 3. (Magia asíncrona de DOC-05 NVMe entra aquí)
        // 4. Devolver datos
        
        Ok(buf.len())
    }
    // ...
}
```

---

## FASE 4: Integración con el Stack FastOS

La cadena obligatoria se consolida aquí de manera brillante:
1. **El Hardware (DOC-05 NVMe):** Implementa las rutinas DMA para leer a velocidades GigaByte/s usando MSI-X. Todo empacado en `trait BlockDevice`.
2. **El Formato (DOC-07 BMOFS - *Este documento*):** Entiende el `BlockDevice`, y sabe que en el Bloque 0 está el Superbloque. Traduce "Nombres y Archivos" a "Punteros LBA en el SSD". Implementa `trait FsDriver`.
3. **El VFS (DOC-06):** Envuelve el `FsDriver` y proporciona los handles a los procesos.
4. **Syscalls (Syscall Table Spec):** El desarrollador escribe en su programa `sys_read()`, conectando Ring 3 con todo el flujo hacia el SSD de Ring 0.
5. **Memory Manager (DOC-01):** El bloque de disco es de `4096` bytes. El bloque de la RAM paginada (`PageTable`) es de `4096` bytes. Esto garantiza que las transferencias DMA del NVMe no sufran colisiones en los límites de las páginas (*Page Faults* de DMA).

---

## Conclusión

**Qué aprendimos y mejoramos vs Windows:**
NTFS es el equivalente a usar un castillo gótico para guardar un coche. BMOFS es la expresión máxima del minimalismo: una tabla de *inodes* y apuntadores directos de 4KB. Al diseñarlo exclusivamente para SSDs (donde buscar el bloque 1,000,000 toma exactamente el mismo tiempo que buscar el bloque 1, *zero seek penalty*), se elimina por completo la necesidad de empaquetar bloques de metadatos cercanos a los datos. El resultado es un código brutalmente corto, ultra-veloz, y nativo de Rust que explota al máximo las velocidades PCIe.
