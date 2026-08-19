//! ESTRATOS montado: el kernel leyendo su propio sistema de ficheros.
//!
//! Paso 4d del orden de construccion. **Solo lectura, y es estructural**: este
//! modulo no llama a `write` en ningun sitio. Escribir es el paso 5 y necesita
//! el log, las barreras y el recolector; nada de eso existe todavia, y un
//! ESTRATOS que escribe a medias no es medio sistema de ficheros, es uno roto.
//!
//! ## Habla con el contrato, no con SATA
//!
//! Todo lo de aqui pasa por [`bmo_block::device()`]. Es lo que el diseno pedia
//! en su section 10.3 y la razon de que la capa de bloques se hiciera antes: el dia
//! que haya un NVMe cableado, este modulo no se entera.
//!
//! ## Donde busca
//!
//! No se le dice en que particion esta: se le pregunta al disco. Se recorren
//! las particiones de la GPT leyendo su primer bloque, y la que lleve la firma
//! `ESTRATOS` gana. Las demas devuelven `BadMagic`, que significa "aqui no hay
//! un volumen ESTRATOS" y **no** "esta corrupto" -- distinguir las dos cosas es
//! lo que permite mirar una NTFS sin dar un susto.

use bmo_estratos as es;
use bmo_estratos::objects::{Attr, BlockPtr, Entrada, Tipo, BLOQUE, ENTRADA_LEN};
use crate::ring0::dev::disk;

/// FOLLOWING THE POINTERS: given a block number, bring the block. No policy,
/// and **walking never writes** -- a bug here cannot corrupt a volume.
pub(crate) mod walk;
pub(crate) use walk::*;
/// DIRECTORIES: the only layer of ESTRATOS that deals in NAMES.
pub(crate) mod dir;
pub(crate) use dir::*;
/// ** CREAR UN FICHERO: la mitad que toca el disco. El QUE bytes vive en
/// `bmo_estratos::escritura` y se prueba en el anfitrion; aqui solo se pide
/// sitio, se escriben cuatro bloques y se cierra.
pub mod escribir;
/// ** TRAER UN FICHERO DE FUERA: el contenido lo lee el kernel y no cruza el
/// renglon de 96 bytes del syscall. Es lo que hace util el techo levantado.
pub(crate) mod copiar;
/// LA HISTORIA DEL VOLUMEN: la cadena de versiones hacia atras. Existia en el
/// disco desde el primer dia --cada estrato guarda su padre-- y no tenia puerta.
pub(crate) mod historia;
/// UN NIVEL de la ruta: su nodo, su listado y el detalle de cada hijo, leidos
/// UNA VEZ al pasar por el. Es lo que permite pintar un arbol y lo que quita el
/// martillo sobre el disco en cada repintado.
mod nivel;
/// THE CURSOR: ESTRATOS driven from Ring 3, through TWO operations and not ten.
#[path = "cursor.rs"]
mod cursor_file;
pub use cursor_file::*;

/// Un nodo abierto de ESTRATOS. **Se re-exporta porque `open` lo devuelve.**
///
/// Estaba importado en privado, asi que `est::open()` entregaba un valor cuyo
/// tipo no se podia nombrar desde fuera del modulo: quien lo recibiera solo
/// podia guardarlo en un `let` con inferencia, y en el momento en que hiciera
/// falta meterlo en un campo o en un `enum` --que es justo lo que pide
/// `task::launch::Fuente`-- no compilaba.
///
/// Una funcion publica que devuelve un tipo privado es una API a medias: deja
/// pasar el valor y no la palabra con la que llamarlo.
pub use bmo_estratos::objects::Nodo;

/// Sectores de 512 B por bloque de ESTRATOS.
pub(crate) const SECTORES_POR_BLOQUE: u16 = (BLOQUE / 512) as u16;

// -- Estado del montaje ------------------------------------------------------

pub(crate) static mut MONTADO: bool = false;
pub(crate) static mut BASE_LBA: u64 = 0;
pub(crate) static mut PARTICION: u32 = 0;
pub(crate) static mut SUPER: Option<es::Superblock> = None;
/// El volumen dice haber nacido en ESTE disco?
pub(crate) static mut IDENTIDAD_OK: bool = false;

