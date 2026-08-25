//! # CARA -- el formato de una maquetacion que VIAJA
//!
//! generacion: abuelo -- no depende de nadie
//!
//! Lo que `PLAN_LA_CARA_VIAJA.md` pide en su escalon 1: *"el FORMATO en un crate
//! sin E/S (como `estratos` y `trim`: **un formato mal empaquetado no da un
//! fallo, da algo peor -- se lee mal y nadie se entera**)"*.
//!
//! ## Que es una cara
//!
//! Una maquetacion **ya resuelta**: rectangulos con sus colores y sus letras, en
//! orden de pintado. Las cinco generaciones de MAQUETA corrieron en el
//! anfitrion y lo caro se quedo alli; lo que viaja es el resultado.
//!
//! ```text
//!    un navegador   manda el documento Y trae el motor que lo maqueta
//!    una CARA       manda solo el resultado. El aparato no maqueta
//! ```
//!
//! Por eso cabe en menos de un kilobyte: la cara entera de la calculadora --28
//! cajas, 17 textos, 1 isla-- son ~950 bytes.
//!
//! ## ** POR QUE EL LECTOR DESCONFIA, Y NO ES PARANOIA
//!
//! El veredicto de MAQUETA --sus diez comprobaciones-- corre en el anfitrion
//! sobre el `.maqueta`. **Y no viaja con la cara.** Un recurso que llega de un
//! fichero editado a mano, del `.bex` de otro o de la red **no ha pasado por
//! ningun juez**, y el que lo lee es el compositor.
//!
//! El precedente esta escrito en `PLAN_DIRECTOR.md`, palabra por palabra:
//!
//! > `Cabecera::leer` valida ancho/alto/stride **contra los bytes que dijo el
//! > kernel**, en `u64` (en 32 bits el producto se desborda y da un total
//! > pequeno). Sin eso, una app que declare 4000x4000 en 1 MiB hace que el
//! > compositor lea fuera del prestamo.
//!
//! *** Y LAS DOS LISTAS SON DISTINTAS, que es lo que hay que tener claro:
//!
//! ```text
//!    el VEREDICTO   juzga si la maquetacion es BUENA (el texto cabe, la caja
//!                   no se sale de su padre). Se puede quedar en el anfitrion
//!    el LECTOR      comprueba si el fichero es SEGURO DE LEER. Siempre, y
//!                   tambien cuando el fichero lo hizo uno mismo
//! ```
//!
//! **Un recurso corrupto no debe dar un `#PF` en el compositor. Una app rota no
//! se lleva el escritorio** -- la misma ley que ya rige las superficies.
//!
//! ## El plano, y por que en este orden
//!
//! ```text
//!    cabecera    20 B     magico, version, lienzo, y las tres cuentas
//!    trazos      20 B c/u lo que se pinta, en orden de pintado
//!    golpes      12 B c/u donde se puede pulsar, y como se llama
//!    cadenas     N B      los textos y los nombres, uno detras de otro
//! ```
//!
//! Las cuentas van **todas en la cabecera** y los bloques son de tamano fijo, asi
//! que **donde empieza cada uno se sabe sin recorrer nada**. Un formato que
//! obligue a recorrer para localizar es un formato que hay que recorrer con
//! datos que todavia no se han comprobado.

#![no_std]
#![forbid(unsafe_code)]

/// `CARA` en little-endian. Cuatro bytes que no son texto por casualidad.
pub const MAGICO: u32 = u32::from_le_bytes(*b"CARA");

/// La unica version que este lector entiende.
///
/// [!] Se compara por IGUALDAD y no por "mayor o igual". Un lector que acepte
/// versiones futuras esta prometiendo entender algo que todavia no existe.
pub const VERSION: u16 = 1;

/// Bytes de la cabecera.
pub const CABECERA: usize = 20;
/// Bytes de un trazo.
pub const TRAZO: usize = 20;
/// Bytes de un golpe.
pub const GOLPE: usize = 12;

