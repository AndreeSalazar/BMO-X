//! **BMO ANTENA** -- el protocolo ANTENA/1 del CLOUD LOCAL, del lado de BMO-X.
//!
//! generacion: nieto -- lineas de texto y sus veredictos; no sabe de sockets ni de video
//! capa: puro -- ni un `unsafe`, ni un aparato: lo usan Ring 3 y el banco
//!
//! [carril]  AMARILLO  lee lo que manda otra maquina de la casa
//! [cuesta]  DATO      una cabecera mal leida da un video que no es el pedido
//! [riesgo]  AJENO     cada linea la escribe la antena, no BMO-X
//!
//! # S2 de `docs/plan/PLAN_CLOUD_LOCAL.md` (2026-09-14)
//!
//! Eddi: *"mi celular se convierte en antena y puedo ver todo; BMO ya no navega
//! pero la antena si"*. La antena (un movil con Termux) hace la web y convierte;
//! BMO-X pide y ensena. Entre los dos, esto:
//!
//! ```text
//!    BMO-X -> antena                 antena -> BMO-X
//!    HOLA ANTENA/1                   HOLA ANTENA/1 <nombre>
//!    LISTA                           LISTA <n>, y n lineas ENTRADA <id> <titulo>
//!    PIDE <id>                       VIDEO <bytes> mpeg1 <ancho>x<alto>, y el flujo
//!                                    NO <motivo>
//!    (cerrar la conexion)            es PARAR: no hace falta otra palabra
//! ```
//!
//! # Lista blanca, como `bmo-pila`
//!
//! ```text
//!    una linea de mas de 256 bytes, o con un byte no imprimible   Largo / NoAscii
//!    un verbo que no esta arriba                                   Verbo
//!    un id fuera de [a-z0-9_-]{1,32}                               Id
//!    un formato que no es mpeg1                                    Formato
//!    un tamano impar, menor de 16 o mayor que 1280x720             Tamano
//!    otra version del protocolo                                    Version
//! ```
//!
//! ** `bytes` a cero es "en vivo": la antena convierte mientras envia y no sabe
//! el largo. El flujo acaba cuando se cierra la conexion.

#![cfg_attr(not(test), no_std)]
#![forbid(unsafe_code)]

pub const VERSION: &[u8] = b"ANTENA/1";
/// El puerto de la antena. Alto, sin dueno conocido, y facil de recordar.
pub const PUERTO: u16 = 7117;
pub const LINEA_MAX: usize = 256;
pub const ID_MAX: usize = 32;
pub const TEXTO_MAX: usize = 160;
pub const LISTA_MAX: u32 = 64;
pub const ANCHO_MAX: u32 = 1280;
pub const ALTO_MAX: u32 = 720;
pub const LADO_MIN: u32 = 16;

/// **Por que no.** Uno por motivo.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Rechazo {
    Largo,
    NoAscii,
    Verbo,
    Campos,
    Numero,
    Id,
    Formato,
    Tamano,
    Version,
    Corto,
}

impl Rechazo {
    pub fn texto(self) -> &'static str {
        match self {
            Rechazo::Largo => "linea de mas de 256 bytes",
            Rechazo::NoAscii => "un byte que no es texto imprimible",
            Rechazo::Verbo => "una palabra que el protocolo no tiene",
            Rechazo::Campos => "faltan o sobran campos",
            Rechazo::Numero => "un numero imposible",
            Rechazo::Id => "un id fuera de [a-z0-9_-]{1,32}",
            Rechazo::Formato => "un formato de video que no es mpeg1",
            Rechazo::Tamano => "un tamano impar, pequeno o mayor que 1280x720",
            Rechazo::Version => "otra version del protocolo",
            Rechazo::Corto => "no cabe en el bufer",
        }
    }
}

/// **Lo que dice la antena.** Los trozos apuntan dentro de la linea recibida.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Respuesta<'a> {
    Hola { nombre: &'a [u8] },
    Lista { cuantas: u32 },
    Entrada { id: &'a [u8], titulo: &'a [u8] },
    /// `bytes == 0`: en vivo, acaba al cerrar.
    Video { bytes: u64, ancho: u32, alto: u32 },
    No { motivo: &'a [u8] },
}

fn id_valido(id: &[u8]) -> bool {
    !id.is_empty()
        && id.len() <= ID_MAX
        && id.iter().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || *c == b'_' || *c == b'-')
}

