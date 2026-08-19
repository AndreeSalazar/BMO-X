//! `KIND_ARCHIVO` -- **leer y escribir lo que hay dentro**, como capability.
//!
//! Hermano de [`crate::ring0::obj::directory`]. Aquel deja PREGUNTAR que hay en el
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
//! Al ESCRIBIR, ese buffer se pide al abrir y **crece al doble** cuando se
//! llena; el techo es la RAM, que es donde debe estar un techo. Lo que queda de
//! limite se dice entero, sin adornos:
//!
//! - Se piden marcos **contiguos**, porque el buffer se recorre como un `&[u8]`
//!   lineal. Si la RAM esta fragmentada y no hay hueco seguido, se rechaza con
//!   `ERROR_TOO_LARGE` -- entregar un archivo a trozos sin que el
//!   llamante lo sepa seria peor.
//! - Lo escrito no llega al disco hasta `close`, y ahi `bmo_fat32` lo guarda de
//!   una vez.
//!
//! ## ** LEER NO ES TRAERSE EL ARCHIVO. Es reflejarlo.
//!
//! Hasta el 2026-08-11 `open` hacia esto: preguntar cuanto mide, reservar
//! **sus marcos contiguos**, y leerlo ENTERO antes de devolver el handle. Para
//! un `.txt` no se nota. Para `doom1.wad` son **4.196.020 bytes contiguos en
//! fisico** pedidos justo despues de que DOOM se llevara sus 12 MiB de zona, y
//! una lectura bloqueante de cuatro megas dentro de un syscall.
//!
//! Y lo que lo delata no es el coste: es que **nadie los pedia**. `w_file_stdc.c`
//! no se traga el WAD -- lee el directorio de lumps al abrir y luego cada lump
//! por `fseek`+`fread`, decenas de KB cada vez. DOOM estaba haciendo lo
//! correcto; era BMO el que le traia la bodega entera para servirle una copa.
//!
//! Asi que un archivo de LECTURA ya no se trae: **se refleja**. La ranura guarda
//! un cursor de FAT32 --doce bytes-- y cada peticion trae **solo su rango**:
//!
//! | | antes | ahora |
//! |---|---|---|
//! | Abrir el WAD | 4 MiB contiguos + leer 4 MB | un cursor y una ventana |
//! | Un lump de 40 KB | ya estaba en RAM | 40 KB del disco, a donde se pida |
//! | Tope de tamano | la RAM contigua que haya | **ninguno** |
//!
//! Es la misma pieza que `lanzar.rs` estreno para los `.bex` --el cargador dejo
//! de traerse un paquete de 5,5 MB para ejecutar 812 KB-- y que se habia quedado
//! sin aplicar aqui. Las dos mitades ya existian y estaban probadas en metal:
//! `fs::abrir_rangos` y `fs::leer_rango`.
//!
//! ### La ventana, y por que sigue habiendo un buffer
//!
//! `ARCH_OP_LEER` entrega **siete bytes por llamada**: ir al disco por cada
//! siete seria un sector por byte y medio. Asi que la ranura mantiene una
//! **ventana** de [`WINDOW`] --el ultimo trozo leido, con su offset-- y esas
//! llamadas se sirven de ahi. Un archivo mas pequeno que la ventana entra
//! entero en la primera lectura y se comporta exactamente como antes.
//!
//! `ARCH_OP_LEER_EN` --el camino de `fread`-- **no pasa por la ventana**: el
//! rango va del disco al bloque del que pregunta, sin escala.
//!
//! ### El cursor solo avanza, y `fseek` va hacia atras
//!
//! Llegar al byte `N` en FAT32 es seguir la cadena, asi que el cursor de
//! `bmo_fat32` **no retrocede**: uno que lo hiciera en silencio volveria
//! cuadratica cualquier carga. Pero DOOM salta entre lumps en los dos sentidos,
//! y aqui eso no es la excepcion sino el caso normal.
//!
//! Se resuelve como en `launch::Fuente::rango_suelto`, con **el cursor sin
//! estrenar guardado aparte**: pedir hacia atras vuelve a empezar desde el
//! principio del archivo. Cuesta un recorrido de la cadena, se cuenta
//! ([`cuentas`]) y se puede mirar -- que es distinto de que no cueste nada.
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
pub const INITIAL: usize = 16 * 1024;

/// Tamano de pagina. El buffer se pide en marcos, que es lo que el asignador
/// entrega.
const PAGE: usize = 4096;

pub const NO_OWNER: u32 = u32::MAX;

