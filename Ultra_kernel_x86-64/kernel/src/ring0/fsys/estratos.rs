//! ESTRATOS montado: el kernel leyendo su propio sistema de ficheros.
//!
//! Paso 4d del orden de construcción. **Solo lectura, y es estructural**: este
//! módulo no llama a `write` en ningún sitio. Escribir es el paso 5 y necesita
//! el log, las barreras y el recolector; nada de eso existe todavía, y un
//! ESTRATOS que escribe a medias no es medio sistema de ficheros, es uno roto.
//!
//! ## Habla con el contrato, no con SATA
//!
//! Todo lo de aquí pasa por [`bmo_block::device()`]. Es lo que el diseño pedía
//! en su §10.3 y la razón de que la capa de bloques se hiciera antes: el día
//! que haya un NVMe cableado, este módulo no se entera.
//!
//! ## Dónde busca
//!
//! No se le dice en qué partición está: se le pregunta al disco. Se recorren
//! las particiones de la GPT leyendo su primer bloque, y la que lleve la firma
//! `ESTRATOS` gana. Las demás devuelven `BadMagic`, que significa "aquí no hay
//! un volumen ESTRATOS" y **no** "está corrupto" — distinguir las dos cosas es
//! lo que permite mirar una NTFS sin dar un susto.

use bmo_estratos as es;
use bmo_estratos::objects::{Attr, BlockPtr, Entrada, Nodo, Tipo, BLOQUE, ENTRADA_LEN, PTR_LEN};
use crate::ring0::dev::disk;

/// Sectores de 512 B por bloque de ESTRATOS.
const SECTORES_POR_BLOQUE: u16 = (BLOQUE / 512) as u16;

// ── Estado del montaje ──────────────────────────────────────────────────────

static mut MONTADO: bool = false;
static mut BASE_LBA: u64 = 0;
static mut PARTICION: u32 = 0;
static mut SUPER: Option<es::Superblock> = None;
/// ¿El volumen dice haber nacido en ESTE disco?
static mut IDENTIDAD_OK: bool = false;

/// Buffers de trabajo, uno por nivel de indirección.
///
/// Estáticos y no locales: bajar por un árbol de cuatro niveles con un buffer
/// de 4 KiB en cada marco son 16 KiB de pila, y la del kernel son 64 KiB para
/// todo. Aquí el gasto es fijo, visible y está en `.bss`.
const NIVELES: usize = 5; // NIVELES_MAX + 1
static mut SCRATCH: [[u8; BLOQUE]; NIVELES] = [[0u8; BLOQUE]; NIVELES];

pub fn is_mounted() -> bool { unsafe { MONTADO } }
pub fn particion() -> u32 { unsafe { PARTICION } }
pub fn base_lba() -> u64 { unsafe { BASE_LBA } }
pub fn identidad_ok() -> bool { unsafe { IDENTIDAD_OK } }
pub fn superbloque() -> Option<es::Superblock> { unsafe { SUPER } }

/// Cuánto del volumen está usado, y en qué nivel de aviso está.
///
/// ★ La cuenta es una resta porque ESTRATOS reserva con un puntero que **sólo
/// avanza**: `log_head` es el primer bloque libre, así que todo lo de debajo
/// está usado. No hay mapa de bits ni fragmentación que medir, y eso es
/// consecuencia directa de no sobreescribir nunca.
///
/// La política —dónde caen el ámbar, el rojo y el solo-lectura— vive en
/// `bmo_estratos::espacio` y **se prueba en el anfitrión**. Aquí sólo se le
/// pasan los dos números que tiene el superbloque: un umbral escrito a mano en
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

/// Igual, pero sobre una partición concreta — para sondear antes de montar.
fn leer_bloque_de(base: u64, bloque: u64, dst: &mut [u8; BLOQUE]) -> bool {
    let dev = match bmo_block::device() { Some(d) => d, None => return false };
    let lba = base + bloque * SECTORES_POR_BLOQUE as u64;
    matches!(dev.read(lba, SECTORES_POR_BLOQUE, dst), Ok(n) if n == SECTORES_POR_BLOQUE)
}

