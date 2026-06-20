//! Loop device driver wrapping bmofs.img as a block device.
//!
//! v1.7.4: sólo lectura desde RAM fallback.
//! v2.0 (futuro): reabrir el path USB/ATA físico cuando se re-introduzca
//! el driver de almacenamiento.

#![allow(dead_code)]

use bmofs::BlockDevice;

/// Dispositivo de bloques para BMO-FS
pub struct BmoBlockDevice {
    pub start_cluster: u32,
    pub file_size: u32,
    /// v2.0: tabla FAT32 para path físico (USB/ATA).
    /// v1.7.4: sin uso, se conserva para evitar migración al re-introducir USB.
    pub fat_volume: Option<()>,
    pub fallback_ram: Option<&'static [u8]>,
}

impl BlockDevice for BmoBlockDevice {
    type Error = &'static str;

    fn read_block(&mut self, block_idx: u64, buf: &mut [u8; bmofs::BLOCK_SIZE]) -> Result<(), Self::Error> {
        // v1.7.4: sólo lectura desde RAM fallback. El driver USB/ATA físico
        // se eliminó en ring0/drivers/usb. Si en el futuro se re-introduce
        // (v2.0 con soporte de USB mass storage + AHCI), este método
        // despacha a `crate::dev::usb::storage::ACTIVE_USB_DISK` o
        // equivalente.
        let ram_data = self.fallback_ram.ok_or("No hay fallback RAM ni disco físico")?;
        let offset = (block_idx as usize) * bmofs::BLOCK_SIZE;
        if offset + bmofs::BLOCK_SIZE <= ram_data.len() {
            buf.copy_from_slice(&ram_data[offset..offset + bmofs::BLOCK_SIZE]);
            Ok(())
        } else {
            Err("Lectura fuera de límites de RAM fallback")
        }
    }

    fn write_block(&mut self, _block_idx: u64, _buf: &[u8; bmofs::BLOCK_SIZE]) -> Result<(), Self::Error> {
        // Driver en modo lectura de arranque
        Ok(())
    }
}

pub static mut MOUNTED_BMO_DEVICE: Option<BmoBlockDevice> = None;

/// Inicializa el volumen BMO-FS y retorna el contenido de "readme.txt" para validar
#[allow(static_mut_refs)]
pub fn read_readme_from_bmofs() -> Result<alloc::string::String, &'static str> {
    unsafe {
        let dev = MOUNTED_BMO_DEVICE.as_mut().ok_or("No hay volumen BMO-FS montado")?;
        
        // 1. Leer superblock
        let mut sb_buf = [0u8; bmofs::BLOCK_SIZE];
        dev.read_block(0, &mut sb_buf)?;
        
        let sb: bmofs::Superblock = core::ptr::read(sb_buf.as_ptr() as *const bmofs::Superblock);
        if !sb.is_valid() {
            return Err("Superblock de BMO-FS no válido en el volumen montado");
        }

        // 2. Leer inode raíz (inode 2)
        let root_inode = bmofs::read_inode(dev, &sb, sb.root_inode)?;

        // 3. Buscar "readme.txt" en el directorio raíz
        let mut target_inode_idx: Option<u32> = None;
        bmofs::iterate_dir(dev, &root_inode, |inode_num, file_type, name_bytes| {
            if name_bytes == b"readme.txt" && file_type == bmofs::TYPE_FILE {
                target_inode_idx = Some(inode_num);
                false // detener iteración
            } else {
                true
            }
        })?;

        let inode_idx = target_inode_idx.ok_or("No se encontró 'readme.txt' en BMO-FS")?;
        let _inode = bmofs::read_inode(dev, &sb, inode_idx)?;

        // 4. Leer contenido del archivo
        let mut data_buf = [0u8; 512];
        let bytes_read = bmofs::read_file_data(dev, &sb, inode_idx, &mut data_buf)?;
        
        let clean_len = bytes_read.min(data_buf.len());
        let s = core::str::from_utf8(&data_buf[..clean_len])
            .map_err(|_| "El contenido de readme.txt no es UTF-8 válido")?;
            
        Ok(alloc::string::String::from(s))
    }
}
