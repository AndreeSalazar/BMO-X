//! **EL JUEZ DE LA ADMISION: cabe lo que este programa DECLARA que va a pedir?**
//!
//! # La regla 7, y por que estaba sin cumplir
//!
//! `docs/identidad/LA_RAM.md`, Parte III, regla 7: *"lo que se declara, se
//! cumple o se grita"*. Y la Parte IV lo remata:
//!
//! > *"hoy se dice 'no' al quinto `malloc`; con el manifiesto se puede decir
//! > 'no' **antes de empezar**, que es cuando el fallo no cuesta nada."*
//!
//! La seccion `Requisitos = 0x15` existe, tiene formato, tiene lector, y **cada
//! `.bex` que sale del escritor ya la lleva escrita**. Lo unico que faltaba era
//! que alguien la leyera antes de asignar el primer marco: el kernel importaba
//! `SECTION_REQUISITOS` en `task/bex.rs` y no la usaba en ninguna linea.
//!
//! # Que decide, y que NO decide
//!
//! Decide **una comparacion**: lo declarado obligatorio contra lo que hay. Nada
//! mas. No lee el fichero, no conoce el formato BEF, no sabe que es un marco de
//! 4 KiB. Se le pasan dos numeros y contesta.
//!
//! *** Esa pobreza es el diseno. Un juez que supiera abrir la seccion tendria
//! que saber de formatos, y entonces el formato podria equivocarlo. Aqui lo que
//! entra ya son cantidades, asi que **lo unico que puede fallar es la
//! aritmetica** -- y la aritmetica se prueba.
//!
//! # Por que el margen no es cero
//!
//! Un programa que declara exactamente lo que hay entra... y deja la maquina
//! sin un solo marco para el kernel: ni una pila de hilo, ni un buffer de DMA,
//! ni la pagina de rebote del disco. La admision se pasa y la maquina muere
//! diez milisegundos despues, que es peor que un "no" porque el "no" ya no se
//! puede dar.
//!
//! > Aceptar lo que cabe JUSTO no es ser generoso: es mover el fallo a un sitio
//! > donde ya no hay nadie para contarlo.
//!
//! El margen lo pone el llamante, como el espejo de `bmo-fisica-juicio`: **este
//! crate no tiene ni una constante de tamano**, y por eso no puede equivocarse
//! en el techo. La regla 3 de L6g cumplida quitando la posibilidad.

#![no_std]

/// Lo que el juez contesta. Cada variante es una frase distinta para el dueno.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Veredicto {
    /// Cabe, con el margen pedido.
    Cabe,
    /// No cabe: falta memoria. Lleva **cuantos bytes** faltan.
    NoCabe { faltan: u64 },
    /// Cabe la memoria, pero no quedaria margen para el kernel.
    SinMargen { faltan: u64 },
    /// El programa no declara nada obligatorio. **No es un fallo**: ver abajo.
    NoDeclara,
}

impl Veredicto {
    /// `true` solo si se puede admitir.
    pub const fn admite(&self) -> bool {
        matches!(self, Veredicto::Cabe | Veredicto::NoDeclara)
    }
}

/// **Cabe?** Todo en bytes.
///
/// - `pide`      lo que el programa declara OBLIGATORIO (clase memoria + monton).
/// - `libre`     lo que la maquina tiene libre AHORA.
/// - `margen`    lo que hay que dejarle al kernel despues de admitir.
///
/// # Por que `NoDeclara` admite
///
/// Un `.bex` viejo --compilado antes de que la seccion se escribiera-- no
/// declara nada, y rechazarlo seria romper todo lo que ya existe en el disco
/// del dueno el dia que se enciende esta regla.
///
/// *** Y la eleccion tiene su precio dicho: un programa sin declaracion entra
/// **como entraba antes**, o sea que para el sigue valiendo el "no" tardio del
/// quinto `malloc`. La regla 7 se cumple para quien declara, y quien no declara
/// no empeora. Es un trinquete, como L6a: **lo nuevo obliga, lo viejo se
/// tolera con su motivo escrito.**
pub const fn cabe(pide: u64, libre: u64, margen: u64) -> Veredicto {
    if pide == 0 {
        return Veredicto::NoDeclara;
    }
    if pide > libre {
        return Veredicto::NoCabe {
            faltan: pide - libre,
        };
    }
    // ** El margen se comprueba APARTE y con su propia variante, no sumado a lo
    // que pide. Dos motivos distintos que dan el mismo "no" son dos frases
    // distintas para quien lo lee: *"tu programa es muy grande"* y *"tu programa
    // cabe pero no dejaria respirar a la maquina"* mandan a hacer cosas
    // distintas.
    let queda = libre - pide;
    if queda < margen {
        return Veredicto::SinMargen {
            faltan: margen - queda,
        };
    }
    Veredicto::Cabe
}

