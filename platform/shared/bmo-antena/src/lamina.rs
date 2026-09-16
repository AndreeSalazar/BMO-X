//! **LA LAMINA** -- una pagina web YA MAQUETADA por la antena: cajas, tiras de
//! texto e imagenes con sus coordenadas. BMO-X la pinta; nunca ve HTML.
//!
//! [carril]  AMARILLO  lee coordenadas que escribio otra maquina de la casa
//! [cuesta]  DATO      una caja fuera de la lamina es un pixel fuera del bloque:
//!                     el #PF que ya le costo dos fallos a `raycaster_C.c`
//! [riesgo]  AJENO     cada linea la escribe la antena; aqui se comprueba TODO
//!                     antes de que el DIRECTOR toque un pixel
//!
//! # L1 de `docs/plan/PLAN_CLOUD_LOCAL.md`, seccion 11 (2026-09-16)
//!
//! Eddi: *"mi celular mastica TODO por mi BMO-X y solo entregan codigos
//! procesados; BMO-X procesa todo en interior"*. Navegar tiene tres formas:
//! TEXTO (se pierde la forma), ESPEJO (pixeles, caro y ciego) y ESTA: la antena
//! corre el navegador entero -- JavaScript, CSS, fuentes, layout -- y manda **la
//! salida de un compilador**, que es lo que MAQUETA ya emite para el escritorio
//! (`docs/plan/PLAN_MAQUETA.md`: "las coordenadas ya calculadas"). Opera Mini lo
//! hacia en 2005 con sus servidores; aqui el servidor es el movil de uno.
//!
//! ```text
//!    LAMINA <ancho> <alto> <n>                   cabecera: el tamano de la pagina
//!                                                ENTERA y cuantas lineas siguen
//!    CAJA   <x> <y> <ancho> <alto> <rrggbb>      un rectangulo relleno
//!    TEXTO  <x> <y> <escala> <rrggbb> <bytes>    una tira YA partida en lineas
//!    IMAGEN <x> <y> <ancho> <alto> <id>          se pide aparte: IMAGEN <id> (QOI)
//!    ENLACE <x> <y> <ancho> <alto> <id>          zona clicable: CLIC <id>
//!    CAMPO  <x> <y> <ancho> <alto> <id>          zona de teclado: TECLA <id> <bytes>
//! ```
//!
//! # Lo que la lamina NO puede decir, y por que
//!
//! ```text
//!    tamano de letra en puntos   BMO-X tiene UNA fuente: 8x16, un bit por pixel
//!                                (`toolchain/tools/fontgen`). `escala` es 1..4
//!                                de ese 8x16; la antena redondea
//!    Unicode                     la fuente tiene 95 glifos ASCII y 25 Latin-1
//!                                (los del espanol). El texto va en Latin-1 y lo
//!                                que no cabe la antena lo vuelve `?`
//!    una caja fuera de la pagina se rechaza ENTERA (`Fuera`): recortar en
//!                                silencio seria taparle a la antena su fallo
//!    mas de 4096 lineas          `Charla`: una pagina que no cabe no se pinta a
//!                                medias, se niega con nombre
//! ```
//!
//! ** La lamina llega ENTERA, no solo lo visible: el scroll se hace en local.
//! Por la LAN medida (~10 Mbit, 16 ms) una pagina son 0,1..0,5 s; un scroll por
//! red seria Opera Mini en lo peor que tenia.
//!
//! [!] Esto es el FORMATO, puro y con banco. Cablearlo en `Conversacion`
//! (`PAGINA`, `CLIC`, `TECLA`) y pintarlo en el DIRECTOR es L1, y espera a TCP.

use crate::{id_valido, Rechazo, ANCHO_MAX, LINEA_MAX};

/// Una pagina puede ser muy alta: llega entera y se hace scroll en local.
pub const ALTO_LAMINA_MAX: u32 = 32_768;
/// Lineas que caben en una lamina. Pasarse es `Charla`.
pub const ELEMENTOS_MAX: u32 = 4_096;
/// La fuente de BMO-X, y sus escalas.
pub const LETRA_ANCHO: u32 = 8;
pub const LETRA_ALTO: u32 = 16;
pub const ESCALA_MAX: u32 = 4;
/// Un `id` de imagen, enlace o campo: como los de la LISTA.
pub const ID_MAX: usize = crate::ID_MAX;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Cabecera {
    pub ancho: u32,
    pub alto: u32,
    pub elementos: u32,
}

