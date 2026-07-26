//! FAT32 and exFAT filesystem reader/writer — minimal implementation.
//!
//! Supports both FAT32 (S: FASTOS-EFI) and exFAT (T: FastOS-Data, X: Commit-Real).
//! Reads BPB, locates root directory, finds files by 8.3 name,
//! and reads clusters via the FAT chain. El almacenamiento entra por el
//! contrato `BlockReader`/`BlockWriter`: no sabe si debajo hay SATA o NVMe.

#![no_std]

/// Filesystem type detected.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FsType {
    Fat32,
    ExFat,
}

/// exFAT BIOS Parameter Block at sector 0, offset 0.
/// exFAT has a different layout than FAT32 — see exFAT spec section 3.1.
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct ExFatBpb {
    pub jump: [u8; 3],
    pub fs_name: [u8; 8],       // "EXFAT   "
    pub must_be_zero: [u8; 53],
    pub partition_offset: u64,
    pub volume_length: u64,
    pub fat_offset: u32,
    pub fat_length: u32,
    pub cluster_heap_offset: u32,
    pub cluster_count: u32,
    pub first_cluster_of_root_directory: u32,
    pub volume_serial_number: u32,
    pub fs_revision: u16,
    pub volume_flags: u16,
    pub bytes_per_sector_shift: u8,
    pub sectors_per_cluster_shift: u8,
    pub number_of_fats: u8,
    pub drive_select: u8,
    pub percent_in_use: u8,
    pub reserved: [u8; 7],
    pub boot_code: [u8; 390],
    pub boot_signature: u16,
}

/// FAT32 BIOS Parameter Block at sector 0, offset 11.
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct FatBpb {
    pub jmp: [u8; 3],
    pub oem: [u8; 8],
    pub bytes_per_sector: u16,
    pub sectors_per_cluster: u8,
    pub reserved_sectors: u16,
    pub num_fats: u8,
    _root_entries: u16,
    _total_sectors16: u16,
    pub media: u8,
    _fat_size16: u16,
    pub sectors_per_track: u16,
    pub num_heads: u16,
    pub hidden_sectors: u32,
    pub total_sectors: u32,
    pub fat_size: u32,
    pub ext_flags: u16,
    pub fs_version: u16,
    pub root_cluster: u32,
    pub fs_info: u16,
    pub backup_boot_sector: u16,
    _reserved: [u8; 12],
    pub drive_number: u8,
    _reserved1: u8,
    pub boot_sig: u8,
    pub volume_id: u32,
    pub volume_label: [u8; 11],
    pub fs_type: [u8; 8],
}

#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct DirEntry {
    pub name: [u8; 11],
    pub attr: u8,
    _nt_reserved: u8,
    _create_time_tenth: u8,
    pub create_time: u16,
    pub create_date: u16,
    pub last_access: u16,
    pub first_cluster_hi: u16,
    pub write_time: u16,
    pub write_date: u16,
    pub first_cluster_lo: u16,
    pub file_size: u32,
}

/// exFAT File Directory Entry (type 0x85)
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct ExFatFileEntry {
    pub entry_type: u8,      // 0x85
    pub secondary_count: u8,
    pub set_checksum: u16,
    pub file_attributes: u16,
    _reserved1: u16,
    pub create_timestamp: u32,
    pub last_modified_timestamp: u32,
    pub last_accessed_timestamp: u32,
    _create_millis: u8,
    _last_modified_millis: u8,
    _create_utc_offset: u8,
    _last_modified_utc_offset: u8,
    _last_accessed_utc_offset: u8,
    _reserved2: [u8; 7],
}

/// exFAT Stream Extension Entry (type 0xC0) — follows File Entry
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct ExFatStreamEntry {
    pub entry_type: u8,      // 0xC0
    pub general_secondary_flags: u8,
    _reserved1: u8,
    _reserved2: u8,
    pub name_length: u8,
    pub name_hash: u16,
    _reserved3: u16,
    pub valid_data_length: u64,
    _reserved4: u32,
    pub first_cluster: u32,
    pub data_length: u64,
}

/// exFAT Filename Entry (type 0xC1) — follows Stream Entry
/// Contains up to 15 UTF-16 characters of the filename
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct ExFatNameEntry {
    pub entry_type: u8,      // 0xC1
    pub general_secondary_flags: u8,
    pub name_string: [u16; 15],  // UTF-16LE filename (up to 15 chars)
}

