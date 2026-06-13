#![no_std]

//! BMO-FS (Bare Metal Orchestrator Filesystem) Core Library
//!
//! A custom, modular, and metadata-robust filesystem designed for the BMO ecosystem.
//! Fully `no_std` compatible, designed to be easily integrated into both user-space
//! utilities and the Ring 0 kernel.

pub const BLOCK_SIZE: usize = 4096;
pub const INODE_SIZE: usize = 128;
pub const INODES_PER_BLOCK: usize = BLOCK_SIZE / INODE_SIZE;

pub const BMOFS_MAGIC: &[u8; 8] = b"BMO-FS\x00\x01";

pub const TYPE_FREE: u8 = 0;
pub const TYPE_FILE: u8 = 1;
pub const TYPE_DIR:  u8 = 2;

/// Superblock: sitúa la metadata del volumen BMO-FS.
/// Ocupa la primera sección del bloque 0.
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct Superblock {
    pub magic: [u8; 8],
    pub block_size: u32,
    pub total_blocks: u64,
    pub inode_bitmap_block: u64, // Bloque del bitmap de inodes
    pub data_bitmap_block: u64,  // Bloque del bitmap de bloques de datos
    pub inode_table_block: u64,  // Inicio de la tabla de inodes
    pub inode_count: u32,        // Cantidad de inodes totales
    pub root_inode: u32,         // Inode del directorio raíz (usualmente 2)
    pub checksum: u64,
}

impl Superblock {
    pub fn is_valid(&self) -> bool {
        self.magic == *BMOFS_MAGIC && self.block_size == BLOCK_SIZE as u32
    }
}

/// Inode: metadatos de un archivo o directorio.
/// Tamaño fijo de 128 bytes para facilitar alineación.
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct Inode {
    pub size: u64,                 // Tamaño del archivo en bytes
    pub file_type: u8,             // Tipo de archivo: 1=Regular, 2=Directorio, 0=Libre
    pub reserved1: u8,
    pub reserved2: u16,
    pub capabilities: u32,         // Flag para el sandbox de BMO
    pub direct_blocks: [u64; 12],  // Direcciones de bloques directos (hasta 48 KB)
    pub indirect_block: u64,       // Dirección de bloque indirecto (apunta a 512 bloques -> 2 MB)
    pub mtime: u64,                // Tiempo de modificación
    pub ctime: u64,                // Tiempo de creación
    pub pad: [u8; 8],              // Relleno a 128 bytes
}

impl Inode {
    pub const fn new_empty() -> Self {
        Self {
            size: 0,
            file_type: TYPE_FREE,
            reserved1: 0,
            reserved2: 0,
            capabilities: 0,
            direct_blocks: [0; 12],
            indirect_block: 0,
            mtime: 0,
            ctime: 0,
            pad: [0; 8],
        }
    }
}

/// Directory Entry Header.
/// En un bloque de directorio, los registros están alineados.
/// Cada registro consiste en este header seguido inmediatamente por el nombre en UTF-8.
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct DirEntryHeader {
    pub inode: u32,
    pub rec_len: u16,   // Longitud total del registro (header + nombre + alineación)
    pub name_len: u8,   // Longitud real del nombre
    pub file_type: u8,  // Tipo (1=Archivo, 2=Directorio)
}

/// Interfaz para que el filesystem lea/escriba bloques genéricos de 4096 bytes.
/// Facilita probar la lógica en archivos de imagen bajo Windows, o controladores NVMe en Ring 0.
pub trait BlockDevice {
    type Error;
    fn read_block(&mut self, block_idx: u64, buf: &mut [u8; BLOCK_SIZE]) -> Result<(), Self::Error>;
    fn write_block(&mut self, block_idx: u64, buf: &[u8; BLOCK_SIZE]) -> Result<(), Self::Error>;
}

// ── Operaciones del Filesystem ──────────────────────────────────────