fn texto_valido(t: &[u8]) -> bool {
    !t.is_empty() && t.len() <= TEXTO_MAX
}

fn numero(b: &[u8], maximo: u64) -> Result<u64, Rechazo> {
    if b.is_empty() || b.len() > 20 || !b.iter().all(u8::is_ascii_digit) {
        return Err(Rechazo::Numero);
    }
    let mut n = 0u64;
    for &c in b {
        n = n.checked_mul(10).and_then(|n| n.checked_add((c - b'0') as u64)).ok_or(Rechazo::Numero)?;
    }
    if n > maximo {
        return Err(Rechazo::Numero);
    }
    Ok(n)
}

/// Parte en la primera espacio: `(palabra, resto)`.
fn partir(b: &[u8]) -> (&[u8], &[u8]) {
    match b.iter().position(|&c| c == b' ') {
        Some(i) => (&b[..i], &b[i + 1..]),
        None => (b, &[]),
    }
}

/// **Lee UNA linea** de la antena, con o sin su `\n` (y `\r\n` tambien vale).
pub fn leer(linea: &[u8]) -> Result<Respuesta<'_>, Rechazo> {
    let linea = linea.strip_suffix(b"\n").unwrap_or(linea);
    let linea = linea.strip_suffix(b"\r").unwrap_or(linea);
    if linea.len() > LINEA_MAX {
        return Err(Rechazo::Largo);
    }
    if !linea.iter().all(|&c| (0x20..=0x7E).contains(&c)) {
        return Err(Rechazo::NoAscii);
    }
    let (verbo, resto) = partir(linea);
    match verbo {
        b"HOLA" => {
            let (version, nombre) = partir(resto);
            if version != VERSION {
                return Err(Rechazo::Version);
            }
            if !texto_valido(nombre) {
                return Err(Rechazo::Campos);
            }
            Ok(Respuesta::Hola { nombre })
        }
        b"LISTA" => Ok(Respuesta::Lista { cuantas: numero(resto, LISTA_MAX as u64)? as u32 }),
        b"ENTRADA" => {
            let (id, titulo) = partir(resto);
            if !id_valido(id) {
                return Err(Rechazo::Id);
            }
            if !texto_valido(titulo) {
                return Err(Rechazo::Campos);
            }
            Ok(Respuesta::Entrada { id, titulo })
        }
        b"VIDEO" => {
            let (bytes, resto) = partir(resto);
            let (formato, resto) = partir(resto);
            let (tamano, sobra) = partir(resto);
            if tamano.is_empty() || !sobra.is_empty() {
                return Err(Rechazo::Campos);
            }
            let bytes = numero(bytes, u64::MAX)?;
            if formato != b"mpeg1" {
                return Err(Rechazo::Formato);
            }
            let x = tamano.iter().position(|&c| c == b'x').ok_or(Rechazo::Tamano)?;
            let ancho = numero(&tamano[..x], ANCHO_MAX as u64).map_err(|_| Rechazo::Tamano)? as u32;
            let alto = numero(&tamano[x + 1..], ALTO_MAX as u64).map_err(|_| Rechazo::Tamano)? as u32;
            if ancho < LADO_MIN || alto < LADO_MIN || ancho % 2 != 0 || alto % 2 != 0 {
                return Err(Rechazo::Tamano);
            }
            Ok(Respuesta::Video { bytes, ancho, alto })
        }
        b"NO" => {
            if !texto_valido(resto) {
                return Err(Rechazo::Campos);
            }
            Ok(Respuesta::No { motivo: resto })
        }
        _ => Err(Rechazo::Verbo),
    }
}

/// **Lo que pide BMO-X.**
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pedido<'a> {
    Hola,
    Lista,
    Pide(&'a [u8]),
}