/// Buffers de trabajo, uno por nivel de indireccion.
///
/// Estaticos y no locales: bajar por un arbol de cuatro niveles con un buffer
/// de 4 KiB en cada marco son 16 KiB de pila, y la del kernel son 64 KiB para
/// todo. Aqui el gasto es fijo, visible y esta en `.bss`.
pub(crate) const NIVELES: usize = 5; // NIVELES_MAX + 1
pub(crate) static mut SCRATCH: [[u8; BLOQUE]; NIVELES] = [[0u8; BLOQUE]; NIVELES];

pub fn is_mounted() -> bool { unsafe { MONTADO } }
pub fn particion() -> u32 { unsafe { PARTICION } }
pub fn base_lba() -> u64 { unsafe { BASE_LBA } }
pub fn identidad_ok() -> bool { unsafe { IDENTIDAD_OK } }
pub fn superbloque() -> Option<es::Superblock> { unsafe { SUPER } }

/// Cuanto del volumen esta usado, y en que nivel de aviso esta.
///
/// * La cuenta es una resta porque ESTRATOS reserva con un puntero que **solo
/// avanza**: `log_head` es el primer bloque libre, asi que todo lo de debajo
/// esta usado. No hay mapa de bits ni fragmentacion que medir, y eso es
/// consecuencia directa de no sobreescribir nunca.
///
/// La politica --donde caen el ambar, el rojo y el solo-lectura-- vive en
/// `bmo_estratos::espacio` y **se prueba en el anfitrion**. Aqui solo se le
/// pasan los dos numeros que tiene el superbloque: un umbral escrito a mano en
/// el kernel es un umbral que nadie puede ejecutar en un test.
pub fn ocupacion() -> Option<es::Ocupacion> {
    let sb = unsafe { SUPER }?;
    Some(es::Ocupacion::de(sb.log_head, sb.total_blocks, sb.block_size))
}

/// **LA COLA LIBRE DEL VOLUMEN, en LBA absolutos.** `(lba, sectores)`.
///
/// === Que es exactamente, y por que se puede afirmar ===
///
/// Todo lo que hay **por encima de `log_head`** en un volumen ESTRATOS. Y no es
/// una estimacion: `log_head` es el primer bloque libre de un puntero que **solo
/// avanza**, asi que por encima de el no hay un solo bloque que ningun estrato
/// alcance. No hace falta recorrer nada para saberlo -- es la misma resta que
/// [`ocupacion`], leida al reves.
///
/// === Para que existe: es la mitad HONESTA del recolector ===
///
/// La seccion 9 del diseno tiene dos trabajos dentro y conviene no confundirlos:
///
/// ```text
///   marcar lo alcanzable y soltar lo demas   <- eso es el RECOLECTOR, y no existe
///   decirle al disco que lo libre es libre   <- esto, y hoy ya se puede
/// ```
///
/// Lo segundo no necesita al primero y ya hace falta: sin ello el SSD sigue
/// creyendo vivos --y copiando en cada recogida interna suya-- todos los bloques
/// que este volumen no ha usado nunca (R-DISCO10). Es lo que en otros sistemas
/// hace un `fstrim`, y aqui se puede decir con precision porque **la frontera es
/// un numero del superbloque**, no un mapa que haya que reconstruir.
///
/// [!] `None` cuando no hay volumen montado o cuando la cola esta vacia. Y el
/// rango se acota al TOTAL que declara el superbloque: si un volumen dijera
/// tener mas bloques que la particion, quien lo pare es la ventana de escritura
/// --que mira el rango entero-- y no una resta optimista de aqui.
pub fn cola_libre() -> Option<(u64, u64)> {
    let sb = unsafe { SUPER }?;
    if !unsafe { MONTADO } {
        return None;
    }
    let libres = sb.total_blocks.checked_sub(sb.log_head)?;
    if libres == 0 {
        return None;
    }
    let base = unsafe { BASE_LBA };
    let lba = base.checked_add(sb.log_head.checked_mul(SECTORES_POR_BLOQUE as u64)?)?;
    let sectores = libres.checked_mul(SECTORES_POR_BLOQUE as u64)?;
    Some((lba, sectores))
}

