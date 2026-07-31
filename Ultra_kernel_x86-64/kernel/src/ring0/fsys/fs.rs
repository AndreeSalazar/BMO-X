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

/// La entrada `n` de un directorio del volumen de DATOS: `(nombre 8.3,
/// es_dir, tamano)`. `None` cuando se acaban.
///
/// Es lo que faltaba para que Ring 3 pueda PREGUNTAR QUE HAY en vez de tener
/// que saberse los nombres de memoria. Ver `ring0/directorio.rs`.
pub fn entrada_datos(dir_cluster: u32, n: usize) -> Option<([u8; 11], bool, u32)> {
    unsafe {
        let v = (*core::ptr::addr_of_mut!(DATA_VOLUME)).as_mut()?;
        v.entry_at(dir_cluster, n)
    }
}

/// Resuelve una ruta de DIRECTORIO a su cluster, en el volumen de datos.
/// Ruta vacia = la raiz.
pub fn dir_datos(ruta: &str) -> Option<u32> {
    unsafe {
        let v = (*core::ptr::addr_of_mut!(DATA_VOLUME)).as_mut()?;
        let mut cluster = v.root_cluster();
        let mut resto = ruta.trim();
        if resto.len() >= 2 && resto.as_bytes()[1] == b':' { resto = &resto[2..]; }
        while resto.starts_with('/') || resto.starts_with('\\') { resto = &resto[1..]; }
        while !resto.is_empty() {
            let corte = resto.find(['/', '\\']).unwrap_or(resto.len());
            let (comp, rest) = resto.split_at(corte);
            if !comp.is_empty() {
                let nombre = nombre_8_3(comp)?;
                cluster = v.find_subdir_in(&nombre, cluster)?;
            }
            resto = rest;
            while resto.starts_with('/') || resto.starts_with('\\') { resto = &resto[1..]; }
        }
        Some(cluster)
    }
}

/// La misma conversión, para quien está fuera de este módulo.
///
/// La necesita `archivo::crear`, que tiene que validar el nombre ANTES de
/// aceptar un archivo de escritura: descubrir al final que no era un 8.3
/// válido significaría haber dejado a un programa acumulando bytes para nada.
pub fn nombre_8_3_pub(s: &str) -> Option<[u8; 11]> {
    nombre_8_3(s)
}

/// Nombre a 8.3 crudo (11 bytes, relleno con espacios). `None` si no cabe —
/// **nunca se recorta**: un nombre recortado en silencio abre otra cosa.
fn nombre_8_3(s: &str) -> Option<[u8; 11]> {
    let b = s.as_bytes();
    let punto = b.iter().rposition(|&c| c == b'.');
    let (tallo, ext) = match punto {
        Some(i) => (&b[..i], &b[i + 1..]),
        None => (&b[..], &b[0..0]),
    };
    if tallo.is_empty() || tallo.len() > 8 || ext.len() > 3 {
        return None;
    }
    let mut out = [b' '; 11];
    for (i, &c) in tallo.iter().enumerate() {
        out[i] = if c.is_ascii_lowercase() { c - 32 } else { c };
    }
    for (i, &c) in ext.iter().enumerate() {
        out[8 + i] = if c.is_ascii_lowercase() { c - 32 } else { c };
    }
    Some(out)
}

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

// ── Rutas: de "c/holac.bex" a un archivo ──────────────────────────────────

/// Por qué no se pudo cargar una ruta.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoadError {
    /// No hay volumen de datos montado.
    NoVolume,
    /// La ruta está vacía o tiene un componente vacío.
    BadPath,
    /// Un nombre no cabe en 8.3 (ocho de nombre, tres de extensión).
    NameTooLong,
    /// No existe un directorio del camino.
    DirNotFound,
    /// El archivo no está.
    NotFound,
    /// El archivo no cabe en el buffer del llamante.
    TooBig,
}

impl LoadError {
    pub fn name(self) -> &'static str {
        match self {
            LoadError::NoVolume => "no hay volumen de datos montado",
            LoadError::BadPath => "ruta vacia o mal formada",
            LoadError::NameTooLong => "un nombre no cabe en 8.3",
            LoadError::DirNotFound => "no existe una carpeta del camino",
            LoadError::NotFound => "el archivo no esta",
            LoadError::TooBig => "el archivo no cabe en el buffer",
        }
    }
}

/// Convierte `hola.bex` en los once bytes que FAT guarda: `"HOLA    BEX"`.
///
/// Devuelve error si no cabe en vez de recortar. Un nombre recortado en
/// silencio abre otro archivo — y "abrir otro archivo" en un cargador de
/// programas significa ejecutar otro binario.
fn to_8_3(name: &str) -> Result<[u8; 11], LoadError> {
    let b = name.as_bytes();
    if b.is_empty() { return Err(LoadError::BadPath); }
    let mut out = [b' '; 11];
    // El punto separa; el ÚLTIMO punto, porque "a.b.c" tiene extensión "c".
    let dot = b.iter().rposition(|&c| c == b'.');
    let (stem, ext) = match dot {
        Some(i) => (&b[..i], &b[i + 1..]),
        None => (&b[..], &b[0..0]),
    };
    if stem.is_empty() || stem.len() > 8 || ext.len() > 3 {
        return Err(LoadError::NameTooLong);
    }
    for (i, &c) in stem.iter().enumerate() { out[i] = upper(c); }
    for (i, &c) in ext.iter().enumerate() { out[8 + i] = upper(c); }
    Ok(out)
}

