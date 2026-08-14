//! El sistema de ficheros montado: de sectores a ARCHIVOS.
//!
//! Un sector es 512 bytes en una posicion. Un archivo es un nombre, un tamano
//! y una lista de bloques desperdigados. Entre las dos cosas esta esta capa.
//!
//! ## Que monta, y por que esa particion
//!
//! `disk::scan_partitions` lee la GPT del disco. Aqui se elige la particion
//! **de arranque** (la ESP, tipo GUID C12A7328-...), que es donde vive el propio
//! `BOOTX64.EFI` con el que este kernel arranco: el primer archivo que BMO-X
//! abre es el mismo. No se adivina "la primera FAT32 que aparezca": se pide
//! por TIPO, la misma leccion que dejo el NVMe con el Windows del dueno.
//!
//! ## Dos volumenes, y solo uno se puede escribir
//!
//! - **ARRANQUE** (la ESP): se monta SIN `BlockWriter`. Su inmutabilidad no es
//!   una politica que alguien deba acordarse de respetar: es que no existe la
//!   funcion con la que escribir. Ahi vive el `BOOTX64.EFI` con el que arranco
//!   esta misma ejecucion.
//! - **DATOS** (la primera particion que no es la de arranque): se monta con
//!   escritor, y solo si el gate de identidad del disco la ha armado. Es donde
//!   BMO deja lo suyo -- empezando por la caja negra de CABINA.
//!
//! Separarlos es lo que permite que un bug del sistema de ficheros cueste un
//! archivo y no la capacidad de arrancar la maquina.

use bmo_fat32::{FatVolume, FsType, WriteError};
use crate::ring0::dev::disk;

static mut VOLUME: Option<FatVolume> = None;
static mut MOUNTED_LBA: u64 = 0;
static mut DATA_VOLUME: Option<FatVolume> = None;
static mut DATA_LBA: u64 = 0;

/// Hay un volumen montado?
pub fn is_mounted() -> bool { unsafe { (*core::ptr::addr_of!(VOLUME)).is_some() } }

/// Primer LBA de la particion montada (0 = ninguna).
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

/// Monta la particion de arranque del disco. Se llama una vez, tras
/// `disk::scan_partitions`.
pub fn mount() {
    if !disk::is_ready() {
        crate::ring0::cabina::warn("fs", "sin disco: no hay nada que montar", 0);
        return;
    }
    // La ESP: donde el firmware encontro BOOTX64.EFI, o sea donde vive BMO-X.
    let esp = disk::partitions().iter().find(|p| p.is_esp()).copied();
    let part = match esp {
        Some(p) => p,
        None => {
            crate::ring0::cabina::warn("fs", "el disco no tiene particion de arranque (ESP)", 0);
            return;
        }
    };

    // Sin writer: solo lectura por construccion (ver la nota de cabecera).
    match bmo_fat32::mount(disk::block_read, None, part.first_lba) {
        Some(v) => {
            unsafe {
                core::ptr::write(core::ptr::addr_of_mut!(VOLUME), Some(v));
                MOUNTED_LBA = part.first_lba;
            }
            crate::ring0::cabina::info("fs", fs_name(), part.first_lba);
        }
        None => {
            // Distinguir "no hay FAT ahi" de "el disco no contesta" importa:
            // el LBA dice donde se miro.
            crate::ring0::cabina::fault("fs", "la particion no tiene un FAT reconocible", part.first_lba);
        }
    }
}

/// Busca un archivo en la RAIZ del volumen. Devuelve `(primer_cluster, bytes)`.
///
/// El nombre va en formato 8.3 tal como esta en disco: `"BOOTX64 EFI"` son
/// once bytes, ocho de nombre y tres de extension, rellenos con espacios. Es
/// feo y es lo que hay: FAT lo guarda asi.
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

/// Busca un subdirectorio de la raiz y devuelve su cluster.
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

/// Lee el contenido de un archivo en `dst`. Devuelve los bytes leidos.
pub fn read(first_cluster: u32, size: u32, dst: &mut [u8]) -> usize {
    unsafe {
        let v = match (*core::ptr::addr_of_mut!(VOLUME)).as_mut() { Some(v) => v, None => return 0 };
        v.read_file(first_cluster, size, dst)
    }
}

