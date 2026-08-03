//! El contrato de bloques: lo único que un sistema de ficheros necesita saber
//! del almacenamiento.
//!
//! Es el **paso 3** del orden de construcción de ESTRATOS
//! (la especificacion de ESTRATOS, §10): *"el contrato único
//! leer / escribir / capacidad / identidad, con AHCI y NVMe debajo. ESTRATOS
//! habla con eso, no con SATA."*
//!
//! ## Por qué esto es un contrato y no una capa
//!
//! La regla de la casa es **contratos y formatos, nunca cerebros**. Aquí no se
//! procesa nada: no hay caché, ni planificador de peticiones, ni traducción de
//! direcciones. Solo se declara la forma que tiene un dispositivo de bloques
//! para todo el que esté por encima. Por eso esta crate **no depende de nadie**
//! — en cuanto dependiera de un driver concreto dejaría de ser la frontera
//! entre las capas para pasar a ser una capa más.
//!
//! El día que haya un NVMe cableado, ESTRATOS y FAT32 no se enteran: alguien
//! registra otro dispositivo y ya está.
//!
//! ## Por qué la IDENTIDAD es parte del contrato
//!
//! Podría parecer que un dispositivo de bloques es solo `leer` y `escribir`.
//! No en esta máquina. Aquí hay tres discos y en uno vive el sistema
//! operativo del dueño; un dispositivo que no puede decir QUIÉN ES no se puede
//! escribir con seguridad, así que la identidad no es un extra informativo:
//! es la mitad del contrato. El superbloque de ESTRATOS graba el `disco_id`
//! DENTRO del volumen justamente para poder comparar contra esto al montar, y
//! negarse a escribir en un volumen clonado a otro disco.
//!
//! ## Estado
//!
//! Implementado por AHCI/SATA en el kernel (`ring0/dev/disk.rs`). **NVMe no**:
//! la crate `bmo-nvme` existe y tiene lectura y escritura, pero nadie la ha
//! puesto detrás de este contrato todavía, y en esta máquina el NVMe es el
//! disco de Windows. Se dice, no se insinúa.

#![no_std]

/// Bytes de un bloque lógico. Todo LBA de BMO es de 512 B por ahora; el campo
/// existe en [`DeviceId`] porque los discos de 4 KiB nativos existen y el día
/// que aparezca uno, el que se rompa tiene que ser el driver, no el contrato.
pub const SECTOR: usize = 512;

/// Por qué falló una operación de bloques.
///
/// Un `bool` no distingue "el disco está roto" de "me pediste un sector que no
/// existe", y son dos conversaciones distintas: una es un fallo de hardware y
/// la otra un bug de quien llama.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockError {
    /// No hay dispositivo, o no terminó de inicializarse.
    NotReady,
    /// El rango pedido se sale de la capacidad del dispositivo.
    OutOfRange,
    /// El dispositivo, o el rango, es de solo lectura.
    ReadOnly,
    /// El buffer del llamante no da para los bloques pedidos.
    ShortBuffer,
    /// El dispositivo respondió con error.
    Device,
    /// La operación no terminó dentro del límite.
    Timeout,
    /// El dispositivo no implementa esta operación.
    Unsupported,
}

impl BlockError {
    pub fn name(self) -> &'static str {
        match self {
            BlockError::NotReady => "el dispositivo no esta listo",
            BlockError::OutOfRange => "fuera de la capacidad del dispositivo",
            BlockError::ReadOnly => "solo lectura",
            BlockError::ShortBuffer => "el buffer no da para los bloques pedidos",
            BlockError::Device => "el dispositivo respondio con error",
            BlockError::Timeout => "la operacion no termino a tiempo",
            BlockError::Unsupported => "operacion no soportada",
        }
    }
}

/// Quién es este dispositivo, según él mismo.
///
/// Los tamaños salen de lo que declara `IDENTIFY DEVICE` de ATA: 40 bytes de
/// modelo y 20 de serie. Se guardan como bytes con su longitud útil y no como
/// cadenas porque en Ring 0 no hay reservas de memoria: el buffer viaja
/// entero y el que lo lee decide qué hacer con él.
#[derive(Clone, Copy)]
pub struct DeviceId {
    pub model: [u8; 40],
    pub model_len: usize,
    pub serial: [u8; 20],
    pub serial_len: usize,
    /// Bloques direccionables. La CAPACIDAD del contrato.
    pub blocks: u64,
    /// Bytes por bloque.
    pub block_size: u32,
}