/// Lee `count` sectores de 512 B desde `lba` ABSOLUTO del dispositivo.
///
/// Es TODO lo que este sistema de ficheros necesita saber del almacenamiento.
/// No sabe si debajo hay SATA, NVMe o un disco en RAM, y no debe saberlo:
/// antes estaba soldado a `bmo_ahci` y por tanto no habria podido leer jamas
/// un NVMe. Un puntero a funcion en vez de un trait porque en Ring 0 no hay
/// alloc y no hace falta mas.
pub type BlockReader = fn(lba: u64, count: u16, buf: &mut [u8]) -> bool;
/// Escribe sectores. `None` al montar = volumen de SOLO LECTURA, y entonces
/// la imposibilidad de escribir es ESTRUCTURAL, no una promesa.
pub type BlockWriter = fn(lba: u64, count: u16, data: &[u8]) -> bool;

/// Cual de los dos buffers internos usa una operacion. Existe para que el
/// prestamo del buffer y el del dispositivo no se pisen: se copia el puntero
/// a funcion primero y el buffer se toma despues.
#[allow(non_camel_case_types)]
#[derive(Clone, Copy)]
enum Buf { buf, fat_cache }

pub struct FatVolume {
    read: BlockReader,
    write: Option<BlockWriter>,
    /// Primer LBA de la PARTICION dentro del disco. El sistema de ficheros
    /// piensa en sectores relativos a su volumen y no sabe que existe una
    /// tabla de particiones; aqui se suma. Sin esto, `mount` leia el sector 0
    /// del DISCO —la GPT— creyendo que era el arranque del volumen.
    part_lba: u64,
    pub fs_type: FsType,
    #[allow(dead_code)]
    bytes_per_sector: u16,
    sectors_per_cluster: u8,
    num_fats: u8,
    fat_start: u32,
    fat_size_sectors: u32,
    data_start: u32,
    root_cluster: u32,
    /// Ultimo numero de cluster que EXISTE en la zona de datos.
    ///
    /// La FAT casi siempre tiene mas entradas que clusters reales: se
    /// dimensiona en sectores enteros y el final sobra. Ese sobrante esta a
    /// cero, o sea que parece "libre". Buscar un hueco sin este tope devuelve
    /// clusters que no existen, y `cluster_to_lba` de un cluster inexistente
    /// da un LBA FUERA del volumen. Es la diferencia entre "no cabe" y
    /// "escribir en la particion del vecino".
    max_cluster: u32,
    buf: [u8; 512],
    fat_cache: [u8; 512],
}

/// Por que fallo una escritura. Un `false` pelado no dice si el disco esta
/// lleno, si el volumen es de solo lectura o si el nombre ya existia.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WriteError {
    /// El volumen se monto sin `BlockWriter`.
    ReadOnly,
    /// Ya hay un archivo con ese nombre en ese directorio.
    Exists,
    /// No quedan clusters libres para todos los datos.
    NoSpace,
    /// El directorio no tiene entradas libres y no se pudo extender.
    DirFull,
    /// El dispositivo fallo al leer o escribir un sector.
    Io,
    /// Crear archivos no esta implementado para este formato.
    Unsupported,
}

impl WriteError {
    pub fn name(self) -> &'static str {
        match self {
            WriteError::ReadOnly => "el volumen es de solo lectura",
            WriteError::Exists => "ya existe un archivo con ese nombre",
            WriteError::NoSpace => "no quedan clusters libres",
            WriteError::DirFull => "el directorio esta lleno",
            WriteError::Io => "el disco fallo al leer o escribir",
            WriteError::Unsupported => "crear archivos no soportado en este formato",
        }
    }
}