/// Lee un bloque de ESTRATOS del volumen montado.
pub(crate) fn read_block(bloque: u64, dst: &mut [u8; BLOQUE]) -> bool {
    let dev = match bmo_block::device() { Some(d) => d, None => return false };
    let lba = unsafe { BASE_LBA } + bloque * SECTORES_POR_BLOQUE as u64;
    matches!(dev.read(lba, SECTORES_POR_BLOQUE, dst), Ok(n) if n == SECTORES_POR_BLOQUE)
}

/// Igual, pero sobre una particion concreta -- para sondear antes de montar.
pub(crate) fn read_block_from(base: u64, bloque: u64, dst: &mut [u8; BLOQUE]) -> bool {
    let dev = match bmo_block::device() { Some(d) => d, None => return false };
    let lba = base + bloque * SECTORES_POR_BLOQUE as u64;
    matches!(dev.read(lba, SECTORES_POR_BLOQUE, dst), Ok(n) if n == SECTORES_POR_BLOQUE)
}

/// **Escribe un bloque de ESTRATOS.** El espejo exacto de [`read_block_from`].
///
/// El indice es de BLOQUE, no de sector: la conversion vive aqui y en un solo
/// sitio, porque mezclar las dos unidades es como se escribe ocho veces mas
/// lejos de donde se queria.
///
/// Delante hay dos guardianes que este archivo **no** implementa y de los que
/// depende: el gate de identidad del disco (`disk::write_armed`) y la ventana
/// de la particion (`disk::write_window`). Aqui no se repiten -- repetir un
/// guardian es tener dos sitios donde relajarlo.
pub(crate) fn write_block(bloque: u64, src: &[u8; BLOQUE]) -> bool {
    let base = unsafe { BASE_LBA };
    let lba = base + bloque * SECTORES_POR_BLOQUE as u64;
    disk::write(lba, SECTORES_POR_BLOQUE, src) == SECTORES_POR_BLOQUE
}

/// **Escribe el superbloque: UN SOLO SECTOR.** Y ese es el punto entero.
///
/// === Por que no se reutiliza [`write_block`] ===
///
/// Porque escribiria **ocho sectores**, y con eso se cae la unica garantia
/// sobre la que esta construido ESTRATOS. La cabecera de
/// `bmo_estratos::escritura` lo dice sin ambiguedad:
///
/// > *el punto de no retorno cabe en **un solo sector**, que es la unidad que
/// > el disco garantiza atomica.*
///
/// Un `Superblock` mide [`es::SUPER_LEN`] = **512 bytes**, o sea exactamente un
/// sector. Escribir el bloque de 4 KiB que lo contiene convierte el commit en
/// una escritura de ocho sectores: si el corte llega a mitad, el disco puede
/// haber puesto unos si y otros no. Los siete de relleno no importan -- pero
/// **el commit deja de ser una operacion atomica y pasa a ser ocho**, y toda la
/// transaccion se apoyaba en que no lo fuera.
///
/// Y hay un segundo motivo, mas silencioso: escribir el bloque entero **pone a
/// cero los 3.5 KiB restantes**. Hoy ahi no hay nada, asi que no se nota; el
/// dia que el formato guarde algo detras del superbloque, esto se lo comeria
/// sin decir una palabra.
///
/// La regla que deja: **la unidad de escritura la decide la garantia que hace
/// falta, no el tamano del buffer que hay a mano.**
pub(crate) fn write_superblock(bloque: u64, sb: &[u8; es::SUPER_LEN]) -> bool {
    let base = unsafe { BASE_LBA };
    let lba = base + bloque * SECTORES_POR_BLOQUE as u64;
    disk::write(lba, 1, sb) == 1
}