fn upper(c: u8) -> u8 {
    if c >= b'a' && c <= b'z' { c - 32 } else { c }
}

/// Carga un archivo del volumen de DATOS en `dst`. Devuelve los bytes leídos.
///
/// Acepta `c/holac.bex`, `/c/holac.bex` y `A:/c/holac.bex` — la letra
/// es la que Windows le da a esta misma partición, y escribirla es lo que
/// hace cualquiera que acabe de copiar ahí el archivo desde el anfitrión.
/// También se aceptan barras invertidas.
///
/// Es la pieza que saca los programas de dentro del kernel. Hasta ahora los
/// `.bex` viajaban con `include_bytes!` y cambiar un "hola mundo" obligaba a
/// recompilar el sistema operativo entero y reflashear.
pub fn load(path: &str, dst: &mut [u8]) -> Result<usize, LoadError> {
    let (cluster, size) = resolver(path)?;
    if size as usize > dst.len() {
        return Err(LoadError::TooBig);
    }
    let v = unsafe {
        match (*core::ptr::addr_of_mut!(DATA_VOLUME)).as_mut() {
            Some(v) => v,
            None => return Err(LoadError::NoVolume),
        }
    };
    Ok(v.read_file(cluster, size, dst))
}

/// Cuántos bytes mide el archivo, SIN leerlo.
///
/// Existe para poder reservar el buffer del tamaño justo antes de traerlo:
/// hasta ahora un archivo abierto se copiaba a una fila estática de 4 KiB, y
/// ese número era el techo de lo que un programa podía leer. Preguntar primero
/// convierte el techo en "lo que quepa en la RAM".
pub fn tamano(path: &str) -> Result<u32, LoadError> {
    resolver(path).map(|(_, size)| size)
}

/// Recorre la ruta y devuelve `(cluster, tamaño)` del archivo.
///
/// Lo comparten `load` y `tamano` a propósito: dos copias del recorrido de
/// directorios es la forma clásica de que una acepte una ruta que la otra
/// rechaza.
fn resolver(path: &str) -> Result<(u32, u32), LoadError> {
    let v = unsafe {
        match (*core::ptr::addr_of_mut!(DATA_VOLUME)).as_mut() {
            Some(v) => v,
            None => return Err(LoadError::NoVolume),
        }
    };

    // Quitar la letra de unidad y las barras iniciales.
    let mut p = path;
    if p.len() >= 2 && p.as_bytes()[1] == b':' { p = &p[2..]; }
    while p.starts_with('/') || p.starts_with('\\') { p = &p[1..]; }
    if p.is_empty() { return Err(LoadError::BadPath); }

    // Bajar por los directorios; el último componente es el archivo.
    let mut dir = v.root_cluster();
    let mut rest = p;
    loop {
        let cut = rest.as_bytes().iter().position(|&c| c == b'/' || c == b'\\');
        match cut {
            Some(i) => {
                let comp = &rest[..i];
                if comp.is_empty() { return Err(LoadError::BadPath); }
                let name = to_8_3(comp)?;
                dir = v.find_subdir_in(&name, dir).ok_or(LoadError::DirNotFound)?;
                rest = &rest[i + 1..];
                if rest.is_empty() { return Err(LoadError::BadPath); }
            }
            None => break,
        }
    }

    let name = to_8_3(rest)?;
    v.find_file_in(&name, dir).ok_or(LoadError::NotFound)
}

/// Crea un archivo en la raíz del volumen de datos.
///
/// `name_8_3` son once bytes tal como FAT los guarda: `b"CABINA  LOG"`.
/// Devuelve el motivo cuando falla — el disco lleno, un nombre repetido y un
/// volumen de solo lectura son tres problemas distintos y se distinguen.
pub fn create(name_8_3: &[u8; 11], data: &[u8]) -> Result<(), WriteError> {
    let root = unsafe {
        match (*core::ptr::addr_of_mut!(DATA_VOLUME)).as_mut() {
            Some(v) => v.root_cluster(),
            None => return Err(WriteError::ReadOnly),
        }
    };
    crear_en(root, name_8_3, data)
}

/// Crea un archivo en un directorio CONCRETO del volumen de datos.
///
/// `create` es esto con la raíz. Se separan porque un programa de Ring 3
/// escribe donde le dijeron —`datos/movim.dat`—, y obligarle a dejarlo todo en
/// la raíz convertiría el volumen en un cajón: es justo lo que hace ilegible un
/// disco a los seis meses.
///
/// El `dir_cluster` sale de `dir_datos`, que ya recorrió la ruta. Aquí no se
/// vuelve a interpretar texto: quien llama trae el directorio resuelto.
pub fn crear_en(dir_cluster: u32, name_8_3: &[u8; 11], data: &[u8]) -> Result<(), WriteError> {
    let v = unsafe {
        match (*core::ptr::addr_of_mut!(DATA_VOLUME)).as_mut() {
            Some(v) => v,
            None => return Err(WriteError::ReadOnly),
        }
    };
    let r = v.create_file_in_dir(dir_cluster, name_8_3, data);
    if r.is_ok() {
        // El punto de no retorno: hasta que el disco vacíe su caché, lo escrito
        // vive en un chip que un corte se lleva por delante.
        disk::flush();
    }
    r
}
