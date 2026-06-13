//! `fs` — Sistema de archivos de FastOS.
//!
//! Coordina la detección del USB, la partición FAT32 y el montaje de BMO-FS.
//! Implementa también un fallback en RAM totalmente dinámico para garantizar
//! el arranque del kernel en entornos sin disco físico (como emuladores puros).

#![allow(dead_code)]

extern crate alloc;

pub mod ramdisk;
pub mod fat32;
pub mod bmofs_loop;

use crate::drivers::serial;
use crate::drivers::usb::storage::ACTIVE_USB_DISK;

pub trait DiskReader {
    fn read_sectors(&mut self, lba: u64, count: u32, buf: &mut [u8]) -> Result<(), DiskError>;
}

pub trait DiskWriter {
    fn write_sectors(&mut self, lba: u64, count: u32, buf: &[u8]) -> Result<(), DiskError>;
}

#[derive(Debug, Clone, Copy)]
pub enum DiskError {
    ControllerError,
    InvalidLba,
    Timeout,
    IOError,
}

/// Dispositivo en RAM para el fallback dinámico
struct RamBlockDevice {
    data: alloc::vec::Vec<[u8; bmofs::BLOCK_SIZE]>,
}

impl bmofs::BlockDevice for RamBlockDevice {
    type Error = &'static str;

    fn read_block(&mut self, block_idx: u64, buf: &mut [u8; bmofs::BLOCK_SIZE]) -> Result<(), Self::Error> {
        if block_idx >= self.data.len() as u64 {
            return Err("Acceso de lectura fuera de límites del disco RAM");
        }
        buf.copy_from_slice(&self.data[block_idx as usize]);
        Ok(())
    }

    fn write_block(&mut self, block_idx: u64, buf: &[u8; bmofs::BLOCK_SIZE]) -> Result<(), Self::Error> {
        if block_idx >= self.data.len() as u64 {
            return Err("Acceso de escritura fuera de límites del disco RAM");
        }
        self.data[block_idx as usize].copy_from_slice(buf);
        Ok(())
    }
}

/// Inicializa el sistema de archivos principal detectando la partición de arranque y montando BMO-FS
pub fn init() {
    serial::serial_write("[FS] Inicializando File System...\n");

    let mut mounted_from_usb = false;

    // Intentar montar desde el USB
    unsafe {
        if let Some(ref mut disk) = ACTIVE_USB_DISK {
            // Solo intentar si es un disco real (slot_id != 0 significa dispositivo xHCI probed)
            if disk.slot_id != 0 {
                serial::serial_write("[FS] Intentando montar USB físico...\n");
                match fat32::Fat32Volume::parse(disk) {
                    Ok(volume) => {
                        match volume.locate_file(disk, "BMOFS.IMG") {
                            Ok((start_cluster, file_size)) => {
                                let bmo_dev = bmofs_loop::BmoBlockDevice {
                                    start_cluster,
                                    file_size,
                                    fat_volume: Some(volume),
                                    fallback_ram: None,
                                };
                                bmofs_loop::MOUNTED_BMO_DEVICE = Some(bmo_dev);
                                serial::serial_write("[FS] BMO-FS montado correctamente desde bmofs.img en USB.\n");
                                mounted_from_usb = true;
                            }
                            Err(e) => {
                                serial::serial_write("[FS] WARN: No se encontró 'bmofs.img' en FAT32: ");
                                serial::serial_write(e);
                                serial::serial_write("\n");
                            }
                        }
                    }
                    Err(e) => {
                        serial::serial_write("[FS] WARN: Falló lectura de partición FAT32: ");
                        serial::serial_write(e);
                        serial::serial_write("\n");
                    }
                }
            }
        }
    }

    // Si falló el montaje físico, iniciar fallback dinámico en RAM
    if !mounted_from_usb {
        serial::serial_write("[FS] Iniciando fallback dinámico en RAM para BMO-FS...\n");
        
        let total_blocks = 64; // Mínimo soportado por bmofs::format_volume
        let mut ram_disk = RamBlockDevice {
            data: alloc::vec![[0u8; bmofs::BLOCK_SIZE]; total_blocks],
        };

        // Formatear volumen en RAM con 16 inodes
        if bmofs::format_volume(&mut ram_disk, total_blocks as u64, 16).is_ok() {
            // Obtener el superblock
            let mut sb_buf = [0u8; bmofs::BLOCK_SIZE];
            bmofs::BlockDevice::read_block(&mut ram_disk, 0, &mut sb_buf).unwrap();
            let sb: bmofs::Superblock = unsafe { core::ptr::read(sb_buf.as_ptr() as *const bmofs::Superblock) };

            // Reservar un inode para readme.txt
            let new_inode_idx = bmofs::allocate_inode(&mut ram_disk, &sb).unwrap();

            // Escribir los datos del readme en el volumen (usar solo caracteres ASCII)
            let readme_content = b"Bienvenido a BMO-FS Fallback! Este archivo fue generado dinamicamente en la RAM del kernel.";
            bmofs::write_file_data(&mut ram_disk, &sb, new_inode_idx, readme_content).unwrap();

            // Configurar el tipo de archivo (File) en el inode
            let mut inode = bmofs::read_inode(&mut ram_disk, &sb, new_inode_idx).unwrap();
            inode.file_type = bmofs::TYPE_FILE;
            bmofs::write_inode(&mut ram_disk, &sb, new_inode_idx, &inode).unwrap();

            // Añadir el archivo al directorio raíz
            bmofs::add_dir_entry(&mut ram_disk, &sb, sb.root_inode, "readme.txt", new_inode_idx, bmofs::TYPE_FILE).unwrap();

            // Mapear los bloques de RAM al BmoBlockDevice estático
            let mut flat_data = alloc::vec![0u8; total_blocks * bmofs::BLOCK_SIZE];
            for i in 0..total_blocks {
                let offset = i * bmofs::BLOCK_SIZE;
                bmofs::BlockDevice::read_block(&mut ram_disk, i as u64, unsafe {
                    &mut *(flat_data.as_mut_ptr().add(offset) as *mut [u8; bmofs::BLOCK_SIZE])
                }).unwrap();
            }

            // Filtrar y referenciar en una sección estática (con una caja Leak segura)
            let leaked_ram: &'static [u8] = alloc::vec::Vec::leak(flat_data);
            
            let bmo_dev = bmofs_loop::BmoBlockDevice {
                start_cluster: 0,
                file_size: leaked_ram.len() as u32,
                fat_volume: None,
                fallback_ram: Some(leaked_ram),
            };

            unsafe {
                bmofs_loop::MOUNTED_BMO_DEVICE = Some(bmo_dev);
            }
            serial::serial_write("[FS] BMO-FS montado correctamente en RAM (Fallback).\n");
        } else {
            serial::serial_write("[FS] ERROR FATAL: No se pudo formatear el volumen BMO-FS de fallback.\n");
        }
    }

    // Auto-test: Validar lectura de readme.txt tras montaje
    match bmofs_loop::read_readme_from_bmofs() {
        Ok(content) => {
            serial::serial_write("[FS] Auto-test de lectura de readme.txt exitoso:\n      ");
            serial::serial_write(&content);
            serial::serial_write("\n");
        }
        Err(e) => {
            serial::serial_write("[FS] ERROR de Auto-test: ");
            serial::serial_write(e);
            serial::serial_write("\n");
        }
    }
}