/// No quedan ranuras de archivo abierto.
pub const ERROR_NO_FREE_SLOT: u32 = 27;
/// La ruta no existe, o no es un archivo.
pub const ERROR_NOT_THERE: u32 = 28;
/// El archivo no cabe en el buffer. Se dice en vez de entregar un trozo.
pub const ERROR_TOO_LARGE: u32 = 29;
/// El nombre no cabe en 8.3 (ocho de nombre, tres de extension).
pub const ERROR_NAME: u32 = 30;
/// No hay volumen de datos montado con escritor.
pub const ERROR_READ_ONLY: u32 = 31;
/// La CARPETA de la ruta no existe. Distinto de que falte el archivo: manda a
/// mirar otra cosa, y un mensaje que no los separa manda a buscar donde no es.
pub const ERROR_DIRECTORY: u32 = 32;
/// La ruta no nombra un archivo -- acaba en barra, o es un directorio.
pub const ERROR_IS_DIRECTORY: u32 = 33;

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
/// razon por la que `next` vive en `directorio.rs` y no en Ring 3.
pub const ARCH_OP_LEER_LINEA: u64 = 0x05;
/// Leer un bloque entero en memoria concedida. Espejo de
/// `bmo_abi::...::ARCH_OP_LEER_EN`; lo despacha `syscall.rs`, que es quien tiene
/// las capabilities a mano.
pub const ARCH_OP_LEER_EN: u64 = 0x06;
/// **Cuanto ha llegado ya, y si falta.** `(entero << 63) | bytes_disponibles`.
///
/// Y **avanza la carga**: preguntar por el archivo es lo que lo trae. Ver
/// [`avanzar`] -- el trabajo ocurre en el turno de quien lo quiere.
///
/// Los dos datos van en la misma respuesta a proposito: "cuanto hay" y "queda
/// mas" son la misma pregunta hecha dos veces, y contestarlas por separado abre
/// la puerta a leerlas de vueltas distintas.
pub const ARCH_OP_LISTO: u64 = 0x09;
/// Mover el cursor. Espejo de `bmo_abi::...::ARCH_OP_SALTAR`.
pub const ARCH_OP_SALTAR: u64 = 0x07;

/// ** ESCRIBIR UN BLOQUE ENTERO. El espejo exacto de [`ARCH_OP_LEER_EN`], y por
/// el mismo motivo.
///
/// `ARCH_OP_ESCRIBIR` mete **siete bytes por llamada**. Para la salida de un
/// programa --unas lineas-- eso no se nota. Para guardar una partida de DOOM,
/// que son cientos de KiB, son **decenas de miles de llamadas al sistema** para
/// mover algo que cabe en un `copy_nonoverlapping`.
///
/// El origen es un bloque que concedio el kernel, asi que aqui tampoco hace
/// falta validar punteros de Ring 3: comprobar es una resta contra lo que se
/// entrego. La asimetria de antes --leer de golpe si, escribir no-- no tenia
/// razon de ser, solo orden de llegada.
///
/// Lo despacha `syscall.rs` y no `operation`, igual que su espejo, porque hay
/// que resolver una SEGUNDA capability y eso vive en el borde.
pub const ARCH_OP_ESCRIBIR_DE: u64 = 0x08;

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
/// Bytes validos: **lo que mide el archivo** si se refleja, lo leido del disco
/// si se trajo a trozos, o lo acumulado si se esta escribiendo.
///
/// Los tres son "cuantos bytes hay que contar", que es lo que preguntan
/// `ARCH_OP_TAMANO` y `ARCH_OP_SALTAR`. Lo que cambia entre los tres es **donde
/// estan esos bytes**, y eso lo dice [`REFLEJO`].
pub(super) static mut LARGO: [usize; MAX_ABIERTOS] = [0; MAX_ABIERTOS];
/// Por donde va la lectura.
static mut CURSOR: [usize; MAX_ABIERTOS] = [0; MAX_ABIERTOS];
/// Nombre 8.3 y directorio destino, para el momento de guardar.
static mut NAME: [[u8; 11]; MAX_ABIERTOS] = [[b' '; 11]; MAX_ABIERTOS];
static mut DIRECTORIO: [u32; MAX_ABIERTOS] = [0; MAX_ABIERTOS];
static mut WRITES: [bool; MAX_ABIERTOS] = [false; MAX_ABIERTOS];
/// Se pidio escribir mas de lo que cabe. `close` lo confiesa en vez de
/// guardar un archivo corto que parece entero.
static mut DESBORDO: [bool; MAX_ABIERTOS] = [false; MAX_ABIERTOS];
static mut OWNER: [u32; MAX_ABIERTOS] = [NO_OWNER; MAX_ABIERTOS];

// -- ** EL ARCHIVO QUE NO ESTA EN RAM: EL REFLEJO ----------------------------
//
// Doce bytes de cursor en vez del fichero. Ver la cabecera del modulo.