// -- El volumen de DATOS: el unico que se puede escribir ---------------------

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
                let name = nombre_8_3(comp)?;
                cluster = v.find_subdir_in(&name, cluster)?;
            }
            resto = rest;
            while resto.starts_with('/') || resto.starts_with('\\') { resto = &resto[1..]; }
        }
        Some(cluster)
    }
}

/// La misma conversion, para quien esta fuera de este modulo.
///
/// La necesita `file::create`, que tiene que validar el nombre ANTES de
/// aceptar un archivo de escritura: descubrir al final que no era un 8.3
/// valido significaria haber dejado a un programa acumulando bytes para nada.
pub fn nombre_8_3_pub(s: &str) -> Option<[u8; 11]> {
    nombre_8_3(s)
}

/// Nombre a 8.3 crudo (11 bytes, relleno con espacios). `None` si no cabe --
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

/// Hay volumen de datos montado para escribir?
pub fn data_mounted() -> bool { unsafe { (*core::ptr::addr_of!(DATA_VOLUME)).is_some() } }

/// Primer LBA del volumen de datos (0 = ninguno).
pub fn data_lba() -> u64 { unsafe { DATA_LBA } }

/// Monta la particion de datos CON escritor.
///
/// Se llama despues de `disk::verify_identity()`. Si el gate no armo la
/// escritura, aqui no se monta nada: pasar un escritor que va a rechazar todo
/// seria montar un volumen que miente sobre lo que puede hacer.
pub fn mount_data() {
    if !disk::write_armed() {
        crate::ring0::cabina::warn("fs", "sin volumen de datos: el gate no armo la escritura", 0);
        return;
    }
    // ** SE PRUEBAN TODAS, Y GANA LA QUE CONTESTA (2026-08-11).
    //
    // === El bug que esto mata ===
    //
    // Aqui se pedia `disk::data_partition()`, que era **la primera que no es la
    // de arranque**. Una sola candidata, elegida por su posicion en la GPT.
    //
    // El disco de esta maquina tiene tres particiones que no son la de arranque:
    // la FAT32 donde viven los programas y la de ESTRATOS. Cual salia "primera"
    // dependia del orden en la tabla -- o sea de una moneda al aire. Y cuando
    // salia la que no era, el sistema montaba algo que no es FAT32, encontraba
    // entradas de directorio que parecian nombres, y leia sectores que no
    // contenian nada suyo. Exactamente lo que se vio el 2026-08-11:
    //
    // ```text
    //   FAULT proc: los 8 primeros bytes de lo que llego =C75FF8548102474
    // ```
    //
    // Codigo x86-64 que no esta en NINGUNO de los 7.610 ficheros del proyecto.
    //
    // === La regla, que es la de la casa ===
    //
    // > **Por lo que ES, nunca por donde esta.**
    //
    // Ya estaba escrita en la cabecera de `dev/disk.rs` --se aprendio eligiendo
    // el CONTROLADOR por tipo y no por orden-- y se seguia incumpliendo una capa
    // mas abajo.
    //
    // Y la prueba de "que es" no es un GUID que haya que mantener al dia:
    // **montarla**. Un volumen que monta como FAT32 ES FAT32. ESTRATOS no puede
    // colarse porque no contesta a esa pregunta.
    //
    // Se dice cada intento, incluidos los que fallan: saber que la particion 1
    // se probo y no era es lo que evita buscar un bug donde solo hay un formato
    // ajeno.
    let mut probadas = 0u64;
    for part in disk::partitions().iter() {
        if part.is_esp() {
            continue;
        }
        probadas += 1;
        match bmo_fat32::mount(disk::block_read, Some(disk::block_write), part.first_lba) {
            Some(v) => {
                unsafe {
                    core::ptr::write(core::ptr::addr_of_mut!(DATA_VOLUME), Some(v));
                    DATA_LBA = part.first_lba;
                }
                // Que la ventana de escritura hable de la MISMA particion que se
                // acaba de montar. Mientras cada uno la elegia por su cuenta,
                // coincidian por casualidad.
                disk::fijar_particion_datos(*part);
                crate::ring0::cabina::info("fs", "volumen de datos montado para ESCRITURA", part.first_lba);
                crate::ring0::cabina::info("fs", "y es la particion", part.index as u64);
                return;
            }
            None => {
                // No es un fallo: es un "no". ESTRATOS y NTFS entran por aqui, y
                // decirlo evita buscar un bug donde solo hay otro formato.
                crate::ring0::cabina::info("fs", "esa particion no es FAT32, se salta", part.first_lba);
            }
        }
    }
    crate::ring0::cabina::warn("fs", "NINGUNA particion contesta como FAT32; probadas", probadas);
}