/// **Escribe un bloque de ESTRATOS.** El espejo exacto de [`leer_bloque_de`].
///
/// El índice es de BLOQUE, no de sector: la conversión vive aquí y en un solo
/// sitio, porque mezclar las dos unidades es como se escribe ocho veces más
/// lejos de donde se quería.
///
/// Delante hay dos guardianes que este archivo **no** implementa y de los que
/// depende: el gate de identidad del disco (`disk::write_armed`) y la ventana
/// de la partición (`disk::write_window`). Aquí no se repiten — repetir un
/// guardián es tener dos sitios donde relajarlo.
fn escribir_bloque(bloque: u64, src: &[u8; BLOQUE]) -> bool {
    let base = unsafe { BASE_LBA };
    let lba = base + bloque * SECTORES_POR_BLOQUE as u64;
    disk::write(lba, SECTORES_POR_BLOQUE, src) == SECTORES_POR_BLOQUE
}

/// Por qué no se pudo cerrar una transacción. Cada una manda a mirar otra cosa.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum FalloEscritura {
    /// No hay volumen montado.
    SinVolumen,
    /// La máquina de estados dijo que no. Trae su motivo.
    Rechazada(es::escritura::Rechazo),
    /// El disco no aceptó los sectores del superbloque.
    NoEscribio,
    /// **El `FLUSH CACHE` falló**, y por eso NO se hizo el commit.
    SinBarrera,
}

impl FalloEscritura {
    pub fn nombre(self) -> &'static str {
        match self {
            FalloEscritura::SinVolumen => "no hay volumen ESTRATOS montado",
            FalloEscritura::Rechazada(r) => r.nombre(),
            FalloEscritura::NoEscribio => "el disco no acepto el superbloque",
            FalloEscritura::SinBarrera => "el FLUSH CACHE fallo: NO se hizo commit",
        }
    }
}

