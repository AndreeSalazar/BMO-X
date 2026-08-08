//! `KIND_ARCHIVO` -- **leer y escribir lo que hay dentro**, como capability.
//!
//! Hermano de [`crate::ring0::obj::directorio`]. Aquel deja PREGUNTAR que hay en el
//! disco; este deja abrir uno de esos nombres y mover sus bytes.
//!
//! Hasta ahora el kernel sabia leer archivos (`fs::load`, con el que se carga
//! el compositor) y escribirlos (`fs::create`, con el que CABINA deja su caja
//! negra), y **Ring 3 no tenia con que pedirselo**. El eslabon que faltaba no
//! era el sistema de ficheros: era la puerta.
//!
//! ## Dos modos, no dos objetos
//!
//! El modo se fija AL ABRIR y no cambia. Un handle de lectura no escribe
//! aunque se le pida -- y no por una comprobacion de permisos que alguien deba
//! acordarse de escribir, sino porque en ese modo **no hay a donde escribir**.
//! Es la misma idea que hace inmutable el volumen de arranque: no existe la
//! funcion.
//!
//! ## Por que hay un buffer, y cuanto cabe
//!
//! La superficie congelada no acepta punteros: los bytes cruzan de 7 en 7
//! dentro de un registro. Y `bmo_fat32` escribe un archivo ENTERO de una vez,
//! no por trozos. Entre esas dos cosas hace falta un sitio donde juntar lo que
//! llega suelto, y ese sitio es un buffer del kernel.
//!
//! * **Ese buffer ya no es una fila estatica de 4 KiB.** Lo era: cuatro filas
//! de `[u8; 4096]` en `.bss`, y de ahi salia "un archivo no puede pasar de 4
//! KiB". Ese numero no lo ponia el disco, ni FAT32, ni la superficie de
//! syscalls -- lo ponia **una constante**, en una maquina con 14.8 GiB libres y
//! un sistema que ocupa 5.4 MiB.
//!
//! Ahora se le pregunta al archivo cuanto mide y se reservan sus marcos al
//! abrir; al escribir, el buffer **crece al doble** cuando se llena. El techo
//! es la RAM, que es donde debe estar un techo. Lo que queda de limite se dice
//! entero, sin adornos:
//!
//! - Se piden marcos **contiguos**, porque el buffer se recorre como un `&[u8]`
//!   lineal. Si la RAM esta fragmentada y no hay hueco seguido, se rechaza con
//!   `ERROR_DEMASIADO_GRANDE` -- entregar un archivo a trozos sin que el
//!   llamante lo sepa seria peor.
//! - El archivo entero pasa por RAM. Un archivo mas grande que la memoria libre
//!   no se abre. Para eso haria falta un escritor por sectores en `bmo_fat32`,
//!   que es otra pieza y va despues.
//! - Lo escrito no llega al disco hasta `cerrar`, y ahi `bmo_fat32` lo guarda de
//!   una vez.
//!
//! ## Escribir es un acto de dos pasos
//!
//! Lo escrito NO esta en el disco hasta `ARCH_OP_CERRAR`. Un proceso que muere
//! a medias no deja un archivo a medias: no deja nada. Para un fichero de
//! movimientos eso es lo correcto -- un extracto truncado se parece demasiado a
//! uno completo.

use crate::ring0::obj::cap;

/// Cuantos archivos pueden estar abiertos a la vez, en todo el sistema.
///
/// Eran **cuatro**, y no por diseno: cada ranura arrastraba una fila estatica
/// de 4 KiB, asi que subir el numero costaba `.bss` aunque nadie abriera nada.
/// Ahora una ranura son unos pocos punteros y el buffer se reserva al abrir, o
/// sea que dieciseis cuestan lo mismo que cuatro cuando estan vacias. Un batch
/// que cruza tres ficheros con su salida ya no se queda sin manos.
pub const MAX_ABIERTOS: usize = 16;

