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
use bmo_estratos::objects::{Attr, BlockPtr, Entrada, Nodo, Tipo, BLOQUE, ENTRADA_LEN, PTR_LEN};
use crate::ring0::dev::disk;

/// Sectores de 512 B por bloque de ESTRATOS.
const SECTORES_POR_BLOQUE: u16 = (BLOQUE / 512) as u16;

// -- Estado del montaje ------------------------------------------------------

static mut MONTADO: bool = false;
static mut BASE_LBA: u64 = 0;
static mut PARTICION: u32 = 0;
static mut SUPER: Option<es::Superblock> = None;
/// El volumen dice haber nacido en ESTE disco?
static mut IDENTIDAD_OK: bool = false;

/// Buffers de trabajo, uno por nivel de indireccion.
///
/// Estaticos y no locales: bajar por un arbol de cuatro niveles con un buffer
/// de 4 KiB en cada marco son 16 KiB de pila, y la del kernel son 64 KiB para
/// todo. Aqui el gasto es fijo, visible y esta en `.bss`.
const NIVELES: usize = 5; // NIVELES_MAX + 1
static mut SCRATCH: [[u8; BLOQUE]; NIVELES] = [[0u8; BLOQUE]; NIVELES];

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

/// Lee un bloque de ESTRATOS del volumen montado.
fn leer_bloque(bloque: u64, dst: &mut [u8; BLOQUE]) -> bool {
    let dev = match bmo_block::device() { Some(d) => d, None => return false };
    let lba = unsafe { BASE_LBA } + bloque * SECTORES_POR_BLOQUE as u64;
    matches!(dev.read(lba, SECTORES_POR_BLOQUE, dst), Ok(n) if n == SECTORES_POR_BLOQUE)
}

/// Igual, pero sobre una particion concreta -- para sondear antes de montar.
fn leer_bloque_de(base: u64, bloque: u64, dst: &mut [u8; BLOQUE]) -> bool {
    let dev = match bmo_block::device() { Some(d) => d, None => return false };
    let lba = base + bloque * SECTORES_POR_BLOQUE as u64;
    matches!(dev.read(lba, SECTORES_POR_BLOQUE, dst), Ok(n) if n == SECTORES_POR_BLOQUE)
}

/// **Escribe un bloque de ESTRATOS.** El espejo exacto de [`leer_bloque_de`].
///
/// El indice es de BLOQUE, no de sector: la conversion vive aqui y en un solo
/// sitio, porque mezclar las dos unidades es como se escribe ocho veces mas
/// lejos de donde se queria.
///
/// Delante hay dos guardianes que este archivo **no** implementa y de los que
/// depende: el gate de identidad del disco (`disk::write_armed`) y la ventana
/// de la particion (`disk::write_window`). Aqui no se repiten -- repetir un
/// guardian es tener dos sitios donde relajarlo.
#[allow(dead_code)] // lo estrena el primer objeto con datos
fn escribir_bloque(bloque: u64, src: &[u8; BLOQUE]) -> bool {
    let base = unsafe { BASE_LBA };
    let lba = base + bloque * SECTORES_POR_BLOQUE as u64;
    disk::write(lba, SECTORES_POR_BLOQUE, src) == SECTORES_POR_BLOQUE
}

/// **Escribe el superbloque: UN SOLO SECTOR.** Y ese es el punto entero.
///
/// === Por que no se reutiliza [`escribir_bloque`] ===
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
fn escribir_superbloque(bloque: u64, sb: &[u8; es::SUPER_LEN]) -> bool {
    let base = unsafe { BASE_LBA };
    let lba = base + bloque * SECTORES_POR_BLOQUE as u64;
    disk::write(lba, 1, sb) == 1
}

/// Por que no se pudo cerrar una transaccion. Cada una manda a mirar otra cosa.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum FalloEscritura {
    /// No hay volumen montado.
    SinVolumen,
    /// La maquina de estados dijo que no. Trae su motivo.
    Rechazada(es::escritura::Rechazo),
    /// El disco no acepto los sectores del superbloque.
    NoEscribio,
    /// **El `FLUSH CACHE` fallo**, y por eso NO se hizo el commit.
    SinBarrera,
}