/// **SELLAR: la transacción más pequeña que existe, y la que estrena el camino.**
///
/// ═══ Qué hace ═══
///
/// Una transacción **sin datos**: no reserva ni un bloque, no toca ni un
/// objeto, y hace el commit apuntando **al mismo estrato que ya había**. Lo
/// único que cambia en el volumen es el número de generación, y se escribe en
/// **la copia del superbloque que NO está en uso**.
///
/// ═══ Por qué esto primero, y no crear un archivo ═══
///
/// Porque recorre el camino ENTERO —reservar, cerrar, `FLUSH CACHE`, barrera,
/// commit, escribir el superbloque alterno, volver a vaciar— y **no puede
/// perder un dato aunque salga mal**:
///
/// - Si falla antes del commit, el volumen es exactamente el de antes.
/// - Si falla escribiendo el superbloque nuevo, se estropea **la copia que no
///   manda**; la que manda sigue entera y el volumen monta igual.
/// - Y como el estrato es el mismo, no hay ningún objeto nuevo al que apuntar
///   mal.
///
/// Es el instrumento antes que la teoría: si esto funciona, el camino de
/// escritura está vivo y lo siguiente es sólo poner datos dentro. Si no
/// funciona, se sabe **exactamente** dónde falla y no hay nada que lamentar.
///
/// ═══ La comprobación, y es preciosa ═══
///
/// Después de sellar, `F12` tiene que decir **`generacion 2`**. Y después de
/// **reiniciar**, tiene que seguir diciendo 2 — eso último es lo que prueba que
/// llegó al plato y no se quedó en la caché del SSD.
pub fn sellar() -> Result<u64, FalloEscritura> {
    let sb = match superbloque() {
        Some(s) => s,
        None => return Err(FalloEscritura::SinVolumen),
    };
    // Cuál de las dos copias mandó al montar. `pick_superblock` eligió la de
    // generación más alta; aquí se deduce igual para no guardar otro estado que
    // pueda separarse del primero.
    let copia = copia_en_uso();

    let mut t = es::escritura::Transaccion::abrir(&sb, copia, identidad_ok())
        .map_err(FalloEscritura::Rechazada)?;

    // Sin datos: se cierra la fase inmediatamente. `reservar(0)` no haría falta
    // y se omite a propósito — una llamada que no hace nada en el camino que
    // estrena el disco es una llamada que confunde al leer el log.
    t.cerrar_datos().map_err(FalloEscritura::Rechazada)?;

    // ★ LA BARRERA. No es opcional y no se puede fingir.
    //
    // Aquí no hay nada escrito todavía, así que este `flush` no protege ningún
    // dato — protege el ORDEN, y sobre todo prueba que el disco **sabe hacer la
    // barrera**. El día que haya datos delante, este mismo `flush` es lo único
    // que separa un commit honesto de un superbloque que apunta a bloques que
    // no llegaron al plato.
    if !disk::flush() {
        t.abandonar();
        return Err(FalloEscritura::SinBarrera);
    }
    t.barrera_hecha().map_err(FalloEscritura::Rechazada)?;

    let (destino, nuevo) = t.commit(sb.estrato).map_err(FalloEscritura::Rechazada)?;

    // El superbloque, serializado en un bloque a cero. Lo que no es el
    // superbloque tiene que ser CERO y no basura de un scratch anterior: un
    // bloque medio lleno de restos se lee igual de bien hoy y es una mina el
    // día que el formato crezca de tamaño.
    let buf = unsafe { &mut (*core::ptr::addr_of_mut!(SCRATCH))[0] };
    *buf = [0u8; BLOQUE];
    buf[..es::SUPER_LEN].copy_from_slice(&nuevo.encode());

    if !escribir_bloque(destino, buf) {
        // El commit no ocurrió. La copia que manda sigue siendo la de antes, y
        // el volumen entero también.
        crate::ring0::cabina::fault("estratos", "no se pudo escribir el superbloque", destino);
        return Err(FalloEscritura::NoEscribio);
    }

    // ★ Y VACIAR OTRA VEZ. El commit tampoco vale si se queda en la caché.
    //
    // Sin esto, apagar la máquina justo después de "sellado" dejaría el volumen
    // en la generación vieja — y el mensaje en pantalla habría mentido. Un
    // commit que no se puede confirmar no es un commit, es una intención.
    if !disk::flush() {
        crate::ring0::cabina::warn("estratos", "el commit no se pudo vaciar al plato", destino);
        return Err(FalloEscritura::SinBarrera);
    }

    unsafe { SUPER = Some(nuevo) };
    crate::ring0::cabina::info("estratos", "COMMIT: generacion nueva", nuevo.generation);
    Ok(nuevo.generation)
}