/// Lo que se reserva al CREAR un archivo, antes de saber cuanto va a escribir.
///
/// No es un techo: cuando se llena, el buffer **crece al doble**. Es solo la
/// primera reserva, elegida para que un informe corriente no tenga que crecer
/// ni una vez.
pub const INICIAL: usize = 16 * 1024;

/// Tamano de pagina. El buffer se pide en marcos, que es lo que el asignador
/// entrega.
const PAGINA: usize = 4096;

pub const SIN_DUENO: u32 = u32::MAX;

/// No quedan ranuras de archivo abierto.
pub const ERROR_SIN_HUECO: u32 = 27;
/// La ruta no existe, o no es un archivo.
pub const ERROR_NO_ESTA: u32 = 28;
/// El archivo no cabe en el buffer. Se dice en vez de entregar un trozo.
pub const ERROR_DEMASIADO_GRANDE: u32 = 29;
/// El nombre no cabe en 8.3 (ocho de nombre, tres de extension).
pub const ERROR_NOMBRE: u32 = 30;
/// No hay volumen de datos montado con escritor.
pub const ERROR_SOLO_LECTURA: u32 = 31;
/// La CARPETA de la ruta no existe. Distinto de que falte el archivo: manda a
/// mirar otra cosa, y un mensaje que no los separa manda a buscar donde no es.
pub const ERROR_CARPETA: u32 = 32;
/// La ruta no nombra un archivo -- acaba en barra, o es un directorio.
pub const ERROR_ES_CARPETA: u32 = 33;

/// Saca hasta 7 bytes: `(n << 56) | bytes_LE`. `n == 0` = se acabo.
///
/// Siete y no ocho porque el octavo lleva la cuenta. Es el mismo trato que la
/// consola y que `DIR_OP_NOMBRE`: un contador honesto vale mas que un byte
/// apretado, y aqui ademas hace que **el NUL viaje** -- un archivo no es texto
/// y cortar en el primer cero corromperia cualquier binario.
pub const ARCH_OP_LEER: u64 = 0x01;

/// Mete hasta 7 bytes: `arg0 = (n << 56) | bytes_LE`, el mismo formato que
/// `LEER` pero al reves. Devuelve cuantos se aceptaron.
pub const ARCH_OP_ESCRIBIR: u64 = 0x02;

/// Bytes del archivo (los que quedan por leer, o los escritos hasta ahora).
pub const ARCH_OP_TAMANO: u64 = 0x03;

/// Cierra. En un archivo de ESCRITURA es donde el contenido llega al disco:
/// devuelve `1` si se guardo, `0` si no. En uno de lectura devuelve `1`.
pub const ARCH_OP_CERRAR: u64 = 0x04;

/// Saca hasta 7 bytes **sin pasar del salto de linea**:
/// `(fin << 63) | (n << 56) | bytes_LE`.
///
/// * Existe porque `ARCH_OP_LEER` **no sirve para leer registros**. Aquel
/// devuelve siete bytes y avanza el cursor siete; si el salto cae en medio del
/// paquete, lo que venia detras se pierde -- el cursor ya paso por encima y
/// nadie de fuera puede devolverselo. Un fichero de movimientos leido asi da
/// bien el primer registro y basura todos los demas.
///
/// El corte lo hace el kernel porque **el cursor es del kernel**. Es la misma
/// razon por la que `siguiente` vive en `directorio.rs` y no en Ring 3.
pub const ARCH_OP_LEER_LINEA: u64 = 0x05;
/// Leer un bloque entero en memoria concedida. Espejo de
/// `bmo_abi::...::ARCH_OP_LEER_EN`; lo despacha `syscall.rs`, que es quien tiene
/// las capabilities a mano.
pub const ARCH_OP_LEER_EN: u64 = 0x06;
/// Mover el cursor. Espejo de `bmo_abi::...::ARCH_OP_SALTAR`.
pub const ARCH_OP_SALTAR: u64 = 0x07;