/// Por que no se pudo cerrar una transaccion. Cada una manda a mirar otra cosa.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum WriteError {
    /// No hay volumen montado.
    SinVolumen,
    /// La maquina de estados dijo que no. Trae su motivo.
    Rechazada(es::escritura::Rechazo),
    /// El disco no acepto los sectores del superbloque.
    NoEscribio,
    /// **El `FLUSH CACHE` fallo**, y por eso NO se hizo el commit.
    SinBarrera,
    /// El contenido no cabe donde se pidio ponerlo.
    ///
    /// ** Hoy significa **mas de 96 bytes**, que es lo que entra dentro del
    /// nodo. Se rechaza en vez de partirlo en bloques a escondidas: partir es
    /// otra operacion, con su arbol y sus niveles, y hacerla sin decirlo dejaria
    /// al que llama sin saber cuanto va a costar.
    NoCabe,
    /// **No se pudieron leer las entradas que YA tiene la raiz.**
    ///
    /// Es el peor de todos y por eso tiene nombre propio: escribir sin ellas
    /// dejaria un directorio con una sola entrada y el resto huerfano. Se para
    /// ANTES de abrir la transaccion, o sea sin haber tocado un solo sector.
    NoSeLeeLaRaiz,
    /// **Un tramo de la ruta no existe, o existe y no es un directorio.**
    ///
    /// Se distingue de `NoCabe` porque manda a hacer algo distinto: aqui no hay
    /// nada que encoger, hay que crear la carpeta o escribir bien la ruta.
    RutaNoEsta,
    /// **La carpeta esta llena.**
    ///
    /// ** Esto contestaba `NoCabe`, o sea *"no cabe: hoy un fichero entra en 96
    /// bytes"* -- y era MENTIRA: el fichero cabia de sobra, la que no admitia
    /// una entrada mas era la carpeta. Dos limites distintos con el mismo
    /// mensaje mandan a encoger el fichero, que no arregla nada.
    ///
    /// El tope es `ENTRADAS_POR_BLOQUE` (36) mientras `:entradas` viva en UN
    /// bloque sin indireccion. No es del formato: es de esta version.
    CarpetaLlena,
    /// **LA CARPETA YA TIENE MAS ENTRADAS DE LAS QUE SE LEEN DE UNA VEZ.**
    ///
    /// ** No es `CarpetaLlena`, y confundirlos seria repetir el error del
    /// 19-08 con otro disfraz. `CarpetaLlena` dice *"no cabe una mas"* y manda
    /// a borrar algo. Esto dice *"lo que ya hay no lo se leer entero"*, y ahi
    /// borrar no arregla nada: la carpeta la creo el formateador del anfitrion,
    /// que no tiene el tope de un bloque, y **esta version del kernel no puede
    /// republicarla sin dejarse nombres fuera**.
    ///
    /// === Por que esto se para en vez de seguir ===
    ///
    /// `leer_entradas` lee a UN bloque y el lector de flujos trunca en silencio
    /// --a proposito: el panel que pinta un listado quiere lo que quepa--. Con
    /// la lista cortada, republicar publicaria la carpeta **sin las entradas de
    /// la 37 en adelante**. El arbol viejo seguiria entero, que es el consuelo
    /// del copy-on-write, pero el vivo perderia nombres.
    ///
    /// Hoy eso lo tumbaba una casualidad --4096 no es multiplo de 112, asi que
    /// el corte deja 64 bytes de sobra y la crate del formato lo rechaza-- y el
    /// gesto moria con `NombreNoVale` o `RutaNoEsta`: **dos mensajes que mandan
    /// a mirar donde no es**. Una garantia de datos no puede depender de una
    /// division que no sale exacta.
    ///
    /// El tope es de ESTA version, no del formato: `:entradas` con indireccion
    /// lo levanta, y es el mismo trabajo que ya hizo `flujo` para el contenido.
    CarpetaNoCabeEntera,
    /// El nombre no vale para lo que se pidio: repetido al crear, ausente al
    /// borrar, o ya ocupado al renombrar.
    ///
    /// Los tres los rechaza `bmo_estratos::escritura` con el mismo error, y
    /// aqui llegan juntos. Separarlos pide errores mas ricos en la crate del
    /// formato -- se dice en vez de inventarse cual de los tres fue.
    NombreNoVale,
    /// La ruta baja mas de lo que esta version sabe republicar.
    ///
    /// ** Tocar una hoja republica la rama ENTERA hasta la raiz, asi que la
    /// profundidad no es un capricho: es cuantos bloques cuesta la operacion.
    /// El tope se dice en vez de descubrirse con una reserva que no cabe.
    MuyHondo,
}

