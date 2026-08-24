//! **ENCONTRAR** en FAT32 y exFAT: un fichero o un subdirectorio, por su nombre.
//!
//! ## Por que es un fichero (L6b)
//!
//! ** Buscar y escribir son dos preguntas, y aqui ademas son **dos formatos**:
//! cada funcion viene por pares --`_fat32` y `_exfat`-- porque los dos guardan
//! los nombres de forma distinta (8.3 rellenado con espacios contra UTF-16
//! repartido en entradas de nombre). Tenerlos juntos es lo que deja ver que son
//! cuatro preguntas y no ocho funciones.
//!
//! ** El reparto es MOVER TEXTO (L6d): ni una linea cambia de contenido.

use super::*;

impl FatVolume {
    /// Find a subdirectory by name in the root directory.
    /// Returns the first cluster of the subdirectory.
    pub fn find_subdir(&mut self, name: &[u8]) -> Option<u32> {
        self.find_subdir_in(name, self.root_cluster)
    }

    /// Find a subdirectory by name in a specific directory (by first cluster).
    pub fn find_subdir_in(&mut self, name: &[u8], dir_cluster: u32) -> Option<u32> {
        match self.fs_type {
            FsType::Fat32 => self.find_subdir_fat32(name, dir_cluster),
            FsType::ExFat => self.find_subdir_exfat(name, dir_cluster),
        }
    }

    fn find_subdir_fat32(&mut self, name: &[u8], dir_cluster: u32) -> Option<u32> {
        let mut cluster = dir_cluster;
        let spc = self.sectors_per_cluster as u64;
        loop {
            let lba = self.cluster_to_lba(cluster);
            for s in 0..spc {
                unsafe {
                    if !self.read_sector(lba + s, Buf::buf) { continue; }
                }
                let entries = self.buf.as_ptr() as *const DirEntry;
                for i in 0..(512/32) {
                    unsafe {
                        let de = &*entries.add(i);
                        if de.name[0] == 0 { return None; }
                        if de.name[0] == 0xE5 { continue; }
                        if de.attr & 0x10 == 0 { continue; } // not a directory
                        if name_match(&de.name, name) {
                            let fc = (de.first_cluster_hi as u32) << 16 | de.first_cluster_lo as u32;
                            return Some(fc);
                        }
                    }
                }
            }
            cluster = match self.read_fat_entry(cluster) { Some(c) => c, None => return None };
        }
    }

    fn find_subdir_exfat(&mut self, name: &[u8], dir_cluster: u32) -> Option<u32> {
        let mut cluster = dir_cluster;
        let spc = self.sectors_per_cluster as u64;
        loop {
            let lba = self.cluster_to_lba(cluster);
            for s in 0..spc {
                unsafe {
                    if !self.read_sector(lba + s, Buf::buf) { continue; }
                }
                for i in 0..16 {
                    let entry_offset = i * 32;
                    let entry_type = self.buf[entry_offset];
                    if entry_type == 0x00 { return None; }
                    if entry_type == 0x05 { continue; }
                    if entry_type == 0x85 {
                        let file_entry = unsafe {
                            &*(self.buf[entry_offset..].as_ptr() as *const ExFatFileEntry)
                        };
                        let secondary_count = file_entry.secondary_count;
                        let is_dir = file_entry.file_attributes & 0x10 != 0;
                        for sec in 1..=secondary_count {
                            let sec_offset = entry_offset + (sec as usize) * 32;
                            if sec_offset + 32 > 512 { break; }
                            let sec_type = self.buf[sec_offset];
                            if sec_type == 0xC0 {
                                let stream = unsafe {
                                    &*(self.buf[sec_offset..].as_ptr() as *const ExFatStreamEntry)
                                };
                                let first_cluster = stream.first_cluster;
                                let name_len = stream.name_length as usize;
                                if sec + 1 <= secondary_count {
                                    let name_offset = entry_offset + ((sec + 1) as usize) * 32;
                                    if name_offset + 32 <= 512 && self.buf[name_offset] == 0xC1 {
                                        let name_entry = unsafe {
                                            &*(self.buf[name_offset..].as_ptr() as *const ExFatNameEntry)
                                        };
                                        let mut fat_name = [0u8; 11];
                                        let mut pos = 0;
                                        for ci in 0..name_len.min(15) {
                                            let ch = name_entry.name_string[ci] as u8;
                                            if ch == b'.' {
                                                while pos < 8 { fat_name[pos] = b' '; pos += 1; }
                                                continue;
                                            }
                                            if pos < 11 {
                                                fat_name[pos] = ch.to_ascii_uppercase();
                                                pos += 1;
                                            }
                                        }
                                        while pos < 11 { fat_name[pos] = b' '; pos += 1; }
                                        if is_dir && name_match(&fat_name, name) {
                                            return Some(first_cluster);
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            cluster = match self.read_fat_entry(cluster) { Some(c) => c, None => return None };
        }
    }

    /// Get the root directory's first cluster.
    pub fn root_cluster(&self) -> u32 { self.root_cluster }

    /// La entrada numero `n` de un directorio: `(name 8.3, es_dir, tamano)`.
    ///
    /// Devuelve `None` cuando se acaban. Existia `find_file_in` --buscar un
    /// nombre que ya conoces-- pero no habia forma de PREGUNTAR QUE HAY, y sin
    /// eso no puede haber un `ls` ni iconos de carpeta: hay que saberse los
    /// nombres de memoria.
    ///
    /// Se salta las borradas (0xE5), las entradas de nombre largo (attr 0x0F)
    /// y la etiqueta de volumen (0x08). Los nombres salen en 8.3 CRUDO, con
    /// sus espacios de relleno: convertirlos a algo legible es decision de
    /// presentacion y no le toca a un driver de disco.
    ///
    /// Indexar por numero en vez de llevar un cursor es O(n) por llamada, y
    /// listar un directorio entero sale O(n^2). Con directorios de decenas de
    /// entradas eso es irrelevante, y a cambio el driver se queda SIN ESTADO:
    /// dos listados a la vez no se pisan, y una entrada que desaparece no deja
    /// un cursor apuntando al vacio.
    pub fn entry_at(&mut self, dir_cluster: u32, n: usize) -> Option<([u8; 11], bool, u32)> {
        if !matches!(self.fs_type, FsType::Fat32) {
            return None;
        }
        let mut cluster = dir_cluster;
        let spc = self.sectors_per_cluster as u64;
        let mut vistas = 0usize;
        loop {
            let lba = self.cluster_to_lba(cluster);
            for s in 0..spc {
                unsafe {
                    if !self.read_sector(lba + s, Buf::buf) { continue; }
                }
                let entries = self.buf.as_ptr() as *const DirEntry;
                for i in 0..(512 / 32) {
                    let de = unsafe { &*entries.add(i) };
                    // 0x00 = fin del directorio: no hay nada mas, nunca.
                    if de.name[0] == 0 { return None; }
                    if de.name[0] == 0xE5 { continue; }
                    let attr = de.attr;
                    if attr & 0x0F == 0x0F { continue; } // fragmento de nombre largo
                    if attr & 0x08 != 0 { continue; }    // etiqueta de volumen
                    if vistas == n {
                        return Some((de.name, attr & 0x10 != 0, de.file_size));
                    }
                    vistas += 1;
                }
            }
            cluster = self.read_fat_entry(cluster)?;
        }
    }
}
