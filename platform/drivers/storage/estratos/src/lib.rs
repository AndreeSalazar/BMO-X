//! ESTRATOS — el formato en disco.
//!
//! Diseño completo en `platform/services/timeback/ESTRATOS.md`. Esta crate es
//! el **paso 4** de su §10, y solo la primera mitad: el FORMATO. Aquí no se
//! lee ni se escribe un sector — se declara cómo son las estructuras y cómo se
//! comprueban. La E/S vive en quien tenga el dispositivo.
//!
//! Está separado a propósito, y por la razón que el propio documento da: *"este
//! documento existe para que el formato se decida ANTES de tocar un sector —
//! en un sistema de ficheros, equivocarse cuesta datos"*. Una crate sin E/S se
//! puede probar entera en el anfitrión, y el formateador y el kernel leen
//! exactamente la misma definición: si divergieran, el síntoma sería un
//! volumen que se formatea bien y no se monta.
//!
//! ## Lo que hay y lo que no
//!
//! - ✅ **Superbloque**: el único sitio con posición fija, el gate de identidad
//!   grabado en el volumen y el contador de generación que decide cuál de las
//!   dos copias manda.
//! - ✅ **Estrato**: la raíz. El commit que también es el superbloque.
//! - ✅ **Nodos y atributos**: el modelo de objetos (§4). Ver [`objects`], que
//!   documenta las tres decisiones que el diseño dejaba abiertas: el puntero
//!   lleva dirección Y suma, los archivos crecen por niveles de indirección, y
//!   lo pequeño vive dentro del atributo sin gastar bloque.
//! - ⬜ **Formateador** y **montaje**: los dos necesitan E/S, así que viven
//!   fuera de aquí — en el toolchain y en el kernel.

#![cfg_attr(not(test), no_std)]

/// La contabilidad del espacio y los avisos de la §9. Va aparte porque es lo
/// unico de esta crate que es POLITICA y no formato: los umbrales salen del
/// diseno, no del disco.
/// El LOG de escritura: la maquina de estados de una transaccion. Aqui no se
/// escribe un sector — se decide el ORDEN, que es lo que cuesta datos si se
/// equivoca, y por eso se prueba en el anfitrion.
pub mod escritura;
pub mod espacio;
pub mod objects;
pub mod read;
pub use escritura::{Fase, Rechazo, Transaccion};
pub use espacio::{Nivel, Ocupacion};
pub use objects::{Attr, BlockPtr, Entrada, Nodo, Tipo};
pub use read::{descender, Fuente};

pub use bmo_hash::hash as blake3;

/// Suma BLAKE3 de 256 bits. Todo en ESTRATOS se identifica y se comprueba con
/// esto — contenido, raíces y la identidad del disco.
pub type Hash = [u8; 32];

/// Hash de todo ceros: "no hay". Un estrato con `padre` a cero es el primero.
pub const NO_HASH: Hash = [0u8; 32];

/// Firma del volumen. Ocho bytes, en el primer sector de cada superbloque.
pub const MAGIC: [u8; 8] = *b"ESTRATOS";

/// Versión del formato. Se sube cuando cambia el LAYOUT, no cuando cambia el
/// código: montar un volumen de una versión que no se entiende es solo lectura
/// y un aviso, nunca una interpretación a la buena de dios.
pub const VERSION: u32 = 1;

/// Tamaño de bloque de ESTRATOS. Ocho sectores de 512 B.
pub const BLOCK_SIZE: u32 = 4096;

/// El superbloque se escribe en un sector completo aunque no lo llene: es la
/// unidad que el disco garantiza atómica, y el punto de no retorno de una
/// transacción tiene que caber en ella.
pub const SUPER_LEN: usize = 512;

/// Las DOS copias, en bloques 0 y 1 del volumen. Se escribe siempre la que NO
/// está en uso; si el corte llega a media escritura, la otra sigue entera.
pub const SUPER_A_BLOCK: u64 = 0;
pub const SUPER_B_BLOCK: u64 = 1;