// -- El buffer de cada archivo abierto -----------------------------------
//
// * Esto era `BUF: [[u8; 4096]; 4]` -- cuatro filas estaticas de 4 KiB. El
// numero no era un limite del disco ni del formato: era **el tamano de una
// fila**, y de ahi salia "un archivo no puede pasar de 4 KiB". En una maquina
// con 14.8 GiB libres y un sistema que ocupa 5.4 MiB, ese techo no lo ponia la
// fisica: lo ponia una constante.
//
// Ahora cada ranura guarda un puntero fisico y cuantas paginas mide, y el
// buffer se pide al asignador de marcos AL ABRIR, del tamano que diga el
// archivo. El techo pasa a ser la RAM -- que es donde debe estar.
//
// Se piden marcos CONTIGUOS porque el buffer se recorre como un `&[u8]` lineal.
// Si no hay un hueco contiguo, se dice: un archivo entregado a trozos sin que
// el llamante lo sepa es peor que un "no".
static mut BUF_FIS: [u64; MAX_ABIERTOS] = [0; MAX_ABIERTOS];
static mut BUF_PAGS: [u64; MAX_ABIERTOS] = [0; MAX_ABIERTOS];
/// Bytes validos: lo leido del disco, o lo acumulado para escribir.
static mut LARGO: [usize; MAX_ABIERTOS] = [0; MAX_ABIERTOS];
/// Por donde va la lectura.
static mut CURSOR: [usize; MAX_ABIERTOS] = [0; MAX_ABIERTOS];
/// Nombre 8.3 y directorio destino, para el momento de guardar.
static mut NOMBRE: [[u8; 11]; MAX_ABIERTOS] = [[b' '; 11]; MAX_ABIERTOS];
static mut DIRECTORIO: [u32; MAX_ABIERTOS] = [0; MAX_ABIERTOS];
static mut ESCRIBE: [bool; MAX_ABIERTOS] = [false; MAX_ABIERTOS];
/// Se pidio escribir mas de lo que cabe. `cerrar` lo confiesa en vez de
/// guardar un archivo corto que parece entero.
static mut DESBORDO: [bool; MAX_ABIERTOS] = [false; MAX_ABIERTOS];
static mut DUENO: [u32; MAX_ABIERTOS] = [SIN_DUENO; MAX_ABIERTOS];

fn hueco() -> Option<usize> {
    unsafe { (0..MAX_ABIERTOS).find(|&i| DUENO[i] == SIN_DUENO) }
}

/// El buffer de la ranura como rebanada. Vacio si no hay nada reservado.
///
/// La direccion sale de `phys_to_virt`: el kernel ve toda la RAM por el mapa
/// fisico, asi que no hace falta mapear nada para tocar estos marcos.
unsafe fn buf(i: usize) -> &'static mut [u8] {
    if BUF_FIS[i] == 0 {
        return &mut [];
    }
    let base = crate::ring0::mm::phys_to_virt(BUF_FIS[i]) as *mut u8;
    core::slice::from_raw_parts_mut(base, (BUF_PAGS[i] as usize) * PAGINA)
}

/// Cuantos bytes caben ahora mismo en la ranura.
unsafe fn capacidad(i: usize) -> usize {
    (BUF_PAGS[i] as usize) * PAGINA
}

/// Reserva un buffer de al menos `bytes` para la ranura. `false` = no hay RAM.
unsafe fn reservar(i: usize, bytes: usize) -> bool {
    let pags = ((bytes.max(1) + PAGINA - 1) / PAGINA) as u64;
    match crate::ring0::mm::phys::alloc_frames_contig(pags) {
        Some(fis) => {
            BUF_FIS[i] = fis;
            BUF_PAGS[i] = pags;
            true
        }
        None => false,
    }
}