/// Offsets dentro de la cabecera. Los declara este crate y **nadie mas**: si el
/// emisor tuviera los suyos, serian dos formatos con el mismo nombre.
pub mod cabecera {
    pub const MAGICO: usize = 0;
    pub const VERSION: usize = 4;
    pub const ANCHO: usize = 6;
    pub const ALTO: usize = 8;
    pub const N_TRAZOS: usize = 10;
    pub const N_GOLPES: usize = 12;
    pub const CADENAS: usize = 14;
    /// Tiene que ser CERO. Ver [`super::Falta::ReservadoSucio`].
    pub const RESERVADO: usize = 16;
}

/// Offsets dentro de un trazo.
pub mod trazo {
    pub const CLASE: usize = 0;
    pub const ESTADO: usize = 1;
    pub const X: usize = 2;
    pub const Y: usize = 4;
    pub const W: usize = 6;
    pub const H: usize = 8;
    pub const COLOR: usize = 10;
    pub const CAD_OFF: usize = 14;
    pub const CAD_LEN: usize = 16;
    /// Tiene que ser CERO.
    pub const RESERVADO: usize = 18;
}

/// Offsets dentro de un golpe.
pub mod golpe {
    pub const X: usize = 0;
    pub const Y: usize = 2;
    pub const W: usize = 4;
    pub const H: usize = 6;
    pub const CAD_OFF: usize = 8;
    pub const CAD_LEN: usize = 10;
}

/// Un rectangulo macizo.
pub const CLASE_RECT: u8 = 0;
/// Letras.
pub const CLASE_TEXTO: u8 = 1;

/// Se pinta siempre.
pub const ESTADO_REPOSO: u8 = 0;
/// Solo mientras el puntero esta encima.
pub const ESTADO_ENCIMA: u8 = 1;

/// **Por que esta cara no se puede leer.**
///
/// Una variante por motivo y no un `bool`, por lo mismo que en `bmo-bex-gate`:
/// *"no se pudo"* manda a mirar el fichero entero, y el nombre manda al campo.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Falta {
    /// No hay ni cabecera.
    NoLlegaNiALaCabecera,
    /// Los cuatro bytes del principio no son `CARA`.
    NoEsUnaCara,
    /// Es una cara de otra version.
    OtraVersion,
    /// Un campo que tiene que ser cero no lo es. **Es la senal mas barata de que
    /// el fichero viene de otro sitio**: nadie escribe basura ahi por accidente,
    /// y un emisor futuro que use ese hueco tendra que subir la version.
    ReservadoSucio,
    /// Las cuentas declaradas piden mas bytes de los que hay. **La comprobacion
    /// de la que penden todas las demas.**
    LasCuentasNoCaben,
    /// Una cadena se sale del bloque de cadenas.
    CadenaFuera,
    /// Un rectangulo se sale del lienzo que la propia cara declara.
    TrazoFueraDelLienzo,
    /// El lienzo declarado no cabe en la pantalla que hay.
    LienzoMasGrandeQueLaPantalla,
    /// El lienzo es de ancho o alto cero: no se puede pintar nada y **todo rect
    /// se saldria**, o sea que el error de verdad seria el de al lado.
    LienzoVacio,
}

/// Una cara ya comprobada. **Solo se construye pasando por [`leer`]**, asi que
/// tener una es la prueba de que las cinco comprobaciones se hicieron.
///
/// * Ese es el punto entero del tipo, y es el mismo truco que `Revisada` en
/// `bmo-bex-gate`: si el lector devolviera `&[u8]` y una lista de avisos, nada
/// impediria pintar sin mirarlos.
#[derive(Clone, Copy)]
pub struct Cara<'a> {
    bytes: &'a [u8],
    ancho: u16,
    alto: u16,
    n_trazos: usize,
    n_golpes: usize,
    cadenas_off: usize,
    cadenas_len: usize,
}

/// **A mano, y no `derive`.** Un `derive` volcaria el buffer entero -- cientos
/// de bytes de ruido en el mensaje de un test que fallo por un campo. Lo que
/// hace falta ver cuando esto sale impreso es **que decia la cabecera**.
impl core::fmt::Debug for Cara<'_> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "Cara {}x{} ({} trazos, {} golpes, {} B de cadenas)",
            self.ancho, self.alto, self.n_trazos, self.n_golpes, self.cadenas_len
        )
    }
}