/// Escribe un pedido con su `\n`. Un id invalido no se escribe.
pub fn escribir(dst: &mut [u8], p: &Pedido) -> Result<usize, Rechazo> {
    let mut i = 0;
    let mut poner = |dst: &mut [u8], b: &[u8]| -> Result<(), Rechazo> {
        if dst.len() < i + b.len() {
            return Err(Rechazo::Corto);
        }
        dst[i..i + b.len()].copy_from_slice(b);
        i += b.len();
        Ok(())
    };
    match p {
        Pedido::Hola => {
            poner(dst, b"HOLA ")?;
            poner(dst, VERSION)?;
        }
        Pedido::Lista => poner(dst, b"LISTA")?,
        Pedido::Pide(id) => {
            if !id_valido(id) {
                return Err(Rechazo::Id);
            }
            poner(dst, b"PIDE ")?;
            poner(dst, id)?;
        }
    }
    poner(dst, b"\n")?;
    Ok(i)
}

/// **Junta bytes de TCP en lineas.** Una linea demasiado larga se dice UNA vez y
/// se tira hasta el siguiente `\n`: una antena rota no bloquea la conversacion.
pub struct Lineas {
    buf: [u8; LINEA_MAX + 2],
    n: usize,
    tirando: bool,
}

impl Default for Lineas {
    fn default() -> Self {
        Self::nueva()
    }
}

impl Lineas {
    pub const fn nueva() -> Self {
        Self { buf: [0; LINEA_MAX + 2], n: 0, tirando: false }
    }