/// **Un rectangulo de la lamina.** Siempre dentro de la cabecera: el lector
/// no devuelve uno que no lo este.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Zona {
    pub x: u32,
    pub y: u32,
    pub ancho: u32,
    pub alto: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Elemento<'a> {
    Caja { zona: Zona, color: u32 },
    /// `zona` ya es la de la tira: `largo * 8 * escala` por `16 * escala`.
    Texto { zona: Zona, escala: u32, color: u32, texto: &'a [u8] },
    Imagen { zona: Zona, id: &'a [u8] },
    Enlace { zona: Zona, id: &'a [u8] },
    Campo { zona: Zona, id: &'a [u8] },
}

impl Elemento<'_> {
    pub fn zona(&self) -> Zona {
        match *self {
            Elemento::Caja { zona, .. }
            | Elemento::Texto { zona, .. }
            | Elemento::Imagen { zona, .. }
            | Elemento::Enlace { zona, .. }
            | Elemento::Campo { zona, .. } => zona,
        }
    }
}

/// Latin-1 imprimible: lo que la fuente sabe, o un byte que el DIRECTOR pinta
/// como hueco. Nunca un control.
fn letra_valida(c: u8) -> bool {
    (0x20..=0x7E).contains(&c) || c >= 0xA0
}

fn numero(b: &[u8], maximo: u32) -> Result<u32, Rechazo> {
    if b.is_empty() || b.len() > 10 || !b.iter().all(u8::is_ascii_digit) {
        return Err(Rechazo::Numero);
    }
    let mut n = 0u32;
    for &c in b {
        n = n.checked_mul(10).and_then(|n| n.checked_add((c - b'0') as u32)).ok_or(Rechazo::Numero)?;
    }
    if n > maximo {
        return Err(Rechazo::Numero);
    }
    Ok(n)
}

/// `rrggbb`, seis digitos hexadecimales, minusculas o mayusculas.
fn color(b: &[u8]) -> Result<u32, Rechazo> {
    if b.len() != 6 {
        return Err(Rechazo::Color);
    }
    let mut n = 0u32;
    for &c in b {
        let d = match c {
            b'0'..=b'9' => c - b'0',
            b'a'..=b'f' => c - b'a' + 10,
            b'A'..=b'F' => c - b'A' + 10,
            _ => return Err(Rechazo::Color),
        };
        n = (n << 4) | d as u32;
    }
    Ok(n)
}

fn partir(b: &[u8]) -> (&[u8], &[u8]) {
    match b.iter().position(|&c| c == b' ') {
        Some(i) => (&b[..i], &b[i + 1..]),
        None => (b, &[]),
    }
}

fn recortar(linea: &[u8]) -> Result<&[u8], Rechazo> {
    let linea = linea.strip_suffix(b"\n").unwrap_or(linea);
    let linea = linea.strip_suffix(b"\r").unwrap_or(linea);
    if linea.len() > LINEA_MAX {
        return Err(Rechazo::Largo);
    }
    Ok(linea)
}

/// Cuatro numeros `x y ancho alto` y lo que sobra. Una zona sin area es `Tamano`.
fn zona(resto: &[u8]) -> Result<(Zona, &[u8]), Rechazo> {
    let (x, resto) = partir(resto);
    let (y, resto) = partir(resto);
    let (ancho, resto) = partir(resto);
    let (alto, resto) = partir(resto);
    let z = Zona {
        x: numero(x, ANCHO_MAX)?,
        y: numero(y, ALTO_LAMINA_MAX)?,
        ancho: numero(ancho, ANCHO_MAX)?,
        alto: numero(alto, ALTO_LAMINA_MAX)?,
    };
    if z.ancho == 0 || z.alto == 0 {
        return Err(Rechazo::Tamano);
    }
    Ok((z, resto))
}

/// Dentro de la lamina, o `Fuera`. Sin desbordar: las sumas caben en u32 por
/// los topes de `numero`.
fn dentro(z: Zona, cab: &Cabecera) -> Result<Zona, Rechazo> {
    if z.x + z.ancho > cab.ancho || z.y + z.alto > cab.alto {
        return Err(Rechazo::Fuera);
    }
    Ok(z)
}

/// **La primera linea.** `LAMINA <ancho> <alto> <n>`.
pub fn leer_cabecera(linea: &[u8]) -> Result<Cabecera, Rechazo> {
    let linea = recortar(linea)?;
    if !linea.iter().all(|&c| (0x20..=0x7E).contains(&c)) {
        return Err(Rechazo::NoAscii);
    }
    let (verbo, resto) = partir(linea);
    if verbo != b"LAMINA" {
        return Err(Rechazo::Verbo);
    }
    let (ancho, resto) = partir(resto);
    let (alto, resto) = partir(resto);
    let (n, sobra) = partir(resto);
    if n.is_empty() || !sobra.is_empty() {
        return Err(Rechazo::Campos);
    }
    let cab = Cabecera {
        ancho: numero(ancho, ANCHO_MAX)?,
        alto: numero(alto, ALTO_LAMINA_MAX)?,
        elementos: numero(n, ELEMENTOS_MAX)?,
    };
    if cab.ancho == 0 || cab.alto == 0 {
        return Err(Rechazo::Tamano);
    }
    Ok(cab)
}

/// **Una linea de la lamina**, juzgada contra su cabecera. Lo que devuelve
/// esta DENTRO; lo demas tiene nombre.
pub fn leer_elemento<'a>(linea: &'a [u8], cab: &Cabecera) -> Result<Elemento<'a>, Rechazo> {
    let linea = recortar(linea)?;
    let (verbo, resto) = partir(linea);
    // Solo el TEXTO lleva Latin-1; el resto de la linea, y las otras lineas
    // enteras, son ASCII imprimible.
    let ascii = |b: &[u8]| b.iter().all(|&c| (0x20..=0x7E).contains(&c));
    match verbo {
        b"TEXTO" => {
            let (x, resto) = partir(resto);
            let (y, resto) = partir(resto);
            let (escala, resto) = partir(resto);
            let (rgb, texto) = partir(resto);
            if !ascii(x) || !ascii(y) || !ascii(escala) || !ascii(rgb) {
                return Err(Rechazo::NoAscii);
            }
            if texto.is_empty() {
                return Err(Rechazo::Campos);
            }
            if !texto.iter().all(|&c| letra_valida(c)) {
                return Err(Rechazo::NoAscii);
            }
            let escala = numero(escala, ESCALA_MAX)?;
            if escala == 0 {
                return Err(Rechazo::Tamano);
            }
            let z = Zona {
                x: numero(x, ANCHO_MAX)?,
                y: numero(y, ALTO_LAMINA_MAX)?,
                ancho: texto.len() as u32 * LETRA_ANCHO * escala,
                alto: LETRA_ALTO * escala,
            };
            Ok(Elemento::Texto { zona: dentro(z, cab)?, escala, color: color(rgb)?, texto })
        }
        _ => {
            if !ascii(linea) {
                return Err(Rechazo::NoAscii);
            }
            match verbo {
                b"CAJA" => {
                    let (z, rgb) = zona(resto)?;
                    let (rgb, sobra) = partir(rgb);
                    if !sobra.is_empty() {
                        return Err(Rechazo::Campos);
                    }
                    Ok(Elemento::Caja { zona: dentro(z, cab)?, color: color(rgb)? })
                }
                b"IMAGEN" | b"ENLACE" | b"CAMPO" => {
                    let (z, id) = zona(resto)?;
                    let (id, sobra) = partir(id);
                    if !sobra.is_empty() {
                        return Err(Rechazo::Campos);
                    }
                    if !id_valido(id) {
                        return Err(Rechazo::Id);
                    }
                    let zona = dentro(z, cab)?;
                    Ok(match verbo {
                        b"IMAGEN" => Elemento::Imagen { zona, id },
                        b"ENLACE" => Elemento::Enlace { zona, id },
                        _ => Elemento::Campo { zona, id },
                    })
                }
                _ => Err(Rechazo::Verbo),
            }
        }
    }
}