pub fn format_volume(
    dev: &mut impl BlockDevice,
    total_blocks: u64,
    inode_count: u32,
) -> Result<(), &'static str> {
    if total_blocks < 64 {
        return Err("Volumen demasiado pequeño; se necesitan al menos 64 bloques");
    }

    let inode_blocks = (inode_count as usize + INODES_PER_BLOCK - 1) / INODES_PER_BLOCK;

    let sb = Superblock {
        magic: *BMOFS_MAGIC,
        block_size: BLOCK_SIZE as u32,
        total_blocks,
        inode_bitmap_block: 1,
        data_bitmap_block: 2,
        inode_table_block: 3,
        inode_count,
        root_inode: 2, // Inode 0 es NULL, Inode 1 reservado, Inode 2 es el Root Directory
        checksum: 0,
    };

    // Serializar superblock en bloque 0
    let mut sb_buf = [0u8; BLOCK_SIZE];
    unsafe {
        let sb_ptr = &sb as *const Superblock as *const u8;
        core::ptr::copy_nonoverlapping(sb_ptr, sb_buf.as_mut_ptr(), core::mem::size_of::<Superblock>());
    }
    dev.write_block(0, &sb_buf).map_err(|_| "Error escribiendo superblock")?;

    // Inicializar Inode Bitmap (bloque 1)
    let mut inode_bitmap = [0u8; BLOCK_SIZE];
    inode_bitmap[0] = 0b0000_0111; // Inodes 0, 1 (reservados) y 2 (root dir) ocupados
    dev.write_block(1, &inode_bitmap).map_err(|_| "Error escribiendo inode bitmap")?;

    // Inicializar Data Bitmap (bloque 2)
    let mut data_bitmap = [0u8; BLOCK_SIZE];
    let reserved_blocks = 3 + inode_blocks;
    for i in 0..reserved_blocks {
        let byte_idx = i / 8;
        let bit_idx = i % 8;
        data_bitmap[byte_idx] |= 1 << bit_idx;
    }
    dev.write_block(2, &data_bitmap).map_err(|_| "Error escribiendo data bitmap")?;

    // Inicializar la tabla de Inodes
    let empty_inode = Inode::new_empty();
    let mut inode_block_buf = [0u8; BLOCK_SIZE];
    for i in 0..INODES_PER_BLOCK {
        unsafe {
            let src = &empty_inode as *const Inode as *const u8;
            let dst = inode_block_buf.as_mut_ptr().add(i * INODE_SIZE);
            core::ptr::copy_nonoverlapping(src, dst, INODE_SIZE);
        }
    }
    for b in 0..inode_blocks {
        dev.write_block(3 + b as u64, &inode_block_buf).map_err(|_| "Error inicializando tabla de inodes")?;
    }

    // Reservar el primer bloque de datos para el directorio raíz
    let root_data_block = reserved_blocks as u64;
    let byte_idx = root_data_block as usize / 8;
    let bit_idx = root_data_block as usize % 8;
    data_bitmap[byte_idx] |= 1 << bit_idx;
    dev.write_block(2, &data_bitmap).map_err(|_| "Error reservando bloque de datos raíz")?;

    // Escribir bloque de directorio raíz vacío
    let root_dir_buf = [0u8; BLOCK_SIZE];
    dev.write_block(root_data_block, &root_dir_buf).map_err(|_| "Error inicializando directorio raíz")?;

    // Escribir Inode Raíz (inode 2)
    let mut root_inode = Inode::new_empty();
    root_inode.file_type = TYPE_DIR;
    root_inode.size = BLOCK_SIZE as u64;
    root_inode.direct_blocks[0] = root_data_block;
    root_inode.ctime = 1777777777;
    root_inode.mtime = 1777777777;

    write_inode(dev, &sb, 2, &root_inode)?;

    Ok(())
}