/// Esta ranura **refleja** el archivo en vez de tenerlo? Lo son todas las de
/// lectura desde el 2026-08-11; las de escritura y las de `abrir_asinc` no.
static mut REFLEJO: [bool; MAX_ABIERTOS] = [false; MAX_ABIERTOS];
/// Por donde va el reflejo. **Solo avanza** -- ver la cabecera.
static mut CUR: [bmo_fat32::Cursor; MAX_ABIERTOS] =
    [bmo_fat32::Cursor::vacio(); MAX_ABIERTOS];
/// El mismo cursor **sin estrenar**, que es lo que hace posible retroceder.
///
/// Cuesta doce bytes por ranura y es la diferencia entre que `fseek` hacia atras
/// funcione o devuelva cero. La copia se hace al abrir: reabrir de verdad
/// obligaria a volver a recorrer el arbol de directorios.
static mut START: [bmo_fat32::Cursor; MAX_ABIERTOS] =
    [bmo_fat32::Cursor::vacio(); MAX_ABIERTOS];
/// En que byte del archivo empieza lo que hay ahora en el buffer.
static mut WINDOW_OFF: [usize; MAX_ABIERTOS] = [0; MAX_ABIERTOS];
/// Cuantos bytes validos hay en la ventana. `0` = no hay nada leido.
static mut WINDOW_LEN: [usize; MAX_ABIERTOS] = [0; MAX_ABIERTOS];

/// Lo que se trae de una vez para las lecturas de siete bytes.
///
/// 64 KiB: cabe la inmensa mayoria de los ficheros del sistema **entera** --y
/// entonces esto se comporta igual que el `open` de antes, con una sola lectura--
/// y son dieciseis marcos por archivo abierto, que no es un numero que haya que
/// pensar. El que lea un WAD de 4 MiB no pasa por aqui: va por `LEER_EN`.
pub const WINDOW: usize = 64 * 1024;

/// Bytes que se han traido del disco por reflejo, y cuantas veces hubo que
/// **volver al principio** del archivo.
///
/// Se cuenta porque retroceder cuesta un recorrido de la cadena FAT, y un coste
/// que nadie mide es un coste que un dia se multiplica sin que nada lo diga. Si
/// este segundo numero crece con el primero, el patron de acceso esta pidiendo
/// un cursor por lump y no uno por archivo -- y eso se sabra mirandolo, no
/// suponiendolo.
static mut BYTES_REFLEJADOS: u64 = 0;
static mut RETROCESOS: u64 = 0;

/// Las dos cuentas **en el momento de abrir cada archivo**, para poder decir el
/// delta al cerrarlo.
///
/// Es el mismo truco que `lanzar` usa con `cuentas_dma`: el total del arranque
/// no dice nada, y *"de un fichero de 4.196.020 bytes se trajeron 812.736"* es
/// el escalon entero en una linea.
static mut REF_AL_ABRIR: [u64; MAX_ABIERTOS] = [0; MAX_ABIERTOS];
static mut RET_AL_ABRIR: [u64; MAX_ABIERTOS] = [0; MAX_ABIERTOS];

/// `(bytes reflejados, retrocesos)` desde el arranque.
pub fn cuentas() -> (u64, u64) {
    unsafe { (BYTES_REFLEJADOS, RETROCESOS) }
}

fn free_slot() -> Option<usize> {
    unsafe { (0..MAX_ABIERTOS).find(|&i| OWNER[i] == NO_OWNER) }
}

/// El buffer de la ranura como rebanada. Vacio si no hay nada reservado.
///
/// La direccion sale de `phys_to_virt`: el kernel ve toda la RAM por el mapa
/// fisico, asi que no hace falta mapear nada para tocar estos marcos.
pub(super) unsafe fn buf(i: usize) -> &'static mut [u8] {
    if BUF_FIS[i] == 0 {
        return &mut [];
    }
    let base = crate::ring0::mm::phys_to_virt(BUF_FIS[i]) as *mut u8;
    core::slice::from_raw_parts_mut(base, (BUF_PAGS[i] as usize) * PAGE)
}

/// Cuantos bytes caben ahora mismo en la ranura.
unsafe fn capacity(i: usize) -> usize {
    (BUF_PAGS[i] as usize) * PAGE
}