/// Devuelve el buffer de la ranura al asignador. Idempotente.
///
/// Se llama SIEMPRE al soltar la ranura, incluso cuando el guardado fallo: un
/// archivo que no se pudo escribir no es motivo para quedarse con su memoria.
unsafe fn soltar_buffer(i: usize) {
    if BUF_FIS[i] != 0 {
        for p in 0..BUF_PAGS[i] {
            crate::ring0::mm::phys::free_frame(BUF_FIS[i] + p * PAGINA as u64);
        }
    }
    BUF_FIS[i] = 0;
    BUF_PAGS[i] = 0;
}

/// Hace sitio para `minimo` bytes DOBLANDO el buffer. `false` = no hay RAM.
///
/// Doblar y no crecer justo lo pedido: escribir un informe son miles de
/// llamadas de siete bytes, y crecer en cada una seria copiar el archivo entero
/// miles de veces. Doblando, el numero de copias es logaritmico.
unsafe fn crecer(i: usize, minimo: usize) -> bool {
    let mut nueva = capacidad(i).max(PAGINA);
    while nueva < minimo {
        nueva *= 2;
    }
    let pags = (nueva / PAGINA) as u64;
    let fis = match crate::ring0::mm::phys::alloc_frames_contig(pags) {
        Some(f) => f,
        None => return false,
    };
    // Copiar lo que ya habia ANTES de soltar lo viejo.
    let destino = crate::ring0::mm::phys_to_virt(fis) as *mut u8;
    let viejo = buf(i);
    let n = LARGO[i].min(viejo.len());
    core::ptr::copy_nonoverlapping(viejo.as_ptr(), destino, n);
    soltar_buffer(i);
    BUF_FIS[i] = fis;
    BUF_PAGS[i] = pags;
    true
}

/// Abre un archivo del volumen de datos para LEER y entrega su handle a `pid`.
///
/// El archivo entero se trae al buffer aqui, no segun se pide. Asi una lectura
/// no puede fallar a mitad por un error de disco: o el archivo esta entero en
/// memoria antes de que Ring 3 vea el primer byte, o `abrir` falla y no hay
/// handle.
pub fn abrir(pid: u32, ruta: &str) -> Result<u64, u32> {
    let i = match hueco() {
        Some(i) => i,
        None => return Err(ERROR_SIN_HUECO),
    };
    // Cada motivo manda a hacer algo distinto, y por eso no se aplanan todos a
    // "no esta": quien escribe `lee apps/` tiene que enterarse de que eso es
    // una carpeta, no ponerse a buscar un archivo que nunca existio.
    use crate::ring0::fsys::fs::LoadError;
    // Primero CUANTO mide, y despues se reserva justo eso. Antes se copiaba a
    // una fila estatica de 4 KiB y el techo lo ponia esa fila.
    let mide = match crate::ring0::fsys::fs::tamano(ruta) {
        Ok(n) => n as usize,
        Err(LoadError::BadPath) => return Err(ERROR_ES_CARPETA),
        Err(LoadError::NameTooLong) => return Err(ERROR_NOMBRE),
        Err(LoadError::DirNotFound) => return Err(ERROR_CARPETA),
        Err(_) => return Err(ERROR_NO_ESTA),
    };
    let leidos = unsafe {
        if !reservar(i, mide) {
            // No hay RAM contigua para el archivo. Se dice: entregarlo a
            // trozos sin que el llamante lo sepa seria peor.
            crate::ring0::cabina::warn("arch", "sin RAM contigua para el archivo", mide as u64);
            return Err(ERROR_DEMASIADO_GRANDE);
        }
        let dst = buf(i);
        match crate::ring0::fsys::fs::load(ruta, dst) {
            Ok(n) => n,
            Err(e) => {
                soltar_buffer(i);
                return Err(match e {
                    LoadError::TooBig => ERROR_DEMASIADO_GRANDE,
                    LoadError::BadPath => ERROR_ES_CARPETA,
                    LoadError::NameTooLong => ERROR_NOMBRE,
                    LoadError::DirNotFound => ERROR_CARPETA,
                    _ => ERROR_NO_ESTA,
                });
            }
        }
    };
    unsafe {
        LARGO[i] = leidos;
        CURSOR[i] = 0;
        ESCRIBE[i] = false;
        DESBORDO[i] = false;
        DUENO[i] = pid;
        match cap::grant(pid, cap::KIND_ARCHIVO, cap::RIGHT_READ, i as u64) {
            Some(h) => {
                crate::ring0::cabina::info("arch", "archivo abierto para leer", leidos as u64);
                Ok(h)
            }
            None => {
                DUENO[i] = SIN_DUENO;
                Err(cap::ERROR_PERMISSION_DENIED)
            }
        }
    }
}

