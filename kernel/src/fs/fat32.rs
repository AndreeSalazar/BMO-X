//! Minimal read-only FAT32 directory parser and sector mapping engine.
//!
//! Permite al kernel leer la partición de arranque UEFI del USB,
//! buscar la imagen de contenedor `bmofs.img` y mapear sus sectores
//! lógicos a sectores físicos del USB de forma perezosa (on-demand).

#![allow(dead_code)]

use crate::drivers::serial;
use crate::fs::DiskReader;

#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct BiosParameterBlock {
    pub jmp: [u8; 3],
    pub oem: [u8; 8],
    pub bytes_per_sector: u16,      // offset 11
    pub sectors_per_cluster: u8,    // offset 13
    pub reserved_sectors: u16,      // offset 14
    pub num_fats: u8,               // offset 16
    pub root_entries: u16,
    pub total_sectors_16: u16,
    pub media: u8,
    pub sectors_per_fat_16: u16,
    pub sectors_per_track: u16,
    pub heads: u16,
    pub hidden_sectors: u32,
    pub total_sectors_32: u32,
    // FAT32 Extended fields
    pub sectors_per_fat_32: u32,    // offset 36
    pub ext_flags: u16,
    pub fs_version: u16,
    pub root_cluster: u32,          // offset 44
    pub fs_info: u16,
    pub backup_boot: u16,
    pub reserved: [u8; 12],
    pub drive_num: u8,
    pub reserved1: u8,
    pub boot_sig: u8,
    pub volume_id: u32,
    pub volume_label: [u8; 11],
    pub file_sys_type: [u8; 8],
}

pub struct Fat32Volume {
    pub bytes_per_sector: u32,
    pub sectors_per_cluster: u32,
    pub reserved_sectors: u32,
    pub num_fats: u32,
    pub sectors_per_fat: u32,
    pub root_cluster: u32,
    pub fat_start_sector: u32,
    pub data_start_sector: u32,
}

impl Fat32Volume {
    pub fn parse(dev: &mut impl DiskReader) -> Result<Self, &'static str> {
        let mut buf = [0u8; 512];
        dev.read_sectors(0, 1, &mut buf).map_err(|_| "Error leyendo sector de arranque FAT32")?;

        let bpb: BiosParameterBlock = unsafe {
            core::ptr::read_unaligned(buf.as_ptr() as *const BiosParameterBlock)
        };

        if bpb.bytes_per_sector != 512 {
            return Err("Solo se soportan sectores de 512 bytes");
        }

        let bytes_per_sector = bpb.bytes_per_sector as u32;
        let sectors_per_cluster = bpb.sectors_per_cluster as u32;
        let reserved_sectors = bpb.reserved_sectors as u32;
        let num_fats = bpb.num_fats as u32;
        let sectors_per_fat = if bpb.sectors_per_fat_16 != 0 {
            bpb.sectors_per_fat_16 as u32
        } else {
            bpb.sectors_per_fat_32
        };
        let root_cluster = bpb.root_cluster;

        let fat_start_sector = reserved_sectors;
        let data_start_sector = reserved_sectors + (num_fats * sectors_per_fat);

        serial::serial_write("[FAT32] Volume parsed. Sectors per cluster: ");
        crate::serial_hex(sectors_per_cluster as u64);
        serial::serial_write(" | Data start sector: ");
        crate::serial_hex(data_start_sector as u64);
        serial::serial_write("\n");