/// Cuantos de cada, para decirlo en CABINA y para el cupo.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Cuenta {
    pub cajas: u32,
    pub textos: u32,
    pub imagenes: u32,
    pub enlaces: u32,
    pub campos: u32,
    pub bytes: u64,
}

impl Cuenta {
    pub fn total(&self) -> u32 {
        self.cajas + self.textos + self.imagenes + self.enlaces + self.campos
    }
}

/// Lo que dice el lector tras cada linea.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Paso<'a> {
    Cabecera(Cabecera),
    Elemento(Elemento<'a>),
    /// La ultima linea ya entro: la lamina esta completa.
    Fin(Cuenta),
}

/// **Lee una lamina linea a linea**: primero la cabecera, luego exactamente `n`
/// elementos. Una linea de mas, de menos o fuera de sitio cierra con nombre, y
/// despues todo es `Cerrada` -- como `Conversacion`.
pub struct Lector {
    cab: Option<Cabecera>,
    faltan: u32,
    cuenta: Cuenta,
    cerrado: bool,
}

impl Default for Lector {
    fn default() -> Self {
        Self::nuevo()
    }
}

impl Lector {
    pub const fn nuevo() -> Self {
        Self { cab: None, faltan: 0, cuenta: Cuenta { cajas: 0, textos: 0, imagenes: 0, enlaces: 0, campos: 0, bytes: 0 }, cerrado: false }
    }