// == EL LECTOR DE LA TABLA DECLARADA ========================================
//
// ** Vivia en `bmo-abi::bef::requisitos` y se MUDO aqui el 2026-08-31, entero,
// para que **no haya dos**.
//
// `bmo-abi` usa `alloc` --su mitad escritora construye un `Vec`-- asi que el
// kernel no puede depender de ella. Ese es el motivo de que exista
// `bmo-bex-gate`, que es la copia sin `alloc` de la parte del formato que toca
// el cargador... y copiar aqui el lector habria sido **la tercera** version del
// mismo hecho.
//
// *** Asi que el lector baja a donde no hay dependencias y los dos lados suben
// a buscarlo: `bmo-abi` lo reexporta para el escritor y el toolchain, Ring 0 lo
// usa directo. Una sola verdad, dos consumidores. Es la misma leccion que la
// linea base de kinds del contrato --dos tablas que dicen lo mismo son cinco
// divergencias esperando-- aplicada ANTES de que pasara.
//
// [!] Lo que NO baja: `construir` y `Declaracion`. Necesitan `Vec` y solo las
// usa quien escribe un `.bex`, o sea nunca el kernel.

// -- Lectores acotados -------------------------------------------------------
//
// Los mismos tres de siempre y por el mismo motivo: los bytes vienen del disco,
// asi que **nada se indexa sin comprobar**. Un `bytes[o+3]` en un lector de
// formato es un `panic` en Ring 0 esperando a un fichero mal escrito.

fn leer_u16(b: &[u8], o: usize) -> Option<u16> {
    Some(u16::from_le_bytes(b.get(o..o + 2)?.try_into().ok()?))
}
fn leer_u32(b: &[u8], o: usize) -> Option<u32> {
    Some(u32::from_le_bytes(b.get(o..o + 4)?.try_into().ok()?))
}
fn leer_u64(b: &[u8], o: usize) -> Option<u64> {
    Some(u64::from_le_bytes(b.get(o..o + 8)?.try_into().ok()?))
}

pub const REQUISITOS_MAGIC: u32 = u32::from_le_bytes(*b"BREQ");

/// Bytes de la cabecera de la tabla.
pub const CABECERA_LEN: usize = 16;
/// Bytes de cada requisito.
pub const REQUISITO_LEN: usize = 32;
/// Lo que puede medir un motivo. No es una limitacion tecnica: un motivo que no
/// cabe en dos renglones de consola es un motivo que nadie va a leer el dia que
/// salga por pantalla.
pub const MOTIVO_MAX: usize = 160;

// -- Las clases -------------------------------------------------------------
//
// Numeros y no un enum abierto: esto es contrato de fichero. Una clase que se
// retira NO se reutiliza -- el numero se queda quemado, igual que el `1` de
// CHANNEL_KICK.

/// Memoria del proceso que tiene que existir **antes** de la primera
/// instruccion: codigo, datos, pila. En bytes.
pub const CLASE_MEMORIA: u16 = 0x0001;
/// Recursos que el programa quiere RESIDENTES en RAM mientras corre. En bytes.
/// Lo que se lee a demanda por su puerta **no se declara aqui**: eso vive en el
/// disco y no le cuesta RAM a nadie.
pub const CLASE_RECURSOS: u16 = 0x0002;
/// La pantalla, en exclusiva. `cantidad` = 1.
pub const CLASE_PANTALLA: u16 = 0x0003;
/// El aparato de audio. `cantidad` = 1.
pub const CLASE_AUDIO: u16 = 0x0004;
/// Teclado y raton. `cantidad` = 1.
pub const CLASE_ENTRADA: u16 = 0x0005;
/// Extensiones de CPU cuyo estado el sistema tiene que saber preservar en un
/// cambio de contexto. `cantidad` = mascara de bits (la misma de la cabecera).
pub const CLASE_CPU: u16 = 0x0006;
/// Huecos de proceso: un programa que lanza hijos y necesita que quepan.
pub const CLASE_PROCESOS: u16 = 0x0007;
/// **El MONTON de la tarea: lo que el programa reparte en ejecucion.** En bytes.
///
/// *** Y NO ES [`CLASE_MEMORIA`], aunque las dos se midan en bytes. La de
/// arriba es lo que tiene que existir **antes de la primera instruccion**
/// --codigo, datos, pila-- y la decide el CARGADOR mirando el fichero. Esta es
/// lo que la tarea va a pedirle al sistema **despues de arrancar**, y solo la
/// sabe el programa.
///
/// ** Se anade una clase en vez de sumar las dos cantidades porque un sistema
/// que no pueda dar el monton puede querer cargar el programa igual --y dejar
/// que muera con su codigo-- mientras que no poder dar la pila es no poder
/// cargarlo. Son dos decisiones distintas y necesitan dos numeros distintos.
pub const CLASE_MONTON: u16 = 0x0008;