pub fn read_inode(
    dev: &mut impl BlockDevice,
    sb: &Superblock,
    inode_idx: u32,
) -> Result<Inode, &'static str> {
    if inode_idx >= sb.inode_count {
        return Err("Índice de inode fuera de límites");
    }
    let block_offset = inode_idx as u64 / INODES_PER_BLOCK as u64;
    let inode_offset = (inode_idx as usize % INODES_PER_BLOCK) * INODE_SIZE;
    
    let mut buf = [0u8; BLOCK_SIZE];
    dev.read_block(sb.inode_table_block + block_offset, &mut buf).map_err(|_| "Error leyendo bloque de inode")?;
    
    let mut inode = Inode::new_empty();
    unsafe {
        let src = buf.as_ptr().add(inode_offset);
        let dst = &mut inode as *mut Inode as *mut u8;
        core::ptr::copy_nonoverlapping(src, dst, INODE_SIZE);
    }
    Ok(inode)
}

pub fn write_inode(
    dev: &mut impl BlockDevice,
    sb: &Superblock,
    inode_idx: u32,
    inode: &Inode,
) -> Result<(), &'static str> {
    if inode_idx >= sb.inode_count {
        return Err("Índice de inode fuera de límites");
    }
    let block_offset = inode_idx as u64 / INODES_PER_BLOCK as u64;
    let inode_offset = (inode_idx as usize % INODES_PER_BLOCK) * INODE_SIZE;
    
    let mut buf = [0u8; BLOCK_SIZE];
    dev.read_block(sb.inode_table_block + block_offset, &mut buf).map_err(|_| "Error leyendo bloque para actualizar inode")?;
    
    unsafe {
        let src = inode as *const Inode as *const u8;
        let dst = buf.as_mut_ptr().add(inode_offset);
        core::ptr::copy_nonoverlapping(src, dst, INODE_SIZE);
    }
    
    dev.write_block(sb.inode_table_block + block_offset, &buf).map_err(|_| "Error guardando bloque de inode")?;
    Ok(())
}

pub fn allocate_inode(
    dev: &mut impl BlockDevice,
    sb: &Superblock,
) -> Result<u32, &'static str> {
    let mut bitmap = [0u8; BLOCK_SIZE];
    dev.read_block(sb.inode_bitmap_block, &mut bitmap).map_err(|_| "Error leyendo bitmap de inodes")?;
    
    for idx in 0..sb.inode_count {
        let byte_idx = idx as usize / 8;
        let bit_idx = idx as usize % 8;
        if (bitmap[byte_idx] & (1 << bit_idx)) == 0 {
            bitmap[byte_idx] |= 1 << bit_idx;
            dev.write_block(sb.inode_bitmap_block, &bitmap).map_err(|_| "Error guardando bitmap de inodes")?;
            return Ok(idx);
        }
    }
    Err("No quedan inodes libres")
}

pub fn allocate_block(
    dev: &mut impl BlockDevice,
    sb: &Superblock,
) -> Result<u64, &'static str> {
    let mut bitmap = [0u8; BLOCK_SIZE];
    dev.read_block(sb.data_bitmap_block, &mut bitmap).map_err(|_| "Error leyendo bitmap de bloques de datos")?;
    
    for idx in 0..sb.total_blocks {
        let byte_idx = idx as usize / 8;
        let bit_idx = idx as usize % 8;
        if (bitmap[byte_idx] & (1 << bit_idx)) == 0 {
            bitmap[byte_idx] |= 1 << bit_idx;
            dev.write_block(sb.data_bitmap_block, &bitmap).map_err(|_| "Error guardando bitmap de bloques de datos")?;
            return Ok(idx);
        }
    }
    Err("No quedan bloques de datos libres")
}

