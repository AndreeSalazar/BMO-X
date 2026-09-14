//! **DNS**: preguntar por la IPv4 de un nombre (RFC 1035), del lado del cliente.
//!
//! [carril]  AMARILLO  lee lo que contesta un servidor de nombres
//! [cuesta]  DATO      una respuesta mal leida manda la conexion a otra maquina
//! [riesgo]  AJENO     los punteros de compresion los escribe quien contesta
//!
//! # G4 del camino a Gemini (`docs/plan/PLAN_RED_TX.md`, 2026-09-14)
//!
//! Una pregunta A/IN con recursion pedida, y una respuesta que se lee sin
//! creerse nada:
//!
//! ```text
//!    otro id, o una pregunta que no es la nuestra     NoEsParaMi
//!    un puntero hacia delante, al mismo sitio o
//!    mas de 16 saltos                                  Cabecera: la trampa del bucle
//!    una etiqueta de mas de 63 o un nombre de mas
//!    de 253                                            Largo
//!    truncada (TC)                                     Largo: aqui no se reintenta por TCP
//!    un A de un nombre que no se pregunto              se IGNORA: no es la respuesta
//! ```
//!
//! ** Un CNAME se sigue DENTRO de la misma respuesta y solo hacia delante: vale
//! el A del nombre preguntado o el de a quien apunte un CNAME de ese nombre. Un A
//! suelto de otro dominio metido en la respuesta es exactamente el envenenamiento
//! de cache que no se quiere comprar.

use crate::ipv4::Ip;
use crate::{be16, be32, pon16, Rechazo};

pub const PUERTO: u16 = 53;
const CABECERA: usize = 12;
pub const NOMBRE_MAX: usize = 253;
const SALTOS_MAX: usize = 16;
pub const IPS_MAX: usize = 4;
const TIPO_A: u16 = 1;
const TIPO_CNAME: u16 = 5;
const CLASE_IN: u16 = 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Respuesta {
    /// Hasta cuatro IPv4, y el TTL mas corto de ellas.
    Ips { ips: [Ip; IPS_MAX], cuantas: usize, ttl: u32 },
    /// El nombre existe y no tiene IPv4.
    SinIpv4,
    /// El nombre no existe (NXDOMAIN).
    NoExiste,
    /// El servidor contesto con otro codigo de error.
    Fallo(u8),
}

/// El nombre como se compara: minusculas, sin punto final, validado. Su largo.
fn normalizar(nombre: &[u8], dst: &mut [u8; NOMBRE_MAX + 1]) -> Result<usize, Rechazo> {
    let nombre = nombre.strip_suffix(b".").unwrap_or(nombre);
    if nombre.is_empty() || nombre.len() > NOMBRE_MAX {
        return Err(Rechazo::Largo);
    }
    for etiqueta in nombre.split(|&c| c == b'.') {
        if etiqueta.is_empty() || etiqueta.len() > 63 {
            return Err(Rechazo::Largo);
        }
        if !etiqueta.iter().all(|c| c.is_ascii_alphanumeric() || *c == b'-') {
            return Err(Rechazo::Cabecera);
        }
    }
    for (k, c) in nombre.iter().enumerate() {
        dst[k] = c.to_ascii_lowercase();
    }
    Ok(nombre.len())
}

/// **La pregunta**: un mensaje DNS con UNA pregunta A/IN y recursion pedida.
pub fn preguntar(dst: &mut [u8], id: u16, nombre: &[u8]) -> Result<usize, Rechazo> {
    let mut limpio = [0u8; NOMBRE_MAX + 1];
    let n = normalizar(nombre, &mut limpio)?;
    let total = CABECERA + 1 + n + 1 + 4;
    if dst.len() < total {
        return Err(Rechazo::Corto);
    }
    dst[..CABECERA].fill(0);
    pon16(dst, 0, id);
    pon16(dst, 2, 0x0100); // RD: que busque el por nosotros
    pon16(dst, 4, 1); // una pregunta
    let mut i = CABECERA;
    for etiqueta in limpio[..n].split(|&c| c == b'.') {
        dst[i] = etiqueta.len() as u8;
        dst[i + 1..i + 1 + etiqueta.len()].copy_from_slice(etiqueta);
        i += 1 + etiqueta.len();
    }
    dst[i] = 0;
    i += 1;
    pon16(dst, i, TIPO_A);
    pon16(dst, i + 2, CLASE_IN);
    Ok(i + 4)
}

