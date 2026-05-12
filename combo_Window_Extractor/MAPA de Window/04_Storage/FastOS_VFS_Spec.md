# FastOS Virtual File System (VFS) Specification
**Capa:** Storage
**Prioridad:** ALTA
**Depende de:** `FastOS_NVMe_Driver_Spec.md`, `FastOS_Syscall_Table_Spec.md`
**Inspiración:** Linux VFS (Virtual File System), Windows `IoManager`.

---

## FASE 1: ADN Extraído (¿Qué hace Windows/Linux aquí?)
Windows utiliza el I/O Manager para enviar IRPs (I/O Request Packets) enormes que atraviesan decenas de capas de drivers apilados. Linux usa el concepto de *Inodes*, `dentry` (Directory Entry) y descriptores de archivo (`fd`).
- **Qué conservamos:** La abstracción mágica donde un programa solo pide leer `/system/bin/app.bef` y no tiene que saber si ese archivo vive en un disco NVMe PCIe o en un SSD SATA.
- **Qué tiramos:** Los links simbólicos (Symlinks) en la v1 para mantener el código microscópico. Los *File Descriptors* enteros de POSIX (usamos Handles tipados `BmoHandle`).

---

## FASE 2: Diseño BMO Nativo

El VFS en FastOS actúa como un *Switchboard* (Centralita). Recibe peticiones limpias a través de las Syscalls y se las rutea al driver del sistema de archivos específico (BMOFS), quien a su vez habla con el disco físico subyacente (`BlockDevice`).

### 1. El VFS Node
Cada archivo, directorio o dispositivo abierto en FastOS es un `VfsNode`.
```rust
// bmo_vfs/node.rs
use alloc::sync::Arc;
use crate::storage::FsDriver;

pub enum NodeType {
    File,
    Directory,
    BlockDevice,
}

pub struct VfsNode {
    pub name: String,
    pub node_type: NodeType,
    pub size: u64,
    // Puntero de abstracción dinámica hacia el FileSystem real (ej. BMOFS)
    pub fs_driver: Arc<dyn FsDriver>, 
    pub fs_internal_id: u64, // El Inode ID del BMOFS
}
```

### 2. El Trait `FsDriver`
Un sistema de archivos como BMOFS o FAT32 (si se agregara para boot) debe implementar este Trait.
```rust
// bmo_vfs/driver.rs
use crate::storage::block::BlockDevice;

/// Abstracción de un Sistema de Archivos montado sobre un BlockDevice
pub trait FsDriver: Send + Sync {
    /// Resuelve un nombre de archivo dentro de un directorio y devuelve su ID interno
    fn lookup(&self, parent_id: u64, name: &str) -> Result<u64, VfsError>;
    
    /// Lee datos de un archivo montado
    fn read_file(&self, node_id: u64, offset: u64, buf: &mut [u8]) -> Result<usize, VfsError>;
    
    /// Escribe datos a un archivo
    fn write_file(&self, node_id: u64, offset: u64, buf: &[u8]) -> Result<usize, VfsError>;
}
```

---

## FASE 3: Implementación (Resolución y Lectura)

Cuando el usuario llama a `sys_open("/app/game.bef")`, el VFS debe fragmentar el string y buscarlo nodo por nodo de forma jerárquica.

```rust
// bmo_vfs/resolver.rs

/// Resolución de ruta estricta (Sin Symlinks en v1 para evitar Path Traversal)
pub fn resolve_path(path: &str) -> Result<Arc<VfsNode>, VfsError> {
    if !path.starts_with('/') { return Err(VfsError::MustBeAbsolute); }
    
    let parts = path.trim_start_matches('/').split('/');
    let mut current_node = get_root_vfs_node(); // Ej. BMOFS root
    
    for part in parts {
        if part.is_empty() { continue; }
        
        let driver = current_node.fs_driver.clone();
        
        // Pide al driver BMOFS que busque la carpeta 'part' dentro de 'current_node'
        let next_internal_id = driver.lookup(current_node.fs_internal_id, part)?;
        
        // Construimos el nuevo VFS Node
        current_node = Arc::new(VfsNode {
            name: part.to_string(),
            node_type: NodeType::File, // Asumido para el ejemplo
            size: 0, 
            fs_driver: driver,
            fs_internal_id: next_internal_id,
        });
    }
    
    Ok(current_node)
}
```

---

## FASE 4: Integración con el Stack FastOS

El VFS es el mediador supremo:

1. **Abajo (BlockDevice):** El Driver BMOFS (Siguiente documento) toma el trait `BlockDevice` proveído por `FastOS_NVMe_Driver_Spec.md` y lo usa para leer sus tablas de partición. BMOFS se registra a sí mismo en el VFS como implementador de `FsDriver`.
2. **Arriba (Syscalls):** Conecta con `FastOS_Syscall_Table_Spec.md`.
   - Cuando el usuario llama a `sys_open`, FastOS ejecuta `resolve_path`, crea un objeto `FileHandle` (que envuelve el `VfsNode`) y lo mete en la `HandleTable` del proceso devolviendo el `BmoHandle` opaco.
   - Cuando el usuario llama a `sys_read(handle, buf, len)`, el VFS hace `node.fs_driver.read_file(...)`.

---

## Conclusión

**Qué aprendimos y mejoramos:**
Mantuvimos la pureza de la abstracción UNIX de que "todo es un archivo", pero eliminamos la burocracia. Al abstraer todo a través de los Traits puros de Rust `BlockDevice` (físico) y `FsDriver` (Lógico), aseguramos que el sistema operativo principal (VFS y Syscalls) nunca contenga código espagueti de drivers físicos, permitiendo la compatibilidad cruzada NVMe/AHCI y cualquier futuro sistema de archivos sin modificar el Kernel base.