impl FalloEscritura {
    pub fn name(self) -> &'static str {
        match self {
            FalloEscritura::SinVolumen => "no hay volumen ESTRATOS montado",
            FalloEscritura::Rechazada(r) => r.name(),
            FalloEscritura::NoEscribio => "el disco no acepto el superbloque",
            FalloEscritura::SinBarrera => "el FLUSH CACHE fallo: NO se hizo commit",
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
/// Despues de sellar, `F12` tiene que decir **`generacion 2`**. Y despues de
/// **reiniciar**, tiene que seguir diciendo 2 -- eso ultimo es lo que prueba que
/// llego al plato y no se quedo en la cache del SSD.
pub fn sellar() -> Result<u64, FalloEscritura> {
    let sb = match superbloque() {
        Some(s) => s,
        None => return Err(FalloEscritura::SinVolumen),
    };
    // Cual de las dos copias mando al montar. `pick_superblock` eligio la de
    // generacion mas alta; aqui se deduce igual para no guardar otro estado que
    // pueda separarse del primero.
    let copia = copia_en_uso();

    let mut t = es::escritura::Transaccion::open(&sb, copia, identidad_ok())
        .map_err(FalloEscritura::Rechazada)?;

    // Sin datos: se cierra la fase inmediatamente. `reserve(0)` no haria falta
    // y se omite a proposito -- una llamada que no hace nada en el camino que
    // estrena el disco es una llamada que confunde al leer el log.
    t.cerrar_datos().map_err(FalloEscritura::Rechazada)?;

    // * LA BARRERA. No es opcional y no se puede fingir.
    //
    // Aqui no hay nada escrito todavia, asi que este `flush` no protege ningun
    // dato -- protege el ORDEN, y sobre todo prueba que el disco **sabe hacer la
    // barrera**. El dia que haya datos delante, este mismo `flush` es lo unico
    // que separa un commit honesto de un superbloque que apunta a bloques que
    // no llegaron al plato.
    if !disk::flush() {
        t.abandonar();
        return Err(FalloEscritura::SinBarrera);
    }
    t.barrera_hecha().map_err(FalloEscritura::Rechazada)?;

    let (destino, nuevo) = t.commit(sb.estrato).map_err(FalloEscritura::Rechazada)?;

    // El superbloque serializado: 512 bytes, o sea UN SECTOR. Ver
    // `escribir_superbloque` -- el tamano de esta escritura es la garantia de
    // atomicidad, no un detalle de implementacion.
    let sector = nuevo.encode();

    if !escribir_superbloque(destino, &sector) {
        // El commit no ocurrio. La copia que manda sigue siendo la de antes, y
        // el volumen entero tambien.
        crate::ring0::cabina::fault("estratos", "no se pudo escribir el superbloque", destino);
        return Err(FalloEscritura::NoEscribio);
    }

    // * Y VACIAR OTRA VEZ. El commit tampoco vale si se queda en la cache.
    //
    // Sin esto, apagar la maquina justo despues de "sellado" dejaria el volumen
    // en la generacion vieja -- y el mensaje en pantalla habria mentido. Un
    // commit que no se puede confirmar no es un commit, es una intencion.
    if !disk::flush() {
        crate::ring0::cabina::warn("estratos", "el commit no se pudo vaciar al plato", destino);
        return Err(FalloEscritura::SinBarrera);
    }

    unsafe { SUPER = Some(nuevo) };
    crate::ring0::cabina::info("estratos", "COMMIT: generacion nueva", nuevo.generation);
    Ok(nuevo.generation)
}

/// Cual de las dos copias del superbloque manda ahora mismo.
///
/// Se recalcula leyendo, en vez de guardarse al montar: dos fuentes de la misma
/// verdad se separan, y esta decide **donde se escribe el commit**. Equivocarse
/// aqui es pisar la copia buena.
fn copia_en_uso() -> u64 {
    let (a, b) = unsafe {
        let s = &mut *core::ptr::addr_of_mut!(SCRATCH);
        let (x, y) = s.split_at_mut(1);
        (&mut x[0], &mut y[0])
    };
    let base = unsafe { BASE_LBA };
    if !leer_bloque_de(base, es::SUPER_A_BLOCK, a) || !leer_bloque_de(base, es::SUPER_B_BLOCK, b) {
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
        if !leer_bloque_de(p.first_lba, es::SUPER_A_BLOCK, a) { continue; }
        if !leer_bloque_de(p.first_lba, es::SUPER_B_BLOCK, b) { continue; }

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

// -- Seguir punteros ---------------------------------------------------------

/// Lee lo que un puntero promete y **lo comprueba**.
///
/// Devuelve los bytes dentro del scratch del nivel indicado. Un bloque que no
/// cuadra con su suma es un FAULT en CABINA, no un archivo raro: es el
/// principio 2 del diseno y la unica razon de que el puntero lleve la suma.
fn seguir(p: &BlockPtr, nivel: usize) -> Option<&'static [u8]> {
    if nivel >= NIVELES { return None; }
    let buf = unsafe {
        let s = core::ptr::addr_of_mut!(SCRATCH) as *mut [u8; BLOQUE];
        &mut *s.add(nivel)
    };
    if !leer_bloque(p.lba, buf) {
        crate::ring0::cabina::fault("estratos", "no se pudo leer un bloque", p.lba);
        return None;
    }
    let ini = p.off as usize;
    let fin = ini + p.len as usize;
    if fin > BLOQUE { return None; }
    let datos = &buf[ini..fin];
    if !p.verifica(datos) {
        crate::ring0::cabina::fault("estratos", "un bloque no cuadra con su suma", p.lba);
        return None;
    }
    Some(unsafe { core::slice::from_raw_parts(datos.as_ptr(), datos.len()) })
}

/// Lee el nodo al que apunta `p`.
pub fn nodo(p: &BlockPtr) -> Option<Nodo> {
    let d = seguir(p, 0)?;
    match Nodo::decode(d) {
        Ok(n) => Some(n),
        Err(e) => { crate::ring0::cabina::fault("estratos", e.name(), p.lba); None }
    }
}

/// El disco visto como fuente de bloques para el recorrido compartido.
struct DelDisco;

impl es::Fuente for DelDisco {
    fn bloque(&mut self, lba: u64, dst: &mut [u8; BLOQUE]) -> bool {
        leer_bloque(lba, dst)
    }
}

/// Reconstruye un flujo entero en `dst`. Devuelve los bytes escritos.
///
/// El recorrido del arbol NO vive aqui: es `bmo_estratos::descender`, el mismo
/// que usa el formateador del anfitrion. Tenerlo dos veces --una en cada lado--
/// era la trampa que casi cuesta el BLAKE3: dos copias que pueden separarse, y
/// el sintoma seria "un archivo que se lee mal" sin nada que apunte al
/// recorrido. Aqui solo se pone el disco, la memoria de trabajo y donde cae.
pub fn flujo(a: &Attr, dst: &mut [u8]) -> Option<usize> {
    if let Some(d) = a.datos_residentes() {
        if dst.len() < d.len() { return None; }
        dst[..d.len()].copy_from_slice(d);
        return Some(d.len());
    }
    let raiz = a.raiz()?;

    // El nivel 0 del scratch lo usa `seguir()` para nodos y estratos; el
    // recorrido se queda con los de abajo para no pisarselo.
    let scratch = unsafe {
        let s = &mut *core::ptr::addr_of_mut!(SCRATCH);
        &mut s[1..]
    };

    let mut escritos = 0usize;
    let r = es::descender(&mut DelDisco, &raiz, a.levels, scratch, &mut |trozo| {
        let free_slot = dst.len().saturating_sub(escritos);
        let n = trozo.len().min(free_slot);
        if n > 0 {
            dst[escritos..escritos + n].copy_from_slice(&trozo[..n]);
            escritos += n;
        }
        // Parar en cuanto el buffer del llamante se llena: seguir leyendo
        // bloques que no caben es gastar disco para tirar los bytes.
        escritos < dst.len()
    });
    if let Err(e) = r {
        crate::ring0::cabina::fault("estratos", e.name(), raiz.lba);
        return None;
    }
    Some((a.size as usize).min(escritos))
}

/// **Lee el principio del archivo Y comprueba su firma, en UNA sola pasada.**
///
/// Devuelve `(copiados, tamano_real, veredicto)`.
///
/// === El problema que resuelve, que es el escalon 2 entero ===
///
/// El gate de firma compara el `:firma` del nodo contra el hash del CONTENIDO,
/// y hasta hoy eso obligaba a tener el contenido entero en RAM: `read` a un
/// buffer y `firma(&nd, buf)`. Con un paquete que lleva un WAD dentro, eso son
/// megabytes de bodega para comprobar unos pocos que se van a ejecutar.
///
/// ** Pero un hash no necesita el fichero: necesita **sus bytes, en orden**. Y
/// eso es exactamente lo que `descender` ya entrega, trozo a trozo. Asi que los
/// trozos pasan por el hasher **todos**, y de ellos solo se COPIA lo que cabe en
/// `dst`. La firma sigue cubriendo el archivo entero -- el gate no se relaja ni
/// un byte-- y la RAM solo guarda el principio.
///
/// > **Los bytes pasan por delante; no se quedan.**
///
/// === Por que una funcion y no dos ===
///
/// Porque leer dos veces el mismo archivo --una para copiar y otra para hashear--
/// es pagar el disco dos veces por un dato que ya paso por aqui. Y porque son
/// dos respuestas de la MISMA pasada: separarlas invita a comprobar la firma de
/// una lectura y a usar los bytes de otra.
pub fn leer_y_firmar(n: &Nodo, dst: &mut [u8]) -> Option<(usize, usize, Firma)> {
    if n.tipo != Tipo::Archivo {
        return None;
    }
    let a = n.attr(bmo_estratos::objects::ATTR_DATOS)?;

    // Lo que dice el `:firma`, si lo hay. Se mira ANTES de leer para no gastar
    // el disco en un archivo que el gate va a rechazar de todas formas.
    let guardada = match n.attr(bmo_estratos::objects::ATTR_FIRMA) {
        Some(f) => match f.datos_residentes() {
            Some(d) if d.len() == 32 => {
                let mut copia = [0u8; 32];
                copia.copy_from_slice(d);
                Some(copia)
            }
            _ => None,
        },
        None => None,
    };

    let mut h = bmo_estratos::Hasher::new();
    let mut copiados = 0usize;
    let mut vistos = 0usize;

    if let Some(d) = a.datos_residentes() {
        // Residente: cabe en el propio nodo, asi que ya esta en RAM. No hay
        // nada que ahorrar -- y aun asi va por el mismo camino, para que el
        // veredicto se calcule en un solo sitio.
        h.update(d);
        vistos = d.len();
        copiados = d.len().min(dst.len());
        dst[..copiados].copy_from_slice(&d[..copiados]);
    } else {
        let raiz = a.raiz()?;
        let scratch = unsafe {
            let s = &mut *core::ptr::addr_of_mut!(SCRATCH);
            &mut s[1..]
        };
        let tam = a.size as usize;
        let r = es::descender(&mut DelDisco, &raiz, a.levels, scratch, &mut |trozo| {
            // ** El hasher se come SOLO lo que el archivo mide. `descender`
            // entrega bloques enteros y el ultimo lleva relleno detras: hashear
            // ese relleno daria un digest que no cuadra con el que escribio
            // quien firmo, y el sintoma seria "la firma NO cuadra" en TODOS los
            // archivos cuyo tamano no sea multiplo del bloque.
            let util = trozo.len().min(tam.saturating_sub(vistos));
            if util == 0 {
                return false;
            }
            h.update(&trozo[..util]);
            vistos += util;
            let hueco = dst.len().saturating_sub(copiados);
            let n = util.min(hueco);
            if n > 0 {
                dst[copiados..copiados + n].copy_from_slice(&trozo[..n]);
                copiados += n;
            }
            // * Se sigue leyendo AUNQUE `dst` este lleno, al reves que `flujo`.
            // Parar ahi seria dejar el hash a medias, y un hash a medias no es
            // una firma mas barata: es una firma que no vale.
            vistos < tam
        });
        if let Err(e) = r {
            crate::ring0::cabina::fault("estratos", e.name(), raiz.lba);
            return None;
        }
    }

    let veredicto = match guardada {
        None => Firma::Ausente,
        Some(g) => {
            if h.finalize() == g {
                Firma::Cuadra
            } else {
                Firma::NoCuadra
            }
        }
    };
    Some((copiados, vistos, veredicto))
}

// -- Directorios -------------------------------------------------------------

/// Entradas que caben en un listado de una vez.
///
/// Tope honesto: sin `alloc`, el buffer es fijo. Un directorio con mas
/// entradas se lista TRUNCADO y se dice -- no se calla.
pub const ENTRADAS_MAX: usize = 64;
static mut DIR_BUF: [u8; ENTRADAS_MAX * ENTRADA_LEN] = [0u8; ENTRADAS_MAX * ENTRADA_LEN];

/// El nodo raiz del volumen, siguiendo superbloque -> estrato -> raiz.
pub fn raiz() -> Option<(BlockPtr, Nodo)> {
    let sb = superbloque()?;
    if sb.estrato.es_nulo() { return None; }
    let e = {
        let d = seguir(&sb.estrato, 0)?;
        es::Estrato::decode(d).ok()?
    };
    let n = nodo(&e.raiz)?;
    Some((e.raiz, n))
}

/// El estrato mas reciente.
pub fn estrato() -> Option<es::Estrato> {
    let sb = superbloque()?;
    if sb.estrato.es_nulo() { return None; }
    let d = seguir(&sb.estrato, 0)?;
    es::Estrato::decode(d).ok()
}

/// Lee las entradas de un directorio a UN buffer cualquiera.
///
/// Existe separada de [`entradas`] porque hay dos listados en vuelo a la vez y
/// **no pueden compartir buffer**: el de `open()`, que recorre una ruta y lo
/// pisa entero en cada tramo, y el del cursor de Ring 3, que tiene que seguir
/// siendo valido entre dos preguntas del panel. Con un solo buffer, lanzar un
/// programa mientras la ventana de Datos esta abierta le cambiaba los nombres
/// bajo los pies.
fn listar_en(dir: &Nodo, buf: &mut [u8]) -> Option<(usize, bool)> {
    let a = dir.attr(bmo_estratos::objects::ATTR_ENTRADAS)?;
    let cabe_todo = a.size as usize <= buf.len();
    let n = flujo(a, buf)?;
    Some((n / ENTRADA_LEN, !cabe_todo))
}

/// Lee las entradas de un directorio al buffer estatico.
/// Devuelve `(cuantas, si_se_trunco)`.
pub fn entradas(dir: &Nodo) -> Option<(usize, bool)> {
    let buf = unsafe { &mut *core::ptr::addr_of_mut!(DIR_BUF) };
    listar_en(dir, buf)
}

/// La entrada numero `i` del ultimo `entradas()`.
pub fn entrada(i: usize) -> Option<Entrada> {
    if i >= ENTRADAS_MAX { return None; }
    let buf = unsafe { &*core::ptr::addr_of!(DIR_BUF) };
    Entrada::decode(&buf[i * ENTRADA_LEN..(i + 1) * ENTRADA_LEN]).ok()
}

// -- El CURSOR: ESTRATOS recorrido desde Ring 3 ------------------------------
//
// === Por que un cursor y no un handle por nodo ===
//
// Un `KIND_ESTRATOS_NODO` con su capability por cada nodo abierto seria lo
// ortodoxo, y es exactamente lo que no hace falta: la ventana de Datos mira UN
// sitio a la vez y lo que quiere es *bajar, subir y listar*. Un handle por nodo
// pediria tabla, ciclo de vida y revocacion para modelar un puntero que se
// mueve -- y un puntero que se mueve es un cursor.
//
// === Por que esto no concede nada ===
//
// Es el mismo trato que `OP_INFO` y que el klog: **contesta, no autoriza**.
// Leer los nombres de un directorio no ejerce ningun poder que Ring 3 no tenga
// ya --`ls` sobre FAT32 hace justo eso--, y ESCRIBIR sigue sin existir aqui: el
// cursor no tiene ninguna operacion que cambie el volumen.
//
// === Lo que faltaba, dicho ===
//
// `raiz`, `nodo`, `entradas` y `entrada` llevaban desde el principio siendo
// funciones de Ring 0 sin puerta. La ventana F12 podia ensenar los NUMEROS del
// volumen --generacion, ocupacion, nivel-- y no podia ensenar **que hay dentro**,
// porque no tenia de donde sacarlo. Esto es esa puerta.
pub mod cursor {
    use super::*;

    /// Cuanto se puede bajar. Dieciseis niveles de directorio es mas de lo que
    /// tiene ningun volumen razonable, y un tope explicito es mejor que una
    /// pila que crece hasta que algo se rompe.
    pub const HONDO_MAX: usize = 16;
    /// Lo que se guarda del nombre de cada nivel de la ruta. Un nombre mas
    /// largo se recorta **y se dice** -- ver `nombre_nivel`.
    pub const NOMBRE_NIVEL: usize = 32;

    /// El buffer del cursor, SUYO. Ver [`super::listar_en`].
    static mut BUF: [u8; ENTRADAS_MAX * ENTRADA_LEN] = [0u8; ENTRADAS_MAX * ENTRADA_LEN];
    /// La ruta desde la raiz: `PILA[0]` es la raiz y `PILA[HONDO]` el actual.
    static mut PILA: [Option<BlockPtr>; HONDO_MAX] = [None; HONDO_MAX];
    /// El NOMBRE de cada nivel, para poder ensenar la ruta de verdad.
    ///
    /// * Se guarda al bajar y no se reconstruye despues, y ese es el motivo de
    /// que exista: un `BlockPtr` sabe DONDE esta un nodo y **no sabe como se
    /// llama** -- el nombre vive en la entrada del padre, no en el hijo. Para
    /// sacarlo a posteriori habria que releer el directorio de arriba y buscar
    /// que entrada apunta aqui. Anotarlo al pasar cuesta 64 bytes por nivel.
    ///
    /// Sin esto, la ventana solo puede decir `profundidad 2`, y dos carpetas
    /// distintas con los mismos nombres dentro se ven identicas.
    static mut NOMBRES: [[u8; NOMBRE_NIVEL]; HONDO_MAX] = [[0; NOMBRE_NIVEL]; HONDO_MAX];
    static mut NOMBRES_LEN: [usize; HONDO_MAX] = [0; HONDO_MAX];
    static mut HONDO: usize = 0;
    static mut ACTUAL: Option<Nodo> = None;
    static mut CUANTAS: usize = 0;
    static mut TRUNCADO: bool = false;

    /// Relista el nodo actual. Todo lo que mueve el cursor acaba aqui.
    fn relistar() -> bool {
        unsafe {
            let Some(n) = ACTUAL else {
                CUANTAS = 0;
                TRUNCADO = false;
                return false;
            };
            let buf = &mut *core::ptr::addr_of_mut!(BUF);
            match listar_en(&n, buf) {
                Some((c, t)) => {
                    CUANTAS = c.min(ENTRADAS_MAX);
                    TRUNCADO = t;
                    true
                }
                // Un archivo no tiene `:entradas`, y eso no es un fallo: es que
                // no tiene hijos. Se contesta cero y se sigue.
                None => {
                    CUANTAS = 0;
                    TRUNCADO = false;
                    true
                }
            }
        }
    }

    /// Pone el cursor en la raiz del volumen. `false` si no hay volumen.
    pub fn a_la_raiz() -> bool {
        let Some((ptr, n)) = super::raiz() else {
            unsafe { ACTUAL = None; HONDO = 0; CUANTAS = 0; }
            return false;
        };
        unsafe {
            PILA[0] = Some(ptr);
            HONDO = 0;
            ACTUAL = Some(n);
        }
        relistar()
    }

    /// Cuantos hijos tiene el nodo actual.
    pub fn hijos() -> u64 {
        unsafe { CUANTAS as u64 }
    }

    /// 1 si el listado no cabia entero. **Se dice en vez de callarse**: un
    /// directorio truncado en silencio se ve igual que uno corto.
    pub fn truncado() -> u64 {
        unsafe { TRUNCADO as u64 }
    }

    /// Cuantos niveles se ha bajado desde la raiz.
    pub fn hondo() -> u64 {
        unsafe { HONDO as u64 }
    }

    /// El tipo del nodo actual: 0 archivo, 1 directorio, 2 no hay nada.
    pub fn tipo() -> u64 {
        unsafe {
            match ACTUAL {
                Some(n) => if n.tipo == Tipo::Directorio { 1 } else { 0 },
                None => 2,
            }
        }
    }

    fn entrada_i(i: usize) -> Option<bmo_estratos::objects::Entrada> {
        unsafe {
            if i >= CUANTAS { return None; }
            let buf = &*core::ptr::addr_of!(BUF);
            bmo_estratos::objects::Entrada::decode(&buf[i * ENTRADA_LEN..(i + 1) * ENTRADA_LEN]).ok()
        }
    }

    /// El tipo del hijo `i`: 0 archivo, 1 directorio, 2 no se pudo leer.
    ///
    /// Cuesta un salto al disco porque el tipo vive en el NODO y la entrada
    /// solo guarda el nombre y a donde apunta. Es lo que hay: meter el tipo en
    /// la entrada seria duplicar un dato que el nodo ya tiene, y dos copias de
    /// un dato es una que puede mentir.
    pub fn hijo_tipo(i: usize) -> u64 {
        let Some(e) = entrada_i(i) else { return 2 };
        match super::nodo(&e.nodo) {
            Some(n) => if n.tipo == Tipo::Directorio { 1 } else { 0 },
            None => 2,
        }
    }

    /// Ocho bytes del nombre del hijo `i`, empaquetados en LE. `trozo` los
    /// numera. Es el mismo trato que `klog_texto`: la superficie no acepta
    /// punteros, asi que un nombre viaja de ocho en ocho.
    pub fn hijo_nombre(i: usize, trozo: usize) -> u64 {
        let Some(e) = entrada_i(i) else { return 0 };
        let name = e.nombre_str().as_bytes();
        let ini = trozo * 8;
        if ini >= name.len() { return 0; }
        let fin = (ini + 8).min(name.len());
        let mut w = [0u8; 8];
        w[..fin - ini].copy_from_slice(&name[ini..fin]);
        u64::from_le_bytes(w)
    }

    /// Baja al hijo `i`. `false` si no existe, si no es directorio, o si ya no
    /// se puede bajar mas.
    pub fn entrar(i: usize) -> bool {
        let Some(e) = entrada_i(i) else { return false };
        unsafe {
            if HONDO + 1 >= HONDO_MAX { return false; }
        }
        let Some(n) = super::nodo(&e.nodo) else { return false };
        if n.tipo != Tipo::Directorio { return false; }
        unsafe {
            HONDO += 1;
            PILA[HONDO] = Some(e.nodo);
            // El nombre se anota AL PASAR. Despues ya no se sabe: la entrada
            // que lo lleva es del padre y aqui ya no la tenemos delante.
            let b = e.nombre_str().as_bytes();
            let k = b.len().min(NOMBRE_NIVEL);
            NOMBRES[HONDO][..k].copy_from_slice(&b[..k]);
            NOMBRES_LEN[HONDO] = k;
            ACTUAL = Some(n);
        }
        relistar()
    }

    /// Ocho bytes del nombre del nivel `nivel` de la ruta. `nivel = 0` es la
    /// raiz, que no tiene nombre y contesta vacio -- la ventana pinta `/`.
    pub fn nombre_nivel(nivel: usize, trozo: usize) -> u64 {
        unsafe {
            if nivel == 0 || nivel > HONDO || nivel >= HONDO_MAX {
                return 0;
            }
            let n = NOMBRES_LEN[nivel];
            let ini = trozo * 8;
            if ini >= n {
                return 0;
            }
            let fin = (ini + 8).min(n);
            let mut w = [0u8; 8];
            w[..fin - ini].copy_from_slice(&NOMBRES[nivel][ini..fin]);
            u64::from_le_bytes(w)
        }
    }

    // -- El DETALLE de un hijo -------------------------------------------
    //
    // * Un grafo que solo ensena nombres contesta *que hay*; no contesta *que
    // es esto*. Lo de abajo es lo que el nodo ya lleva dentro y la ventana no
    // podia pedir: cuanto mide, cuantos atributos tiene y si va firmado.

    /// Bytes del contenido del hijo `i`. Un directorio contesta el tamano de su
    /// lista de entradas, que tambien es un dato: dice cuanto ocupa el propio
    /// directorio, no lo que hay dentro.
    pub fn hijo_bytes(i: usize) -> u64 {
        let Some(e) = entrada_i(i) else { return 0 };
        let Some(n) = super::nodo(&e.nodo) else { return 0 };
        let cual = if n.tipo == Tipo::Directorio {
            bmo_estratos::objects::ATTR_ENTRADAS
        } else {
            bmo_estratos::objects::ATTR_DATOS
        };
        n.attr(cual).map(|a| a.size).unwrap_or(0)
    }

    /// Cuantos atributos lleva el hijo `i`.
    ///
    /// Es el numero que dice que ESTRATOS no es un sistema de archivos de
    /// carpetas: un nodo es **un conjunto de atributos**, y la diferencia entre
    /// un archivo y un directorio es cual lleva, no dos estructuras distintas.
    pub fn hijo_atributos(i: usize) -> u64 {
        let Some(e) = entrada_i(i) else { return 0 };
        let Some(n) = super::nodo(&e.nodo) else { return 0 };
        n.attrs().count() as u64
    }

    /// Lleva `:firma` el hijo `i`? `1` si, `0` no.
    ///
    /// **Solo dice si LA LLEVA, no si cuadra.** Comprobarlo exige leer el
    /// contenido entero y hacerle el BLAKE3, y eso no puede pasar en cada
    /// repintado de una ventana. Para eso esta [`verificar`], que se pide.
    pub fn hijo_firmado(i: usize) -> u64 {
        let Some(e) = entrada_i(i) else { return 0 };
        let Some(n) = super::nodo(&e.nodo) else { return 0 };
        n.attr(bmo_estratos::objects::ATTR_FIRMA).is_some() as u64
    }

    /// El buffer donde se lee un archivo para verificarlo. Un tope honesto:
    /// mas grande que esto no se puede comprobar y **se dice** en vez de
    /// contestar "no cuadra", que mandaria a buscar una corrupcion que no hay.
    const VERIFICA_MAX: usize = 256 * 1024;
    static mut VERIFICA_BUF: [u8; VERIFICA_MAX] = [0u8; VERIFICA_MAX];

    /// **Lee el hijo `i` y compara su BLAKE3 con su `:firma`.**
    ///
    /// `0` no lleva firma - `1` CUADRA - `2` NO CUADRA - `3` no se pudo leer -
    /// `4` **no cabe** en el buffer de verificacion.
    ///
    /// * El `4` no estaba y hacia falta: "no cabe" contestaba `3`, o sea **el
    /// mismo codigo que un fallo de lectura**. El panel lo pintaba en rojo como
    /// *"no se pudo leer"*, y en esa ventana el rojo significa "hay un problema
    /// en el disco". Un archivo sano de 300 KiB acusaba al disco de una averia
    /// que no existia. El tope es NUESTRO y ahora lo dice el.
    ///
    /// Se pide a mano y no se calcula al pintar: leer un archivo entero y
    /// hacerle un hash sesenta veces por segundo convertiria un panel en un
    /// martillo sobre el disco.
    ///
    /// [!] Lo que esto demuestra y lo que no, dicho aqui como en `super::firma`:
    /// demuestra que **los bytes son los que se guardaron** --caza corrupcion,
    /// una escritura a medias, un bloque mal leido--. NO demuestra
    /// autenticidad: quien pueda escribir en el volumen puede cambiar el
    /// archivo *y* recalcular su hash.
    pub fn verificar(i: usize) -> u64 {
        let Some(e) = entrada_i(i) else { return 3 };
        let Some(n) = super::nodo(&e.nodo) else { return 3 };
        if n.attr(bmo_estratos::objects::ATTR_FIRMA).is_none() {
            return 0;
        }
        let a = match n.attr(bmo_estratos::objects::ATTR_DATOS) {
            Some(a) => a,
            None => return 3,
        };
        if a.size as usize > VERIFICA_MAX {
            return 4; // no cabe -- el limite es nuestro, no del disco
        }
        let buf = unsafe { &mut *core::ptr::addr_of_mut!(VERIFICA_BUF) };
        let leidos = match super::flujo(a, buf) {
            Some(k) => k,
            None => return 3,
        };
        match super::firma(&n, &buf[..leidos]) {
            super::Firma::Cuadra => 1,
            super::Firma::NoCuadra => 2,
            super::Firma::Ausente => 0,
        }
    }

    /// Vuelve al padre. `false` si ya se esta en la raiz.
    pub fn subir() -> bool {
        unsafe {
            if HONDO == 0 { return false; }
            HONDO -= 1;
            let Some(ptr) = PILA[HONDO] else { return false };
            let Some(n) = super::nodo(&ptr) else { return false };
            ACTUAL = Some(n);
        }
        relistar()
    }
}

/// Busca un hijo por nombre dentro de un directorio, sin distinguir mayusculas.
fn buscar_en(dir: &Nodo, name: &str) -> Option<BlockPtr> {
    let (n, _) = entradas(dir)?;
    for i in 0..n {
        let e = entrada(i)?;
        if e.se_llama(name) { return Some(e.nodo); }
    }
    None
}

/// Busca un nodo por ruta: `c/holac.bex`.
pub fn open(ruta: &str) -> Option<Nodo> {
    let (_, mut actual) = raiz()?;
    let mut resto = ruta.trim_start_matches('/');
    loop {
        match resto.as_bytes().iter().position(|&c| c == b'/' || c == b'\\') {
            Some(i) => {
                let ptr = buscar_en(&actual, &resto[..i])?;
                let n = nodo(&ptr)?;
                if n.tipo != Tipo::Directorio { return None; }
                actual = n;
                resto = &resto[i + 1..];
            }
            None => break,
        }
    }
    let ptr = buscar_en(&actual, resto)?;
    nodo(&ptr)
}

/// Lee el `:datos` de un nodo. Devuelve los bytes leidos.
pub fn read(n: &Nodo, dst: &mut [u8]) -> Option<usize> {
    if n.tipo != Tipo::Archivo { return None; }
    let a = n.attr(bmo_estratos::objects::ATTR_DATOS)?;
    flujo(a, dst)
}

/// Que dijo la firma.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Firma {
    /// El `:firma` del nodo cuadra con el contenido leido.
    Cuadra,
    /// Hay `:firma` y NO cuadra: el archivo no es el que se guardo.
    NoCuadra,
    /// El nodo no lleva `:firma`.
    Ausente,
}

/// El gate del section 7: `open(nodo, EJECUTAR)`.
///
/// Compara el atributo `:firma` con el BLAKE3 del contenido que se acaba de
/// leer. **Lo que esto demuestra**: que los bytes son los que se guardaron --
/// caza corrupcion del disco, una escritura a medias o un bloque mal leido.
///
/// **Lo que NO demuestra**: autenticidad. Quien pueda escribir en el volumen
/// puede cambiar el archivo *y* recalcular su hash; no hay clave por medio.
/// Para eso hace falta firmar el hash con una clave que el kernel conozca y el
/// atacante no (esqueleto en `bmo-abi/src/bef/signing.rs`). Se dice en vez de
/// dejar que la palabra "firma" prometa de mas.
///
/// Y esto es lo que un `.bex` en FAT32 **no puede tener**: un sistema de
/// ficheros sin atributos con nombre obliga a un `.sig` suelto que se pierde
/// al copiar. Aqui la firma viaja dentro del mismo nodo que los datos.
pub fn firma(n: &Nodo, datos: &[u8]) -> Firma {
    let a = match n.attr(bmo_estratos::objects::ATTR_FIRMA) {
        Some(a) => a,
        None => return Firma::Ausente,
    };
    let guardada = match a.datos_residentes() {
        Some(d) if d.len() == 32 => d,
        _ => return Firma::Ausente,
    };
    if bmo_estratos::blake3(datos) == guardada { Firma::Cuadra } else { Firma::NoCuadra }
}