// -- Rutas: de "c/holac.bex" a un archivo ----------------------------------

/// Por que no se pudo cargar una ruta.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoadError {
    /// No hay volumen de datos montado.
    NoVolume,
    /// La ruta esta vacia o tiene un componente vacio.
    BadPath,
    /// Un nombre no cabe en 8.3 (ocho de nombre, tres de extension).
    NameTooLong,
    /// No existe un directorio del camino.
    DirNotFound,
    /// El archivo no esta.
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
/// silencio abre otro archivo -- y "abrir otro archivo" en un cargador de
/// programas significa ejecutar otro binario.
fn to_8_3(name: &str) -> Result<[u8; 11], LoadError> {
    let b = name.as_bytes();
    if b.is_empty() { return Err(LoadError::BadPath); }
    let mut out = [b' '; 11];
    // El punto separa; el ULTIMO punto, porque "a.b.c" tiene extension "c".
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

/// Carga un archivo del volumen de DATOS en `dst`. Devuelve los bytes leidos.
///
/// Acepta `c/holac.bex`, `/c/holac.bex` y `A:/c/holac.bex` -- la letra
/// es la que Windows le da a esta misma particion, y escribirla es lo que
/// hace cualquiera que acabe de copiar ahi el archivo desde el anfitrion.
/// Tambien se aceptan barras invertidas.
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

/// **Abre un archivo para leerlo por RANGOS.** Devuelve `(cursor, tamano)`.
///
/// Es lo que sostiene la pieza B: el cargador ya no trae el fichero a una mesa,
/// pide **el rango de cada seccion** y lo deja caer en los marcos del proceso.
/// Para eso hace falta un cursor que sobreviva entre peticiones -- ver
/// `bmo_fat32::Cursor` para por que uno y no un offset suelto.
pub fn abrir_rangos(path: &str) -> Result<(bmo_fat32::Cursor, u32), LoadError> {
    let (cluster, size) = resolver(path)?;
    let v = unsafe {
        match (*core::ptr::addr_of_mut!(DATA_VOLUME)).as_mut() {
            Some(v) => v,
            None => return Err(LoadError::NoVolume),
        }
    };
    Ok((v.cursor(cluster), size))
}

/// **De donde va a leer.** `(cluster, LBA absoluto, primer LBA de la particion)`.
///
/// Existe para que un fallo de lectura pueda decir **de que sector** salieron los
/// bytes que llegaron. El 2026-08-11 el sistema supo decir que los ocho primeros
/// bytes no eran los del fichero, y no supo decir de donde venian -- y esas son
/// las dos mitades de la misma pregunta:
///
/// | | |
/// |---|---|
/// | LBA raro | el mapa cluster->sector esta mal |
/// | LBA razonable | el disco tiene ahi otra cosa, o la particion es otra |
///
/// El tercer numero --donde empieza la particion-- es el que distingue "el
/// fichero esta en otro sitio" de **"estamos leyendo el volumen equivocado"**.
pub fn donde(cur: &bmo_fat32::Cursor) -> (u32, u64, u64) {
    let v = unsafe {
        match (*core::ptr::addr_of_mut!(DATA_VOLUME)).as_mut() {
            Some(v) => v,
            None => return (0, 0, 0),
        }
    };
    let c = cur.cluster();
    (c, v.lba_de_cluster(c), unsafe { DATA_LBA })
}

/// **Trae el rango `[offset, offset + dst.len())` del archivo.** Devuelve
/// cuantos bytes entraron.
///
/// Los sectores enteros van **del disco a `dst` sin pasar por ningun sitio**: si
/// `dst` cae sobre un marco del proceso, ahi es donde escribe el HBA. Ver
/// `bmo_fat32::leer_en`.
pub fn leer_rango(
    cur: &mut bmo_fat32::Cursor,
    offset: usize,
    size: u32,
    dst: &mut [u8],
) -> usize {
    let v = unsafe {
        match (*core::ptr::addr_of_mut!(DATA_VOLUME)).as_mut() {
            Some(v) => v,
            None => return 0,
        }
    };
    // ** UN CERO POR RETROCEDER NO ES UN CERO POR EL DISCO.
    //
    // El cursor solo avanza --a proposito: llegar al byte N en FAT32 es seguir
    // la cadena, y uno que retrocede en silencio vuelve cuadratica la carga--
    // asi que pedirle un offset por debajo de donde va contesta `0`. El mismo
    // `0` que da un sector ilegible.
    //
    // Y esos dos ceros mandan a sitios opuestos. El 2026-08-11 el cargador leyo
    // uno de ellos como `una seccion se quedo a medias al aterrizar =0` y la
    // frase manda a mirar el disco, que estaba perfecto: lo que habia pasado es
    // que la tabla de hashes --que vive al FINAL del fichero-- se habia leido
    // antes que el codigo. Ver `launch::Fuente::rango_suelto`.
    //
    // > Dos causas que producen el mismo numero necesitan que **una de las dos
    // > diga su nombre**, o el numero no sirve para elegir donde mirar.
    if offset < cur.base() {
        crate::ring0::cabina::fault(
            "fs",
            "se pidio un rango HACIA ATRAS: el cursor solo avanza (usa rango_suelto)",
            cur.base() as u64,
        );
        return 0;
    }
    v.leer_en(cur, offset, size, dst)
}

/// **Prepara una lectura A TROZOS.** Devuelve `(primer cluster, tamano)`.
///
/// Es `load` partido en dos momentos: aqui se resuelve la ruta --lo unico que
/// hay que hacer una sola vez-- y los bytes se traen despues con
/// [`leer_trozo`], tantas veces como haga falta.
pub fn abrir_trozos(path: &str) -> Result<(u32, u32), LoadError> {
    resolver(path)
}

/// **Trae un trozo y dice por donde iba.** `(leidos, cluster siguiente)`.
///
/// `siguiente == 0` = se acabo. Ver `bmo_fat32::leer_tramo`: el cursor es el
/// CLUSTER, no un offset, porque seguir la cadena desde el principio en cada
/// llamada seria recorrer el archivo entero por cada trozo.
pub fn leer_trozo(
    cluster: u32,
    ya: usize,
    size: u32,
    dst: &mut [u8],
    tope: usize,
) -> (usize, u32) {
    let v = unsafe {
        match (*core::ptr::addr_of_mut!(DATA_VOLUME)).as_mut() {
            Some(v) => v,
            None => return (0, 0),
        }
    };
    v.leer_tramo(cluster, ya, size, dst, tope)
}

/// **Trae solo el PRINCIPIO del archivo.** Devuelve `(leidos, tamano_real)`.
///
/// === Por que existe, y por que no es `load` con otro nombre ===
///
/// `load` dice `TooBig` cuando el archivo no cabe entero en el buffer, y hasta
/// hoy eso era lo correcto: un cargador que se queda con media imagen carga
/// medio programa.
///
/// ** Pero un `.bex` no es una tira de bytes que haya que tragarse entera. Lleva
/// su cabecera y su tabla de secciones al principio --el escritor pone la tabla
/// en el byte 48, siempre-- y esa tabla dice **cuanto hace falta de verdad**: el
/// codigo, los datos, las relocations y los hashes. Lo que va detras --los
/// recursos, los simbolos, la depuracion-- **el cargador no lo mira**, y en un
/// paquete con un WAD dentro eso es casi todo el fichero.
///
/// Asi que se lee el principio, se le pregunta al formato que necesita, y se lee
/// eso. El tamano real se devuelve aparte porque **sigue haciendo falta**: es
/// contra el que la cabecera comprueba que la imagen llego entera, y confundirlo
/// con "lo que cabia en el buffer" convertiria un fichero cortado en uno valido.
///
/// `read_file` ya para en `dst.len()` por su cuenta: no lee de mas para tirarlo.
pub fn load_prefijo(path: &str, dst: &mut [u8]) -> Result<(usize, usize), LoadError> {
    let (cluster, size) = resolver(path)?;
    let v = unsafe {
        match (*core::ptr::addr_of_mut!(DATA_VOLUME)).as_mut() {
            Some(v) => v,
            None => return Err(LoadError::NoVolume),
        }
    };
    let leidos = v.read_file(cluster, size, dst);
    Ok((leidos, size as usize))
}

/// Cuantos bytes mide el archivo, SIN leerlo.
///
/// Existe para poder reservar el buffer del tamano justo antes de traerlo:
/// hasta ahora un archivo abierto se copiaba a una fila estatica de 4 KiB, y
/// ese numero era el techo de lo que un programa podia leer. Preguntar primero
/// convierte el techo en "lo que quepa en la RAM".
pub fn tamano(path: &str) -> Result<u32, LoadError> {
    resolver(path).map(|(_, size)| size)
}

/// Recorre la ruta y devuelve `(cluster, tamano)` del archivo.
///
/// Lo comparten `load` y `tamano` a proposito: dos copias del recorrido de
/// directorios es la forma clasica de que una acepte una ruta que la otra
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

    // Bajar por los directorios; el ultimo componente es el archivo.
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

/// Crea un archivo en la raiz del volumen de datos.
///
/// `name_8_3` son once bytes tal como FAT los guarda: `b"CABINA  LOG"`.
/// Devuelve el motivo cuando falla -- el disco lleno, un nombre repetido y un
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
/// `create` es esto con la raiz. Se separan porque un programa de Ring 3
/// escribe donde le dijeron --`datos/movim.dat`--, y obligarle a dejarlo todo en
/// la raiz convertiria el volumen en un cajon: es justo lo que hace ilegible un
/// disco a los seis meses.
///
/// El `dir_cluster` sale de `dir_datos`, que ya recorrio la ruta. Aqui no se
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
        // El punto de no retorno: hasta que el disco vacie su cache, lo escrito
        // vive en un chip que un corte se lleva por delante.
        disk::flush();
    }
    r
}