// -- Las unidades -----------------------------------------------------------

/// `cantidad` se cuenta a secas (1 pantalla, 3 procesos).
pub const UNIDAD_UNIDADES: u16 = 0;
/// `cantidad` son bytes.
pub const UNIDAD_BYTES: u16 = 1;
/// `cantidad` es una mascara de bits.
pub const UNIDAD_MASCARA: u16 = 2;

// -- Las banderas -----------------------------------------------------------

/// **Sin esto no funciono.** Si el sistema no puede concederlo --o no sabe lo
/// que es-- el programa no arranca. Ver la regla de arriba.
pub const OBLIGATORIO: u32 = 1 << 0;

/// Un requisito, ya leido. Es una copia: 32 bytes, no compensa prestarlos.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Requisito {
    pub clase: u16,
    pub unidad: u16,
    pub banderas: u32,
    pub cantidad: u64,
    /// Offset del motivo dentro del blob, y su largo. Se guardan crudos para
    /// que leer un requisito no obligue a validar su texto: quien quiera el
    /// motivo lo pide con [`Tabla::motivo`], y quien solo quiera decidir no
    /// paga por una cadena que no va a mirar.
    pub motivo_off: u32,
    pub motivo_len: u16,
}

impl Requisito {
    /// Sin esto, no arranca?
    pub fn es_obligatorio(&self) -> bool {
        self.banderas & OBLIGATORIO != 0
    }
}

/// **La tabla, leida sobre los bytes de la seccion. Cero copias, cero `alloc`.**
///
/// Esta es la mitad que corre en Ring 0, y por eso no reserva nada y no falla a
/// medias: o la cabecera cuadra y hay tabla, o no hay tabla.
pub struct Tabla<'a> {
    bytes: &'a [u8],
    cuantos: usize,
    motivos_off: usize,
    motivos_len: usize,
}

impl<'a> Tabla<'a> {
    /// Abre la tabla sobre los bytes de la seccion `Requisitos`.
    ///
    /// `None` si esto no es una tabla: magic malo, o una cabecera que promete
    /// mas registros de los que hay bytes. **No se confia en `cuantos`**: es un
    /// numero que viene del disco, y el disco es de quien tenga la maquina.
    pub fn abrir(bytes: &'a [u8]) -> Option<Self> {
        if bytes.len() < CABECERA_LEN {
            return None;
        }
        if leer_u32(bytes, 0)? != REQUISITOS_MAGIC {
            return None;
        }
        let cuantos = leer_u32(bytes, 4)? as usize;
        let motivos_off = leer_u32(bytes, 8)? as usize;
        let motivos_len = leer_u32(bytes, 12)? as usize;

        let fin_registros = CABECERA_LEN.checked_add(cuantos.checked_mul(REQUISITO_LEN)?)?;
        if fin_registros > bytes.len() {
            return None;
        }
        // El blob puede estar vacio (`motivos_len == 0`); lo que no puede es
        // salirse. Un offset que se sale convierte cada `motivo()` en una
        // comprobacion mas, y esa comprobacion es justo la que un dia falta.
        let fin_motivos = motivos_off.checked_add(motivos_len)?;
        if motivos_len > 0 && (motivos_off < fin_registros || fin_motivos > bytes.len()) {
            return None;
        }
        Some(Self { bytes, cuantos, motivos_off, motivos_len })
    }

    /// Cuantos requisitos declara el programa.
    pub fn cuantos(&self) -> usize {
        self.cuantos
    }

    /// El requisito `i`. `None` si se pide uno que no existe.
    pub fn requisito(&self, i: usize) -> Option<Requisito> {
        if i >= self.cuantos {
            return None;
        }
        let e = CABECERA_LEN + i * REQUISITO_LEN;
        Some(Requisito {
            clase: leer_u16(self.bytes, e)?,
            unidad: leer_u16(self.bytes, e + 2)?,
            banderas: leer_u32(self.bytes, e + 4)?,
            cantidad: leer_u64(self.bytes, e + 8)?,
            motivo_off: leer_u32(self.bytes, e + 16)?,
            motivo_len: leer_u16(self.bytes, e + 20)?,
        })
    }