/// Monta el volumen que empieza en `part_lba` del dispositivo.
///
/// `write = None` monta en SOLO LECTURA: no es una politica que alguien deba
/// recordar respetar, es que no hay con que escribir.
pub fn mount(read: BlockReader, write: Option<BlockWriter>, part_lba: u64) -> Option<FatVolume> {
    let mut buf = [0u8; 512];
    if !read(part_lba, 1, &mut buf) { return None; }

    // Check for exFAT signature ("EXFAT   ") at offset 3
    let fs_name = &buf[3..11];
    if fs_name == b"EXFAT   " {
        return mount_exfat(read, write, part_lba, &buf);
    }

    // Otherwise try FAT32
    let bpb = unsafe { &*(buf.as_ptr() as *const FatBpb) };
    if bpb.bytes_per_sector != 512 { return None; }
    if bpb.boot_sig != 0x29 && bpb.boot_sig != 0x28 { return None; }
    let fat_start = bpb.reserved_sectors as u32;
    let fat_size_sectors = bpb.fat_size;
    let num_fats = bpb.num_fats;
    let data_start = fat_start + (num_fats as u32) * fat_size_sectors;
    let spc = bpb.sectors_per_cluster;
    if spc == 0 { return None; }
    // Clusters que EXISTEN de verdad: los sectores de datos divididos entre el
    // tamano de cluster. La numeracion empieza en 2, asi que el ultimo valido
    // es cuenta+1.
    let total = bpb.total_sectors;
    if total <= data_start { return None; }
    let max_cluster = (total - data_start) / spc as u32 + 1;
    Some(FatVolume { read, write, part_lba, fs_type: FsType::Fat32, bytes_per_sector: bpb.bytes_per_sector, sectors_per_cluster: spc,
        num_fats, fat_start, fat_size_sectors, data_start, root_cluster: bpb.root_cluster, max_cluster, buf: [0; 512], fat_cache: [0; 512] })
}

fn mount_exfat(read: BlockReader, write: Option<BlockWriter>, part_lba: u64, buf: &[u8; 512]) -> Option<FatVolume> {
    let epb = unsafe { &*(buf.as_ptr() as *const ExFatBpb) };
    if epb.boot_signature != 0xAA55 { return None; }
    let bps_shift = epb.bytes_per_sector_shift;
    let bytes_per_sector: u16 = 1u16 << bps_shift;
    let spc_shift = epb.sectors_per_cluster_shift;
    let sectors_per_cluster: u8 = 1u8 << spc_shift;
    let fat_start = epb.fat_offset;
    let fat_size_sectors = epb.fat_length;
    let data_start = epb.cluster_heap_offset;
    let root_cluster = epb.first_cluster_of_root_directory;
    let num_fats = epb.number_of_fats;


    // exFAT lo dice en su propio BPB, sin tener que deducirlo.
    let max_cluster = epb.cluster_count + 1;
    Some(FatVolume { read, write, part_lba, fs_type: FsType::ExFat, bytes_per_sector, sectors_per_cluster,
        num_fats, fat_start, fat_size_sectors, data_start, root_cluster, max_cluster, buf: [0; 512], fat_cache: [0; 512] })
}

impl FatVolume {
    /// Lee un sector del VOLUMEN a uno de los buffers internos.
    ///
    /// El puntero a funcion se copia ANTES de tomar el buffer: si no, seria un
    /// doble prestamo de `self` y no compilaria.
    fn read_sector(&mut self, lba: u64, which: Buf) -> bool {
        let rd = self.read;
        let abs = self.part_lba + lba;
        match which {
            Buf::buf => rd(abs, 1, &mut self.buf),
            Buf::fat_cache => rd(abs, 1, &mut self.fat_cache),
        }
    }

    /// Escribe uno de los buffers internos. `false` si el volumen se monto en
    /// solo lectura — no hay writer que llamar.
    fn write_sector(&mut self, lba: u64, which: Buf) -> bool {
        let wr = match self.write { Some(w) => w, None => return false };
        let abs = self.part_lba + lba;
        match which {
            Buf::buf => wr(abs, 1, &self.buf),
            Buf::fat_cache => wr(abs, 1, &self.fat_cache),
        }
    }

    /// Escribe datos externos (un sector ya armado por el llamante).
    fn write_from(&mut self, lba: u64, data: &[u8]) -> bool {
        let wr = match self.write { Some(w) => w, None => return false };
        wr(self.part_lba + lba, 1, data)
    }

    /// Primer LBA de la particion montada, por si alguien de arriba lo
    /// necesita para diagnostico.
    pub fn partition_lba(&self) -> u64 { self.part_lba }

    fn cluster_to_lba(&self, cluster: u32) -> u64 {
        self.data_start as u64 + (cluster as u64 - 2) * self.sectors_per_cluster as u64
    }