/// Un trazo ya descodificado.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Pincelada<'a> {
    pub clase: u8,
    pub estado: u8,
    pub x: i16,
    pub y: i16,
    pub w: u16,
    pub h: u16,
    pub color: u32,
    /// Las letras. Vacio si `clase` es [`CLASE_RECT`].
    pub texto: &'a [u8],
}

/// Una region que se puede pulsar.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Pulsable<'a> {
    pub x: i16,
    pub y: i16,
    pub w: u16,
    pub h: u16,
    /// Como se llama. Es lo que el programa recibe cuando alguien pulsa aqui.
    pub nombre: &'a [u8],
}

fn u16_en(b: &[u8], i: usize) -> Option<u16> {
    Some(u16::from_le_bytes([*b.get(i)?, *b.get(i + 1)?]))
}
fn i16_en(b: &[u8], i: usize) -> Option<i16> {
    u16_en(b, i).map(|v| v as i16)
}
fn u32_en(b: &[u8], i: usize) -> Option<u32> {
    Some(u32::from_le_bytes([
        *b.get(i)?,
        *b.get(i + 1)?,
        *b.get(i + 2)?,
        *b.get(i + 3)?,
    ]))
}

/// **ABRIR UNA CARA, DESCONFIANDO.** Las cinco comprobaciones de
/// `PLAN_LA_CARA_VIAJA.md` seccion 6, en el orden en que se sostienen.
///
/// `pantalla_*` es lo que hay de verdad. Se pasa y no se supone: el mismo
/// recurso es legal en una pantalla y absurdo en otra, y **el fichero no puede
/// ser quien diga cual tiene delante**.
///
/// # El orden no es estetico
///
/// ```text
///    1. el magico y la version     sin esto lo demas ni siquiera son campos
///    2. las CUENTAS caben          <- de esta penden todas las siguientes
///    3. cada cadena cae dentro
///    4. ningun rect se sale del lienzo declarado
///    5. el lienzo cabe en la pantalla
/// ```
///
/// *** La 2 va antes que la 3 y la 4 **porque las otras dos leen usando esas
/// cuentas**. Comprobar que un rect cabe leyendo el rect de una tabla cuyo
/// tamano no se ha comprobado es hacer la pregunta con la respuesta ya perdida.
pub fn leer(bytes: &[u8], pantalla_ancho: u16, pantalla_alto: u16) -> Result<Cara<'_>, Falta> {
    // -- 1. Que esto sea una cara, y de esta version ------------------------
    if bytes.len() < CABECERA {
        return Err(Falta::NoLlegaNiALaCabecera);
    }
    if u32_en(bytes, cabecera::MAGICO) != Some(MAGICO) {
        return Err(Falta::NoEsUnaCara);
    }
    if u16_en(bytes, cabecera::VERSION) != Some(VERSION) {
        return Err(Falta::OtraVersion);
    }
    if u32_en(bytes, cabecera::RESERVADO) != Some(0) {
        return Err(Falta::ReservadoSucio);
    }

    let ancho = u16_en(bytes, cabecera::ANCHO).ok_or(Falta::NoLlegaNiALaCabecera)?;
    let alto = u16_en(bytes, cabecera::ALTO).ok_or(Falta::NoLlegaNiALaCabecera)?;
    let n_trazos = u16_en(bytes, cabecera::N_TRAZOS).ok_or(Falta::NoLlegaNiALaCabecera)? as usize;
    let n_golpes = u16_en(bytes, cabecera::N_GOLPES).ok_or(Falta::NoLlegaNiALaCabecera)? as usize;
    let cadenas_len = u16_en(bytes, cabecera::CADENAS).ok_or(Falta::NoLlegaNiALaCabecera)? as usize;

    // -- 2. *** QUE LAS CUENTAS QUEPAN, Y LA CUENTA SE HACE EN u64 ----------
    //
    // Es la comprobacion de la que penden las otras tres, y es la que el
    // precedente del compositor dice como hacer: **en `u64`**. Con `usize` de 32
    // bits, `n_trazos * TRAZO` de una cabecera hostil da la vuelta y contesta un
    // total pequeno -- que pasa la comprobacion y luego lee fuera.
    //
    // Aqui `usize` son 64 en las dos maquinas de hoy, y aun asi se hace en `u64`
    // explicito: **la correccion no puede depender de en que maquina se compila.**
    let falta = CABECERA as u64
        + (n_trazos as u64) * (TRAZO as u64)
        + (n_golpes as u64) * (GOLPE as u64)
        + cadenas_len as u64;
    if falta > bytes.len() as u64 {
        return Err(Falta::LasCuentasNoCaben);
    }

    let trazos_off = CABECERA;
    let golpes_off = trazos_off + n_trazos * TRAZO;
    let cadenas_off = golpes_off + n_golpes * GOLPE;

    // -- 5. El lienzo, antes de mirar ningun rect --------------------------
    //
    // Se adelanta a la 3 y la 4 porque un lienzo de cero hace que **todo** rect
    // se salga, y entonces el error que saldria seria `TrazoFueraDelLienzo` --
    // que manda a mirar los trazos cuando el roto es el lienzo.
    if ancho == 0 || alto == 0 {
        return Err(Falta::LienzoVacio);
    }
    if ancho > pantalla_ancho || alto > pantalla_alto {
        return Err(Falta::LienzoMasGrandeQueLaPantalla);
    }

    let cara = Cara {
        bytes,
        ancho,
        alto,
        n_trazos,
        n_golpes,
        cadenas_off,
        cadenas_len,
    };

    // -- 3 y 4. Cada trazo y cada golpe, uno por uno ------------------------
    //
    // ** SE COMPRUEBAN TODOS AL ABRIR y no al pintar. Un lector que validara
    // sobre la marcha dejaria la mitad del dibujo hecho antes de descubrir que
    // el fichero estaba roto -- y entonces "una app rota no se lleva el
    // escritorio" seria falso a medias: no lo tumba, pero lo ensucia.
    for i in 0..n_trazos {
        let b = trazos_off + i * TRAZO;
        if u16_en(bytes, b + trazo::RESERVADO) != Some(0) {
            return Err(Falta::ReservadoSucio);
        }
        let off = u16_en(bytes, b + trazo::CAD_OFF).ok_or(Falta::LasCuentasNoCaben)? as usize;
        let len = u16_en(bytes, b + trazo::CAD_LEN).ok_or(Falta::LasCuentasNoCaben)? as usize;
        cara.cadena_valida(off, len)?;
        let x = i16_en(bytes, b + trazo::X).ok_or(Falta::LasCuentasNoCaben)?;
        let y = i16_en(bytes, b + trazo::Y).ok_or(Falta::LasCuentasNoCaben)?;
        let w = u16_en(bytes, b + trazo::W).ok_or(Falta::LasCuentasNoCaben)?;
        let h = u16_en(bytes, b + trazo::H).ok_or(Falta::LasCuentasNoCaben)?;
        cara.rect_dentro(x, y, w, h)?;
    }
    for i in 0..n_golpes {
        let b = golpes_off + i * GOLPE;
        let off = u16_en(bytes, b + golpe::CAD_OFF).ok_or(Falta::LasCuentasNoCaben)? as usize;
        let len = u16_en(bytes, b + golpe::CAD_LEN).ok_or(Falta::LasCuentasNoCaben)? as usize;
        cara.cadena_valida(off, len)?;
        let x = i16_en(bytes, b + golpe::X).ok_or(Falta::LasCuentasNoCaben)?;
        let y = i16_en(bytes, b + golpe::Y).ok_or(Falta::LasCuentasNoCaben)?;
        let w = u16_en(bytes, b + golpe::W).ok_or(Falta::LasCuentasNoCaben)?;
        let h = u16_en(bytes, b + golpe::H).ok_or(Falta::LasCuentasNoCaben)?;
        cara.rect_dentro(x, y, w, h)?;
    }

    Ok(cara)
}