    /// **El renglon que escribio quien hizo el programa.**
    ///
    /// `""` si no lo trae o si no cuadra. Un motivo ilegible no invalida el
    /// requisito: la decision se toma con `clase` y `cantidad`, que son numeros.
    /// Lo que se pierde es la explicacion, y perder la explicacion no puede
    /// costar el arranque.
    pub fn motivo(&self, r: &Requisito) -> &'a str {
        let n = r.motivo_len as usize;
        if n == 0 || n > MOTIVO_MAX {
            return "";
        }
        let ini = match self.motivos_off.checked_add(r.motivo_off as usize) {
            Some(v) => v,
            None => return "",
        };
        let fin = match ini.checked_add(n) {
            Some(v) => v,
            None => return "",
        };
        if r.motivo_off as usize + n > self.motivos_len || fin > self.bytes.len() {
            return "";
        }
        core::str::from_utf8(&self.bytes[ini..fin]).unwrap_or("")
    }

    /// Recorre los requisitos en el orden en que los escribio el programa.
    pub fn iter(&self) -> impl Iterator<Item = Requisito> + '_ {
        (0..self.cuantos).filter_map(move |i| self.requisito(i))
    }

    /// **Cuanto pide de una clase.** Suma, porque un programa puede declarar la
    /// misma clase dos veces con motivos distintos -- y esa es una funcion util,
    /// no un error que haya que rechazar: *"2 MB para el mapa"* y *"1 MB para
    /// los sonidos"* dicen mas que *"3 MB"*.
    pub fn total_de(&self, clase: u16) -> u64 {
        self.iter()
            .filter(|r| r.clase == clase)
            .fold(0u64, |a, r| a.saturating_add(r.cantidad))
    }
}

/// Que clases entiende ESTE sistema. Lo que no este aqui es desconocido, y
/// entonces manda [`OBLIGATORIO`].
pub fn clase_conocida(clase: u16) -> bool {
    matches!(
        clase,
        CLASE_MEMORIA
            | CLASE_RECURSOS
            | CLASE_PANTALLA
            | CLASE_AUDIO
            | CLASE_ENTRADA
            | CLASE_CPU
            | CLASE_PROCESOS
            | CLASE_MONTON
    )
}

/// Lo que hay que escribir para declarar un requisito. Es la vista del que


#[cfg(test)]
mod pruebas {
    use super::*;

    /// El caso normal: DOOM declara ~890 KB de imagen y la maquina tiene 15 GiB.
    #[test]
    fn doom_cabe_de_sobra() {
        assert_eq!(cabe(889_280, 15_161 * 1024 * 1024, 64 * 1024 * 1024), Veredicto::Cabe);
    }

    /// Un `.bex` de antes de la regla: entra igual, y eso es deliberado.
    #[test]
    fn el_que_no_declara_entra_como_antes() {
        assert_eq!(cabe(0, 1024, 0), Veredicto::NoDeclara);
        assert!(cabe(0, 0, 0).admite());
    }

    /// **El caso por el que existe la regla 7.** Pedir mas de lo que hay se
    /// contesta ANTES de asignar, y con el numero que falta.
    #[test]
    fn pedir_de_mas_se_contesta_con_cuanto_falta() {
        assert_eq!(cabe(3000, 1000, 0), Veredicto::NoCabe { faltan: 2000 });
        assert!(!cabe(3000, 1000, 0).admite());
    }

    /// ** El que casi nadie escribe, y es el que mata la maquina: cabe JUSTO.
    ///
    /// Sin margen, el programa entra y no queda un marco para la pila del
    /// siguiente hilo. La admision dice que si y la maquina se muere despues,
    /// cuando ya no hay nadie para decir que no.
    #[test]
    fn caber_justo_no_es_caber() {
        assert_eq!(cabe(1000, 1000, 1), Veredicto::SinMargen { faltan: 1 });
        assert!(!cabe(1000, 1000, 1).admite());
        // Y con margen 0 el mismo caso SI entra: el margen lo decide el
        // llamante, no este crate.
        assert_eq!(cabe(1000, 1000, 0), Veredicto::Cabe);
    }

    /// Los dos "no" son frases distintas, y por eso son variantes distintas.
    #[test]
    fn no_cabe_y_sin_margen_no_se_confunden() {
        let grande = cabe(5000, 1000, 100);
        let justo = cabe(950, 1000, 100);
        assert!(matches!(grande, Veredicto::NoCabe { .. }));
        assert!(matches!(justo, Veredicto::SinMargen { .. }));
        assert_ne!(grande, justo);
    }

    /// Sin restas que se den la vuelta. `pide == libre` es el borde exacto.
    #[test]
    fn el_borde_exacto_no_desborda() {
        assert_eq!(cabe(u64::MAX, 1, 0), Veredicto::NoCabe { faltan: u64::MAX - 1 });
        assert_eq!(cabe(1, 1, 0), Veredicto::Cabe);
        assert_eq!(cabe(2, 1, 0), Veredicto::NoCabe { faltan: 1 });
    }
}