/// Lee un nombre desde `i` siguiendo punteros **solo hacia atras**, en
/// minusculas y con puntos. Devuelve `(largo, donde sigue el mensaje)`.
fn leer_nombre(b: &[u8], mut i: usize, dst: &mut [u8; NOMBRE_MAX + 1]) -> Result<(usize, usize), Rechazo> {
    let mut n = 0;
    let mut sigue = None;
    let mut saltos = 0;
    // ** Cada puntero tiene que caer ANTES que el anterior: la secuencia de
    // saltos baja siempre, asi que no hay bucle posible aunque se mienta.
    let mut limite = i;
    loop {
        if i >= b.len() {
            return Err(Rechazo::Corto);
        }
        let l = b[i] as usize;
        if l & 0xC0 == 0xC0 {
            if i + 1 >= b.len() {
                return Err(Rechazo::Corto);
            }
            let destino = ((l & 0x3F) << 8) | b[i + 1] as usize;
            saltos += 1;
            if destino >= limite || destino < CABECERA || saltos > SALTOS_MAX {
                return Err(Rechazo::Cabecera);
            }
            if sigue.is_none() {
                sigue = Some(i + 2);
            }
            limite = destino;
            i = destino;
            continue;
        }
        if l & 0xC0 != 0 {
            return Err(Rechazo::Cabecera);
        }
        if l == 0 {
            break;
        }
        if i + 1 + l > b.len() {
            return Err(Rechazo::Corto);
        }
        if n != 0 {
            if n >= NOMBRE_MAX {
                return Err(Rechazo::Largo);
            }
            dst[n] = b'.';
            n += 1;
        }
        if n + l > NOMBRE_MAX {
            return Err(Rechazo::Largo);
        }
        for k in 0..l {
            dst[n + k] = b[i + 1 + k].to_ascii_lowercase();
        }
        n += l;
        i += 1 + l;
    }
    Ok((n, sigue.unwrap_or(i + 1)))
}

/// **Lee la respuesta** a la pregunta `id` por `nombre`.
pub fn leer(b: &[u8], id: u16, nombre: &[u8]) -> Result<Respuesta, Rechazo> {
    if b.len() < CABECERA {
        return Err(Rechazo::Corto);
    }
    if be16(b, 0) != id {
        return Err(Rechazo::NoEsParaMi);
    }
    let banderas = be16(b, 2);
    if banderas & 0x8000 == 0 || banderas & 0x7800 != 0 {
        return Err(Rechazo::Cabecera);
    }
    if banderas & 0x0200 != 0 {
        return Err(Rechazo::Largo);
    }
    if be16(b, 4) != 1 {
        return Err(Rechazo::Cabecera);
    }
    let mut esperado = [0u8; NOMBRE_MAX + 1];
    let ne = normalizar(nombre, &mut esperado)?;
    let mut leido = [0u8; NOMBRE_MAX + 1];
    let (n, mut i) = leer_nombre(b, CABECERA, &mut leido)?;
    if leido[..n] != esperado[..ne] {
        return Err(Rechazo::NoEsParaMi);
    }
    if i + 4 > b.len() {
        return Err(Rechazo::Corto);
    }
    if be16(b, i) != TIPO_A || be16(b, i + 2) != CLASE_IN {
        return Err(Rechazo::NoEsParaMi);
    }
    i += 4;
    match (banderas & 0x000F) as u8 {
        0 => {}
        3 => return Ok(Respuesta::NoExiste),
        otro => return Ok(Respuesta::Fallo(otro)),
    }

    let mut buscado = esperado;
    let mut nb = ne;
    let mut ips = [[0u8; 4]; IPS_MAX];
    let mut cuantas = 0;
    let mut ttl = u32::MAX;
    for _ in 0..be16(b, 6) {
        let (n, j) = leer_nombre(b, i, &mut leido)?;
        if j + 10 > b.len() {
            return Err(Rechazo::Corto);
        }
        let (tipo, clase, t, largo) = (be16(b, j), be16(b, j + 2), be32(b, j + 4), be16(b, j + 8) as usize);
        let datos = j + 10;
        if datos + largo > b.len() {
            return Err(Rechazo::Corto);
        }
        if clase == CLASE_IN && leido[..n] == buscado[..nb] {
            if tipo == TIPO_A {
                if largo != 4 {
                    return Err(Rechazo::Cabecera);
                }
                if cuantas < IPS_MAX {
                    ips[cuantas] = [b[datos], b[datos + 1], b[datos + 2], b[datos + 3]];
                    cuantas += 1;
                    ttl = ttl.min(t);
                }
            } else if tipo == TIPO_CNAME {
                let mut destino = [0u8; NOMBRE_MAX + 1];
                let (m, _) = leer_nombre(b, datos, &mut destino)?;
                buscado = destino;
                nb = m;
            }
        }
        i = datos + largo;
    }
    if cuantas == 0 {
        return Ok(Respuesta::SinIpv4);
    }
    Ok(Respuesta::Ips { ips, cuantas, ttl })
}