        Ok(Self {
            bytes_per_sector,
            sectors_per_cluster,
            reserved_sectors,
            num_fats,
            sectors_per_fat,
            root_cluster,
            fat_start_sector,
            data_start_sector,
        })
    }

    /// Lee el siguiente cluster en la tabla FAT
    pub fn next_cluster(&self, dev: &mut impl DiskReader, cluster: u32) -> Result<u32, &'static str> {
        let fat_offset = cluster * 4;
        let sector = self.fat_start_sector + (fat_offset / self.bytes_per_sector);
        let offset = (fat_offset % self.bytes_per_sector) as usize;

        let mut buf = [0u8; 512];
        dev.read_sectors(sector as u64, 1, &mut buf).map_err(|_| "Error leyendo tabla FAT")?;

        let next = u32::from_le_bytes([buf[offset], buf[offset + 1], buf[offset + 2], buf[offset + 3]]);
        Ok(next & 0x0FFF_FFFF) // Limpiar bits superiores reservados
    }

    /// Obtiene el sector físico correspondiente al inicio de un cluster
    pub fn cluster_to_sector(&self, cluster: u32) -> u32 {
        self.data_start_sector + (cluster - 2) * self.sectors_per_cluster
    }

    /// Busca un archivo en el directorio raíz por su nombre (ej: "BMOFS.IMG")
    pub fn locate_file(&self, dev: &mut impl DiskReader, filename: &str) -> Result<(u32, u32), &'static str> {
        let mut cluster = self.root_cluster;
        let mut file_buf = [0u8; 4096]; // Buffer para leer un cluster (soporta cluster_size de 4KB)
        let cluster_size_sectors = self.sectors_per_cluster;
        let cluster_bytes = (cluster_size_sectors * 512) as usize;

        // Limitar la búsqueda para evitar bucles infinitos en discos corruptos
        for _ in 0..128 {
            if cluster >= 0x0FFF_FFF8 {
                break;
            }

            let sector = self.cluster_to_sector(cluster);
            
            // Leer cluster completo de forma segura
            let read_size = cluster_size_sectors.min(8); // Máximo 8 sectores (4KB)
            dev.read_sectors(sector as u64, read_size, &mut file_buf[.. (read_size as usize * 512)]).map_err(|_| "Error leyendo directorio raíz")?;

            // Iterar por las entradas de directorio (cada una de 32 bytes)
            for entry_idx in 0..(cluster_bytes / 32) {
                let offset = entry_idx * 32;
                if offset + 32 > file_buf.len() { break; }
                let entry = &file_buf[offset..offset + 32];
                
                let first_char = entry[0];
                if first_char == 0x00 {
                    // Fin del directorio
                    break;
                }
                if first_char == 0xE5 {
                    // Entrada eliminada, continuar
                    continue;
                }

                // Filtrar entradas LFN (Long File Name)
                let attr = entry[11];
                if attr == 0x0F {
                    continue;
                }

                // Extraer nombre 8.3
                let mut name_buf = [b' '; 11];
                name_buf.copy_from_slice(&entry[0..11]);
                
                // Formatear nombre sin espacios
                let mut clean_name = [0u8; 12];
                let mut clean_len = 0;
                
                // Nombre principal
                for i in 0..8 {
                    if name_buf[i] != b' ' {
                        clean_name[clean_len] = name_buf[i].to_ascii_uppercase();
                        clean_len += 1;
                    }
                }
                
                // Extensión
                if name_buf[8] != b' ' || name_buf[9] != b' ' || name_buf[10] != b' ' {
                    clean_name[clean_len] = b'.';
                    clean_len += 1;
                    for i in 8..11 {
                        if name_buf[i] != b' ' {
                            clean_name[clean_len] = name_buf[i].to_ascii_uppercase();
                            clean_len += 1;
                        }
                    }
                }

                let clean_name_str = core::str::from_utf8(&clean_name[..clean_len]).unwrap_or("");
                if clean_name_str == filename.to_ascii_uppercase() {
                    // Archivo encontrado!
                    let cluster_high = u16::from_le_bytes([entry[20], entry[21]]) as u32;
                    let cluster_low = u16::from_le_bytes([entry[26], entry[27]]) as u32;
                    let start_cluster = (cluster_high << 16) | cluster_low;
                    let file_size = u32::from_le_bytes([entry[28], entry[29], entry[30], entry[31]]);

                    serial::serial_write("[FAT32] Archivo encontrado: ");
                    serial::serial_write(filename);
                    serial::serial_write(" | Cluster Inicial: ");
                    crate::serial_hex(start_cluster as u64);
                    serial::serial_write(" | Tamaño: ");
                    crate::serial_hex(file_size as u64);
                    serial::serial_write(" bytes\n");

                    return Ok((start_cluster, file_size));
                }
            }

            cluster = self.next_cluster(dev, cluster)?;
        }

        Err("No se pudo localizar el archivo en la partición FAT32")
    }

    /// Mapea un bloque lógico de la imagen bmofs.img (4096 bytes) a un sector físico del disco
    pub fn get_physical_sector_for_block(
        &self,
        dev: &mut impl DiskReader,
        start_cluster: u32,
        block_idx: u64,
    ) -> Result<u64, &'static str> {
        // Un bloque de BMO-FS es 4096 bytes (8 sectores de 512 bytes)
        let file_sector_offset = block_idx * 8;
        let cluster_offset = (file_sector_offset / self.sectors_per_cluster as u64) as u32;
        let sector_in_cluster = (file_sector_offset % self.sectors_per_cluster as u64) as u32;

        let mut current_cluster = start_cluster;
        for _ in 0..cluster_offset {
            current_cluster = self.next_cluster(dev, current_cluster)?;
            if current_cluster >= 0x0FFF_FFF8 {
                return Err("Límites del archivo excedidos en el mapeo de cluster");
            }
        }

        let physical_sector = self.cluster_to_sector(current_cluster) + sector_in_cluster;
        Ok(physical_sector as u64)
    }
}