/// Abre un archivo del volumen de datos para ESCRIBIR.
///
/// El directorio se resuelve AHORA y el nombre se valida AHORA, aunque no se
/// escriba nada hasta cerrar. Descubrir al final que la carpeta no existia
/// significaria haber dejado a un programa acumulando bytes para nada.
pub fn crear(pid: u32, ruta: &str) -> Result<u64, u32> {
    if !crate::ring0::fsys::fs::data_mounted() {
        return Err(ERROR_SOLO_LECTURA);
    }
    // Partir la ruta en carpeta + nombre por la ULTIMA barra.
    let limpia = {
        let mut p = ruta.trim();
        if p.len() >= 2 && p.as_bytes()[1] == b':' { p = &p[2..]; }
        while p.starts_with('/') || p.starts_with('\\') { p = &p[1..]; }
        p
    };
    let corte = limpia.rfind(['/', '\\']);
    let (carpeta, nombre_txt) = match corte {
        Some(k) => (&limpia[..k], &limpia[k + 1..]),
        None => ("", limpia),
    };
    if nombre_txt.is_empty() {
        return Err(ERROR_NOMBRE);
    }
    let nombre = match crate::ring0::fsys::fs::nombre_8_3_pub(nombre_txt) {
        Some(n) => n,
        None => return Err(ERROR_NOMBRE),
    };
    let dir = match crate::ring0::fsys::fs::dir_datos(carpeta) {
        Some(c) => c,
        // La carpeta, no el archivo. `escribe datos/x.txt` cuando no hay
        // `datos/` tiene que decir que falta la CARPETA: el archivo es
        // justamente lo que se venia a crear.
        None => return Err(ERROR_CARPETA),
    };

    let i = match hueco() {
        Some(i) => i,
        None => return Err(ERROR_SIN_HUECO),
    };
    unsafe {
        // La primera reserva. No es un techo: `escribir` dobla cuando se llena.
        if !reservar(i, INICIAL) {
            crate::ring0::cabina::warn("arch", "sin RAM para el buffer de escritura", 0);
            return Err(ERROR_DEMASIADO_GRANDE);
        }
        LARGO[i] = 0;
        CURSOR[i] = 0;
        NOMBRE[i] = nombre;
        DIRECTORIO[i] = dir;
        ESCRIBE[i] = true;
        DESBORDO[i] = false;
        DUENO[i] = pid;
        // Se conceden los dos derechos: `invoke` resuelve con RIGHT_READ, asi
        // que sin el ni siquiera llegaria el `ESCRIBIR`. Lo que impide leer un
        // archivo de escritura no es el derecho, es el modo -- ver `operacion`.
        match cap::grant(pid, cap::KIND_ARCHIVO, cap::RIGHT_READ | cap::RIGHT_WRITE, i as u64) {
            Some(h) => {
                crate::ring0::cabina::info("arch", "archivo abierto para escribir", pid as u64);
                Ok(h)
            }
            None => {
                DUENO[i] = SIN_DUENO;
                Err(cap::ERROR_PERMISSION_DENIED)
            }
        }
    }
}