impl WriteError {
    pub fn name(self) -> &'static str {
        match self {
            WriteError::SinVolumen => "no hay volumen ESTRATOS montado",
            WriteError::Rechazada(r) => r.name(),
            WriteError::NoEscribio => "el disco no acepto el superbloque",
            WriteError::SinBarrera => "el FLUSH CACHE fallo: NO se hizo commit",
            WriteError::NoCabe => "no cabe: hoy un fichero entra en 96 bytes",
            WriteError::NoSeLeeLaRaiz => "no se pudieron leer las entradas de la raiz",
            WriteError::RutaNoEsta => "esa ruta no existe, o no es un directorio",
            WriteError::MuyHondo => "la ruta baja demasiado para republicarla",
            WriteError::CarpetaLlena => "la carpeta esta llena: 36 entradas es el tope de hoy",
            WriteError::CarpetaNoCabeEntera => {
                "esa carpeta pasa de 36 entradas: esta version no la sabe republicar"
            }
            WriteError::NombreNoVale => "ese nombre no vale: repetido, ausente o ya ocupado",
        }
    }
}

/// **SELLAR: la transaccion mas pequena que existe, y la que estrena el camino.**
///
/// === Que hace ===
///
/// Una transaccion **sin datos**: no reserva ni un bloque, no toca ni un
/// objeto, y hace el commit apuntando **al mismo estrato que ya habia**. Lo
/// unico que cambia en el volumen es el numero de generacion, y se escribe en
/// **la copia del superbloque que NO esta en uso**.
///
/// === Por que esto primero, y no crear un archivo ===
///
/// Porque recorre el camino ENTERO --reservar, cerrar, `FLUSH CACHE`, barrera,
/// commit, escribir el superbloque alterno, volver a vaciar-- y **no puede
/// perder un dato aunque salga mal**:
///
/// - Si falla antes del commit, el volumen es exactamente el de antes.
/// - Si falla escribiendo el superbloque nuevo, se estropea **la copia que no
///   manda**; la que manda sigue entera y el volumen monta igual.
/// - Y como el estrato es el mismo, no hay ningun objeto nuevo al que apuntar
///   mal.
///
/// Es el instrumento antes que la teoria: si esto funciona, el camino de
/// escritura esta vivo y lo siguiente es solo poner datos dentro. Si no
/// funciona, se sabe **exactamente** donde falla y no hay nada que lamentar.
///
/// === La comprobacion, y es preciosa ===
///
/// Despues de seal, `F12` tiene que decir **`generacion 2`**. Y despues de
/// **reiniciar**, tiene que seguir diciendo 2 -- eso ultimo es lo que prueba que
/// llego al plato y no se quedo en la cache del SSD.
pub fn seal() -> Result<u64, WriteError> {
    let sb = match superbloque() {
        Some(s) => s,
        None => return Err(WriteError::SinVolumen),
    };
    // Cual de las dos copias mando al montar. `pick_superblock` eligio la de
    // generacion mas alta; aqui se deduce igual para no guardar otro estado que
    // pueda separarse del primero.
    let copia = copia_en_uso();

    let mut t = es::escritura::Transaccion::open(&sb, copia, identidad_ok())
        .map_err(WriteError::Rechazada)?;

    // Sin datos: se cierra la fase inmediatamente. `reserve(0)` no haria falta
    // y se omite a proposito -- una llamada que no hace nada en el camino que
    // estrena el disco es una llamada que confunde al leer el log.
    t.cerrar_datos().map_err(WriteError::Rechazada)?;

    // * LA BARRERA. No es opcional y no se puede fingir.
    //
    // Aqui no hay nada escrito todavia, asi que este `flush` no protege ningun
    // dato -- protege el ORDEN, y sobre todo prueba que el disco **sabe hacer la
    // barrera**. El dia que haya datos delante, este mismo `flush` es lo unico
    // que separa un commit honesto de un superbloque que apunta a bloques que
    // no llegaron al plato.
    if !disk::flush() {
        t.abandonar();
        return Err(WriteError::SinBarrera);
    }
    t.barrera_hecha().map_err(WriteError::Rechazada)?;

    let (destino, nuevo) = t.commit(sb.estrato).map_err(WriteError::Rechazada)?;

    // El superbloque serializado: 512 bytes, o sea UN SECTOR. Ver
    // `write_superblock` -- el tamano de esta escritura es la garantia de
    // atomicidad, no un detalle de implementacion.
    let sector = nuevo.encode();

    if !write_superblock(destino, &sector) {
        // El commit no ocurrio. La copia que manda sigue siendo la de antes, y
        // el volumen entero tambien.
        crate::ring0::cabina::fault("estratos", "no se pudo escribir el superbloque", destino);
        return Err(WriteError::NoEscribio);
    }

    // * Y VACIAR OTRA VEZ. El commit tampoco vale si se queda en la cache.
    //
    // Sin esto, apagar la maquina justo despues de "sellado" dejaria el volumen
    // en la generacion vieja -- y el mensaje en pantalla habria mentido. Un
    // commit que no se puede confirmar no es un commit, es una intencion.
    if !disk::flush() {
        crate::ring0::cabina::warn("estratos", "el commit no se pudo vaciar al plato", destino);
        return Err(WriteError::SinBarrera);
    }

    unsafe { SUPER = Some(nuevo) };
    crate::ring0::cabina::info("estratos", "COMMIT: generacion nueva", nuevo.generation);
    Ok(nuevo.generation)
}