/// Cuál de las dos copias del superbloque manda ahora mismo.
///
/// Se recalcula leyendo, en vez de guardarse al montar: dos fuentes de la misma
/// verdad se separan, y ésta decide **dónde se escribe el commit**. Equivocarse
/// aquí es pisar la copia buena.
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
    // ★ La ventana se quita ANTES de nada.
    //
    // Si este montaje falla, o encuentra otro volumen, o la identidad no
    // cuadra, no puede quedar en pie la ventana del montaje anterior. Una
    // autorización que sobrevive a la razón que la concedió es exactamente
    // cómo se escribe donde no se debe.
    disk::desarmar_ventana_estratos();
    if bmo_block::device().is_none() {
        crate::ring0::cabina::warn("estratos", "sin dispositivo de bloques", 0);
        return;
    }

    // Dos buffers: las dos copias del superbloque. Se leen ANTES de decidir,
    // porque `pick_superblock` necesita las dos para poder descartar la que
    // se quedó a medias.
    let (a, b) = unsafe {
        let s = &mut *core::ptr::addr_of_mut!(SCRATCH);
        let (x, y) = s.split_at_mut(1);
        (&mut x[0], &mut y[0])
    };

    for p in disk::partitions() {
        // La de arranque no se toca: ahí vive el BOOTX64.EFI con el que se
        // arrancó, y ESTRATOS no vive nunca en ella (regla del §2.6).
        if p.is_esp() { continue; }
        if !leer_bloque_de(p.first_lba, es::SUPER_A_BLOCK, a) { continue; }
        if !leer_bloque_de(p.first_lba, es::SUPER_B_BLOCK, b) { continue; }

        let (sb, copia) = match es::pick_superblock(&a[..es::SUPER_LEN], &b[..es::SUPER_LEN]) {
            Ok(v) => v,
            Err(es::FormatError::BadMagic) => continue, // aquí no hay ESTRATOS
            Err(e) => {
                // Magia correcta pero algo no cuadra: ESO sí merece un grito.
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

        // El gate del §5: ¿nació este volumen en el disco que tenemos delante?
        // Modelo, serie Y capacidad. Si no cuadra es un volumen clonado, y con
        // escritura sería un desastre; hoy solo se lee, así que se avisa.
        let id = bmo_block::device().map(|d| es::disk_id_of(&d.identity())).unwrap_or([0u8; 32]);
        let ok = sb.belongs_to(&id);
        unsafe { IDENTIDAD_OK = ok; }
        if ok {
            crate::ring0::cabina::info("estratos", "volumen montado y es de este disco", p.first_lba);
            // ★ Y AQUI, y sólo aquí, se abre la puerta de escribir.
            //
            // Las dos condiciones juntas: hay volumen y **nació en este
            // disco**. Un volumen clonado se monta y se lee, pero no registra
            // ventana — así que sus escrituras las para `write_window` aunque
            // el disco esté armado. Dos cerrojos distintos para dos preguntas
            // distintas: "¿es mi disco?" y "¿es mi volumen?".
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

// ── Seguir punteros ─────────────────────────────────────────────────────────

/// Lee lo que un puntero promete y **lo comprueba**.
///
/// Devuelve los bytes dentro del scratch del nivel indicado. Un bloque que no
/// cuadra con su suma es un FAULT en CABINA, no un archivo raro: es el
/// principio 2 del diseño y la única razón de que el puntero lleve la suma.
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
/// El recorrido del árbol NO vive aquí: es `bmo_estratos::descender`, el mismo
/// que usa el formateador del anfitrión. Tenerlo dos veces —una en cada lado—
/// era la trampa que casi cuesta el BLAKE3: dos copias que pueden separarse, y
/// el síntoma sería "un archivo que se lee mal" sin nada que apunte al
/// recorrido. Aquí solo se pone el disco, la memoria de trabajo y dónde cae.
pub fn flujo(a: &Attr, dst: &mut [u8]) -> Option<usize> {
    if let Some(d) = a.datos_residentes() {
        if dst.len() < d.len() { return None; }
        dst[..d.len()].copy_from_slice(d);
        return Some(d.len());
    }
    let raiz = a.raiz()?;

    // El nivel 0 del scratch lo usa `seguir()` para nodos y estratos; el
    // recorrido se queda con los de abajo para no pisárselo.
    let scratch = unsafe {
        let s = &mut *core::ptr::addr_of_mut!(SCRATCH);
        &mut s[1..]
    };

    let mut escritos = 0usize;
    let r = es::descender(&mut DelDisco, &raiz, a.levels, scratch, &mut |trozo| {
        let hueco = dst.len().saturating_sub(escritos);
        let n = trozo.len().min(hueco);
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

// ── Directorios ─────────────────────────────────────────────────────────────

/// Entradas que caben en un listado de una vez.
///
/// Tope honesto: sin `alloc`, el buffer es fijo. Un directorio con más
/// entradas se lista TRUNCADO y se dice — no se calla.
pub const ENTRADAS_MAX: usize = 64;
static mut DIR_BUF: [u8; ENTRADAS_MAX * ENTRADA_LEN] = [0u8; ENTRADAS_MAX * ENTRADA_LEN];

/// El nodo raíz del volumen, siguiendo superbloque → estrato → raíz.
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

/// El estrato más reciente.
pub fn estrato() -> Option<es::Estrato> {
    let sb = superbloque()?;
    if sb.estrato.es_nulo() { return None; }
    let d = seguir(&sb.estrato, 0)?;
    es::Estrato::decode(d).ok()
}

/// Lee las entradas de un directorio al buffer estático.
/// Devuelve `(cuantas, si_se_trunco)`.
pub fn entradas(dir: &Nodo) -> Option<(usize, bool)> {
    let a = dir.attr(bmo_estratos::objects::ATTR_ENTRADAS)?;
    let buf = unsafe { &mut *core::ptr::addr_of_mut!(DIR_BUF) };
    let cabe_todo = a.size as usize <= buf.len();
    let n = flujo(a, buf)?;
    Some((n / ENTRADA_LEN, !cabe_todo))
}

/// La entrada número `i` del último `entradas()`.
pub fn entrada(i: usize) -> Option<Entrada> {
    if i >= ENTRADAS_MAX { return None; }
    let buf = unsafe { &*core::ptr::addr_of!(DIR_BUF) };
    Entrada::decode(&buf[i * ENTRADA_LEN..(i + 1) * ENTRADA_LEN]).ok()
}

/// Busca un hijo por nombre dentro de un directorio, sin distinguir mayúsculas.
fn buscar_en(dir: &Nodo, nombre: &str) -> Option<BlockPtr> {
    let (n, _) = entradas(dir)?;
    for i in 0..n {
        let e = entrada(i)?;
        if e.se_llama(nombre) { return Some(e.nodo); }
    }
    None
}

/// Busca un nodo por ruta: `c/holac.bex`.
pub fn abrir(ruta: &str) -> Option<Nodo> {
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

/// Lee el `:datos` de un nodo. Devuelve los bytes leídos.
pub fn leer(n: &Nodo, dst: &mut [u8]) -> Option<usize> {
    if n.tipo != Tipo::Archivo { return None; }
    let a = n.attr(bmo_estratos::objects::ATTR_DATOS)?;
    flujo(a, dst)
}

/// Qué dijo la firma.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Firma {
    /// El `:firma` del nodo cuadra con el contenido leído.
    Cuadra,
    /// Hay `:firma` y NO cuadra: el archivo no es el que se guardó.
    NoCuadra,
    /// El nodo no lleva `:firma`.
    Ausente,
}

/// El gate del §7: `abrir(nodo, EJECUTAR)`.
///
/// Compara el atributo `:firma` con el BLAKE3 del contenido que se acaba de
/// leer. **Lo que esto demuestra**: que los bytes son los que se guardaron —
/// caza corrupción del disco, una escritura a medias o un bloque mal leído.
///
/// **Lo que NO demuestra**: autenticidad. Quien pueda escribir en el volumen
/// puede cambiar el archivo *y* recalcular su hash; no hay clave por medio.
/// Para eso hace falta firmar el hash con una clave que el kernel conozca y el
/// atacante no (esqueleto en `bmo-abi/src/bef/signing.rs`). Se dice en vez de
/// dejar que la palabra "firma" prometa de más.
///
/// Y esto es lo que un `.bex` en FAT32 **no puede tener**: un sistema de
/// ficheros sin atributos con nombre obliga a un `.sig` suelto que se pierde
/// al copiar. Aquí la firma viaja dentro del mismo nodo que los datos.
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