pub fn iterate_dir<F>(
    dev: &mut impl BlockDevice,
    dir_inode: &Inode,
    mut f: F,
) -> Result<(), &'static str>
where
    F: FnMut(u32, u8, &[u8]) -> bool,
{
    let mut buf = [0u8; BLOCK_SIZE];
    let ptr = core::ptr::addr_of!(dir_inode.direct_blocks);
    let direct_blocks = unsafe { core::ptr::read_unaligned(ptr) };
    for &block_idx in &direct_blocks {
        if block_idx == 0 { continue; }
        dev.read_block(block_idx, &mut buf).map_err(|_| "Error leyendo bloque de directorio")?;
        
        let mut offset = 0;
        while offset + 8 <= BLOCK_SIZE {
            let header_ptr = unsafe { buf.as_ptr().add(offset) as *const DirEntryHeader };
            let header = unsafe { &*header_ptr };
            if header.inode == 0 {
                // Entrada vacía
                offset += 8;
                continue;
            }
            if offset + header.rec_len as usize > BLOCK_SIZE {
                break;
            }
            
            let name_start = offset + 8;
            let name_end = name_start + header.name_len as usize;
            if name_end <= offset + header.rec_len as usize {
                let name = &buf[name_start..name_end];
                let keep_going = f(header.inode, header.file_type, name);
                if !keep_going {
                    return Ok(());
                }
            }
            offset += header.rec_len as usize;
        }
    }
    Ok(())
}

pub fn add_dir_entry(
    dev: &mut impl BlockDevice,
    sb: &Superblock,
    dir_inode_idx: u32,
    name: &str,
    target_inode: u32,
    file_type: u8,
) -> Result<(), &'static str> {
    let mut dir_inode = read_inode(dev, sb, dir_inode_idx)?;
    let mut buf = [0u8; BLOCK_SIZE];
    
    let required_len = 8 + name.len();
    let aligned_len = (required_len + 3) & !3; // Alinear a 4 bytes
    
    for i in 0..12 {
        let mut block_idx = dir_inode.direct_blocks[i];
        if block_idx == 0 {
            block_idx = allocate_block(dev, sb)?;
            dir_inode.direct_blocks[i] = block_idx;
            dir_inode.size += BLOCK_SIZE as u64;
            write_inode(dev, sb, dir_inode_idx, &dir_inode)?;
            dev.write_block(block_idx, &[0u8; BLOCK_SIZE]).map_err(|_| "Error iniciando bloque de dir nuevo")?;
        }
        
        dev.read_block(block_idx, &mut buf).map_err(|_| "Error leyendo bloque de directorio")?;
        
        let mut offset = 0;
        while offset + aligned_len <= BLOCK_SIZE {
            let header_ptr = unsafe { buf.as_mut_ptr().add(offset) as *mut DirEntryHeader };
            let header = unsafe { &mut *header_ptr };
            
            if header.inode == 0 {
                header.inode = target_inode;
                header.rec_len = aligned_len as u16;
                header.name_len = name.len() as u8;
                header.file_type = file_type;
                
                let name_bytes = name.as_bytes();
                unsafe {
                    core::ptr::copy_nonoverlapping(
                        name_bytes.as_ptr(),
                        buf.as_mut_ptr().add(offset + 8),
                        name_bytes.len()
                    );
                }
                
                dev.write_block(block_idx, &buf).map_err(|_| "Error guardando directorio actualizado")?;
                return Ok(());
            }
            offset += header.rec_len as usize;
        }
    }
    
    Err("Directorio lleno")
}

pub fn write_file_data(
    dev: &mut impl BlockDevice,
    sb: &Superblock,
    inode_idx: u32,
    data: &[u8],
) -> Result<(), &'static str> {
    let mut inode = read_inode(dev, sb, inode_idx)?;
    inode.size = data.len() as u64;
    
    let mut offset = 0;
    let mut block_ptr = 0;
    
    while offset < data.len() {
        if block_ptr >= 12 {
            return Err("Archivo demasiado grande (máximo 48KB en modo de prueba)");
        }
        
        let mut block_idx = inode.direct_blocks[block_ptr];
        if block_idx == 0 {
            block_idx = allocate_block(dev, sb)?;
            inode.direct_blocks[block_ptr] = block_idx;
        }
        
        let chunk_size = (data.len() - offset).min(BLOCK_SIZE);
        let mut buf = [0u8; BLOCK_SIZE];
        buf[..chunk_size].copy_from_slice(&data[offset..offset + chunk_size]);
        
        dev.write_block(block_idx, &buf).map_err(|_| "Error escribiendo bloque de datos")?;
        offset += chunk_size;
        block_ptr += 1;
    }
    
    write_inode(dev, sb, inode_idx, &inode)?;
    Ok(())
}