    pub fn cabecera(&self) -> Option<Cabecera> {
        self.cab
    }

    pub fn cuenta(&self) -> Cuenta {
        self.cuenta
    }

    fn cerrar<T>(&mut self, r: Rechazo) -> Result<T, Rechazo> {
        self.cerrado = true;
        Err(r)
    }

    pub fn empujar<'a>(&mut self, linea: &'a [u8]) -> Result<Paso<'a>, Rechazo> {
        if self.cerrado {
            return Err(Rechazo::Cerrada);
        }
        self.cuenta.bytes += linea.len() as u64;
        let Some(cab) = self.cab else {
            let cab = match leer_cabecera(linea) {
                Ok(c) => c,
                Err(e) => return self.cerrar(e),
            };
            self.cab = Some(cab);
            self.faltan = cab.elementos;
            return if cab.elementos == 0 { Ok(Paso::Fin(self.cuenta)) } else { Ok(Paso::Cabecera(cab)) };
        };
        if self.faltan == 0 {
            // Anuncio n y manda n+1: una linea en el momento equivocado.
            return self.cerrar(Rechazo::Orden);
        }
        let e = match leer_elemento(linea, &cab) {
            Ok(e) => e,
            Err(e) => return self.cerrar(e),
        };
        match e {
            Elemento::Caja { .. } => self.cuenta.cajas += 1,
            Elemento::Texto { .. } => self.cuenta.textos += 1,
            Elemento::Imagen { .. } => self.cuenta.imagenes += 1,
            Elemento::Enlace { .. } => self.cuenta.enlaces += 1,
            Elemento::Campo { .. } => self.cuenta.campos += 1,
        }
        self.faltan -= 1;
        // El ultimo se entrega como los demas: el que llama sabe que lo era
        // por `completa()`. Una linea mas es `Orden`.
        Ok(Paso::Elemento(e))
    }

    /// Verdad cuando entraron exactamente las `n` lineas anunciadas.
    pub fn completa(&self) -> bool {
        matches!(self.cab, Some(c) if self.cuenta.total() == c.elementos)
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    const CAB: Cabecera = Cabecera { ancho: 640, alto: 2000, elementos: 5 };

    #[test]
    fn la_cabecera_y_sus_topes() {
        assert_eq!(leer_cabecera(b"LAMINA 640 2000 5\n"), Ok(CAB));
        assert_eq!(leer_cabecera(b"LAMINA 640 2000 5\r\n"), Ok(CAB), "CRLF vale");
        assert_eq!(leer_cabecera(b"PAGINA 640 2000 5"), Err(Rechazo::Verbo));
        assert_eq!(leer_cabecera(b"LAMINA 640 2000"), Err(Rechazo::Campos));
        assert_eq!(leer_cabecera(b"LAMINA 640 2000 5 x"), Err(Rechazo::Campos));
        assert_eq!(leer_cabecera(b"LAMINA 1281 2000 5"), Err(Rechazo::Numero), "mas ancho que la pantalla");
        assert_eq!(leer_cabecera(b"LAMINA 640 40000 5"), Err(Rechazo::Numero), "mas alta que el tope");
        assert_eq!(leer_cabecera(b"LAMINA 640 2000 4097"), Err(Rechazo::Numero), "mas lineas que el cupo");
        assert_eq!(leer_cabecera(b"LAMINA 0 2000 5"), Err(Rechazo::Tamano));
        assert_eq!(leer_cabecera(b"LAMINA 640 2000 -1"), Err(Rechazo::Numero));
    }

    #[test]
    fn cada_elemento_se_lee_y_queda_dentro() {
        let z = Zona { x: 10, y: 20, ancho: 100, alto: 30 };
        assert_eq!(leer_elemento(b"CAJA 10 20 100 30 1e1e2e", &CAB), Ok(Elemento::Caja { zona: z, color: 0x1E1E2E }));
        assert_eq!(leer_elemento(b"CAJA 10 20 100 30 FFFFFF", &CAB), Ok(Elemento::Caja { zona: z, color: 0xFFFFFF }), "mayusculas valen");
        assert_eq!(leer_elemento(b"IMAGEN 10 20 100 30 foto-1", &CAB), Ok(Elemento::Imagen { zona: z, id: b"foto-1" }));
        assert_eq!(leer_elemento(b"ENLACE 10 20 100 30 e7", &CAB), Ok(Elemento::Enlace { zona: z, id: b"e7" }));
        assert_eq!(leer_elemento(b"CAMPO 10 20 100 30 busca", &CAB), Ok(Elemento::Campo { zona: z, id: b"busca" }));
        // El texto mide por la fuente: 4 letras a escala 2 son 64x32.
        assert_eq!(
            leer_elemento(b"TEXTO 10 20 2 e6edf7 hola", &CAB),
            Ok(Elemento::Texto { zona: Zona { x: 10, y: 20, ancho: 64, alto: 32 }, escala: 2, color: 0xE6EDF7, texto: b"hola" })
        );
        // El texto puede llevar espacios: todo lo que sigue al color es texto.
        assert!(matches!(leer_elemento(b"TEXTO 0 0 1 000000 dos palabras", &CAB), Ok(Elemento::Texto { texto: b"dos palabras", .. })));
    }

    #[test]
    fn fuera_de_la_lamina_se_niega_entera() {
        assert_eq!(leer_elemento(b"CAJA 600 0 100 30 000000", &CAB), Err(Rechazo::Fuera), "se sale por la derecha");
        assert_eq!(leer_elemento(b"CAJA 0 1990 10 30 000000", &CAB), Err(Rechazo::Fuera), "se sale por abajo");
        assert_eq!(leer_elemento(b"CAJA 540 0 100 30 000000", &CAB).map(|e| e.zona().x), Ok(540), "justo al borde cabe");
        assert_eq!(leer_elemento(b"TEXTO 600 0 1 000000 123456", &CAB), Err(Rechazo::Fuera), "6 letras son 48 px: 648 > 640");
        assert_eq!(leer_elemento(b"TEXTO 0 1990 1 000000 a", &CAB), Err(Rechazo::Fuera), "16 px de alto: 2006 > 2000");
        assert_eq!(leer_elemento(b"CAJA 0 0 0 30 000000", &CAB), Err(Rechazo::Tamano), "sin area no es una caja");
    }

    #[test]
    fn lo_que_no_es_del_formato_tiene_nombre() {
        assert_eq!(leer_elemento(b"CIRCULO 0 0 10 10 000000", &CAB), Err(Rechazo::Verbo));
        assert_eq!(leer_elemento(b"CAJA 0 0 10 10", &CAB), Err(Rechazo::Color), "sin color");
        assert_eq!(leer_elemento(b"CAJA 0 0 10 10 12345", &CAB), Err(Rechazo::Color), "cinco digitos");
        assert_eq!(leer_elemento(b"CAJA 0 0 10 10 12345g", &CAB), Err(Rechazo::Color));
        assert_eq!(leer_elemento(b"CAJA 0 0 10 10 000000 sobra", &CAB), Err(Rechazo::Campos));
        assert_eq!(leer_elemento(b"CAJA a 0 10 10 000000", &CAB), Err(Rechazo::Numero));
        assert_eq!(leer_elemento(b"ENLACE 0 0 10 10 Mayus", &CAB), Err(Rechazo::Id));
        assert_eq!(leer_elemento(b"ENLACE 0 0 10 10", &CAB), Err(Rechazo::Id), "sin id");
        assert_eq!(leer_elemento(b"TEXTO 0 0 5 000000 a", &CAB), Err(Rechazo::Numero), "escala 5 no existe");
        assert_eq!(leer_elemento(b"TEXTO 0 0 0 000000 a", &CAB), Err(Rechazo::Tamano), "escala 0 tampoco");
        assert_eq!(leer_elemento(b"TEXTO 0 0 1 000000", &CAB), Err(Rechazo::Campos), "sin texto");
        assert_eq!(leer_elemento(b"TEXTO 0 0 1 000000 con\ttab", &CAB), Err(Rechazo::NoAscii), "un control dentro del texto");
        let larga = [b'a'; LINEA_MAX + 1];
        assert_eq!(leer_elemento(&larga, &CAB), Err(Rechazo::Largo));
    }

    #[test]
    fn el_texto_habla_latin1_y_el_resto_no() {
        // n con tilde en Latin-1 es 0xF1: la fuente la tiene.
        let linea = b"TEXTO 0 0 1 000000 ma\xF1ana";
        assert!(matches!(leer_elemento(linea, &CAB), Ok(Elemento::Texto { texto: b"ma\xF1ana", .. })));
        // Pero un id no: solo [a-z0-9_-].
        assert_eq!(leer_elemento(b"ENLACE 0 0 10 10 ma\xF1ana", &CAB), Err(Rechazo::NoAscii));
        // Y un control (0x9F esta entre 0x7F y 0xA0) tampoco en el texto.
        assert_eq!(leer_elemento(b"TEXTO 0 0 1 000000 a\x9Fb", &CAB), Err(Rechazo::NoAscii));
    }

    #[test]
    fn el_lector_exige_exactamente_las_anunciadas() {
        let mut l = Lector::nuevo();
        assert_eq!(l.empujar(b"LAMINA 640 100 2\n"), Ok(Paso::Cabecera(Cabecera { ancho: 640, alto: 100, elementos: 2 })));
        assert!(matches!(l.empujar(b"CAJA 0 0 640 100 ffffff\n"), Ok(Paso::Elemento(Elemento::Caja { .. }))));
        assert!(!l.completa());
        assert!(matches!(l.empujar(b"TEXTO 8 8 1 000000 hola\n"), Ok(Paso::Elemento(Elemento::Texto { .. }))));
        assert!(l.completa());
        assert_eq!(l.cuenta(), Cuenta { cajas: 1, textos: 1, imagenes: 0, enlaces: 0, campos: 0, bytes: 65 });
        // La tercera sobra: anuncio dos. Y despues, nada.
        assert_eq!(l.empujar(b"CAJA 0 0 1 1 000000\n"), Err(Rechazo::Orden));
        assert_eq!(l.empujar(b"CAJA 0 0 1 1 000000\n"), Err(Rechazo::Cerrada));
    }

    #[test]
    fn una_linea_mala_cierra_la_lamina_entera() {
        let mut l = Lector::nuevo();
        assert!(l.empujar(b"LAMINA 640 100 3").is_ok());
        assert!(l.empujar(b"CAJA 0 0 10 10 000000").is_ok());
        assert_eq!(l.empujar(b"CAJA 700 0 10 10 000000"), Err(Rechazo::Fuera));
        assert_eq!(l.empujar(b"CAJA 0 0 10 10 000000"), Err(Rechazo::Cerrada), "no hay segunda oportunidad");
        assert!(!l.completa());
    }

    #[test]
    fn una_lamina_vacia_es_una_lamina() {
        let mut l = Lector::nuevo();
        assert_eq!(l.empujar(b"LAMINA 640 100 0"), Ok(Paso::Fin(Cuenta { bytes: 16, ..Cuenta::default() })));
        assert!(l.completa());
    }

    #[test]
    fn veinte_mil_lineas_mutadas_no_revientan_y_lo_que_pasa_esta_dentro() {
        let base: &[&[u8]] = &[
            b"LAMINA 1280 32768 4096",
            b"CAJA 1270 32760 10 8 ffffff",
            b"TEXTO 1200 32700 4 000000 abcdefghij",
            b"IMAGEN 0 0 1280 32768 foto",
            b"ENLACE 640 100 640 100 e-1",
            b"CAMPO 0 0 8 8 c",
        ];
        let cab = Cabecera { ancho: 1280, alto: 32768, elementos: 4096 };
        let mut semilla = 0x1A1Au64;
        let mut azar = || {
            semilla = semilla.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            (semilla >> 33) as usize
        };
        for _ in 0..20_000 {
            let mut m = base[azar() % base.len()].to_vec();
            for _ in 0..1 + azar() % 4 {
                let i = azar() % m.len();
                m[i] = azar() as u8;
            }
            m.truncate(azar() % (m.len() + 1));
            let _ = leer_cabecera(&m);
            if let Ok(e) = leer_elemento(&m, &cab) {
                let z = e.zona();
                assert!(z.x + z.ancho <= cab.ancho && z.y + z.alto <= cab.alto, "salio algo fuera: {:?}", e);
            }
        }
    }

    /// La lamina de una pagina REAL (example.com a 640 px), escrita por
    /// `toolchain/tools/antena/lamina.js` en un navegador de escritorio el
    /// 2026-09-16, con `metricaBMO()` puesta: las tres lineas del parrafo estan
    /// a 16 px exactos una de otra, que es el alto de la fuente de BMO-X.
    #[test]
    fn la_lamina_de_una_pagina_real_entra_entera() {
        let fichero = include_bytes!("../../../../toolchain/tools/antena/ejemplo.lamina");
        let mut l = Lector::nuevo();
        let mut alturas = [0u32; 8];
        let mut n = 0;
        for linea in fichero.split(|&b| b == b'\n').filter(|l| !l.is_empty()) {
            match l.empujar(linea) {
                Ok(Paso::Elemento(Elemento::Texto { zona, escala: 1, .. })) => {
                    alturas[n] = zona.y;
                    n += 1;
                }
                Ok(_) => {}
                Err(e) => panic!("linea {:?}: {}", core::str::from_utf8(linea).unwrap_or("?"), e.texto()),
            }
        }
        assert!(l.completa());
        // Los bytes contados son los de las lineas sin su `\n` (con `\r` si git
        // lo puso al sacar el fichero en Windows: el lector lo recorta igual).
        let saltos = fichero.iter().filter(|&&b| b == b'\n').count() as u64;
        assert_eq!(l.cuenta(), Cuenta { cajas: 1, textos: 5, imagenes: 0, enlaces: 1, campos: 0, bytes: fichero.len() as u64 - saltos });
        assert_eq!(&alturas[..3], &[169, 185, 201], "el parrafo cae de 16 en 16, como la fuente");
    }

    #[test]
    fn antes_de_la_cabecera_no_hay_elementos() {
        let mut l = Lector::nuevo();
        assert_eq!(l.empujar(b"CAJA 0 0 10 10 000000"), Err(Rechazo::Verbo));
        assert_eq!(l.empujar(b"LAMINA 640 100 0"), Err(Rechazo::Cerrada));
    }
}