    fn read_fat_entry(&mut self, cluster: u32) -> Option<u32> {
        let fat_offset = cluster * 4;
        let fat_sector = self.fat_start + (fat_offset / 512);
        let fat_index = (fat_offset % 512) as usize;
        unsafe {
            if !self.read_sector(fat_sector as u64, Buf::fat_cache) { return None; }
        }
        let entry = u32::from_le_bytes([self.fat_cache[fat_index], self.fat_cache[fat_index+1],
            self.fat_cache[fat_index+2], self.fat_cache[fat_index+3]]) & 0x0FFF_FFFF;
        match entry {
            0 => None,
            n if n >= 0x0FFF_FFF7 => None,
            n => Some(n),
        }
    }

    pub fn find_file(&mut self, name: &[u8]) -> Option<(u32, u32)> {
        match self.fs_type {
            FsType::Fat32 => self.find_file_fat32(name),
            FsType::ExFat => self.find_file_exfat(name),
        }
    }

    /// Busca un archivo DENTRO de un directorio ya localizado.
    ///
    /// Existe porque `find_file` mira solo la raiz, y en un volumen de
    /// arranque real lo que interesa vive en `EFI/BOOT`. Encontrar el
    /// directorio y luego buscar el archivo en la raiz de todas formas es el
    /// error que se comio el primer intento.
    pub fn find_file_in(&mut self, name: &[u8], dir_cluster: u32) -> Option<(u32, u32)> {
        match self.fs_type {
            FsType::Fat32 => self.find_file_fat32_from(name, dir_cluster),
            FsType::ExFat => self.find_file_exfat(name),
        }
    }

    fn find_file_fat32(&mut self, name: &[u8]) -> Option<(u32, u32)> {
        let root = self.root_cluster;
        self.find_file_fat32_from(name, root)
    }

    fn find_file_fat32_from(&mut self, name: &[u8], start_cluster: u32) -> Option<(u32, u32)> {
        let mut cluster = start_cluster;
        let spc = self.sectors_per_cluster as u64;
        loop {
            let lba = self.cluster_to_lba(cluster);
            for s in 0..spc {
                unsafe {
                    if !self.read_sector(lba + s, Buf::buf) { continue; }
                }
                let entries = self.buf.as_ptr() as *const DirEntry;
                for i in 0..(512/32) {
                    let de = unsafe { &*entries.add(i) };
                    if de.name[0] == 0 { return None; }
                    if de.name[0] == 0xE5 { continue; }
                    if name_match(&de.name, name) {
                        let fc = (de.first_cluster_hi as u32) << 16 | de.first_cluster_lo as u32;
                        return Some((fc, de.file_size));
                    }
                }
            }
            cluster = match self.read_fat_entry(cluster) { Some(c) => c, None => return None };
        }
    }