    /// Un byte mas. `Some` cuando hay una linea completa, o cuando se acaba de
    /// pasar del tope.
    pub fn empujar(&mut self, b: u8) -> Option<Result<&[u8], Rechazo>> {
        if b == b'\n' {
            let n = self.n;
            self.n = 0;
            if self.tirando {
                self.tirando = false;
                return None;
            }
            return Some(Ok(&self.buf[..n]));
        }
        if self.tirando {
            return None;
        }
        if self.n == self.buf.len() {
            self.n = 0;
            self.tirando = true;
            return Some(Err(Rechazo::Largo));
        }
        self.buf[self.n] = b;
        self.n += 1;
        None
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn el_saludo_y_la_version() {
        assert_eq!(leer(b"HOLA ANTENA/1 movil de casa\n"), Ok(Respuesta::Hola { nombre: b"movil de casa" }));
        assert_eq!(leer(b"HOLA ANTENA/1 movil\r\n"), Ok(Respuesta::Hola { nombre: b"movil" }), "CRLF vale");
        assert_eq!(leer(b"HOLA ANTENA/2 movil"), Err(Rechazo::Version));
        assert_eq!(leer(b"HOLA ANTENA/1"), Err(Rechazo::Campos), "la version es buena: falta el nombre");
        assert_eq!(leer(b"HOLA"), Err(Rechazo::Version), "sin version");
        assert_eq!(leer(b"HOLA ANTENA/1 "), Err(Rechazo::Campos));
    }

    #[test]
    fn la_lista_y_sus_entradas() {
        assert_eq!(leer(b"LISTA 3"), Ok(Respuesta::Lista { cuantas: 3 }));
        assert_eq!(leer(b"LISTA 65"), Err(Rechazo::Numero), "mas de 64");
        assert_eq!(leer(b"LISTA -1"), Err(Rechazo::Numero));
        assert_eq!(
            leer(b"ENTRADA v12 Un video de prueba (libre).mp4"),
            Ok(Respuesta::Entrada { id: b"v12", titulo: b"Un video de prueba (libre).mp4" })
        );
        assert_eq!(leer(b"ENTRADA V12 x"), Err(Rechazo::Id), "mayusculas no");
        assert_eq!(leer(b"ENTRADA ../etc x"), Err(Rechazo::Id));
        assert_eq!(leer(b"ENTRADA v1"), Err(Rechazo::Campos), "sin titulo");
    }

    #[test]
    fn la_cabecera_del_video() {
        assert_eq!(leer(b"VIDEO 0 mpeg1 640x360"), Ok(Respuesta::Video { bytes: 0, ancho: 640, alto: 360 }));
        assert_eq!(leer(b"VIDEO 12345 mpeg1 1280x720"), Ok(Respuesta::Video { bytes: 12345, ancho: 1280, alto: 720 }));
        assert_eq!(leer(b"VIDEO 0 h264 640x360"), Err(Rechazo::Formato));
        assert_eq!(leer(b"VIDEO 0 mpeg1 641x360"), Err(Rechazo::Tamano), "impar");
        assert_eq!(leer(b"VIDEO 0 mpeg1 1920x1080"), Err(Rechazo::Tamano), "demasiado");
        assert_eq!(leer(b"VIDEO 0 mpeg1 8x8"), Err(Rechazo::Tamano), "demasiado pequeno");
        assert_eq!(leer(b"VIDEO 0 mpeg1 640360"), Err(Rechazo::Tamano));
        assert_eq!(leer(b"VIDEO 0 mpeg1 640x360 extra"), Err(Rechazo::Campos));
        assert_eq!(leer(b"VIDEO 99999999999999999999 mpeg1 640x360"), Err(Rechazo::Numero), "no cabe en u64");
    }

    #[test]
    fn un_no_y_lo_que_no_es_protocolo() {
        assert_eq!(leer(b"NO no hay video con ese id"), Ok(Respuesta::No { motivo: b"no hay video con ese id" }));
        assert_eq!(leer(b"NO"), Err(Rechazo::Campos));
        assert_eq!(leer(b"BORRA todo"), Err(Rechazo::Verbo));
        assert_eq!(leer(b"HOLA ANTENA/1 m\xC3\xB3vil"), Err(Rechazo::NoAscii));
        assert_eq!(leer(&[b'N'; LINEA_MAX + 1]), Err(Rechazo::Largo));
    }

    #[test]
    fn los_pedidos_se_escriben_como_se_leen() {
        let mut b = [0u8; 64];
        let n = escribir(&mut b, &Pedido::Hola).unwrap();
        assert_eq!(&b[..n], b"HOLA ANTENA/1\n");
        let n = escribir(&mut b, &Pedido::Lista).unwrap();
        assert_eq!(&b[..n], b"LISTA\n");
        let n = escribir(&mut b, &Pedido::Pide(b"v3")).unwrap();
        assert_eq!(&b[..n], b"PIDE v3\n");
        assert_eq!(escribir(&mut b, &Pedido::Pide(b"v3; rm")), Err(Rechazo::Id));
        assert_eq!(escribir(&mut [0u8; 4], &Pedido::Hola), Err(Rechazo::Corto));
    }

    #[test]
    fn las_lineas_se_juntan_aunque_lleguen_a_trozos() {
        let mut l = Lineas::nueva();
        let mut vistas = Vec::new();
        for trozo in [&b"HOLA ANT"[..], b"ENA/1 movil\nLIS", b"TA 2\n"] {
            for &c in trozo {
                if let Some(r) = l.empujar(c) {
                    vistas.push(r.unwrap().to_vec());
                }
            }
        }
        assert_eq!(vistas, vec![b"HOLA ANTENA/1 movil".to_vec(), b"LISTA 2".to_vec()]);
    }

    #[test]
    fn una_linea_eterna_se_dice_una_vez_y_se_sigue() {
        let mut l = Lineas::nueva();
        let mut errores = 0;
        for _ in 0..5000 {
            if let Some(Err(Rechazo::Largo)) = l.empujar(b'A') {
                errores += 1;
            }
        }
        assert_eq!(errores, 1, "se dice UNA vez");
        assert_eq!(l.empujar(b'\n'), None, "el final de la eterna se traga");
        for &c in b"LISTA 1" {
            assert_eq!(l.empujar(c), None);
        }
        assert_eq!(l.empujar(b'\n'), Some(Ok(&b"LISTA 1"[..])), "y la siguiente llega entera");
    }

    #[test]
    fn veinte_mil_lineas_mutadas_no_revientan() {
        let base: &[&[u8]] = &[b"HOLA ANTENA/1 movil", b"LISTA 12", b"ENTRADA v1 titulo", b"VIDEO 0 mpeg1 640x360", b"NO motivo"];
        let mut semilla = 0xA7E4u64;
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
            let _ = leer(&m);
        }
    }
}
