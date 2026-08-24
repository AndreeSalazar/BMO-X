//! **LA FORMA DE UN FAT32 Y DE UN exFAT EN EL DISCO.** Structs y nada mas.
//!
//! ## Por que soy un fichero y no un trozo del de al lado (L6b)
//!
//! Porque contesto una pregunta distinta que `lib.rs`:
//!
//! ```text
//!    forma    QUE es un BPB, una entrada de directorio, un stream
//!             -> tipos, y CERO decisiones
//!    lib.rs   COMO se monta un volumen y se sigue una cadena
//!             -> lee el disco y decide
//! ```
//!
//! Es el mismo corte que separa `ir/forma.rs` de `ir/descenso.rs` en INTI, y
//! tiene la misma consecuencia util: **esto lo mira todo el mundo --montar,
//! buscar, escribir, las pruebas-- y no pasa nada**, porque un fichero que solo
//! define una forma no puede colar una decision dentro de quien lo lee.
//!
//! [!] Y estos bytes **los escribio otro sistema**. No son una estructura que
//! este proyecto eligio: son el formato que Microsoft publico y que el Windows
//! de esta misma maquina esta usando ahora mismo. Cambiar un campo aqui no es
//! refactorizar -- es dejar de poder leer el disco.
//!
//! ** El reparto es MOVER TEXTO (L6d): ni una linea cambia de contenido.

/// exFAT BIOS Parameter Block at sector 0, offset 0.
/// exFAT has a different layout than FAT32 -- see exFAT spec section 3.1.
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
    pub _root_entries: u16,
    pub _total_sectors16: u16,
    pub media: u8,
    pub _fat_size16: u16,
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
    pub _reserved: [u8; 12],
    pub drive_number: u8,
    pub _reserved1: u8,
    pub boot_sig: u8,
    pub volume_id: u32,
    pub volume_label: [u8; 11],
    pub fs_type: [u8; 8],
}

#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
// [!] LOS CAMPOS `_algo` TAMBIEN SON `pub`, Y NO ES DESCUIDO.
//
// ** El guion bajo dice *"este campo existe porque el formato lo tiene, no
// porque lo usemos"*. Mientras todo vivia en un fichero, ser privados bastaba;
// al mudarse la forma a su propio modulo, `escribir.rs` dejo de alcanzarlos --
// y los pone a cero A PROPOSITO, porque una entrada de directorio con basura en
// los campos que no usamos es una entrada que otro sistema puede leer mal.
//
// Es la unica linea que este reparto no pudo dejar igual, y se dice en vez de
// cambiarla callando: L6d exige que un reparto sea texto movido, y lo que no lo
// es tiene que verse.
pub struct DirEntry {
    pub name: [u8; 11],
    pub attr: u8,
    pub _nt_reserved: u8,
    pub _create_time_tenth: u8,
    pub create_time: u16,
    pub create_date: u16,
    pub last_access: u16,
    pub first_cluster_hi: u16,
    pub write_time: u16,
    pub write_date: u16,
    pub first_cluster_lo: u16,
    pub file_size: u32,
}

/// Una entrada de directorio YA LOCALIZADA: donde estan sus 32 bytes en el
/// disco, ademas de lo que dicen.
///
/// No es un `DirEntry`: aquel son los bytes del formato, este es *el sitio*.
/// La diferencia importa al reemplazar -- para apuntar un nombre a otra cadena
/// hay que reescribir el sector donde vive, y eso solo se sabe habiendolo
/// encontrado.
#[derive(Debug, Clone, Copy)]
pub struct EntradaDir {
    /// LBA relativo a la particion del sector que la contiene.
    pub lba: u64,
    /// Byte de esa entrada dentro del sector. Siempre multiplo de 32.
    pub offset: usize,
    pub first_cluster: u32,
    pub size: u32,
}

/// exFAT File Directory Entry (type 0x85)
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct ExFatFileEntry {
    pub entry_type: u8,      // 0x85
    pub secondary_count: u8,
    pub set_checksum: u16,
    pub file_attributes: u16,
    pub _reserved1: u16,
    pub create_timestamp: u32,
    pub last_modified_timestamp: u32,
    pub last_accessed_timestamp: u32,
    pub _create_millis: u8,
    pub _last_modified_millis: u8,
    pub _create_utc_offset: u8,
    pub _last_modified_utc_offset: u8,
    pub _last_accessed_utc_offset: u8,
    pub _reserved2: [u8; 7],
}

/// exFAT Stream Extension Entry (type 0xC0) -- follows File Entry
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct ExFatStreamEntry {
    pub entry_type: u8,      // 0xC0
    pub general_secondary_flags: u8,
    pub _reserved1: u8,
    pub _reserved2: u8,
    pub name_length: u8,
    pub name_hash: u16,
    pub _reserved3: u16,
    pub valid_data_length: u64,
    pub _reserved4: u32,
    pub first_cluster: u32,
    pub data_length: u64,
}

/// exFAT Filename Entry (type 0xC1) -- follows Stream Entry
/// Contains up to 15 UTF-16 characters of the filename
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct ExFatNameEntry {
    pub entry_type: u8,      // 0xC1
    pub general_secondary_flags: u8,
    pub name_string: [u16; 15],  // UTF-16LE filename (up to 15 chars)
}