/// Reserva un buffer de al menos `bytes` para la ranura. `false` = no hay RAM.
unsafe fn reserve(i: usize, bytes: usize) -> bool {
    let pags = ((bytes.max(1) + PAGE - 1) / PAGE) as u64;
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
unsafe fn release_buffer(i: usize) {
    if BUF_FIS[i] != 0 {
        for p in 0..BUF_PAGS[i] {
            crate::ring0::mm::phys::free_frame(BUF_FIS[i] + p * PAGE as u64);
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
unsafe fn grow(i: usize, minimo: usize) -> bool {
    let mut nueva = capacity(i).max(PAGE);
    while nueva < minimo {
        nueva *= 2;
    }
    let pags = (nueva / PAGE) as u64;
    let fis = match crate::ring0::mm::phys::alloc_frames_contig(pags) {
        Some(f) => f,
        None => return false,
    };
    // Copiar lo que ya habia ANTES de soltar lo viejo.
    let destino = crate::ring0::mm::phys_to_virt(fis) as *mut u8;
    let viejo = buf(i);
    let n = LARGO[i].min(viejo.len());
    core::ptr::copy_nonoverlapping(viejo.as_ptr(), destino, n);
    release_buffer(i);
    BUF_FIS[i] = fis;
    BUF_PAGS[i] = pags;
    true
}

/// **Trae el rango `[offset, offset + dst.len())` del archivo reflejado.**
/// Devuelve cuantos bytes entraron.
///
/// Es el unico sitio del modulo que toca el disco al leer, y por eso es el unico
/// que sabe de retroceder: si se pide por debajo de donde va el cursor, se
/// vuelve a empezar **desde la copia sin estrenar**. Ver la cabecera.
unsafe fn reflejar(i: usize, offset: usize, dst: &mut [u8]) -> usize {
    if dst.is_empty() || offset >= LARGO[i] {
        return 0;
    }
    let cur = &mut *core::ptr::addr_of_mut!(CUR[i]);
    if offset < cur.base() {
        // Volver al principio. `fs::leer_rango` contestaria `0` y lo diria a
        // gritos --y con razon, porque para el cargador eso es un fallo-- asi
        // que aqui se le da un cursor que si puede llegar, en vez de pedirle
        // algo que su contrato no promete.
        *cur = START[i];
        RETROCESOS += 1;
    }
    let n = crate::ring0::fsys::fs::leer_rango(cur, offset, LARGO[i] as u32, dst);
    BYTES_REFLEJADOS += n as u64;
    n
}

/// Deja en la ventana el trozo que contiene `pos`. `false` = ahi no hay nada.
///
/// Si ya esta, no toca el disco: es lo que hace que leer de siete en siete
/// cueste una lectura cada 64 KiB y no una cada siete bytes.
unsafe fn window_at(i: usize, pos: usize) -> bool {
    if pos >= LARGO[i] {
        return false;
    }
    if WINDOW_LEN[i] > 0 && pos >= WINDOW_OFF[i] && pos < WINDOW_OFF[i] + WINDOW_LEN[i] {
        return true;
    }
    let dst = buf(i);
    if dst.is_empty() {
        return false;
    }
    let n = reflejar(i, pos, dst);
    WINDOW_OFF[i] = pos;
    WINDOW_LEN[i] = n;
    n > 0
}

/// El byte `pos` del archivo, venga de donde venga. `None` = se acabo.
///
/// Los dos modos de lectura --el reflejo y el que trae el fichero a trozos--
/// contestan por aqui, y por eso `read` y `read_line` no saben cual de los dos
/// tienen delante. Un `if` repartido por cada lector es como se acaba teniendo
/// un modo que funciona y otro que casi.
unsafe fn byte_en(i: usize, pos: usize) -> Option<u8> {
    if pos >= LARGO[i] {
        return None;
    }
    if !REFLEJO[i] {
        let b = buf(i);
        return if pos < b.len() { Some(b[pos]) } else { None };
    }
    if !window_at(i, pos) {
        return None;
    }
    let d = pos - WINDOW_OFF[i];
    let b = buf(i);
    if d < WINDOW_LEN[i].min(b.len()) { Some(b[d]) } else { None }
}

/// Abre un archivo del volumen de datos para LEER y entrega su handle a `pid`.
///
/// ** No se trae nada. Se guarda un cursor y se reserva la ventana --lo mas
/// pequeno entre el archivo y [`WINDOW`]--, y los bytes van del disco a quien
/// los pida, cuando los pida. Un WAD de 4 MiB cuesta lo mismo que un `.txt`.
///
/// Lo que se pierde con esto, dicho: antes, si el disco fallaba, fallaba `open`
/// y no habia handle. Ahora una lectura puede quedarse corta a mitad del
/// fichero. A cambio, `open` **no puede fallar por falta de RAM contigua**, que
/// es lo que le pasaba a `doom1.wad` -- y un fallo de disco a mitad se cuenta y
/// se ve (`ARCH_OP_LEER_EN` devuelve menos de lo pedido), mientras que "no cabe"
/// dejaba al programa sin manera de seguir.
pub fn open(pid: u32, ruta: &str) -> Result<u64, u32> {
    let i = match free_slot() {
        Some(i) => i,
        None => return Err(ERROR_NO_FREE_SLOT),
    };
    // Cada motivo manda a hacer algo distinto, y por eso no se aplanan todos a
    // "no esta": quien escribe `lee apps/` tiene que enterarse de que eso es
    // una carpeta, no ponerse a buscar un archivo que nunca existio.
    // ** ESTRATOS PRIMERO, Y SI NO ESTA AHI, FAT32.
    //
    // La regla no es de aqui: la lleva usando `task::launch::Fuente::abrir`
    // para localizar un binario. Aplicarla tambien al abrir es lo que hace que
    // abrir un fichero y ejecutarlo resuelvan al MISMO fichero -- dos reglas
    // distintas para la misma ruta seria la peor de las opciones.
    //
    // El fichero entra ENTERO en el buffer y la ranura NO refleja: un fichero de
    // ESTRATOS no es una cadena que se pueda pedir a trozos, y hoy mide como
    // mucho 96 bytes. El porque, y cuando habra que volver a mirarlo, en
    // `obj/estratos.rs`.
    if let Some((nodo, mide)) = super::estratos::buscar(ruta) {
        unsafe {
            if !reserve(i, mide.max(1)) {
                crate::ring0::cabina::warn("arch", "sin RAM para el fichero de ESTRATOS", mide as u64);
                return Err(ERROR_TOO_LARGE);
            }
            let leidos = super::estratos::leer(&nodo, buf(i));
            REFLEJO[i] = false;
            LARGO[i] = leidos;
            CURSOR[i] = 0;
            WRITES[i] = false;
            DESBORDO[i] = false;
            super::cargando::LOAD_CLUSTER[i] = 0;
            OWNER[i] = pid;
            return match cap::grant(pid, cap::KIND_ARCHIVO, cap::RIGHT_READ, i as u64) {
                Some(h) => {
                    crate::ring0::cabina::bytes("arch", "archivo de ESTRATOS leido", leidos as u64);
                    Ok(h)
                }
                None => {
                    release(i);
                    Err(cap::ERROR_PERMISSION_DENIED)
                }
            };
        }
    }
    use crate::ring0::fsys::fs::LoadError;
    // Se resuelve la ruta UNA vez y se guarda por donde empieza. Es lo unico
    // que hay que hacer una sola vez; todo lo demas se hace cuando hace falta.
    let (cursor, mide) = match crate::ring0::fsys::fs::abrir_rangos(ruta) {
        Ok(v) => v,
        Err(LoadError::BadPath) => return Err(ERROR_IS_DIRECTORY),
        Err(LoadError::NameTooLong) => return Err(ERROR_NAME),
        Err(LoadError::DirNotFound) => return Err(ERROR_DIRECTORY),
        Err(_) => return Err(ERROR_NOT_THERE),
    };
    let mide = mide as usize;
    unsafe {
        // La ventana, no el archivo. Un fichero mas pequeno que ella entra
        // entero en la primera lectura y todo esto se comporta como antes.
        if !reserve(i, mide.min(WINDOW)) {
            // Ya no puede pasar por el TAMANO del archivo -- son dieciseis
            // marcos como mucho. Si pasa, es que no queda RAM contigua ni para
            // eso, y entonces el sistema tiene un problema mas grande.
            crate::ring0::cabina::warn("arch", "sin RAM para la ventana del archivo", mide as u64);
            return Err(ERROR_TOO_LARGE);
        }
        REFLEJO[i] = true;
        CUR[i] = cursor;
        START[i] = cursor;
        REF_AL_ABRIR[i] = BYTES_REFLEJADOS;
        RET_AL_ABRIR[i] = RETROCESOS;
        WINDOW_OFF[i] = 0;
        WINDOW_LEN[i] = 0;
        super::cargando::LOAD_CLUSTER[i] = 0;
        LARGO[i] = mide;
        CURSOR[i] = 0;
        WRITES[i] = false;
        DESBORDO[i] = false;
        OWNER[i] = pid;
        match cap::grant(pid, cap::KIND_ARCHIVO, cap::RIGHT_READ, i as u64) {
            Some(h) => {
                crate::ring0::cabina::bytes("arch", "archivo REFLEJADO para leer", mide as u64);
                Ok(h)
            }
            None => {
                release(i);
                Err(cap::ERROR_PERMISSION_DENIED)
            }
        }
    }
}

/// **Abre un archivo y NO lo termina de leer.**
///
/// Devuelve el handle en cuanto sabe que el archivo existe y que hay sitio para
/// el. Los bytes llegan despues, un trozo por cada vez que alguien pregunte
/// (ver [`avanzar`]).
///
/// === Que gana quien lo use ===
///
/// Con `open`, el que pide un `.bex` de 813 KB **deja de existir** durante toda
/// la lectura: esta dentro de una funcion de Ring 0 y nadie puede hacer nada por
/// el. Si el que pide es el escritorio, el escritorio no pinta.
///
/// Con esto vuelve enseguida, y entre trozo y trozo puede **dormirse** sobre su
/// propio handle -- el `wait` sabe hacerlo-- mientras el resto del sistema corre.
/// Es la mitad de Ring 3 del escalon 4.
///
/// El handle sale con `RIGHT_WAIT` ademas de lectura, y es lo que le da sentido:
/// sin ese derecho el unico modo de esperar seria preguntar en un bucle, que es
/// exactamente lo que se estaba quitando.
pub fn abrir_asinc(pid: u32, ruta: &str) -> Result<u64, u32> {
    let i = match free_slot() {
        Some(i) => i,
        None => return Err(ERROR_NO_FREE_SLOT),
    };
    use crate::ring0::fsys::fs::LoadError;
    let (cluster, mide) = match crate::ring0::fsys::fs::abrir_trozos(ruta) {
        Ok(v) => v,
        Err(LoadError::BadPath) => return Err(ERROR_IS_DIRECTORY),
        Err(LoadError::NameTooLong) => return Err(ERROR_NAME),
        Err(LoadError::DirNotFound) => return Err(ERROR_DIRECTORY),
        Err(_) => return Err(ERROR_NOT_THERE),
    };
    let mide = mide as usize;
    unsafe {
        if !reserve(i, mide) {
            crate::ring0::cabina::warn("arch", "sin RAM contigua para el archivo", mide as u64);
            return Err(ERROR_TOO_LARGE);
        }
        LARGO[i] = 0;
        CURSOR[i] = 0;
        WRITES[i] = false;
        DESBORDO[i] = false;
        OWNER[i] = pid;
        super::cargando::LOAD_TOTAL[i] = mide;
        // Un archivo vacio ya esta entero: nunca hay carga en curso para el, y
        // marcarla dejaria a quien pregunte esperando un trozo que no existe.
        super::cargando::LOAD_CLUSTER[i] = if mide == 0 { 0 } else { cluster };
        match cap::grant(pid, cap::KIND_ARCHIVO, cap::RIGHT_READ | cap::RIGHT_WAIT, i as u64) {
            Some(h) => {
                crate::ring0::cabina::info("arch", "archivo abierto SIN terminar de leer", mide as u64);
                Ok(h)
            }
            None => {
                super::cargando::LOAD_CLUSTER[i] = 0;
                release_buffer(i);
                OWNER[i] = NO_OWNER;
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
pub fn create(pid: u32, ruta: &str) -> Result<u64, u32> {
    if !crate::ring0::fsys::fs::data_mounted() {
        return Err(ERROR_READ_ONLY);
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
        return Err(ERROR_NAME);
    }
    let name = match crate::ring0::fsys::fs::nombre_8_3_pub(nombre_txt) {
        Some(n) => n,
        None => return Err(ERROR_NAME),
    };
    let dir = match crate::ring0::fsys::fs::dir_datos(carpeta) {
        Some(c) => c,
        // La carpeta, no el archivo. `escribe datos/x.txt` cuando no hay
        // `datos/` tiene que decir que falta la CARPETA: el archivo es
        // justamente lo que se venia a crear.
        None => return Err(ERROR_DIRECTORY),
    };

    let i = match free_slot() {
        Some(i) => i,
        None => return Err(ERROR_NO_FREE_SLOT),
    };
    unsafe {
        // La primera reserva. No es un techo: `write` dobla cuando se llena.
        if !reserve(i, INITIAL) {
            crate::ring0::cabina::warn("arch", "sin RAM para el buffer de escritura", 0);
            return Err(ERROR_TOO_LARGE);
        }
        LARGO[i] = 0;
        CURSOR[i] = 0;
        NAME[i] = name;
        DIRECTORIO[i] = dir;
        WRITES[i] = true;
        DESBORDO[i] = false;
        OWNER[i] = pid;
        // Se conceden los dos derechos: `invoke` resuelve con RIGHT_READ, asi
        // que sin el ni siquiera llegaria el `ESCRIBIR`. Lo que impide leer un
        // archivo de escritura no es el derecho, es el modo -- ver `operation`.
        match cap::grant(pid, cap::KIND_ARCHIVO, cap::RIGHT_READ | cap::RIGHT_WRITE, i as u64) {
            Some(h) => {
                crate::ring0::cabina::info("arch", "archivo abierto para escribir", pid as u64);
                Ok(h)
            }
            None => {
                OWNER[i] = NO_OWNER;
                Err(cap::ERROR_PERMISSION_DENIED)
            }
        }
    }
}

fn read(i: usize) -> u64 {
    unsafe {
        let mut w = [0u8; 8];
        let mut n = 0usize;
        while n < 7 {
            let b = match byte_en(i, CURSOR[i]) {
                Some(b) => b,
                None => break,
            };
            w[n] = b;
            CURSOR[i] += 1;
            n += 1;
        }
        ((n as u64) << 56) | u64::from_le_bytes(w)
    }
}

/// Como `read`, pero se para en el salto de linea y lo consume.
fn read_line(i: usize) -> u64 {
    unsafe {
        let mut w = [0u8; 8];
        let mut n = 0usize;
        let mut fin = 0u64;
        while n < 7 {
            let b = match byte_en(i, CURSOR[i]) {
                Some(b) => b,
                None => break,
            };
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

fn write(i: usize, palabra: u64) -> u64 {
    let n = ((palabra >> 56) & 0xFF) as usize;
    let n = n.min(7);
    let bytes = palabra.to_le_bytes();
    unsafe {
        let mut puestos = 0usize;
        for k in 0..n {
            // Sin sitio: se DOBLA el buffer. Antes aqui se levantaba la bandera
            // de desbordado contra un techo de 4 KiB; ahora solo se levanta si
            // de verdad no queda RAM, que es un motivo y no una constante.
            if LARGO[i] >= capacity(i) && !grow(i, LARGO[i] + 1) {
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
fn close(i: usize) -> u64 {
    unsafe {
        let ok = if WRITES[i] {
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
                match crate::ring0::fsys::fs::guardar_en(DIRECTORIO[i], &NAME[i], datos) {
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
        release(i);
        ok as u64
    }
}

fn release(i: usize) {
    unsafe {
        // ** LA MEDIDA, AL SOLTAR: cuanto de este archivo hizo falta de verdad.
        //
        // Se dice aqui y no en `close` porque un proceso que muere con el
        // fichero abierto pasa por el mismo sitio, y ese es justo el caso en el
        // que interesa saber por donde iba.
        if REFLEJO[i] && BYTES_REFLEJADOS > REF_AL_ABRIR[i] {
            crate::ring0::cabina::bytes(
                "arch",
                "bytes traidos de este archivo",
                BYTES_REFLEJADOS - REF_AL_ABRIR[i],
            );
            if RETROCESOS > RET_AL_ABRIR[i] {
                crate::ring0::cabina::count(
                    "arch",
                    "veces que hubo que volver al principio",
                    RETROCESOS - RET_AL_ABRIR[i],
                );
            }
        }
        // La memoria se devuelve AQUI y en un solo sitio, pase lo que pase con
        // el guardado. Un archivo que no se pudo escribir no es motivo para
        // quedarse con sus marcos: eso es una fuga que solo se nota tras
        // muchas horas, que es cuando peor se encuentra.
        release_buffer(i);
        OWNER[i] = NO_OWNER;
        LARGO[i] = 0;
        CURSOR[i] = 0;
        WRITES[i] = false;
        DESBORDO[i] = false;
        DIRECTORIO[i] = 0;
        // El reflejo tambien se apaga aqui, y en el mismo sitio que todo lo
        // demas: una ranura que se reutiliza con `REFLEJO` puesto y un cursor
        // del archivo anterior leeria **otro fichero** sin que nada avise.
        REFLEJO[i] = false;
        CUR[i] = bmo_fat32::Cursor::vacio();
        START[i] = bmo_fat32::Cursor::vacio();
        WINDOW_OFF[i] = 0;
        WINDOW_LEN[i] = 0;
        super::cargando::LOAD_CLUSTER[i] = 0;
    }
}

pub fn operation(idx: u64, op: u64, arg0: u64) -> Option<u64> {
    let i = idx as usize;
    if i >= MAX_ABIERTOS {
        return None;
    }
    let escribe = unsafe { WRITES[i] };
    // ** PREGUNTAR POR EL ARCHIVO ES LO QUE LO TRAE.
    //
    // Cualquier operacion de lectura empuja un trozo antes de contestar. Asi un
    // programa que ignore por completo que existe la carga a trozos --que son
    // todos los de hoy-- funciona igual: pide bytes, y los bytes acaban
    // llegando. La diferencia es que ahora **entre trozo y trozo puede dormir**,
    // en vez de estar dentro del kernel hasta el final.
    if !escribe && super::cargando::hay(i) && op != ARCH_OP_CERRAR {
        super::cargando::avanzar(i);
    }
    match op {
        ARCH_OP_LISTO => Some(unsafe {
            let entero = if super::cargando::hay(i) { 0u64 } else { 1u64 << 63 };
            entero | LARGO[i] as u64
        }),
        // El modo manda. Pedirle bytes a un archivo de escritura no es un
        // error de permisos: es una pregunta que ese objeto no responde.
        ARCH_OP_LEER if !escribe => Some(read(i)),
        ARCH_OP_LEER_LINEA if !escribe => Some(read_line(i)),
        ARCH_OP_ESCRIBIR if escribe => Some(write(i, arg0)),
        ARCH_OP_TAMANO => Some(unsafe {
            if escribe { LARGO[i] as u64 } else { (LARGO[i] - CURSOR[i]) as u64 }
        }),
        // * `fseek`. **Sigue costando lo que cuesta poner un numero**, y ahora
        // por otro motivo: el archivo ya no esta en el bufer, pero mover el
        // cursor tampoco lee nada. El disco solo se toca cuando alguien pide
        // bytes de verdad -- y si el salto fue hacia atras, quien lo paga es esa
        // lectura y no este salto (ver `reflejar`).
        //
        // Se acota al tamano en vez de rechazar: un cursor mas alla del final
        // significa "no queda nada", que es lo que contesta `ARCH_OP_TAMANO` sin
        // inventarse un error.
        ARCH_OP_SALTAR if !escribe => Some(unsafe {
            let d = if arg0 as usize > LARGO[i] { LARGO[i] } else { arg0 as usize };
            CURSOR[i] = d;
            d as u64
        }),
        ARCH_OP_CERRAR => Some(close(i)),
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
pub unsafe fn read_into(idx: u64, dst: *mut u8, n: usize) -> usize {
    let i = idx as usize;
    if i >= MAX_ABIERTOS || dst.is_null() || n == 0 {
        return 0;
    }
    unsafe {
        if WRITES[i] {
            return 0;
        }
        let quedan = LARGO[i].saturating_sub(CURSOR[i]);
        let cuantos = if n > quedan { quedan } else { n };
        if cuantos == 0 {
            return 0;
        }
        // ** DEL DISCO AL BLOQUE DEL QUE PREGUNTA, SIN ESCALA.
        //
        // No se pasa por la ventana **a proposito**: un lump de 40 KB copiado a
        // la ventana y de ahi al bloque son 40 KB movidos dos veces para nada, y
        // uno mas grande que la ventana ni siquiera cabria. `fs::leer_rango`
        // deja los sectores enteros donde se le diga -- que es justo para lo que
        // se escribio.
        if REFLEJO[i] {
            let salida = core::slice::from_raw_parts_mut(dst, cuantos);
            let got = reflejar(i, CURSOR[i], salida);
            CURSOR[i] += got;
            return got;
        }
        core::ptr::copy_nonoverlapping(buf(i).as_ptr().add(CURSOR[i]), dst, cuantos);
        CURSOR[i] += cuantos;
        cuantos
    }
}

/// Mete `n` bytes de golpe en un archivo de escritura. Devuelve cuantos entraron.
///
/// El espejo de [`read_into`]. La diferencia esta en el buffer: leer trabaja
/// contra uno que ya tiene el fichero entero, y escribir tiene que **crecer**.
/// Se pide el sitio UNA VEZ para todo el bloque en vez de byte a byte -- con
/// `grow` doblando, escribir 300 KiB de siete en siete pediria memoria unas
/// setenta veces por el camino.
///
/// Si no hay RAM se levanta la bandera de desbordado y **no se guarda nada** al
/// cerrar. Es la misma regla que ya tenia `write`: un archivo a medias es peor
/// que ninguno, porque parece bueno.
///
/// # Safety
///
/// `src` tiene que apuntar a `n` bytes legibles. Quien llama es `syscall.rs`,
/// que lo saca de un bloque `KIND_MEMORIA` del propio proceso tras comprobar el
/// rango contra lo que el kernel le entrego.
pub unsafe fn write_from(idx: u64, src: *const u8, n: usize) -> usize {
    let i = idx as usize;
    if i >= MAX_ABIERTOS || src.is_null() || n == 0 {
        return 0;
    }
    unsafe {
        if !WRITES[i] {
            return 0;
        }
        let necesita = match LARGO[i].checked_add(n) {
            Some(v) => v,
            None => return 0,
        };
        if necesita > capacity(i) && !grow(i, necesita) {
            DESBORDO[i] = true;
            crate::ring0::cabina::warn(
                "arch",
                "sin RAM para seguir escribiendo: no se guardara nada",
                necesita as u64,
            );
            return 0;
        }
        core::ptr::copy_nonoverlapping(src, buf(i).as_mut_ptr().add(LARGO[i]), n);
        LARGO[i] = necesita;
        n
    }
}

/// Lo llama `cap::revoke_all`. Un proceso que muere con un archivo de
/// escritura a medias **no deja nada**: lo acumulado se tira. Guardarlo seria
/// inventar un archivo que su autor nunca dio por terminado.
/// Cuantos archivos siguen abiertos a nombre de `pid`. Cero despues de
/// `process_died`. Ver `directory::pending_of` -- son cuatro ranuras por
/// proceso y una fuga aqui se nota mucho antes.
pub fn pending_of(pid: u32) -> u32 {
    let mut n = 0;
    unsafe {
        for i in 0..MAX_ABIERTOS {
            if OWNER[i] == pid {
                n += 1;
            }
        }
    }
    n
}

pub fn process_died(pid: u32) {
    unsafe {
        for i in 0..MAX_ABIERTOS {
            if OWNER[i] == pid {
                if WRITES[i] && LARGO[i] > 0 {
                    crate::ring0::cabina::warn("arch", "murio sin cerrar: se descarta", LARGO[i] as u64);
                }
                release(i);
            }
        }
    }
}