/// Qué puede ir mal al leer una estructura del disco.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FormatError {
    /// El buffer no tiene el tamaño de la estructura.
    ShortBuffer,
    /// No lleva la firma `ESTRATOS`. Casi siempre significa "aquí no hay un
    /// volumen de ESTRATOS", no "está corrupto" — y son cosas distintas.
    BadMagic,
    /// La versión del formato es de otra época.
    BadVersion,
    /// La suma no cuadra con el contenido: esto SÍ es corrupción.
    BadChecksum,
    /// Un campo tiene un valor imposible.
    BadField,
    /// El dispositivo no entregó el bloque.
    Io,
    /// No hay buffer para bajar otro nivel del árbol.
    SinScratch,
}

impl FormatError {
    pub fn name(self) -> &'static str {
        match self {
            FormatError::ShortBuffer => "el buffer no da para la estructura",
            FormatError::BadMagic => "aqui no hay un volumen ESTRATOS",
            FormatError::BadVersion => "version de formato desconocida",
            FormatError::BadChecksum => "la suma no cuadra: corrupcion",
            FormatError::BadField => "un campo tiene un valor imposible",
            FormatError::Io => "el dispositivo no entrego el bloque",
            FormatError::SinScratch => "sin buffer para bajar otro nivel",
        }
    }
}

// ── Identidad del disco ─────────────────────────────────────────────────────

/// Calcula el `disco_id` que se graba en el volumen.
///
/// **Desviación deliberada del documento**, que lo describe como `[u8; 20]`
/// con "modelo+serie". No caben: el modelo de ATA son 40 bytes y la serie 20.
/// Guardar un recorte sería un gate de identidad que acepta dos discos cuyos
/// primeros 20 bytes coinciden — justo lo que el campo existe para impedir.
///
/// Así que se graba el BLAKE3 de modelo, serie **y capacidad**, que es
/// exactamente lo que compara `bmo_block::DeviceId::same_device`: el modelo
/// dice qué disco es, la serie cuál, y la capacidad caza la imagen clonada a
/// un disco de otro tamaño. Tamaño fijo, comparación exacta, y el mismo hash
/// que todo lo demás.
pub fn disk_id(model: &[u8], serial: &[u8], blocks: u64) -> Hash {
    let mut h = bmo_hash::Hasher::new();
    h.update(model);
    h.update(b"\0");
    h.update(serial);
    h.update(b"\0");
    h.update(&blocks.to_le_bytes());
    h.finalize()
}

/// El `disco_id` de un dispositivo ya identificado.
pub fn disk_id_of(id: &bmo_block::DeviceId) -> Hash {
    disk_id(&id.model[..id.model_len], &id.serial[..id.serial_len], id.blocks)
}

// ── Superbloque ─────────────────────────────────────────────────────────────

/// La raíz del volumen: el único sitio con posición fija.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Superblock {
    pub version: u32,
    pub block_size: u32,
    /// La más alta de las dos copias es la que manda. Es lo que convierte dos
    /// sectores en una transacción.
    pub generation: u64,
    /// Bloques que ocupa el volumen.
    pub total_blocks: u64,
    /// Primer bloque libre del log. El log solo crece hacia adelante.
    pub log_head: u64,
    /// Quién es el disco donde nació este volumen. Ver [`disk_id`].
    pub disk_id: Hash,
    /// El estrato más reciente. Nulo = volumen recién formateado.
    ///
    /// **Es un puntero, no un hash a secas** — corrección al diseño, que lo
    /// describía como `Hash`. Con un hash solo no se puede *encontrar* nada:
    /// haría falta un índice hash→dirección, y la decisión 1 del modelo de
    /// objetos es justamente que **el que lee no necesita índice**. Un puntero
    /// lleva las dos cosas: dónde está y qué debe contener.
    pub estrato: BlockPtr,
}

// Desplazamientos del sector. Se declaran como constantes en vez de escribirse
// a mano en dos sitios: el formateador y el lector tienen que coincidir al
// byte, y un `24` suelto en un lado y un `28` en el otro es un bug que solo
// aparece en hardware.
const OFF_MAGIC: usize = 0;
const OFF_VERSION: usize = 8;
const OFF_BLOCK_SIZE: usize = 12;
const OFF_GENERATION: usize = 16;
const OFF_TOTAL_BLOCKS: usize = 24;
const OFF_LOG_HEAD: usize = 32;
const OFF_DISK_ID: usize = 40;
const OFF_ESTRATO: usize = 72;
/// La suma va AL FINAL y cubre todo lo anterior.
const OFF_SUPER_SUM: usize = SUPER_LEN - 32;

