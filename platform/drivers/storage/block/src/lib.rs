//! El contrato de bloques: lo unico que un sistema de ficheros necesita saber
//! del almacenamiento.
//!
//! Es el **paso 3** del orden de construccion de ESTRATOS
//! (la especificacion de ESTRATOS, section 10): *"el contrato unico
//! leer / escribir / capacidad / identidad, con AHCI y NVMe debajo. ESTRATOS
//! habla con eso, no con SATA."*
//!
//! ## Por que esto es un contrato y no una capa
//!
//! La regla de la casa es **contratos y formatos, nunca cerebros**. Aqui no se
//! procesa nada: no hay cache, ni planificador de peticiones, ni traduccion de
//! direcciones. Solo se declara la forma que tiene un dispositivo de bloques
//! para todo el que este por encima. Por eso esta crate **no depende de nadie**
//! -- en cuanto dependiera de un driver concreto dejaria de ser la frontera
//! entre las capas para pasar a ser una capa mas.
//!
//! El dia que haya un NVMe cableado, ESTRATOS y FAT32 no se enteran: alguien
//! registra otro dispositivo y ya esta.
//!
//! ## Por que la IDENTIDAD es parte del contrato
//!
//! Podria parecer que un dispositivo de bloques es solo `read` y `write`.
//! No en esta maquina. Aqui hay tres discos y en uno vive el sistema
//! operativo del dueno; un dispositivo que no puede decir QUIEN ES no se puede
//! escribir con seguridad, asi que la identidad no es un extra informativo:
//! es la mitad del contrato. El superbloque de ESTRATOS graba el `disco_id`
//! DENTRO del volumen justamente para poder comparar contra esto al montar, y
//! negarse a escribir en un volumen clonado a otro disco.
//!
//! ## Estado
//!
//! Implementado por AHCI/SATA en el kernel (`ring0/dev/disk/`, que es un
//! directorio de ocho ficheros desde el reparto del 14-08).
//!
//! **NVMe no, y ya ni siquiera hay de donde partir.** Esta nota decia que *"la
//! crate `bmo-nvme` existe y tiene lectura y escritura, pero nadie la ha puesto
//! detras de este contrato"*. **No existe**: se la llevo `b33f3966` el
//! 2026-08-03 --*"fuera seis librerias huerfanas, 3.671 lineas que nadie
//! cableo"*-- y esta cabecera siguio prometiendola quince dias.
//!
//! [!] Es el MISMO commit que borro `ESTRATOS.md`. Dejo dos punteros rotos y
//! solo se encontro uno: el del diseno lo destapo el guardian de citas porque
//! apuntaba a una RUTA, y este no, porque afirma que existe una CRATE. El
//! guardian comprueba caminos de fichero, no inventario.
//!
//! Lo que no cambia: en esta maquina el NVMe es el disco de Windows del dueno.
//! Se dice, no se insinua.

#![no_std]

/// Bytes de un bloque logico. Todo LBA de BMO es de 512 B por ahora; el campo
/// existe en [`DeviceId`] porque los discos de 4 KiB nativos existen y el dia
/// que aparezca uno, el que se rompa tiene que ser el driver, no el contrato.
pub const SECTOR: usize = 512;

/// Por que fallo una operacion de bloques.
///
/// Un `bool` no distingue "el disco esta roto" de "me pediste un sector que no
/// existe", y son dos conversaciones distintas: una es un fallo de hardware y
/// la otra un bug de quien llama.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockError {
    /// No hay dispositivo, o no termino de inicializarse.
    NotReady,
    /// El rango pedido se sale de la capacidad del dispositivo.
    OutOfRange,
    /// El dispositivo, o el rango, es de solo lectura.
    ReadOnly,
    /// El buffer del llamante no da para los bloques pedidos.
    ShortBuffer,
    /// El dispositivo respondio con error.
    Device,
    /// La operacion no termino dentro del limite.
    Timeout,
    /// El dispositivo no implementa esta operacion.
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