impl<'a> Cara<'a> {
    /// El lienzo que esta cara declara.
    pub fn lienzo(&self) -> (u16, u16) {
        (self.ancho, self.alto)
    }
    /// Cuantos trazos trae.
    pub fn trazos(&self) -> usize {
        self.n_trazos
    }
    /// Cuantas regiones pulsables trae.
    pub fn golpes(&self) -> usize {
        self.n_golpes
    }

    /// **Cabe esta cadena en el bloque de cadenas?** En `u64`, por lo mismo que
    /// las cuentas: `off + len` con dos `u16` no desborda hoy, pero la regla no
    /// puede depender de que los campos sigan siendo de 16 bits.
    fn cadena_valida(&self, off: usize, len: usize) -> Result<(), Falta> {
        if off as u64 + len as u64 > self.cadenas_len as u64 {
            return Err(Falta::CadenaFuera);
        }
        Ok(())
    }

    /// **Se sale este rect del lienzo declarado?**
    ///
    /// [!] En `i64` y no en `i32`: `x` es `i16` y `w` es `u16`, asi que la suma
    /// cabe de sobra -- pero el dia que alguien suba los campos a 32 bits, esta
    /// linea sigue siendo correcta en vez de empezar a mentir en silencio.
    fn rect_dentro(&self, x: i16, y: i16, w: u16, h: u16) -> Result<(), Falta> {
        if x < 0 || y < 0 {
            return Err(Falta::TrazoFueraDelLienzo);
        }
        if x as i64 + w as i64 > self.ancho as i64 || y as i64 + h as i64 > self.alto as i64 {
            return Err(Falta::TrazoFueraDelLienzo);
        }
        Ok(())
    }