impl Superblock {
    /// Un volumen recién formateado: sin estratos todavía.
    ///
    /// `log_head` arranca en 2 porque los bloques 0 y 1 son las dos copias del
    /// superbloque y no se pueden pisar jamás.
    pub fn new(disk_id: Hash, total_blocks: u64) -> Self {
        Self {
            version: VERSION,
            block_size: BLOCK_SIZE,
            generation: 1,
            total_blocks,
            log_head: 2,
            disk_id,
            estrato: BlockPtr::NULO,
        }
    }

    /// Serializa a un sector de 512 B, con su suma ya calculada.
    pub fn encode(&self) -> [u8; SUPER_LEN] {
        let mut b = [0u8; SUPER_LEN];
        b[OFF_MAGIC..OFF_MAGIC + 8].copy_from_slice(&MAGIC);
        b[OFF_VERSION..OFF_VERSION + 4].copy_from_slice(&self.version.to_le_bytes());
        b[OFF_BLOCK_SIZE..OFF_BLOCK_SIZE + 4].copy_from_slice(&self.block_size.to_le_bytes());
        b[OFF_GENERATION..OFF_GENERATION + 8].copy_from_slice(&self.generation.to_le_bytes());
        b[OFF_TOTAL_BLOCKS..OFF_TOTAL_BLOCKS + 8].copy_from_slice(&self.total_blocks.to_le_bytes());
        b[OFF_LOG_HEAD..OFF_LOG_HEAD + 8].copy_from_slice(&self.log_head.to_le_bytes());
        b[OFF_DISK_ID..OFF_DISK_ID + 32].copy_from_slice(&self.disk_id);
        b[OFF_ESTRATO..OFF_ESTRATO + objects::PTR_LEN].copy_from_slice(&self.estrato.encode());
        let sum = blake3(&b[..OFF_SUPER_SUM]);
        b[OFF_SUPER_SUM..].copy_from_slice(&sum);
        b
    }

    /// Lee un superbloque de un sector, comprobándolo entero.
    ///
    /// El orden de las comprobaciones importa: primero la firma (para poder
    /// decir "aquí no hay un ESTRATOS" en vez de "está corrupto"), después la
    /// versión, y la suma la última — porque una suma que no cuadra en algo
    /// que ni siquiera es un superbloque no informa de nada.
    pub fn decode(b: &[u8]) -> Result<Self, FormatError> {
        if b.len() < SUPER_LEN { return Err(FormatError::ShortBuffer); }
        if b[OFF_MAGIC..OFF_MAGIC + 8] != MAGIC { return Err(FormatError::BadMagic); }
        let version = u32::from_le_bytes([b[8], b[9], b[10], b[11]]);
        if version != VERSION { return Err(FormatError::BadVersion); }
        let sum = blake3(&b[..OFF_SUPER_SUM]);
        if b[OFF_SUPER_SUM..SUPER_LEN] != sum { return Err(FormatError::BadChecksum); }

        let block_size = u32::from_le_bytes([b[12], b[13], b[14], b[15]]);
        if block_size != BLOCK_SIZE { return Err(FormatError::BadField); }
        let generation = read_u64(b, OFF_GENERATION);
        let total_blocks = read_u64(b, OFF_TOTAL_BLOCKS);
        let log_head = read_u64(b, OFF_LOG_HEAD);
        // El log jamás puede empezar sobre las copias del superbloque, ni
        // salirse del volumen. Un valor así solo llega por corrupción o por un
        // formateador roto, y en cualquiera de los dos casos seguir leyendo
        // sería inventarse el volumen.
        if log_head < 2 || (total_blocks != 0 && log_head > total_blocks) {
            return Err(FormatError::BadField);
        }
        let mut disk_id = NO_HASH;
        disk_id.copy_from_slice(&b[OFF_DISK_ID..OFF_DISK_ID + 32]);
        let estrato = BlockPtr::decode(&b[OFF_ESTRATO..OFF_ESTRATO + objects::PTR_LEN])?;

        Ok(Self { version, block_size, generation, total_blocks, log_head, disk_id, estrato })
    }

    /// ¿Nació este volumen en el disco que tenemos delante?
    ///
    /// El gate de identidad del §5 del diseño. Si no cuadra, el volumen se
    /// monta **solo lectura** y CABINA grita: un volumen clonado a otro disco
    /// no se escribe por accidente.
    pub fn belongs_to(&self, disk: &Hash) -> bool {
        self.disk_id == *disk
    }