#[cfg(test)]
mod pruebas {
    use super::*;

    const ID: u16 = 0xB0B0;

    fn nombre_wire(n: &str) -> Vec<u8> {
        let mut v = Vec::new();
        for e in n.split('.') {
            v.push(e.len() as u8);
            v.extend_from_slice(e.as_bytes());
        }
        v.push(0);
        v
    }

    /// Una respuesta: cabecera, la pregunta, y los registros tal cual.
    fn respuesta(id: u16, banderas: u16, pregunta: &str, registros: &[(Vec<u8>, u16, u32, Vec<u8>)]) -> Vec<u8> {
        let mut b = vec![0u8; CABECERA];
        pon16(&mut b, 0, id);
        pon16(&mut b, 2, banderas);
        pon16(&mut b, 4, 1);
        pon16(&mut b, 6, registros.len() as u16);
        b.extend_from_slice(&nombre_wire(pregunta));
        b.extend_from_slice(&[0, 1, 0, 1]);
        for (nombre, tipo, ttl, datos) in registros {
            b.extend_from_slice(nombre);
            b.extend_from_slice(&tipo.to_be_bytes());
            b.extend_from_slice(&[0, 1]);
            b.extend_from_slice(&ttl.to_be_bytes());
            b.extend_from_slice(&(datos.len() as u16).to_be_bytes());
            b.extend_from_slice(datos);
        }
        b
    }

    /// Un puntero a la pregunta, que empieza en el 12.
    fn a_la_pregunta() -> Vec<u8> {
        vec![0xC0, 12]
    }

    #[test]
    fn la_pregunta_es_la_del_rfc() {
        let mut b = [0u8; 64];
        let n = preguntar(&mut b, ID, b"WWW.Example.com.").unwrap();
        assert_eq!(&b[..12], &[0xB0, 0xB0, 0x01, 0x00, 0, 1, 0, 0, 0, 0, 0, 0]);
        assert_eq!(&b[12..n - 4], &nombre_wire("www.example.com")[..], "en minusculas");
        assert_eq!(&b[n - 4..n], &[0, 1, 0, 1]);
    }

    #[test]
    fn nombres_que_no_se_preguntan() {
        let mut b = [0u8; 600];
        assert_eq!(preguntar(&mut b, ID, b""), Err(Rechazo::Largo));
        assert_eq!(preguntar(&mut b, ID, b"a..b"), Err(Rechazo::Largo));
        assert_eq!(preguntar(&mut b, ID, &[b'a'; 64]), Err(Rechazo::Largo));
        assert_eq!(preguntar(&mut b, ID, b"mal_nombre.com"), Err(Rechazo::Cabecera));
        let largo = ["abcdefghi"; 26].join(".");
        assert_eq!(preguntar(&mut b, ID, largo.as_bytes()), Err(Rechazo::Largo), "{} bytes", largo.len());
    }

    #[test]
    fn una_respuesta_con_puntero() {
        let r = respuesta(ID, 0x8180, "gemini.circumlunar.space", &[(a_la_pregunta(), 1, 300, vec![203, 0, 113, 7])]);
        assert_eq!(
            leer(&r, ID, b"gemini.circumlunar.space"),
            Ok(Respuesta::Ips { ips: [[203, 0, 113, 7], [0; 4], [0; 4], [0; 4]], cuantas: 1, ttl: 300 })
        );
    }

    #[test]
    fn mayusculas_en_la_respuesta_valen() {
        let mut r = respuesta(ID, 0x8180, "example.com", &[(a_la_pregunta(), 1, 60, vec![1, 2, 3, 4])]);
        r[13] = b'E';
        assert!(matches!(leer(&r, ID, b"example.com"), Ok(Respuesta::Ips { cuantas: 1, .. })));
    }

