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
//! ## Dos volúmenes, y solo uno se puede escribir
//!
//! - **ARRANQUE** (la ESP): se monta SIN `BlockWriter`. Su inmutabilidad no es
//!   una política que alguien deba acordarse de respetar: es que no existe la
//!   función con la que escribir. Ahí vive el `BOOTX64.EFI` con el que arrancó
//!   esta misma ejecución.
//! - **DATOS** (la primera partición que no es la de arranque): se monta con
//!   escritor, y solo si el gate de identidad del disco la ha armado. Es donde
//!   BMO deja lo suyo — empezando por la caja negra de CABINA.
//!
//! Separarlos es lo que permite que un bug del sistema de ficheros cueste un
//! archivo y no la capacidad de arrancar la máquina.

use bmo_fat32::{FatVolume, FsType, WriteError};
use crate::ring0::dev::disk;

static mut VOLUME: Option<FatVolume> = None;
static mut MOUNTED_LBA: u64 = 0;
static mut DATA_VOLUME: Option<FatVolume> = None;
static mut DATA_LBA: u64 = 0;

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

/// Busca un archivo DENTRO de un directorio ya localizado.
pub fn find_in(name: &[u8], dir_cluster: u32) -> Option<(u32, u32)> {
    unsafe {
        let v = (*core::ptr::addr_of_mut!(VOLUME)).as_mut()?;
        v.find_file_in(name, dir_cluster)
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

// ── El volumen de DATOS: el único que se puede escribir ─────────────────────

/// ¿Hay volumen de datos montado para escribir?
pub fn data_mounted() -> bool { unsafe { (*core::ptr::addr_of!(DATA_VOLUME)).is_some() } }

/// Primer LBA del volumen de datos (0 = ninguno).
pub fn data_lba() -> u64 { unsafe { DATA_LBA } }

/// Monta la partición de datos CON escritor.
///
/// Se llama después de `disk::verify_identity()`. Si el gate no armó la
/// escritura, aquí no se monta nada: pasar un escritor que va a rechazar todo
/// sería montar un volumen que miente sobre lo que puede hacer.
pub fn mount_data() {
    if !disk::write_armed() {
        crate::ring0::cabina::warn("fs", "sin volumen de datos: el gate no armo la escritura", 0);
        return;
    }
    let part = match disk::data_partition() {
        Some(p) => p,
        None => {
            crate::ring0::cabina::warn("fs", "el disco no tiene particion de datos", 0);
            return;
        }
    };
    match bmo_fat32::mount(disk::block_read, Some(disk::block_write), part.first_lba) {
        Some(v) => {
            unsafe {
                core::ptr::write(core::ptr::addr_of_mut!(DATA_VOLUME), Some(v));
                DATA_LBA = part.first_lba;
            }
            crate::ring0::cabina::info("fs", "volumen de datos montado para ESCRITURA", part.first_lba);
        }
        None => {
            // BMO-DATA sigue en NTFS y este driver no lo entiende: es un "no",
            // no un fallo. Decirlo evita buscar un bug donde solo hay un
            // formato ajeno.
            crate::ring0::cabina::warn("fs", "la particion de datos no es FAT32/exFAT", part.first_lba);
        }
    }
}

/// Crea un archivo en la raíz del volumen de datos.
///
/// `name_8_3` son once bytes tal como FAT los guarda: `b"CABINA  LOG"`.
/// Devuelve el motivo cuando falla — el disco lleno, un nombre repetido y un
/// volumen de solo lectura son tres problemas distintos y se distinguen.
pub fn create(name_8_3: &[u8; 11], data: &[u8]) -> Result<(), WriteError> {
    let v = unsafe {
        match (*core::ptr::addr_of_mut!(DATA_VOLUME)).as_mut() {
            Some(v) => v,
            None => return Err(WriteError::ReadOnly),
        }
    };
    let root = v.root_cluster();
    let r = v.create_file_in_dir(root, name_8_3, data);
    if r.is_ok() {
        // El punto de no retorno: hasta que el disco vacíe su caché, lo escrito
        // vive en un chip que un corte se lleva por delante.
        disk::flush();
    }
    r
}