    /// ¿Está recién formateado, sin ningún estrato?
    pub fn is_empty(&self) -> bool { self.estrato.es_nulo() }
}

/// De las dos copias del superbloque, la que manda.
///
/// Gana la generación más alta **de las que son válidas**. Ese matiz es la
/// mitad del valor de tener dos: si el corte llegó escribiendo la copia nueva,
/// esa no pasa la suma y gana la vieja — que es exactamente el estado
/// consistente anterior. Sin comprobar la suma, dos copias son dos formas de
/// leer basura.
pub fn pick_superblock(a: &[u8], b: &[u8]) -> Result<(Superblock, u64), FormatError> {
    let da = Superblock::decode(a);
    let db = Superblock::decode(b);
    match (da, db) {
        (Ok(sa), Ok(sb)) => {
            if sa.generation >= sb.generation { Ok((sa, SUPER_A_BLOCK)) } else { Ok((sb, SUPER_B_BLOCK)) }
        }
        (Ok(sa), Err(_)) => Ok((sa, SUPER_A_BLOCK)),
        (Err(_), Ok(sb)) => Ok((sb, SUPER_B_BLOCK)),
        // Si ninguna vale, se devuelve el fallo de la copia A: es el más
        // informativo, porque B suele fallar por lo mismo.
        (Err(e), Err(_)) => Err(e),
    }
}

/// En qué bloque toca escribir el superbloque siguiente: siempre el OTRO.
pub fn next_super_block(current: u64) -> u64 {
    if current == SUPER_A_BLOCK { SUPER_B_BLOCK } else { SUPER_A_BLOCK }
}

// ── Estrato ─────────────────────────────────────────────────────────────────

/// Longitud fija de un estrato en disco.
pub const ESTRATO_LEN: usize = 224;

const OFF_E_RAIZ: usize = 0;
const OFF_E_PADRE: usize = 48;
const OFF_E_TIEMPO: usize = 96;
const OFF_E_AUTOR: usize = 104;
const OFF_E_PID: usize = 108;
const OFF_E_MOTIVO: usize = 112;
const MOTIVO_LEN: usize = 64;
const OFF_E_SUM: usize = ESTRATO_LEN - 32;

/// Quién creó un estrato.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Autor {
    Kernel,
    Proceso(u32),
    Herramienta,
}

impl Autor {
    fn code(self) -> u32 { match self { Autor::Kernel => 0, Autor::Proceso(_) => 1, Autor::Herramienta => 2 } }
    fn pid(self) -> u32 { match self { Autor::Proceso(p) => p, _ => 0 } }
    fn from(code: u32, pid: u32) -> Autor {
        match code { 1 => Autor::Proceso(pid), 2 => Autor::Herramienta, _ => Autor::Kernel }
    }
}

/// Una raíz: el commit que también es el superbloque.
///
/// Montar el sistema de ficheros es leer el último estrato válido; volver
/// atrás en el tiempo es leer uno anterior. **Son la misma operación** — por
/// eso no hay código de "restaurar", solo de "montar", y se le pasa otro.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Estrato {
    /// El nodo raíz del árbol de directorios. Puntero, no hash: ver la nota de
    /// `Superblock::estrato`.
    pub raiz: BlockPtr,
    /// El estrato anterior. Nulo = este es el primero. Recorrer la cadena hacia
    /// atrás es recorrer la historia, y por eso también tiene que ser un
    /// puntero: montar un estrato viejo es *encontrarlo*, no reconocerlo.
    pub padre: BlockPtr,
    pub tiempo: u64,
    pub autor: Autor,
    /// Por qué existe este estrato. Los que llevan motivo escrito a mano son
    /// los que el recolector **no suelta jamás** (§9).
    pub motivo: [u8; MOTIVO_LEN],
}

impl Estrato {
    pub fn new(raiz: BlockPtr, padre: BlockPtr, tiempo: u64, autor: Autor, motivo: &str) -> Self {
        let mut m = [0u8; MOTIVO_LEN];
        let n = motivo.len().min(MOTIVO_LEN);
        m[..n].copy_from_slice(&motivo.as_bytes()[..n]);
        Self { raiz, padre, tiempo, autor, motivo: m }
    }