    fn cadena(&self, off: usize, len: usize) -> &'a [u8] {
        let a = self.cadenas_off + off;
        self.bytes.get(a..a + len).unwrap_or(&[])
    }

    /// El trazo `i`, ya descodificado. `None` fuera de rango.
    ///
    /// * No entra en panico y no devuelve basura: quien pinte en un bucle no
    /// tiene por que volver a comprobar el limite que este tipo ya conoce.
    pub fn trazo(&self, i: usize) -> Option<Pincelada<'a>> {
        if i >= self.n_trazos {
            return None;
        }
        let b = CABECERA + i * TRAZO;
        let clase = *self.bytes.get(b + trazo::CLASE)?;
        let off = u16_en(self.bytes, b + trazo::CAD_OFF)? as usize;
        let len = u16_en(self.bytes, b + trazo::CAD_LEN)? as usize;
        Some(Pincelada {
            clase,
            estado: *self.bytes.get(b + trazo::ESTADO)?,
            x: i16_en(self.bytes, b + trazo::X)?,
            y: i16_en(self.bytes, b + trazo::Y)?,
            w: u16_en(self.bytes, b + trazo::W)?,
            h: u16_en(self.bytes, b + trazo::H)?,
            color: u32_en(self.bytes, b + trazo::COLOR)?,
            texto: if clase == CLASE_TEXTO {
                self.cadena(off, len)
            } else {
                &[]
            },
        })
    }

    /// La region pulsable `i`. `None` fuera de rango.
    pub fn golpe(&self, i: usize) -> Option<Pulsable<'a>> {
        if i >= self.n_golpes {
            return None;
        }
        let b = CABECERA + self.n_trazos * TRAZO + i * GOLPE;
        let off = u16_en(self.bytes, b + golpe::CAD_OFF)? as usize;
        let len = u16_en(self.bytes, b + golpe::CAD_LEN)? as usize;
        Some(Pulsable {
            x: i16_en(self.bytes, b + golpe::X)?,
            y: i16_en(self.bytes, b + golpe::Y)?,
            w: u16_en(self.bytes, b + golpe::W)?,
            h: u16_en(self.bytes, b + golpe::H)?,
            nombre: self.cadena(off, len),
        })
    }
}

#[cfg(test)]
mod tests;