/// **Guarda un archivo, exista o no.** Si el nombre ya esta, lo REEMPLAZA.
///
/// === Por que existe, y que tapaba no tenerlo ===
///
/// `crear_en` rechaza con `Exists`, que es lo correcto para *crear*. Pero
/// `OPEN OUTPUT` de COBOL --y un `>` de cualquier shell-- no quieren decir
/// "crealo": quieren decir **"que quede esto"**. Mientras la unica puerta fue
/// `crear_en`, un programa que escribia su salida solo funcionaba **la primera
/// vez que se corria**: a partir de la segunda, `close` fallaba y no guardaba
/// nada.
///
/// Y el fallo no se veia. Si el programa releia lo que creia haber escrito,
/// leia lo de la corrida anterior y todo parecia normal -- el nivel 10 de los
/// ejemplos de COBOL hace exactamente eso. Un fallo que se parece a funcionar
/// es peor que uno que revienta.
///
/// La seguridad del reemplazo la pone `save_file_in_dir` en el driver: la
/// cadena nueva se escribe entera **antes** de apuntar el nombre, y la vieja se
/// suelta **despues**. En ningun instante hay un archivo a medias.
pub fn guardar_en(dir_cluster: u32, name_8_3: &[u8; 11], data: &[u8]) -> Result<(), WriteError> {
    let v = unsafe {
        match (*core::ptr::addr_of_mut!(DATA_VOLUME)).as_mut() {
            Some(v) => v,
            None => return Err(WriteError::ReadOnly),
        }
    };
    let r = v.save_file_in_dir(dir_cluster, name_8_3, data);
    if r.is_ok() {
        disk::flush();
    }
    r
}
