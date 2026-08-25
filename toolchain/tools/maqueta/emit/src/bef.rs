//! **COMO se escribe una cara para que VIAJE.** El emisor B.
//!
//! El escalon 8 de `PLAN_MAQUETA.md` y el 2 de `PLAN_LA_CARA_VIAJA.md`. Son el
//! mismo escalon escrito en dos documentos, y se marcan juntos o uno de los dos
//! miente.
//!
//! ```text
//!    rust.rs   ->  codigo, para compilar DENTRO del servicio
//!    bef.rs    ->  bytes, para cambiar la cara SIN recompilar   <- este
//! ```
//!
//! ## Las tres cosas que este modulo NO hace, y son lo que lo mantiene corto
//!
//! ```text
//!    NO decide que se dibuja     eso es `orden::lista` y `orden::golpes`
//!    NO es dueno del formato     eso es la crate `bmo-maqueta-cara`
//!    NO escribe el .bex          eso es `bmo_abi::bef::recursos` + bmo-pack
//! ```
//!
//! *** LA TERCERA ES LA QUE MAS FACIL SE ROMPE, y `bmo-pack` ya lo dejo escrito
//! cuando le paso: el formato de la seccion `Resources 0x0B` tiene **un solo
//! dueno**, y *"si alguien tuviera que mirar dos sitios para saber donde empieza
//! un recurso, seria porque alguien escribio el formato dos veces"*.
//!
//! Asi que esto produce **el contenido del recurso** y se para ahi. Meterlo en
//! un `.bex` es una orden de `bmo-pack`:
//!
//! ```text
//!    bmo-pack app.bex -r cara=calc.cara -o app.bex
//! ```
//!
//! ## Y por que el `de` de cada trazo NO viaja
//!
//! `Orden::de` --*"de que caja salio este trazo"*-- va al comentario del codigo
//! que emite `rust.rs`, y es por donde se sigue un fotograma raro hasta la caja
//! que lo causo. **Aqui se tira a proposito.**
//!
//! Son ~5 bytes por trazo sobre una cara de ~950: cerca del 20% del fichero para
//! algo que en ejecucion no lee nadie. Y la consecuencia hay que decirla en vez
//! de descubrirla: **un recurso no sirve para diagnosticar como sirve el codigo
//! generado.** Cuando una cara salga rara, se mira el emisor A, no este.
//!
//! Lo que si viaja son los nombres de los **golpes**, porque esos no son
//! diagnostico: son lo que el programa recibe cuando alguien pulsa.

use bmo_maqueta_cara as cara;

use crate::orden::{Estado, Golpe, Orden, Trazo};

/// **Por que esta cara no se pudo escribir.**
///
/// [!] Las tres son de DESBORDE, y ninguna es un fallo del `.maqueta`: son el
/// formato diciendo hasta donde llega. Un emisor que recortara en silencio
/// produciria una cara que se abre, se pinta, y esta mal -- que es peor que no
/// producir nada.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum NoCabe {
    /// El lienzo no cabe en `u16`.
    Lienzo { ancho: i64, alto: i64 },
    /// Un rectangulo tiene una coordenada o un tamano que no cabe en el campo.
    Rect { de: String, x: i64, y: i64, w: i64, h: i64 },
    /// Hay mas trazos, mas golpes o mas bytes de cadenas de los que la cabecera
    /// puede contar.
    Demasiado { que: &'static str, cuantos: usize },
}

/// El bloque de cadenas, con las repetidas puestas UNA vez.
///
/// * Los nombres se repiten mucho --diecisiete botones que dicen `#boton0`..
/// `#boton9`, los textos de una fila-- y guardarlos una vez sale gratis: la
/// busqueda es lineal sobre unas decenas de entradas, en el anfitrion, una vez
/// por compilacion.
#[derive(Default)]
struct Cadenas {
    bytes: Vec<u8>,
}

impl Cadenas {
    /// Devuelve `(offset, largo)`. Si la cadena ya estaba, no la vuelve a meter.
    fn mete(&mut self, s: &[u8]) -> (u16, u16) {
        if s.is_empty() {
            return (0, 0);
        }
        if let Some(p) = self.bytes.windows(s.len()).position(|w| w == s) {
            return (p as u16, s.len() as u16);
        }
        let off = self.bytes.len();
        self.bytes.extend_from_slice(s);
        (off as u16, s.len() as u16)
    }
}

fn u16_de(v: i64) -> Option<u16> {
    if (0..=u16::MAX as i64).contains(&v) {
        Some(v as u16)
    } else {
        None
    }
}
fn i16_de(v: i64) -> Option<i16> {
    if (i16::MIN as i64..=i16::MAX as i64).contains(&v) {
        Some(v as i16)
    } else {
        None
    }
}