pub fn read_file_data(
    dev: &mut impl BlockDevice,
    sb: &Superblock,
    inode_idx: u32,
    buf: &mut [u8],
) -> Result<usize, &'static str> {
    let inode = read_inode(dev, sb, inode_idx)?;
    let total_bytes = inode.size as usize;
    let limit = buf.len().min(total_bytes);
    
    let mut offset = 0;
    let mut block_ptr = 0;
    
    while offset < limit {
        if block_ptr >= 12 {
            break;
        }
        let block_idx = inode.direct_blocks[block_ptr];
        if block_idx == 0 {
            break;
        }
        
        let mut block_buf = [0u8; BLOCK_SIZE];
        dev.read_block(block_idx, &mut block_buf).map_err(|_| "Error leyendo bloque de datos")?;
        
        let chunk_size = (limit - offset).min(BLOCK_SIZE);
        buf[offset..offset + chunk_size].copy_from_slice(&block_buf[..chunk_size]);
        
        offset += chunk_size;
        block_ptr += 1;
    }
    
    Ok(offset)
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MemDevice {
        blocks: [[u8; BLOCK_SIZE]; 128],
    }

    impl BlockDevice for MemDevice {
        type Error = ();

        fn read_block(&mut self, block_idx: u64, buf: &mut [u8; BLOCK_SIZE]) -> Result<(), Self::Error> {
            if block_idx >= 128 { return Err(()); }
            buf.copy_from_slice(&self.blocks[block_idx as usize]);
            Ok(())
        }

        fn write_block(&mut self, block_idx: u64, buf: &[u8; BLOCK_SIZE]) -> Result<(), Self::Error> {
            if block_idx >= 128 { return Err(()); }
            self.blocks[block_idx as usize].copy_from_slice(buf);
            Ok(())
        }
    }

    #[test]
    fn test_bmofs_format_and_io() {
        let mut dev = MemDevice { blocks: [[0u8; BLOCK_SIZE]; 128] };
        format_volume(&mut dev, 128, 16).unwrap();

        // Read Superblock
        let mut sb_buf = [0u8; BLOCK_SIZE];
        dev.read_block(0, &mut sb_buf).unwrap();
        let sb: Superblock = unsafe { core::ptr::read(sb_buf.as_ptr() as *const Superblock) };
        assert!(sb.is_valid());
        let total_blocks = sb.total_blocks;
        assert_eq!(total_blocks, 128);

        // Allocate a new file inode
        let file_inode = allocate_inode(&mut dev, &sb).unwrap();
        assert_eq!(file_inode, 3); // Root is 2, next is 3

        // Write file data
        let test_data = b"Hello from BMO-FS! Secure and custom filesystem.";
        write_file_data(&mut dev, &sb, file_inode, test_data).unwrap();

        // Read file data back
        let mut read_buf = [0u8; 100];
        let bytes_read = read_file_data(&mut dev, &sb, file_inode, &mut read_buf).unwrap();
        assert_eq!(bytes_read, test_data.len());
        assert_eq!(&read_buf[..bytes_read], test_data);

        // Add file entry to root directory
        add_dir_entry(&mut dev, &sb, sb.root_inode, "hello.txt", file_inode, TYPE_FILE).unwrap();

        // List directory and verify file is there
        let root_inode_data = read_inode(&mut dev, &sb, sb.root_inode).unwrap();
        let mut found = false;
        iterate_dir(&mut dev, &root_inode_data, |ino, file_type, name| {
            if name == b"hello.txt" {
                assert_eq!(ino, file_inode);
                assert_eq!(file_type, TYPE_FILE);
                found = true;
            }
            true
        }).unwrap();
        assert!(found);
    }
}