    pub fn motivo_str(&self) -> &str {
        let n = self.motivo.iter().position(|&c| c == 0).unwrap_or(MOTIVO_LEN);
        core::str::from_utf8(&self.motivo[..n]).unwrap_or("")
    }

    /// ¿Tiene nombre puesto a mano? Los que sí, son permanentes.
    pub fn con_nombre(&self) -> bool { self.motivo[0] != 0 }

    pub fn encode(&self) -> [u8; ESTRATO_LEN] {
        let mut b = [0u8; ESTRATO_LEN];
        b[OFF_E_RAIZ..OFF_E_RAIZ + objects::PTR_LEN].copy_from_slice(&self.raiz.encode());
        b[OFF_E_PADRE..OFF_E_PADRE + objects::PTR_LEN].copy_from_slice(&self.padre.encode());
        b[OFF_E_TIEMPO..OFF_E_TIEMPO + 8].copy_from_slice(&self.tiempo.to_le_bytes());
        b[OFF_E_AUTOR..OFF_E_AUTOR + 4].copy_from_slice(&self.autor.code().to_le_bytes());
        b[OFF_E_PID..OFF_E_PID + 4].copy_from_slice(&self.autor.pid().to_le_bytes());
        b[OFF_E_MOTIVO..OFF_E_MOTIVO + MOTIVO_LEN].copy_from_slice(&self.motivo);
        let sum = blake3(&b[..OFF_E_SUM]);
        b[OFF_E_SUM..].copy_from_slice(&sum);
        b
    }

    pub fn decode(b: &[u8]) -> Result<Self, FormatError> {
        if b.len() < ESTRATO_LEN { return Err(FormatError::ShortBuffer); }
        let sum = blake3(&b[..OFF_E_SUM]);
        if b[OFF_E_SUM..ESTRATO_LEN] != sum { return Err(FormatError::BadChecksum); }
        let raiz = BlockPtr::decode(&b[OFF_E_RAIZ..OFF_E_RAIZ + objects::PTR_LEN])?;
        let padre = BlockPtr::decode(&b[OFF_E_PADRE..OFF_E_PADRE + objects::PTR_LEN])?;
        let tiempo = read_u64(b, OFF_E_TIEMPO);
        let autor = Autor::from(read_u32(b, OFF_E_AUTOR), read_u32(b, OFF_E_PID));
        let mut motivo = [0u8; MOTIVO_LEN];
        motivo.copy_from_slice(&b[OFF_E_MOTIVO..OFF_E_MOTIVO + MOTIVO_LEN]);
        Ok(Self { raiz, padre, tiempo, autor, motivo })
    }

    /// La identidad del estrato es el hash de su forma en disco. Direccionado
    /// por contenido, igual que todo lo demás.
    pub fn id(&self) -> Hash { blake3(&self.encode()) }
}

fn read_u64(b: &[u8], o: usize) -> u64 {
    let mut v = [0u8; 8];
    v.copy_from_slice(&b[o..o + 8]);
    u64::from_le_bytes(v)
}