/// **Escribir la cara.** Entra lo que `orden` decidio, salen los bytes del
/// recurso.
///
/// `ancho` y `alto` son el lienzo, y se pasan en vez de deducirse de los trazos:
/// una cara cuyo lienzo fuera *"lo que ocupan sus cajas"* cambiaria de tamano al
/// quitar la ultima, y el lector no tendria contra que comprobar nada.
///
/// # El orden de escritura es el del plano, y no es casualidad
///
/// ```text
///    1. se recogen las CADENAS      porque los offsets hacen falta despues
///    2. se arman trazos y golpes    que ya pueden apuntar a ellas
///    3. se escribe la CABECERA      la ultima, porque cuenta lo de arriba
/// ```
///
/// Escribir la cabecera primero obligaria a volver a pisarla con las cuentas de
/// verdad, y **un campo que se escribe dos veces es un campo que un dia se
/// escribe una**.
pub fn escribir(ordenes: &[Orden], golpes: &[Golpe], ancho: i64, alto: i64) -> Result<Vec<u8>, NoCabe> {
    let (Some(ancho_u), Some(alto_u)) = (u16_de(ancho), u16_de(alto)) else {
        return Err(NoCabe::Lienzo { ancho, alto });
    };

    let mut cad = Cadenas::default();
    let mut trazos: Vec<[u8; cara::TRAZO]> = Vec::with_capacity(ordenes.len());

    for o in ordenes {
        let r = o.trazo.area();
        let (x, y, w, h) = (r.x as i64, r.y as i64, r.w as i64, r.h as i64);
        let (Some(xi), Some(yi), Some(wu), Some(hu)) =
            (i16_de(x), i16_de(y), u16_de(w), u16_de(h))
        else {
            return Err(NoCabe::Rect { de: o.de.clone(), x, y, w, h });
        };

        let (clase, color, cadena) = match &o.trazo {
            Trazo::Rect { color, .. } => (cara::CLASE_RECT, *color, &[][..]),
            Trazo::Texto { texto, color, .. } => (cara::CLASE_TEXTO, *color, texto.as_bytes()),
        };
        let (off, len) = cad.mete(cadena);

        let mut t = [0u8; cara::TRAZO];
        t[cara::trazo::CLASE] = clase;
        t[cara::trazo::ESTADO] = match o.estado {
            Estado::Reposo => cara::ESTADO_REPOSO,
            Estado::Encima => cara::ESTADO_ENCIMA,
        };
        pon_i16(&mut t, cara::trazo::X, xi);
        pon_i16(&mut t, cara::trazo::Y, yi);
        pon_u16(&mut t, cara::trazo::W, wu);
        pon_u16(&mut t, cara::trazo::H, hu);
        pon_u32(&mut t, cara::trazo::COLOR, color);
        pon_u16(&mut t, cara::trazo::CAD_OFF, off);
        pon_u16(&mut t, cara::trazo::CAD_LEN, len);
        // El reservado se queda en cero porque el array nace en cero. Se dice
        // aqui y no se escribe: escribir un cero que ya esta invita a que
        // alguien lo cambie por otra cosa sin subir la version.
        trazos.push(t);
    }

    let mut golpes_b: Vec<[u8; cara::GOLPE]> = Vec::with_capacity(golpes.len());
    for g in golpes {
        let (x, y, w, h) = (g.r.x as i64, g.r.y as i64, g.r.w as i64, g.r.h as i64);
        let (Some(xi), Some(yi), Some(wu), Some(hu)) =
            (i16_de(x), i16_de(y), u16_de(w), u16_de(h))
        else {
            return Err(NoCabe::Rect { de: g.nombre.clone(), x, y, w, h });
        };
        let (off, len) = cad.mete(g.nombre.as_bytes());
        let mut b = [0u8; cara::GOLPE];
        pon_i16(&mut b, cara::golpe::X, xi);
        pon_i16(&mut b, cara::golpe::Y, yi);
        pon_u16(&mut b, cara::golpe::W, wu);
        pon_u16(&mut b, cara::golpe::H, hu);
        pon_u16(&mut b, cara::golpe::CAD_OFF, off);
        pon_u16(&mut b, cara::golpe::CAD_LEN, len);
        golpes_b.push(b);
    }

    // ** Las tres cuentas, comprobadas ANTES de escribir la cabecera.
    //
    // Sin esto, un `as u16` las recortaria y saldria una cara que declara menos
    // trazos de los que trae: se abriria sin protestar y pintaria a medias. El
    // lector no puede cazar eso -- las cuentas cuadrarian.
    if trazos.len() > u16::MAX as usize {
        return Err(NoCabe::Demasiado { que: "trazos", cuantos: trazos.len() });
    }
    if golpes_b.len() > u16::MAX as usize {
        return Err(NoCabe::Demasiado { que: "golpes", cuantos: golpes_b.len() });
    }
    if cad.bytes.len() > u16::MAX as usize {
        return Err(NoCabe::Demasiado { que: "bytes de cadenas", cuantos: cad.bytes.len() });
    }

    let mut out = Vec::with_capacity(
        cara::CABECERA + trazos.len() * cara::TRAZO + golpes_b.len() * cara::GOLPE + cad.bytes.len(),
    );
    out.extend_from_slice(&cara::MAGICO.to_le_bytes());
    out.extend_from_slice(&cara::VERSION.to_le_bytes());
    out.extend_from_slice(&ancho_u.to_le_bytes());
    out.extend_from_slice(&alto_u.to_le_bytes());
    out.extend_from_slice(&(trazos.len() as u16).to_le_bytes());
    out.extend_from_slice(&(golpes_b.len() as u16).to_le_bytes());
    out.extend_from_slice(&(cad.bytes.len() as u16).to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());
    debug_assert_eq!(out.len(), cara::CABECERA);
    for t in &trazos {
        out.extend_from_slice(t);
    }
    for g in &golpes_b {
        out.extend_from_slice(g);
    }
    out.extend_from_slice(&cad.bytes);
    Ok(out)
}

fn pon_u16(b: &mut [u8], i: usize, v: u16) {
    b[i..i + 2].copy_from_slice(&v.to_le_bytes());
}
fn pon_i16(b: &mut [u8], i: usize, v: i16) {
    b[i..i + 2].copy_from_slice(&v.to_le_bytes());
}
fn pon_u32(b: &mut [u8], i: usize, v: u32) {
    b[i..i + 4].copy_from_slice(&v.to_le_bytes());
}
