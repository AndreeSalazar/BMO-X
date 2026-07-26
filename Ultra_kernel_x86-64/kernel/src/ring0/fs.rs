//! El sistema de ficheros montado: de sectores a ARCHIVOS.
//!
//! Un sector es 512 bytes en una posición. Un archivo es un nombre, un tamaño
//! y una lista de bloques desperdigados. Entre las dos cosas está esta capa.
//!
//! ## Qué monta, y por qué esa partición
//!
//! `disk::scan_partitions` lee la GPT del disco. Aquí se elige la partición
//! **de arranque** (la ESP, tipo GUID C12A7328-…), que es donde vive el propio
//! `BOOTX64.EFI` con el que este kernel arrancó: el primer archivo que BMO-X
//! abre es él mismo. No se adivina "la primera FAT32 que aparezca": se pide
//! por TIPO, la misma lección que dejó el NVMe con el Windows del dueño.
//!
//! ## SOLO LECTURA, y es estructural
//!
//! `bmo-fat32` se monta sin `BlockWriter`. No es una política que alguien
//! deba acordarse de respetar: es que no hay función con la que escribir.
//! Escribir espera al gate de identidad (comparar modelo y serie del disco
//! antes de tocarlo), que es el paso 2 del plan.

use bmo_fat32::{FatVolume, FsType};
use crate::ring0::dev::disk;

static mut VOLUME: Option<FatVolume> = None;
static mut MOUNTED_LBA: u64 = 0;

/// ¿Hay un volumen montado?
pub fn is_mounted() -> bool { unsafe { (*core::ptr::addr_of!(VOLUME)).is_some() } }

/// Primer LBA de la partición montada (0 = ninguna).
pub fn mounted_lba() -> u64 { unsafe { MOUNTED_LBA } }

/// Tipo del volumen montado.
pub fn fs_name() -> &'static str {
    unsafe {
        match (*core::ptr::addr_of!(VOLUME)).as_ref() {
            Some(v) => match v.fs_type { FsType::Fat32 => "FAT32", FsType::ExFat => "exFAT" },
            None => "-",
        }
    }
}

/// Monta la partición de arranque del disco. Se llama una vez, tras
/// `disk::scan_partitions`.
pub fn mount() {
    if !disk::is_ready() {
        crate::ring0::cabina::warn("fs", "sin disco: no hay nada que montar", 0);
        return;
    }
    // La ESP: donde el firmware encontró BOOTX64.EFI, o sea donde vive BMO-X.
    let esp = disk::partitions().iter().find(|p| p.is_esp()).copied();
    let part = match esp {
        Some(p) => p,
        None => {
            crate::ring0::cabina::warn("fs", "el disco no tiene particion de arranque (ESP)", 0);
            return;
        }
    };

    // Sin writer: solo lectura por construcción (ver la nota de cabecera).
    match bmo_fat32::mount(disk::block_read, None, part.first_lba) {
        Some(v) => {
            unsafe {
                core::ptr::write(core::ptr::addr_of_mut!(VOLUME), Some(v));
                MOUNTED_LBA = part.first_lba;
            }
            crate::ring0::cabina::info("fs", fs_name(), part.first_lba);
        }
        None => {
            // Distinguir "no hay FAT ahí" de "el disco no contesta" importa:
            // el LBA dice dónde se miró.
            crate::ring0::cabina::fault("fs", "la particion no tiene un FAT reconocible", part.first_lba);
        }
    }
}

/// Busca un archivo en la RAÍZ del volumen. Devuelve `(primer_cluster, bytes)`.
///
/// El nombre va en formato 8.3 tal como está en disco: `"BOOTX64 EFI"` son
/// once bytes, ocho de nombre y tres de extensión, rellenos con espacios. Es
/// feo y es lo que hay: FAT lo guarda así.
pub fn find(name: &[u8]) -> Option<(u32, u32)> {
    unsafe {
        let v = (*core::ptr::addr_of_mut!(VOLUME)).as_mut()?;
        v.find_file(name)
    }
}

/// Busca un subdirectorio de la raíz y devuelve su cluster.
pub fn find_dir(name: &[u8]) -> Option<u32> {
    unsafe {
        let v = (*core::ptr::addr_of_mut!(VOLUME)).as_mut()?;
        v.find_subdir(name)
    }
}

/// Busca dentro de un directorio ya localizado.
pub fn find_dir_in(name: &[u8], dir_cluster: u32) -> Option<u32> {
    unsafe {
        let v = (*core::ptr::addr_of_mut!(VOLUME)).as_mut()?;
        v.find_subdir_in(name, dir_cluster)
    }
}

/// Lee el contenido de un archivo en `dst`. Devuelve los bytes leídos.
pub fn read(first_cluster: u32, size: u32, dst: &mut [u8]) -> usize {
    unsafe {
        let v = match (*core::ptr::addr_of_mut!(VOLUME)).as_mut() { Some(v) => v, None => return 0 };
        v.read_file(first_cluster, size, dst)
    }
}