fn read_u32(b: &[u8], o: usize) -> u32 {
    let mut v = [0u8; 4];
    v.copy_from_slice(&b[o..o + 4]);
    u32::from_le_bytes(v)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id_de_prueba() -> Hash {
        disk_id(b"KINGSTON SA400S37480G", b"50026B76846C2058", 937703088)
    }

    #[test]
    fn el_superbloque_sobrevive_a_la_ida_y_vuelta() {
        let sb = Superblock::new(id_de_prueba(), 108_003_328);
        let bytes = sb.encode();
        assert_eq!(bytes.len(), SUPER_LEN);
        assert_eq!(&bytes[..8], &MAGIC);
        assert_eq!(Superblock::decode(&bytes).unwrap(), sb);
        assert!(sb.is_empty());
    }

    #[test]
    fn un_bit_cambiado_es_corrupcion_detectada() {
        // Esto es el principio 2 del diseño: el sistema de ficheros detecta su
        // propia corrupción en vez de confiar en que el disco devuelve lo que
        // guardó.
        let sb = Superblock::new(id_de_prueba(), 108_003_328);
        let mut bytes = sb.encode();
        bytes[OFF_LOG_HEAD] ^= 0x01;
        assert_eq!(Superblock::decode(&bytes), Err(FormatError::BadChecksum));
    }

    #[test]
    fn un_sector_ajeno_no_es_un_volumen_corrupto() {
        // Distinguir "aquí no hay ESTRATOS" de "está roto" importa: lo primero
        // es lo que pasa al mirar una partición NTFS, y no debe alarmar.
        let ajeno = [0x55u8; SUPER_LEN];
        assert_eq!(Superblock::decode(&ajeno), Err(FormatError::BadMagic));
    }

    #[test]
    fn gana_la_generacion_mas_alta() {
        let mut a = Superblock::new(id_de_prueba(), 1000);
        let mut b = a;
        a.generation = 7;
        b.generation = 8;
        let (elegido, donde) = pick_superblock(&a.encode(), &b.encode()).unwrap();
        assert_eq!(elegido.generation, 8);
        assert_eq!(donde, SUPER_B_BLOCK);
    }

    #[test]
    fn si_la_copia_nueva_se_corta_gana_la_vieja() {
        // El escenario que justifica tener dos copias: el corte llegó a mitad
        // de escribir la nueva. La vieja está entera y es un estado
        // consistente; la nueva no pasa la suma y se descarta.
        let mut vieja = Superblock::new(id_de_prueba(), 1000);
        vieja.generation = 41;
        let mut nueva = vieja;
        nueva.generation = 42;
        let mut rota = nueva.encode();
        rota[100] ^= 0xFF; // el disco escribió a medias
        let (elegido, donde) = pick_superblock(&vieja.encode(), &rota).unwrap();
        assert_eq!(elegido.generation, 41);
        assert_eq!(donde, SUPER_A_BLOCK);
    }

    #[test]
    fn el_log_no_puede_empezar_sobre_los_superbloques() {
        let mut sb = Superblock::new(id_de_prueba(), 1000);
        sb.log_head = 1; // pisaría la copia B
        let mut bytes = sb.encode();
        // Re-firmar para que falle por el CAMPO y no por la suma.
        let sum = blake3(&bytes[..OFF_SUPER_SUM]);
        bytes[OFF_SUPER_SUM..].copy_from_slice(&sum);
        assert_eq!(Superblock::decode(&bytes), Err(FormatError::BadField));
    }

    #[test]
    fn el_disco_ajeno_no_es_el_nuestro() {
        let nuestro = id_de_prueba();
        let sb = Superblock::new(nuestro, 1000);
        assert!(sb.belongs_to(&nuestro));
        // Mismo modelo y misma serie, distinta capacidad: una imagen clonada a
        // un disco de otro tamaño. No es el nuestro.
        let clonado = disk_id(b"KINGSTON SA400S37480G", b"50026B76846C2058", 937703087);
        assert!(!sb.belongs_to(&clonado));
    }

    #[test]
    fn el_estrato_sobrevive_a_la_ida_y_vuelta() {
        let raiz = BlockPtr::nuevo(4, 0, b"nodo raiz");
        let e = Estrato::new(raiz, BlockPtr::NULO, 1_700_000_000, Autor::Herramienta, "formato inicial");
        let bytes = e.encode();
        assert_eq!(bytes.len(), ESTRATO_LEN);
        let vuelto = Estrato::decode(&bytes).unwrap();
        assert_eq!(vuelto, e);
        assert_eq!(vuelto.motivo_str(), "formato inicial");
        assert!(vuelto.con_nombre());
        assert_eq!(vuelto.autor, Autor::Herramienta);
    }

    #[test]
    fn el_estrato_se_identifica_por_su_contenido() {
        let r1 = BlockPtr::nuevo(4, 0, b"arbol A");
        let r2 = BlockPtr::nuevo(4, 0, b"arbol B");
        let a = Estrato::new(r1, BlockPtr::NULO, 100, Autor::Kernel, "auto");
        let b = Estrato::new(r1, BlockPtr::NULO, 100, Autor::Kernel, "auto");
        let c = Estrato::new(r2, BlockPtr::NULO, 100, Autor::Kernel, "auto");
        assert_eq!(a.id(), b.id());
        assert_ne!(a.id(), c.id());
    }

    #[test]
    fn un_estrato_automatico_no_lleva_nombre() {
        let e = Estrato::new(BlockPtr::nuevo(4, 0, b"x"), BlockPtr::NULO, 100, Autor::Proceso(3), "");
        assert!(!e.con_nombre());
        assert_eq!(e.autor, Autor::Proceso(3));
    }
}