impl DeviceId {
    pub const EMPTY: DeviceId = DeviceId {
        model: [0; 40], model_len: 0,
        serial: [0; 20], serial_len: 0,
        blocks: 0, block_size: SECTOR as u32,
    };

    pub fn model_str(&self) -> &str {
        core::str::from_utf8(&self.model[..self.model_len]).unwrap_or("")
    }
    pub fn serial_str(&self) -> &str {
        core::str::from_utf8(&self.serial[..self.serial_len]).unwrap_or("")
    }
    /// ¿Ha dicho el dispositivo quién es? Sin modelo Y serie no hay identidad
    /// que comparar, y sin identidad no se escribe.
    pub fn is_known(&self) -> bool {
        self.model_len > 0 && self.serial_len > 0 && self.blocks > 0
    }
    /// ¿Son el mismo disco físico?
    ///
    /// Es la comparación que ESTRATOS hace al montar contra el `disco_id`
    /// grabado en su superbloque. Modelo Y serie Y capacidad: el modelo solo
    /// dice qué disco ES, la serie dice CUÁL, y la capacidad caza el caso de
    /// una imagen clonada a un disco de otro tamaño.
    pub fn same_device(&self, other: &DeviceId) -> bool {
        self.is_known() && other.is_known()
            && self.model_len == other.model_len
            && self.serial_len == other.serial_len
            && self.blocks == other.blocks
            && self.model[..self.model_len] == other.model[..other.model_len]
            && self.serial[..self.serial_len] == other.serial[..other.serial_len]
    }
}

/// Un dispositivo de bloques. Cuatro operaciones y ni una más.
///
/// `&self` y no `&mut self` a propósito: por debajo hay un motor de DMA con su
/// propio estado, no una estructura de datos de Rust. Quien implemente esto se
/// hace responsable de su exclusión mutua — y mientras no haya SMP eso
/// significa que **no puede haber dos escritores** (ESTRATOS §12).
pub trait BlockDevice {
    /// Quién es. Ver [`DeviceId`].
    fn identity(&self) -> DeviceId;

    /// Bloques direccionables.
    fn capacity(&self) -> u64 { self.identity().blocks }

    /// Bytes por bloque.
    fn block_size(&self) -> u32 { self.identity().block_size }

    /// Lee `count` bloques desde `lba`. Devuelve los leídos de verdad.
    fn read(&self, lba: u64, count: u16, buf: &mut [u8]) -> Result<u16, BlockError>;

    /// Escribe `count` bloques en `lba`. Devuelve los escritos de verdad.
    ///
    /// Un dispositivo puede negarse con [`BlockError::ReadOnly`] — y debe
    /// hacerlo si no ha podido establecer su identidad.
    fn write(&self, lba: u64, count: u16, data: &[u8]) -> Result<u16, BlockError>;

    /// Obliga al dispositivo a bajar a la superficie lo que aceptó.
    ///
    /// **No es opcional.** Es el paso 4 de la escritura de ESTRATOS: la
    /// barrera antes del superbloque. Un disco que dice "ya está" con el dato
    /// todavía en su caché convierte cualquier diseño transaccional en
    /// decoración.
    fn flush(&self) -> Result<(), BlockError>;

    /// ¿Se puede escribir en este dispositivo ahora mismo?
    ///
    /// Separado de que `write` falle: quien va a formatear o montar para
    /// escritura quiere saberlo ANTES de empezar, no a mitad.
    fn writable(&self) -> bool { false }
}

// ── El registro ─────────────────────────────────────────────────────────────

static mut DEVICE: Option<&'static dyn BlockDevice> = None;

/// Registra EL dispositivo de bloques de BMO.
///
/// Uno solo, y es deliberado. En esta máquina hay tres discos y dos de ellos
/// son ajenos; un registro que aceptara varios invitaría a que algo de arriba
/// recorriera la lista y eligiera mal. El que elige QUÉ disco es de BMO es el
/// kernel, una vez, mirando el tipo de controlador — no un bucle sobre un
/// vector.
pub fn register(dev: &'static dyn BlockDevice) {
    unsafe { DEVICE = Some(dev); }
}

/// El dispositivo de bloques de BMO, si ya se registró.
pub fn device() -> Option<&'static dyn BlockDevice> {
    unsafe { DEVICE }
}

/// ¿Hay dispositivo y sabe quién es?
pub fn is_identified() -> bool {
    match device() {
        Some(d) => d.identity().is_known(),
        None => false,
    }
}