/// **Deja fijado el superbloque nuevo tras un commit.**
///
/// Existe para que `escribir.rs` no toque el estatico: quien cambia el estado
/// montado es este modulo, y el que escribe le dice lo que quedo. Un `static
/// mut` que se asigna desde dos sitios es un estado con dos duenos.
pub(crate) fn fijar_superbloque(nuevo: es::Superblock) {
    unsafe { SUPER = Some(nuevo) };
}

/// **Cuando se hizo la version que manda ahora**, empaquetada como `INFO_FECHA`.
///
/// === Por que se guarda y no se lee cada vez ===
///
/// Sacarla del estrato cuesta **una lectura de bloque**, y quien la pregunta es
/// un panel que se repinta al mover el raton. Es el mismo martillo sobre el
/// disco que ya costo el detalle de cada hijo del cursor -- y ahi la leccion
/// quedo escrita: lo que solo cambia al escribir se lee al escribir.
///
/// Se pone en dos sitios y en ninguno mas: al montar y al commitear. Los dos son
/// exactamente los momentos en los que la version que manda cambia.
pub(crate) static mut ESTRATO_FECHA: u64 = 0;

/// Cuando se hizo la version en curso. `0` = no se sabe.
///
/// ** El cero no es "el ano cero": es **no hay fecha**, y hay que distinguirlo.
/// Un volumen escrito por una maquina sin reloj creible tiene versiones sin
/// fechar, y pintarlas como 1970 mentiria con mas conviccion que dejarlas en
/// blanco. Es la misma regla que ya sigue `clock::ahora`.
pub fn fecha_estrato() -> u64 {
    unsafe { ESTRATO_FECHA }
}

/// Relee la fecha del estrato en curso. Cuesta un bloque, y por eso se llama
/// solo al montar: despues la mantiene el propio commit.
pub(crate) fn refrescar_fecha() {
    unsafe { ESTRATO_FECHA = estrato().map_or(0, |e| e.tiempo) };
}

/// Cual de las dos copias del superbloque manda ahora mismo.
///
/// Se recalcula leyendo, en vez de guardarse al montar: dos fuentes de la misma
/// verdad se separan, y esta decide **donde se escribe el commit**. Equivocarse
/// aqui es pisar la copia buena.
pub(crate) fn copia_en_uso() -> u64 {
    let (a, b) = unsafe {
        let s = &mut *core::ptr::addr_of_mut!(SCRATCH);
        let (x, y) = s.split_at_mut(1);
        (&mut x[0], &mut y[0])
    };
    let base = unsafe { BASE_LBA };
    if !read_block_from(base, es::SUPER_A_BLOCK, a) || !read_block_from(base, es::SUPER_B_BLOCK, b) {
        return es::SUPER_A_BLOCK;
    }
    match es::pick_superblock(&a[..es::SUPER_LEN], &b[..es::SUPER_LEN]) {
        Ok((_, copia)) => copia,
        Err(_) => es::SUPER_A_BLOCK,
    }
}