fn leer(i: usize) -> u64 {
    unsafe {
        let mut w = [0u8; 8];
        let mut n = 0usize;
        while n < 7 && CURSOR[i] < LARGO[i] {
            w[n] = buf(i)[CURSOR[i]];
            CURSOR[i] += 1;
            n += 1;
        }
        ((n as u64) << 56) | u64::from_le_bytes(w)
    }
}

/// Como `leer`, pero se para en el salto de linea y lo consume.
fn leer_linea(i: usize) -> u64 {
    unsafe {
        let mut w = [0u8; 8];
        let mut n = 0usize;
        let mut fin = 0u64;
        while n < 7 && CURSOR[i] < LARGO[i] {
            let b = buf(i)[CURSOR[i]];
            CURSOR[i] += 1;
            if b == b'\n' {
                // Se consume y NO se entrega: el salto separa registros, no
                // forma parte de ninguno.
                fin = 1;
                break;
            }
            w[n] = b;
            n += 1;
        }
        (fin << 63) | ((n as u64) << 56) | u64::from_le_bytes(w)
    }
}

fn escribir(i: usize, palabra: u64) -> u64 {
    let n = ((palabra >> 56) & 0xFF) as usize;
    let n = n.min(7);
    let bytes = palabra.to_le_bytes();
    unsafe {
        let mut puestos = 0usize;
        for k in 0..n {
            // Sin sitio: se DOBLA el buffer. Antes aqui se levantaba la bandera
            // de desbordado contra un techo de 4 KiB; ahora solo se levanta si
            // de verdad no queda RAM, que es un motivo y no una constante.
            if LARGO[i] >= capacidad(i) && !crecer(i, LARGO[i] + 1) {
                DESBORDO[i] = true;
                crate::ring0::cabina::warn(
                    "arch",
                    "sin RAM para seguir escribiendo: no se guardara nada",
                    LARGO[i] as u64,
                );
                break;
            }
            buf(i)[LARGO[i]] = bytes[k];
            LARGO[i] += 1;
            puestos += 1;
        }
        puestos as u64
    }
}

/// Cierra la ranura y, si era de escritura, guarda. `1` = todo bien.
fn cerrar(i: usize) -> u64 {
    unsafe {
        let ok = if ESCRIBE[i] {
            if DESBORDO[i] {
                // No se guarda NADA. Un archivo recortado en silencio se
                // parece demasiado a uno entero, y el que lo lea manana no
                // tiene forma de saberlo.
                crate::ring0::cabina::warn("arch", "no cabia: no se guarda nada", LARGO[i] as u64);
                false
            } else {
                // El slice se construye desde el puntero crudo, sin autoref
                // implicito sobre el `static mut`: la fila primero, el recorte
                // despues. Encadenarlo en una expresion crea una referencia a
                // la desreferencia del puntero, que es justo lo que el lint
                // prohibe -- y con razon, porque esconde de donde sale.
                let fila = buf(i);
                let datos = &fila[..LARGO[i].min(fila.len())];
                // `guardar_en` y no `crear_en`: si el archivo ya existe, se
                // REEMPLAZA. `crear_en` contestaba `Exists` y aqui eso se
                // convertia en un `warn` a la CABINA y un `0` que casi nadie
                // miraba -- o sea que un programa que escribia su salida solo
                // era honesto la primera vez que se corria.
                match crate::ring0::fsys::fs::guardar_en(DIRECTORIO[i], &NOMBRE[i], datos) {
                    Ok(()) => {
                        crate::ring0::cabina::info("arch", "archivo guardado", LARGO[i] as u64);
                        true
                    }
                    Err(_) => {
                        crate::ring0::cabina::warn("arch", "no se pudo guardar", LARGO[i] as u64);
                        false
                    }
                }
            }
        } else {
            true
        };
        soltar(i);
        ok as u64
    }
}