/// Quien es este dispositivo, segun el mismo.
///
/// Los tamanos salen de lo que declara `IDENTIFY DEVICE` de ATA: 40 bytes de
/// modelo y 20 de serie. Se guardan como bytes con su longitud util y no como
/// cadenas porque en Ring 0 no hay reservas de memoria: el buffer viaja
/// entero y el que lo lee decide que hacer con el.
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
    /// Ha dicho el dispositivo quien es? Sin modelo Y serie no hay identidad
    /// que comparar, y sin identidad no se escribe.
    pub fn is_known(&self) -> bool {
        self.model_len > 0 && self.serial_len > 0 && self.blocks > 0
    }
    /// Son el mismo disco fisico?
    ///
    /// Es la comparacion que ESTRATOS hace al montar contra el `disco_id`
    /// grabado en su superbloque. Modelo Y serie Y capacidad: el modelo solo
    /// dice que disco ES, la serie dice CUAL, y la capacidad caza el caso de
    /// una imagen clonada a un disco de otro tamano.
    pub fn same_device(&self, other: &DeviceId) -> bool {
        self.is_known() && other.is_known()
            && self.model_len == other.model_len
            && self.serial_len == other.serial_len
            && self.blocks == other.blocks
            && self.model[..self.model_len] == other.model[..other.model_len]
            && self.serial[..self.serial_len] == other.serial[..other.serial_len]
    }
}

/// Un dispositivo de bloques. Cuatro operaciones y ni una mas.
///
/// `&self` y no `&mut self` a proposito: por debajo hay un motor de DMA con su
/// propio estado, no una estructura de datos de Rust. Quien implemente esto se
/// hace responsable de su exclusion mutua -- y mientras no haya SMP eso
/// significa que **no puede haber dos escritores** (ESTRATOS section 12).
pub trait BlockDevice {
    /// Quien es. Ver [`DeviceId`].
    fn identity(&self) -> DeviceId;

    /// Bloques direccionables.
    fn capacity(&self) -> u64 { self.identity().blocks }

    /// Bytes por bloque.
    fn block_size(&self) -> u32 { self.identity().block_size }

    /// Lee `count` bloques desde `lba`. Devuelve los leidos de verdad.
    fn read(&self, lba: u64, count: u16, buf: &mut [u8]) -> Result<u16, BlockError>;

    /// Escribe `count` bloques en `lba`. Devuelve los escritos de verdad.
    ///
    /// Un dispositivo puede negarse con [`BlockError::ReadOnly`] -- y debe
    /// hacerlo si no ha podido establecer su identidad.
    fn write(&self, lba: u64, count: u16, data: &[u8]) -> Result<u16, BlockError>;

    /// Obliga al dispositivo a bajar a la superficie lo que acepto.
    ///
    /// **No es opcional.** Es el paso 4 de la escritura de ESTRATOS: la
    /// barrera antes del superbloque. Un disco que dice "ya esta" con el dato
    /// todavia en su cache convierte cualquier diseno transaccional en
    /// decoracion.
    fn flush(&self) -> Result<(), BlockError>;

    /// Se puede escribir en este dispositivo ahora mismo?
    ///
    /// Separado de que `write` falle: quien va a formatear o montar para
    /// escritura quiere saberlo ANTES de empezar, no a mitad.
    fn writable(&self) -> bool { false }
}

// -- El registro -------------------------------------------------------------

static mut DEVICE: Option<&'static dyn BlockDevice> = None;

/// Registra EL dispositivo de bloques de BMO.
///
/// Uno solo, y es deliberado. En esta maquina hay tres discos y dos de ellos
/// son ajenos; un registro que aceptara varios invitaria a que algo de arriba
/// recorriera la lista y eligiera mal. El que elige QUE disco es de BMO es el
/// kernel, una vez, mirando el tipo de controlador -- no un bucle sobre un
/// vector.
pub fn register(dev: &'static dyn BlockDevice) {
    unsafe { DEVICE = Some(dev); }
}

/// El dispositivo de bloques de BMO, si ya se registro.
pub fn device() -> Option<&'static dyn BlockDevice> {
    unsafe { DEVICE }
}

/// Hay dispositivo y sabe quien es?
pub fn is_identified() -> bool {
    match device() {
        Some(d) => d.identity().is_known(),
        None => false,
    }
}

// ============================================================================
// LAS VENTANAS DE ESCRITURA
// ============================================================================