    fn find_file_exfat(&mut self, name: &[u8]) -> Option<(u32, u32)> {
        let mut cluster = self.root_cluster;
        let spc = self.sectors_per_cluster as u64;
        let _entry_buf = [0u8; 32];
        loop {
            let lba = self.cluster_to_lba(cluster);
            for s in 0..spc {
                unsafe {
                    if !self.read_sector(lba + s, Buf::buf) { continue; }
                }
                // Scan 16 entries per 512-byte sector (each entry = 32 bytes)
                for i in 0..16 {
                    let entry_offset = i * 32;
                    let entry_type = self.buf[entry_offset];
                    if entry_type == 0x00 { return None; } // end of directory
                    if entry_type == 0x05 { continue; }   // deleted
                    if entry_type == 0x85 {
                        // File Entry — next entries are Stream + Filename
                        let file_entry = unsafe {
                            &*(self.buf[entry_offset..].as_ptr() as *const ExFatFileEntry)
                        };
                        let secondary_count = file_entry.secondary_count;
                        // Walk secondary entries in subsequent slots
                        for sec in 1..=secondary_count {
                            let sec_offset = entry_offset + (sec as usize) * 32;
                            if sec_offset + 32 > 512 { break; }
                            let sec_type = self.buf[sec_offset];
                            if sec_type == 0xC0 {
                                // Stream Extension — has first_cluster and name_length
                                let stream = unsafe {
                                    &*(self.buf[sec_offset..].as_ptr() as *const ExFatStreamEntry)
                                };
                                let first_cluster = stream.first_cluster;
                                let name_len = stream.name_length as usize;
                                let data_len = stream.valid_data_length as u32;
                                // Next entry should be Filename (0xC1)
                                if sec + 1 <= secondary_count {
                                    let name_offset = entry_offset + ((sec + 1) as usize) * 32;
                                    if name_offset + 32 <= 512 && self.buf[name_offset] == 0xC1 {
                                        let name_entry = unsafe {
                                            &*(self.buf[name_offset..].as_ptr() as *const ExFatNameEntry)
                                        };
                                        // Convert UTF-16LE name to 8.3 for comparison
                                        let mut fat_name = [0u8; 11];
                                        let mut pos = 0;
                                        for ci in 0..name_len.min(15) {
                                            let ch = name_entry.name_string[ci] as u8;
                                            if ch == b'.' {
                                                // Handle extension
                                                while pos < 8 { fat_name[pos] = b' '; pos += 1; }
                                                continue;
                                            }
                                            if pos < 11 {
                                                fat_name[pos] = ch.to_ascii_uppercase();
                                                pos += 1;
                                            }
                                        }
                                        while pos < 11 { fat_name[pos] = b' '; pos += 1; }
                                        if name_match(&fat_name, name) {
                                            return Some((first_cluster, data_len));
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

    pub fn read_file(&mut self, first_cluster: u32, file_size: u32, dst: &mut [u8]) -> usize {
        let mut cluster = first_cluster;
        let mut offset = 0;
        let spc = self.sectors_per_cluster as u64;
        while offset < file_size as usize && offset < dst.len() {
            let lba = self.cluster_to_lba(cluster);
            for s in 0..spc {
                if offset >= file_size as usize || offset >= dst.len() { break; }
                let start = offset;
                let end = (start + 512).min(file_size as usize).min(dst.len());
                let count = end - start;
                if count > 0 {
                    unsafe {
                        if self.read_sector(lba + s, Buf::buf) {
                            dst[start..start+count].copy_from_slice(&self.buf[..count]);
                        }
                    }
                }
                offset += count;
            }
            cluster = match self.read_fat_entry(cluster) { Some(c) => c, None => break };
        }
        offset
    }

    /// Find a free cluster in the FAT.
    /// Busca un cluster libre DENTRO de los que existen.
    ///
    /// El tope `max_cluster` no es cosmetico: sin el, el relleno a cero del
    /// final de la FAT se lee como espacio libre y se acaba escribiendo fuera
    /// del volumen. Ver la nota del campo.
    fn find_free_cluster(&mut self) -> Option<u32> {
        for sector in 0..self.fat_size_sectors {
            unsafe {
                if !self.read_sector((self.fat_start + sector) as u64, Buf::fat_cache) { continue; }
            }
            for i in 0..(512/4) {
                let cluster = sector * (512/4) as u32 + i as u32;
                if cluster < 2 { continue; }
                if cluster > self.max_cluster { return None; }
                let entry = u32::from_le_bytes([
                    self.fat_cache[i*4], self.fat_cache[i*4+1],
                    self.fat_cache[i*4+2], self.fat_cache[i*4+3],
                ]) & 0x0FFF_FFFF;
                if entry == 0 { return Some(cluster); }
            }
        }
        None
    }

    /// Lee la entrada de la FAT tal cual, sin interpretar. `read_fat_entry`
    /// traduce "0" y "fin de cadena" a `None`, que sirve para RECORRER una
    /// cadena pero no para saber si un cluster esta libre.
    fn raw_fat_entry(&mut self, cluster: u32) -> Option<u32> {
        let fat_offset = cluster * 4;
        let fat_sector = self.fat_start + (fat_offset / 512);
        let idx = (fat_offset % 512) as usize;
        unsafe {
            if !self.read_sector(fat_sector as u64, Buf::fat_cache) { return None; }
        }
        Some(u32::from_le_bytes([self.fat_cache[idx], self.fat_cache[idx+1],
            self.fat_cache[idx+2], self.fat_cache[idx+3]]) & 0x0FFF_FFFF)
    }

    /// Escribe una entrada de la FAT en TODAS las copias.
    ///
    /// Actualizar solo la primera deja el volumen incoherente: cualquier
    /// sistema que lea la segunda copia —o un chequeo de disco— vera una
    /// cadena distinta de la real.
    fn set_fat_entry(&mut self, cluster: u32, value: u32) -> bool {
        if cluster < 2 || cluster > self.max_cluster { return false; }
        let fat_offset = cluster * 4;
        let idx = (fat_offset % 512) as usize;
        let sectors_from_fat_start = fat_offset / 512;
        let v = value & 0x0FFF_FFFF;

        for copy in 0..self.num_fats as u32 {
            let fat_sector = self.fat_start + copy * self.fat_size_sectors + sectors_from_fat_start;
            unsafe {
                if !self.read_sector(fat_sector as u64, Buf::fat_cache) { return false; }
            }
            self.fat_cache[idx]   = v as u8;
            self.fat_cache[idx+1] = (v >> 8) as u8;
            self.fat_cache[idx+2] = (v >> 16) as u8;
            self.fat_cache[idx+3] = (v >> 24) as u8;
            unsafe {
                if !self.write_sector(fat_sector as u64, Buf::fat_cache) { return false; }
            }
        }
        true
    }

    /// Marca un cluster como fin de cadena en todas las copias de la FAT.
    fn mark_cluster_eoc(&mut self, cluster: u32) -> bool {
        self.set_fat_entry(cluster, 0x0FFF_FFFF)
    }

    /// Suelta una cadena de clusters entera. Se usa para deshacer una reserva
    /// a medias: si el disco se llena en mitad de un archivo, lo ya cogido se
    /// devuelve en vez de quedar perdido para siempre.
    fn free_chain(&mut self, first: u32) {
        let mut c = first;
        let mut guard = 0u32;
        while c >= 2 && c <= self.max_cluster {
            let next = self.raw_fat_entry(c).unwrap_or(0);
            if !self.set_fat_entry(c, 0) { return; }
            if next < 2 || next >= 0x0FFF_FFF7 { return; }
            c = next;
            // Una FAT corrupta puede tener un ciclo; no se gira para siempre.
            guard += 1;
            if guard > self.max_cluster { return; }
        }
    }

    /// Find a free directory entry in a directory (by first cluster).
    /// Returns (sector_lba, byte_offset_in_sector).
    fn find_free_dir_entry_in(&mut self, dir_cluster: u32) -> Option<(u64, usize)> {
        match self.fs_type {
            FsType::Fat32 => self.find_free_dir_entry_fat32(dir_cluster),
            FsType::ExFat => self.find_free_dir_entry_exfat(dir_cluster),
        }
    }

    fn find_free_dir_entry_fat32(&mut self, dir_cluster: u32) -> Option<(u64, usize)> {
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
                        if de.name[0] == 0 || de.name[0] == 0xE5 {
                            return Some((lba + s, i * 32));
                        }
                    }
                }
            }
            cluster = match self.read_fat_entry(cluster) { Some(c) => c, None => return None };
        }
    }

    /// exFAT: find 3 consecutive free entry slots for File + Stream + Filename
    fn find_free_dir_entry_exfat(&mut self, dir_cluster: u32) -> Option<(u64, usize)> {
        let mut cluster = dir_cluster;
        let spc = self.sectors_per_cluster as u64;
        loop {
            let lba = self.cluster_to_lba(cluster);
            for s in 0..spc {
                unsafe {
                    if !self.read_sector(lba + s, Buf::buf) { continue; }
                }
                // Need 3 consecutive free slots (File=0x85, Stream=0xC0, Name=0xC1)
                for i in 0..(512/32 - 2) {
                    let offset = i * 32;
                    let t0 = self.buf[offset];
                    let t1 = self.buf[offset + 32];
                    let t2 = self.buf[offset + 64];
                    if (t0 == 0x00 || t0 == 0x05) && (t1 == 0x00 || t1 == 0x05) && (t2 == 0x00 || t2 == 0x05) {
                        return Some((lba + s, offset));
                    }
                }
            }
            cluster = match self.read_fat_entry(cluster) { Some(c) => c, None => return None };
        }
    }

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

    /// Crea un archivo dentro de un directorio, dado su primer cluster.
    ///
    /// `name_8_3` son once bytes: ocho de nombre y tres de extension, rellenos
    /// con espacios. Es feo y es lo que hay, FAT lo guarda asi.
    ///
    /// Devuelve el MOTIVO cuando falla. La version anterior devolvia `bool` y
    /// ademas mentia: escribia como mucho UN cluster y apuntaba en el
    /// directorio el tamano completo, asi que cualquier archivo mas grande que
    /// un cluster quedaba registrado con un tamano que sus datos no
    /// respaldaban. Eso no es "incompleto", es un archivo corrupto que parece
    /// bueno hasta que alguien lo lee.
    pub fn create_file_in_dir(&mut self, dir_cluster: u32, name_8_3: &[u8; 11], data: &[u8])
        -> Result<(), WriteError>
    {
        if self.write.is_none() { return Err(WriteError::ReadOnly); }
        match self.fs_type {
            FsType::Fat32 => self.create_file_fat32(dir_cluster, name_8_3, data),
            // El creador de exFAT arrastra las mismas costuras que tenia el de
            // FAT32 y no se ha revisado contra la spec. Se dice, no se
            // disimula: BMO escribe FAT32 hoy.
            FsType::ExFat => Err(WriteError::Unsupported),
        }
    }

    fn create_file_fat32(&mut self, dir_cluster: u32, name_8_3: &[u8; 11], data: &[u8])
        -> Result<(), WriteError>
    {
        // Un nombre repetido deja dos entradas iguales en el directorio: la
        // segunda es inalcanzable y sus clusters, perdidos.
        if self.find_file_in(name_8_3, dir_cluster).is_some() {
            return Err(WriteError::Exists);
        }

        let spc = self.sectors_per_cluster as usize;
        if spc == 0 { return Err(WriteError::Io); }
        let cluster_bytes = spc * 512;
        let clusters_needed = if data.is_empty() { 1 } else { data.len().div_ceil(cluster_bytes) };

        // ── Reservar la CADENA entera, escribiendo a la vez ──
        //
        // Cada cluster se marca como fin de cadena en cuanto se coge: asi la
        // siguiente busqueda de hueco ya no lo ve libre y no se entrega dos
        // veces. Si algo falla a mitad, se suelta lo cogido — un archivo a
        // medias es un error; unos clusters marcados como ocupados que ya no
        // pertenecen a nadie son una fuga permanente.
        let first = match self.find_free_cluster() {
            Some(c) => c, None => return Err(WriteError::NoSpace),
        };
        if !self.mark_cluster_eoc(first) { return Err(WriteError::Io); }

        let mut prev = first;
        for i in 0..clusters_needed {
            let cluster = if i == 0 { first } else {
                let c = match self.find_free_cluster() {
                    Some(c) => c,
                    None => { self.free_chain(first); return Err(WriteError::NoSpace); }
                };
                if !self.mark_cluster_eoc(c) || !self.set_fat_entry(prev, c) {
                    self.free_chain(first);
                    return Err(WriteError::Io);
                }
                prev = c;
                c
            };

            let lba = self.cluster_to_lba(cluster);
            for s in 0..spc {
                // El buffer se reinicia a CEROS en cada sector. Reutilizar uno
                // sucio dejaba la cola del ultimo sector —y el resto del
                // cluster— llena de los datos anteriores, justo donde el
                // comentario prometia ceros.
                let mut temp = [0u8; 512];
                let off = i * cluster_bytes + s * 512;
                if off < data.len() {
                    let n = core::cmp::min(512, data.len() - off);
                    temp[..n].copy_from_slice(&data[off..off + n]);
                }
                if !self.write_from(lba + s as u64, &temp) {
                    self.free_chain(first);
                    return Err(WriteError::Io);
                }
            }
        }

        // ── La entrada de directorio, lo ultimo ──
        //
        // Se apunta cuando los datos YA estan en el disco. Al reves, un corte
        // entre ambos pasos dejaria un nombre visible apuntando a basura.
        let (dir_lba, dir_off) = match self.find_free_dir_entry_in(dir_cluster) {
            Some(v) => v,
            None => { self.free_chain(first); return Err(WriteError::DirFull); }
        };

        unsafe {
            if !self.read_sector(dir_lba, Buf::buf) {
                self.free_chain(first);
                return Err(WriteError::Io);
            }
        }
        let cluster = first;

        // Write directory entry
        let de = unsafe { &mut *(self.buf.as_mut_ptr().add(dir_off) as *mut DirEntry) };
        de.name = *name_8_3;
        de.attr = 0x20; // Archive
        de._nt_reserved = 0;
        de._create_time_tenth = 0;
        de.create_time = 0;
        de.create_date = 0;
        de.last_access = 0;
        de.first_cluster_hi = (cluster >> 16) as u16;
        de.write_time = 0;
        de.write_date = 0;
        de.first_cluster_lo = (cluster & 0xFFFF) as u16;
        de.file_size = data.len() as u32;

        let written = unsafe { self.write_sector(dir_lba, Buf::buf) };
        if !written {
            self.free_chain(first);
            return Err(WriteError::Io);
        }
        Ok(())
    }

    /// exFAT: create file with 3 entries: File(0x85) + Stream(0xC0) + Filename(0xC1)
    ///
    /// SIN CABLEAR: `create_file_in_dir` devuelve `Unsupported` para exFAT. Se
    /// conserva porque la estructura de las tres entradas es trabajo hecho y
    /// correcto, pero arrastra la misma limitacion de un solo cluster que se
    /// acaba de corregir en FAT32. Cablearlo = darle el mismo repaso.
    #[allow(dead_code)]
    fn create_file_exfat(&mut self, dir_cluster: u32, name_8_3: &[u8; 11], data: &[u8]) -> bool {
        let cluster = match self.find_free_cluster() {
            Some(c) => c, None => return false,
        };

        // Write data to cluster
        let lba = self.cluster_to_lba(cluster);
        let spc = self.sectors_per_cluster as u64;
        let total_sectors = (data.len() as u64 + 511) / 512;
        let write_n = total_sectors.min(spc);

        let mut temp = [0u8; 512];
        for s in 0..write_n {
            let off = (s * 512) as usize;
            let count = core::cmp::min(512, data.len().saturating_sub(off));
            temp[..count].copy_from_slice(&data[off..off + count]);
            unsafe {
                if !self.write_from(lba + s, &temp) { return false; }
            }
        }
        for s in write_n..spc {
            unsafe { let _ = self.write_from(lba + s, &temp); }
        }

        if !self.mark_cluster_eoc(cluster) { return false; }

        // Find 3 consecutive free slots
        let (dir_lba, dir_off) = match self.find_free_dir_entry_in(dir_cluster) {
            Some(v) => v, None => return false,
        };

        // Read directory sector
        unsafe {
            if !self.read_sector(dir_lba, Buf::buf) { return false; }
        }

        // Convert 8.3 name to UTF-16LE (up to 15 chars)
        let mut utf16_name = [0u16; 15];
        let mut name_len: usize = 0;
        for &b in name_8_3.iter() {
            if b == b' ' || b == 0 { break; }
            utf16_name[name_len] = b as u16;
            name_len += 1;
        }

        let _zero32 = [0u8; 32];

        // Entry 1: File Directory Entry (0x85)
        let file_entry = ExFatFileEntry {
            entry_type: 0x85,
            secondary_count: 2,
            set_checksum: 0,
            file_attributes: 0x20, // Archive
            _reserved1: 0,
            create_timestamp: 0,
            last_modified_timestamp: 0,
            last_accessed_timestamp: 0,
            _create_millis: 0,
            _last_modified_millis: 0,
            _create_utc_offset: 0,
            _last_modified_utc_offset: 0,
            _last_accessed_utc_offset: 0,
            _reserved2: [0; 7],
        };
        self.buf[dir_off..dir_off + 32].copy_from_slice(unsafe {
            core::slice::from_raw_parts(&file_entry as *const _ as *const u8, 32)
        });

        // Entry 2: Stream Extension Entry (0xC0)
        let stream_entry = ExFatStreamEntry {
            entry_type: 0xC0,
            general_secondary_flags: 0x01,
            _reserved1: 0,
            _reserved2: 0,
            name_length: name_len as u8,
            name_hash: 0,
            _reserved3: 0,
            valid_data_length: data.len() as u64,
            _reserved4: 0,
            first_cluster: cluster,
            data_length: data.len() as u64,
        };
        self.buf[dir_off + 32..dir_off + 64].copy_from_slice(unsafe {
            core::slice::from_raw_parts(&stream_entry as *const _ as *const u8, 32)
        });

        // Entry 3: Filename Entry (0xC1)
        let name_entry = ExFatNameEntry {
            entry_type: 0xC1,
            general_secondary_flags: 0x01,
            name_string: utf16_name,
        };
        self.buf[dir_off + 64..dir_off + 96].copy_from_slice(unsafe {
            core::slice::from_raw_parts(&name_entry as *const _ as *const u8, 32)
        });

        unsafe {
            self.write_sector(dir_lba, Buf::buf)
        }
    }
}

fn name_match(entry: &[u8; 11], query: &[u8]) -> bool {
    if query.len() > 11 { return false; }
    for i in 0..query.len() {
        let e = if i < 11 { entry[i].to_ascii_uppercase() } else { 0x20 };
        let qb = query[i].to_ascii_uppercase();
        if e != qb && !(qb == b' ' && e == 0x20) { return false; }
    }
    for i in query.len()..11 {
        if entry[i] != 0x20 && entry[i] != 0 { return false; }
    }
    true
}