    #[test]
    fn se_sigue_el_cname_y_se_ignora_el_a_ajeno() {
        let registros = [
            (a_la_pregunta(), 5, 100, nombre_wire("web.example.net")),
            (nombre_wire("otro.malo.com"), 1, 100, vec![6, 6, 6, 6]),
            (nombre_wire("web.example.net"), 1, 50, vec![198, 51, 100, 1]),
        ];
        let r = respuesta(ID, 0x8180, "www.example.com", &registros);
        assert_eq!(
            leer(&r, ID, b"www.example.com"),
            Ok(Respuesta::Ips { ips: [[198, 51, 100, 1], [0; 4], [0; 4], [0; 4]], cuantas: 1, ttl: 50 })
        );
        let solo_ajeno = respuesta(ID, 0x8180, "www.example.com", &[(nombre_wire("otro.malo.com"), 1, 1, vec![6; 4])]);
        assert_eq!(leer(&solo_ajeno, ID, b"www.example.com"), Ok(Respuesta::SinIpv4));
    }

    #[test]
    fn errores_del_servidor() {
        assert_eq!(leer(&respuesta(ID, 0x8183, "no.existe", &[]), ID, b"no.existe"), Ok(Respuesta::NoExiste));
        assert_eq!(leer(&respuesta(ID, 0x8182, "a.b", &[]), ID, b"a.b"), Ok(Respuesta::Fallo(2)));
    }

    #[test]
    fn lo_que_no_es_nuestra_respuesta() {
        let ok = respuesta(ID, 0x8180, "a.com", &[(a_la_pregunta(), 1, 1, vec![1, 1, 1, 1])]);
        assert_eq!(leer(&ok, ID + 1, b"a.com"), Err(Rechazo::NoEsParaMi), "otro id");
        assert_eq!(leer(&ok, ID, b"b.com"), Err(Rechazo::NoEsParaMi), "otra pregunta");
        assert_eq!(leer(&respuesta(ID, 0x0100, "a.com", &[]), ID, b"a.com"), Err(Rechazo::Cabecera), "no es respuesta");
        assert_eq!(leer(&respuesta(ID, 0x8380, "a.com", &[]), ID, b"a.com"), Err(Rechazo::Largo), "truncada");
    }

    /// *** LA TRAMPA DEL BUCLE: punteros a si mismos, hacia delante o en circulo.
    #[test]
    fn punteros_que_mienten() {
        let mut propio = respuesta(ID, 0x8180, "a.com", &[]);
        propio[12] = 0xC0;
        propio[13] = 12;
        assert_eq!(leer(&propio, ID, b"a.com"), Err(Rechazo::Cabecera), "a si mismo");
        let delante = respuesta(ID, 0x8180, "a.com", &[(vec![0xC0, 200], 1, 1, vec![1, 1, 1, 1])]);
        assert_eq!(leer(&delante, ID, b"a.com"), Err(Rechazo::Cabecera), "hacia delante");
        let mut circulo = respuesta(ID, 0x8180, "a.com", &[]);
        let base = circulo.len();
        circulo.extend_from_slice(&[0xC0, (base + 2) as u8, 0xC0, base as u8]);
        pon16(&mut circulo, 6, 1);
        circulo.extend_from_slice(&[0, 1, 0, 1, 0, 0, 0, 1, 0, 4, 1, 1, 1, 1]);
        assert!(leer(&circulo, ID, b"a.com").is_err(), "en circulo");
    }

    #[test]
    fn veinte_mil_respuestas_mutadas_no_revientan() {
        let registros = [
            (a_la_pregunta(), 5, 100, nombre_wire("web.example.net")),
            (nombre_wire("web.example.net"), 1, 50, vec![198, 51, 100, 1]),
        ];
        let base = respuesta(ID, 0x8180, "www.example.com", &registros);
        let mut semilla = 0xD45u64;
        let mut azar = || {
            semilla = semilla.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            (semilla >> 33) as usize
        };
        for _ in 0..20_000 {
            let mut m = base.clone();
            for _ in 0..1 + azar() % 6 {
                let i = azar() % m.len();
                m[i] = azar() as u8;
            }
            m.truncate(azar() % (m.len() + 1));
            let _ = leer(&m, ID, b"www.example.com");
        }
    }
}