/// **Donde SE PUEDE escribir, y por que.** Paso 2 de
/// `docs/plan/PLAN_ALMACENAMIENTO.md`.
///
/// # Por que vive en el CONTRATO y no en el kernel
///
/// Dos razones, y la segunda es la que decidio.
///
/// 1. Es lo que el plan ya argumentaba: *"un dispositivo que solo acepta
///    escrituras dentro de un rango de LBA declarado es una idea mejor, y
///    merece vivir en el contrato, no como caso especial del pegamento"*.
///    **Linux no tiene equivalente**: alli un dispositivo es un nombre global
///    y el acceso son bits de permiso, asi que `dd if=/dev/nvme0n1` existe y
///    root lo puede todo.
///
/// 2. ** Y la que lo cerro: EN EL KERNEL ESTAS PRUEBAS NO PUEDEN CORRER.
///    `bmo-kernel` es un binario `no_std` para `x86_64-unknown-none` con asm
///    desnudo; `cargo test -p bmo-kernel` ni compila. Un `#[cfg(test)]` alli
///    seria decoracion -- codigo que parece una prueba y no se ejecuta jamas.
///    Aqui son siete casillas que corren en 0 segundos.
///
/// Cada una de ellas es una forma de perder un disco.
pub mod ventana {
    /// **La decision, y no toca nada de fuera.**
    ///
    /// Todo entra por parametro: si hay disco, si el gate de identidad armo la
    /// escritura, y las dos ventanas como rangos `(primero, ultimo)` con el
    /// ultimo INCLUSIVE. Ni un `static`, ni un registro, ni un dispositivo.
    ///
    /// ** Esa firma es el punto entero. Mientras la decision leia sus cuatro
    /// datos de variables globales del kernel, la unica forma de comprobar
    /// *"que pasa si alguien pide escribir sobre la ESP"* era arrancar la
    /// maquina y pedirlo -- o sea arriesgar el arranque para probar el
    /// guardian que protege el arranque.
    pub fn decidir(
        listo: bool,
        armada: bool,
        datos: Option<(u64, u64)>,
        estratos: Option<(u64, u64)>,
        lba: u64,
        count: u16,
    ) -> Result<(), &'static str> {
        decidir_rango(listo, armada, datos, estratos, lba, count as u64)
    }

    /// **La misma decision, para un rango que no cabe en un `u16`.**
    ///
    /// # Por que hizo falta, y por que es la MISMA funcion y no otra
    ///
    /// Un `WRITE DMA` mueve como mucho 65.535 sectores, asi que el contador de
    /// una escritura cabe en 16 bits y [`decidir`] nacio con esa firma. **TRIM
    /// no mueve datos**: una sola orden puede cubrir 2 GiB, y la cola libre de un
    /// volumen de 414 GiB son 800 millones de sectores.
    ///
    /// ** La tentacion era comprobar la ventana tanda a tanda, con el contador
    /// que cabe. Eso es un guardian mas debil disfrazado del mismo: cada trozo
    /// caeria dentro y **nadie miraria el rango entero**, que es justo lo que
    /// aqui se protege. Se ensancha el tipo del contador, no la ventana.
    ///
    /// La regla de arriba sigue mandando: un rango que empieza dentro y acaba
    /// fuera se rechaza ENTERO. Recortar la peticion hasta lo que cabe seria
    /// obedecer a medias una orden que estaba mal.
    pub fn decidir_rango(
        listo: bool,
        armada: bool,
        datos: Option<(u64, u64)>,
        estratos: Option<(u64, u64)>,
        lba: u64,
        sectores: u64,
    ) -> Result<(), &'static str> {
        if !listo {
            return Err("sin disco");
        }
        if !armada {
            return Err("la escritura no esta armada (gate de identidad)");
        }
        if sectores == 0 {
            return Err("cero sectores");
        }
        // Sin `checked_add`, un LBA cerca del maximo daria la vuelta y el
        // rango pareceria diminuto y valido.
        let Some(end) = lba.checked_add(sectores) else {
            return Err("el rango de LBA desborda");
        };
        for w in [datos, estratos].into_iter().flatten() {
            if lba >= w.0 && end <= w.1 + 1 {
                return Ok(());
            }
        }
        Err("fuera de las ventanas de escritura (datos / ESTRATOS)")
    }

    #[cfg(test)]
    mod casillas {
        use super::*;

        /// BMO-DATA en una maquina de verdad.
        const DATOS: Option<(u64, u64)> = Some((206_848, 1_000_000));

        #[test]
        fn dentro_de_los_datos_se_escribe() {
            assert!(decidir(true, true, DATOS, None, 300_000, 8).is_ok());
        }

        /// ** LA CASILLA QUE JUSTIFICA TODO ESTO.
        ///
        /// La ESP vive en los LBA bajos (2048..206_847 en esta maquina). Ahi
        /// esta el `BOOTX64.EFI` con el que arranca BMO -- y en una maquina con
        /// Windows, el cargador del boss. **No hay ventana que la cubra**, y
        /// escribir ahi tiene que fallar aunque todo lo demas este en orden.
        #[test]
        fn sobre_la_esp_no_se_escribe_ni_con_el_disco_armado() {
            let r = decidir(true, true, DATOS, None, 2048, 1);
            assert_eq!(
                r.unwrap_err(),
                "fuera de las ventanas de escritura (datos / ESTRATOS)"
            );
        }

        #[test]
        fn un_rango_que_empieza_dentro_y_acaba_fuera_se_rechaza() {
            // El ultimo sector exacto SI cabe: `last_lba` es inclusivo.
            assert!(decidir(true, true, DATOS, None, 1_000_000, 1).is_ok());
            // Pidiendo dos, el segundo ya cae fuera.
            assert!(decidir(true, true, DATOS, None, 1_000_000, 2).is_err());
        }

        /// ** El desbordamiento, que es como un rango enorme parece diminuto:
        /// sin `checked_add`, `u64::MAX + 2` da la vuelta y `end` sale MENOR
        /// que `lba`, asi que la comprobacion de rango pasaria.
        #[test]
        fn un_lba_que_desborda_no_se_cuela_por_dar_la_vuelta() {
            let r = decidir(true, true, DATOS, None, u64::MAX, 2);
            assert_eq!(r.unwrap_err(), "el rango de LBA desborda");
        }

        /// ** Las dos ventanas son DOS, y el hueco entre ellas importa.
        ///
        /// Si alguien "simplificara" ensanchando una sola ventana que las
        /// cubriera a ambas, este hueco quedaria abierto. Ensanchar un guardian
        /// es quitarlo.
        #[test]
        fn las_dos_ventanas_no_se_funden_en_una() {
            let es = Some((2_000_000, 3_000_000));
            assert!(decidir(true, true, DATOS, es, 2_500_000, 4).is_ok());
            assert!(decidir(true, true, DATOS, es, 1_500_000, 4).is_err());
        }

        #[test]
        fn sin_el_gate_de_identidad_no_se_escribe_en_ninguna_ventana() {
            let r = decidir(true, false, DATOS, None, 300_000, 8);
            assert_eq!(
                r.unwrap_err(),
                "la escritura no esta armada (gate de identidad)"
            );
        }

        /// El estado inicial de la maquina es "no se puede", y hay que
        /// ganarselo.
        /// ** UN RANGO DE TRIM SE JUZGA ENTERO, no tanda a tanda.
        ///
        /// La cola libre de un volumen son cientos de millones de sectores: no
        /// caben en el `u16` de una escritura. Si el que recorta preguntara por
        /// trozos, cada trozo caeria dentro de la ventana y **el rango completo
        /// que se sale por el final no lo miraria nadie**.
        #[test]
        fn un_rango_gigante_se_juzga_entero_y_no_por_trozos() {
            let es = Some((2_000_000, 900_000_000));
            // Entero y dentro: 800 millones de sectores de una vez.
            assert!(decidir_rango(true, true, None, es, 2_000_000, 800_000_000).is_ok());
            // Se pasa por el final: se rechaza aunque el principio sea legal.
            assert!(decidir_rango(true, true, None, es, 2_000_000, 900_000_000).is_err());
            // Y el trozo suelto SI cabria: por eso no se pregunta por trozos.
            assert!(decidir_rango(true, true, None, es, 2_000_000, 8).is_ok());
        }

        /// Las dos puertas contestan lo mismo para lo que las dos entienden.
        #[test]
        fn el_contador_ancho_no_cambia_ninguna_respuesta() {
            for lba in [2048u64, 206_848, 300_000, 1_000_000] {
                for n in [1u16, 8, 4096] {
                    assert_eq!(
                        decidir(true, true, DATOS, None, lba, n),
                        decidir_rango(true, true, DATOS, None, lba, n as u64),
                    );
                }
            }
        }

        #[test]
        fn sin_ninguna_ventana_no_hay_donde_escribir() {
            assert!(decidir(true, true, None, None, 300_000, 8).is_err());
            assert_eq!(decidir(false, true, DATOS, None, 300_000, 8).unwrap_err(), "sin disco");
            assert_eq!(decidir(true, true, DATOS, None, 300_000, 0).unwrap_err(), "cero sectores");
        }
    }
}