/// Busca un volumen ESTRATOS entre las particiones del disco y lo monta.
pub fn mount() {
    unsafe { MONTADO = false; IDENTIDAD_OK = false; SUPER = None; }
    // * La ventana se quita ANTES de nada.
    //
    // Si este montaje falla, o encuentra otro volumen, o la identidad no
    // cuadra, no puede quedar en pie la ventana del montaje anterior. Una
    // autorizacion que sobrevive a la razon que la concedio es exactamente
    // como se escribe donde no se debe.
    disk::desarmar_ventana_estratos();
    if bmo_block::device().is_none() {
        crate::ring0::cabina::warn("estratos", "sin dispositivo de bloques", 0);
        return;
    }

    // Dos buffers: las dos copias del superbloque. Se leen ANTES de decidir,
    // porque `pick_superblock` necesita las dos para poder descartar la que
    // se quedo a medias.
    let (a, b) = unsafe {
        let s = &mut *core::ptr::addr_of_mut!(SCRATCH);
        let (x, y) = s.split_at_mut(1);
        (&mut x[0], &mut y[0])
    };

    for p in disk::partitions() {
        // La de arranque no se toca: ahi vive el BOOTX64.EFI con el que se
        // arranco, y ESTRATOS no vive nunca en ella (regla del section 2.6).
        if p.is_esp() { continue; }
        if !read_block_from(p.first_lba, es::SUPER_A_BLOCK, a) { continue; }
        if !read_block_from(p.first_lba, es::SUPER_B_BLOCK, b) { continue; }

        let (sb, copia) = match es::pick_superblock(&a[..es::SUPER_LEN], &b[..es::SUPER_LEN]) {
            Ok(v) => v,
            Err(es::FormatError::BadMagic) => continue, // aqui no hay ESTRATOS
            Err(e) => {
                // Magia correcta pero algo no cuadra: ESO si merece un grito.
                crate::ring0::cabina::fault("estratos", e.name(), p.first_lba);
                continue;
            }
        };

        unsafe {
            BASE_LBA = p.first_lba;
            PARTICION = p.index;
            SUPER = Some(sb);
            MONTADO = true;
        }
        // La fecha de la version que manda. Cuesta un bloque y se paga UNA vez,
        // al montar: a partir de aqui la mantiene el propio commit.
        refrescar_fecha();

        // El gate del section 5: nacio este volumen en el disco que tenemos delante?
        // Modelo, serie Y capacidad. Si no cuadra es un volumen clonado, y con
        // escritura seria un desastre; hoy solo se lee, asi que se avisa.
        let id = bmo_block::device().map(|d| es::disk_id_of(&d.identity())).unwrap_or([0u8; 32]);
        let ok = sb.belongs_to(&id);
        unsafe { IDENTIDAD_OK = ok; }
        if ok {
            crate::ring0::cabina::info("estratos", "volumen montado y es de este disco", p.first_lba);
            // * Y AQUI, y solo aqui, se abre la puerta de escribir.
            //
            // Las dos condiciones juntas: hay volumen y **nacio en este
            // disco**. Un volumen clonado se monta y se lee, pero no registra
            // ventana -- asi que sus escrituras las para `write_window` aunque
            // el disco este armado. Dos cerrojos distintos para dos preguntas
            // distintas: "es mi disco?" y "es mi volumen?".
            disk::armar_ventana_estratos(p.first_lba, p.last_lba);
        } else {
            crate::ring0::cabina::warn("estratos", "el volumen NO nacio en este disco (clonado?)", p.first_lba);
        }
        crate::ring0::cabina::info("estratos", "generacion del superbloque", sb.generation);
        let _ = copia;
        return;
    }

    crate::ring0::cabina::info("estratos", "ninguna particion tiene un volumen ESTRATOS", 0);
}