fn soltar(i: usize) {
    unsafe {
        // La memoria se devuelve AQUI y en un solo sitio, pase lo que pase con
        // el guardado. Un archivo que no se pudo escribir no es motivo para
        // quedarse con sus marcos: eso es una fuga que solo se nota tras
        // muchas horas, que es cuando peor se encuentra.
        soltar_buffer(i);
        DUENO[i] = SIN_DUENO;
        LARGO[i] = 0;
        CURSOR[i] = 0;
        ESCRIBE[i] = false;
        DESBORDO[i] = false;
        DIRECTORIO[i] = 0;
    }
}

pub fn operacion(idx: u64, op: u64, arg0: u64) -> Option<u64> {
    let i = idx as usize;
    if i >= MAX_ABIERTOS {
        return None;
    }
    let escribe = unsafe { ESCRIBE[i] };
    match op {
        // El modo manda. Pedirle bytes a un archivo de escritura no es un
        // error de permisos: es una pregunta que ese objeto no responde.
        ARCH_OP_LEER if !escribe => Some(leer(i)),
        ARCH_OP_LEER_LINEA if !escribe => Some(leer_linea(i)),
        ARCH_OP_ESCRIBIR if escribe => Some(escribir(i, arg0)),
        ARCH_OP_TAMANO => Some(unsafe {
            if escribe { LARGO[i] as u64 } else { (LARGO[i] - CURSOR[i]) as u64 }
        }),
        // * `fseek`. Cuesta lo que cuesta poner un numero porque el archivo ya
        // esta entero en el bufer desde que se abrio. Se acota al tamano en vez
        // de rechazar: un cursor mas alla del final significa "no queda nada",
        // que es lo que contesta `ARCH_OP_TAMANO` sin inventarse un error.
        ARCH_OP_SALTAR if !escribe => Some(unsafe {
            let d = if arg0 as usize > LARGO[i] { LARGO[i] } else { arg0 as usize };
            CURSOR[i] = d;
            d as u64
        }),
        ARCH_OP_CERRAR => Some(cerrar(i)),
        _ => None,
    }
}

/// **Copia hasta `n` bytes desde el cursor a `dst`, y avanza.** Devuelve los
/// copiados de verdad.
///
/// [!] `dst` **tiene que estar validado por el llamante**: aqui no se puede
/// comprobar nada porque es una direccion a secas. Quien la resuelve es
/// `syscall.rs`, y lo hace de la unica forma que no exige inventarse un
/// validador de punteros -- pidiendo la *capability* del bloque que el kernel
/// concedio y midiendo contra lo que entrego.
///
/// # Safety
/// `dst` debe apuntar a `n` bytes escribibles y mapeados en el CR3 actual.
pub unsafe fn leer_en(idx: u64, dst: *mut u8, n: usize) -> usize {
    let i = idx as usize;
    if i >= MAX_ABIERTOS || dst.is_null() || n == 0 {
        return 0;
    }
    unsafe {
        if ESCRIBE[i] {
            return 0;
        }
        let quedan = LARGO[i].saturating_sub(CURSOR[i]);
        let cuantos = if n > quedan { quedan } else { n };
        if cuantos == 0 {
            return 0;
        }
        core::ptr::copy_nonoverlapping(buf(i).as_ptr().add(CURSOR[i]), dst, cuantos);
        CURSOR[i] += cuantos;
        cuantos
    }
}

/// Lo llama `cap::revoke_all`. Un proceso que muere con un archivo de
/// escritura a medias **no deja nada**: lo acumulado se tira. Guardarlo seria
/// inventar un archivo que su autor nunca dio por terminado.
pub fn proceso_muerto(pid: u32) {
    unsafe {
        for i in 0..MAX_ABIERTOS {
            if DUENO[i] == pid {
                if ESCRIBE[i] && LARGO[i] > 0 {
                    crate::ring0::cabina::warn("arch", "murio sin cerrar: se descarta", LARGO[i] as u64);
                }
                soltar(i);
            }
        }
    }
}
