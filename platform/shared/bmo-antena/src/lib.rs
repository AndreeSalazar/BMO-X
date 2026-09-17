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
//!                                    LAMINA <ancho> <alto> <n>, y n lineas (2026-09-16)
//!                                    NO <motivo>
//!    PAGINA <url>                    LAMINA ..., o NO: la antena NAVEGA sola (16-09)
//!    (cerrar la conexion)            es PARAR: no hace falta otra palabra
//! ```
//!
//! ** LAS PAGINAS entran por el MISMO `PIDE` (2026-09-16): la LISTA trae videos
//! (`v1`, `v2`...) y paginas (`p1`, `p2`...), y lo que distingue a una de otro
//! es lo que la antena contesta -- `VIDEO` o `LAMINA`. Una lamina no acaba la
//! conversacion: llegan sus `n` lineas (`lamina::Lector` las juzga una a una,
//! en Latin-1) y se vuelve a la charla, porque un clic pide la siguiente.
//! Una antena que solo sabe video contesta `NO` a un `p1`, y eso vale.
//!
//! ** `PAGINA <url>` es la antena NAVEGANDO SOLA: carga la url en su navegador,
//! corre `lamina.js` y contesta como a un `PIDE` de pagina. La url viaja tal
//! cual, y por eso es lo unico del protocolo que BMO-X manda "largo": hasta
//! `URL_MAX` bytes, ASCII imprimible sin espacios, y solo `http://` o
//! `https://`. Lo que la antena haga con ella es de la antena (seccion 12 del
//! plan: una orden sin dueno es lo que EMPAREJAR existe para impedir).
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

/// **El prestamo y su castigo**: si la antena se pasa de lista, se corta y espera
/// hasta aclararse. Seccion 9 del plan.
pub mod cuarentena;
/// **La lamina**: una pagina ya maquetada por la antena. Seccion 11 del plan.
pub mod lamina;

pub const VERSION: &[u8] = b"ANTENA/1";
/// El puerto de la antena. Alto, sin dueno conocido, y facil de recordar.
pub const PUERTO: u16 = 7117;
pub const LINEA_MAX: usize = 256;
pub const ID_MAX: usize = 32;
pub const TEXTO_MAX: usize = 160;
pub const LISTA_MAX: u32 = 64;
/// Lo mas larga que puede ser una url en `PAGINA`: cabe en una linea con el verbo.
pub const URL_MAX: usize = 200;
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
    /// Una linea que no toca en este momento de la conversacion.
    Orden,
    /// Demasiadas lineas sin llegar a un video.
    Charla,
    /// La conversacion ya se cerro por un rechazo anterior.
    Cerrada,
    /// Una url que no es `http(s)://`, lleva espacios o bytes raros, o no cabe.
    Url,
    /// Una zona de la lamina que se sale de la lamina.
    Fuera,
    /// Un color que no es `rrggbb`.
    Color,
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
            Rechazo::Orden => "una linea que no toca ahora: la antena se sale del protocolo",
            Rechazo::Charla => "demasiadas lineas sin llegar a un video",
            Rechazo::Cerrada => "la conversacion ya se cerro por un rechazo",
            Rechazo::Url => "una url que no es http(s)://, o con espacios, o de mas de 200 bytes",
            Rechazo::Fuera => "una zona que se sale de la lamina",
            Rechazo::Color => "un color que no es rrggbb",
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
    /// La cabecera de una pagina ya maquetada; siguen `elementos` lineas.
    Lamina(lamina::Cabecera),
    /// Una linea de la lamina, ya juzgada por `lamina::Lector`.
    Elemento(lamina::Paso<'a>),
    No { motivo: &'a [u8] },
}

pub(crate) fn id_valido(id: &[u8]) -> bool {
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
        b"LAMINA" => Ok(Respuesta::Lamina(lamina::leer_cabecera(linea)?)),
        _ => Err(Rechazo::Verbo),
    }
}

/// **Lo que pide BMO-X.**
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pedido<'a> {
    Hola,
    Lista,
    Pide(&'a [u8]),
    /// La antena navega sola: carga la url y contesta `LAMINA` o `NO`.
    Pagina(&'a [u8]),
}

/// `http://` o `https://`, ASCII imprimible, sin espacios, y cabe en la linea.
pub fn url_valida(url: &[u8]) -> bool {
    !url.is_empty()
        && url.len() <= URL_MAX
        && url.iter().all(|&c| (0x21..=0x7E).contains(&c))
        && (url.starts_with(b"http://") || url.starts_with(b"https://"))
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
        Pedido::Pagina(url) => {
            if !url_valida(url) {
                return Err(Rechazo::Url);
            }
            poner(dst, b"PAGINA ")?;
            poner(dst, url)?;
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

// ===================================================================
//  *** LA CONVERSACION: el orden tambien es lista blanca (2026-09-14)
// ===================================================================
//
// Eddi: *"MAS ESTRICTO ANTENA"*. Leer bien cada linea no basta: una antena que
// manda un VIDEO sin que se pidiera, o cuarenta ENTRADAS a una LISTA de tres,
// escribe lineas validas en el momento equivocado. Esto lleva la cuenta:
//
// ```text
//    Inicio -- pedir HOLA --> Saludo -- oir HOLA --> Charla
//    Charla -- pedir LISTA --> Lista -- oir LISTA n, y n ENTRADA --> Charla
//    Charla -- pedir PIDE  --> Pidiendo -- oir VIDEO --> Video (bytes)
//                                       -- oir NO    --> Charla
// ```
//
// ** UN rechazo CIERRA la conversacion entera: despues todo es `Cerrada`. Una
// antena que se sale del protocolo una vez no tiene segunda oportunidad en la
// misma conexion -- se cuelga y se vuelve a llamar.

/// Lineas que caben en una conversacion antes de un video: la lista entera, el
/// saludo y unos pocos NO.
pub const LINEAS_MAX: u32 = LISTA_MAX + 8;
/// Lo mas que se acepta de un video en vivo: 4 GiB, horas a 1,5 Mbit/s.
pub const VIDEO_MAX: u64 = 4 << 30;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fase {
    Inicio,
    Saludo,
    Charla,
    Lista { faltan: u32, anunciada: bool },
    Pidiendo,
    Video { declarados: u64, recibidos: u64 },
    /// Llegan las lineas de una pagina; el lector lleva la cuenta.
    Lamina(lamina::Lector),
    Cerrada,
}

/// **Una conversacion con la antena.** Una por conexion.
pub struct Conversacion {
    fase: Fase,
    lineas: u32,
}

impl Default for Conversacion {
    fn default() -> Self {
        Self::nueva()
    }
}

impl Conversacion {
    pub const fn nueva() -> Self {
        Self { fase: Fase::Inicio, lineas: 0 }
    }

    pub fn fase(&self) -> Fase {
        self.fase
    }

    fn cerrar<T>(&mut self, r: Rechazo) -> Result<T, Rechazo> {
        self.fase = Fase::Cerrada;
        Err(r)
    }

    /// **Antes de mandar un pedido.** `Err` = no toca, y no se manda.
    pub fn pedir(&mut self, p: &Pedido) -> Result<(), Rechazo> {
        self.fase = match (self.fase, p) {
            (Fase::Cerrada, _) => return Err(Rechazo::Cerrada),
            (Fase::Inicio, Pedido::Hola) => Fase::Saludo,
            (Fase::Charla, Pedido::Lista) => Fase::Lista { faltan: 0, anunciada: false },
            (Fase::Charla, Pedido::Pide(id)) if id_valido(id) => Fase::Pidiendo,
            // Una pagina pedida por url espera lo mismo que un `PIDE`: LAMINA o NO.
            (Fase::Charla, Pedido::Pagina(url)) if url_valida(url) => Fase::Pidiendo,
            _ => return Err(Rechazo::Orden),
        };
        Ok(())
    }

    /// **Una linea de la antena.** Solo vale la que toca; cualquier otra cierra.
    pub fn oir<'a>(&mut self, linea: &'a [u8]) -> Result<Respuesta<'a>, Rechazo> {
        if self.fase == Fase::Cerrada {
            return Err(Rechazo::Cerrada);
        }
        // ** Las lineas de una lamina NO cuentan para `LINEAS_MAX`: una pagina
        // son miles, y tienen su propio tope (`lamina::ELEMENTOS_MAX`) y su
        // propio juez. Tampoco pasan por `leer`: el TEXTO lleva Latin-1.
        if let Fase::Lamina(mut lector) = self.fase {
            let paso = match lector.empujar(linea) {
                Ok(p) => p,
                Err(e) => return self.cerrar(e),
            };
            self.fase = if lector.completa() { Fase::Charla } else { Fase::Lamina(lector) };
            return Ok(Respuesta::Elemento(paso));
        }
        self.lineas += 1;
        if self.lineas > LINEAS_MAX {
            return self.cerrar(Rechazo::Charla);
        }
        let r = match leer(linea) {
            Ok(r) => r,
            Err(e) => return self.cerrar(e),
        };
        let siguiente = match (self.fase, r) {
            (Fase::Saludo, Respuesta::Hola { .. }) => Fase::Charla,
            (Fase::Lista { anunciada: false, .. }, Respuesta::Lista { cuantas: 0 }) => Fase::Charla,
            (Fase::Lista { anunciada: false, .. }, Respuesta::Lista { cuantas }) => Fase::Lista { faltan: cuantas, anunciada: true },
            (Fase::Lista { anunciada: false, .. }, Respuesta::No { .. }) => Fase::Charla,
            (Fase::Lista { anunciada: true, faltan }, Respuesta::Entrada { .. }) => {
                if faltan == 1 {
                    Fase::Charla
                } else {
                    Fase::Lista { faltan: faltan - 1, anunciada: true }
                }
            }
            (Fase::Pidiendo, Respuesta::Video { bytes, .. }) => Fase::Video { declarados: bytes, recibidos: 0 },
            (Fase::Pidiendo, Respuesta::No { .. }) => Fase::Charla,
            // La cabecera de la lamina la vuelve a leer el lector: es quien
            // lleva la cuenta de las `n` lineas que siguen, y una lamina de
            // cero lineas es una lamina entera.
            (Fase::Pidiendo, Respuesta::Lamina(_)) => {
                let mut lector = lamina::Lector::nuevo();
                if lector.empujar(linea).is_err() {
                    return self.cerrar(Rechazo::Orden);
                }
                if lector.completa() { Fase::Charla } else { Fase::Lamina(lector) }
            }
            _ => return self.cerrar(Rechazo::Orden),
        };
        self.fase = siguiente;
        Ok(r)
    }

    /// **Llegaron `n` bytes de video.** Solo en `Video`, y sin pasarse de lo
    /// declarado (o de `VIDEO_MAX` si es en vivo).
    pub fn bytes(&mut self, n: usize) -> Result<(), Rechazo> {
        let Fase::Video { declarados, recibidos } = self.fase else {
            return self.cerrar(Rechazo::Orden);
        };
        let total = recibidos.saturating_add(n as u64);
        let tope = if declarados == 0 { VIDEO_MAX } else { declarados };
        if total > tope {
            return self.cerrar(Rechazo::Largo);
        }
        self.fase = Fase::Video { declarados, recibidos: total };
        Ok(())
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
        let mut g = [0u8; 256];
        let n = escribir(&mut g, &Pedido::Pagina(b"https://example.com/a?b=1")).unwrap();
        assert_eq!(&g[..n], b"PAGINA https://example.com/a?b=1\n");
        assert_eq!(escribir(&mut g, &Pedido::Pagina(b"ftp://x")), Err(Rechazo::Url), "solo http(s)");
        assert_eq!(escribir(&mut g, &Pedido::Pagina(b"https://x y")), Err(Rechazo::Url), "sin espacios");
        assert_eq!(escribir(&mut g, &Pedido::Pagina(b"https://x\xC3\xB1")), Err(Rechazo::Url), "ASCII");
        let larga = [b'a'; URL_MAX + 1];
        assert_eq!(escribir(&mut g, &Pedido::Pagina(&larga)), Err(Rechazo::Url), "no cabe");
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

    fn charlando() -> Conversacion {
        let mut c = Conversacion::nueva();
        c.pedir(&Pedido::Hola).unwrap();
        c.oir(b"HOLA ANTENA/1 movil").unwrap();
        c
    }

    #[test]
    fn la_conversacion_buena_entera() {
        let mut c = charlando();
        c.pedir(&Pedido::Lista).unwrap();
        c.oir(b"LISTA 2").unwrap();
        c.oir(b"ENTRADA v1 uno").unwrap();
        assert_eq!(c.pedir(&Pedido::Pide(b"v1")), Err(Rechazo::Orden), "aun falta una entrada");
        c.oir(b"ENTRADA v2 dos").unwrap();
        assert_eq!(c.fase(), Fase::Charla);
        c.pedir(&Pedido::Pide(b"v2")).unwrap();
        c.oir(b"VIDEO 1000 mpeg1 640x360").unwrap();
        c.bytes(600).unwrap();
        c.bytes(400).unwrap();
        assert_eq!(c.bytes(1), Err(Rechazo::Largo), "ni un byte mas de lo declarado");
        assert_eq!(c.fase(), Fase::Cerrada);
    }

    /// *** Lineas VALIDAS en el momento equivocado: cierran.
    #[test]
    fn lo_que_no_se_pidio_cierra() {
        let mut c = charlando();
        assert_eq!(c.oir(b"VIDEO 0 mpeg1 640x360"), Err(Rechazo::Orden), "un video sin PIDE");
        assert_eq!(c.oir(b"HOLA ANTENA/1 otra"), Err(Rechazo::Cerrada), "y despues ya nada");
        assert_eq!(c.pedir(&Pedido::Lista), Err(Rechazo::Cerrada));

        let mut c = charlando();
        c.pedir(&Pedido::Lista).unwrap();
        c.oir(b"LISTA 1").unwrap();
        c.oir(b"ENTRADA v1 uno").unwrap();
        assert_eq!(c.oir(b"ENTRADA v2 de mas"), Err(Rechazo::Orden), "una entrada de mas");

        let mut c = Conversacion::nueva();
        assert_eq!(c.oir(b"HOLA ANTENA/1 movil"), Err(Rechazo::Orden), "un saludo que nadie pidio");
    }

    #[test]
    fn una_linea_rota_cierra_y_el_no_deja_seguir() {
        let mut c = charlando();
        c.pedir(&Pedido::Pide(b"v9")).unwrap();
        c.oir(b"NO no hay video con ese id").unwrap();
        assert_eq!(c.fase(), Fase::Charla, "un NO no cierra: se puede pedir otro");
        c.pedir(&Pedido::Lista).unwrap();
        assert_eq!(c.oir(b"LISTA 999"), Err(Rechazo::Numero));
        assert_eq!(c.fase(), Fase::Cerrada, "pero una linea rota si");
    }

    /// *** UNA PAGINA POR EL MISMO `PIDE`: la lamina entra linea a linea y la
    /// conversacion SIGUE, porque un clic pide la siguiente.
    #[test]
    fn una_pagina_llega_entera_y_la_charla_sigue() {
        let mut c = charlando();
        c.pedir(&Pedido::Pide(b"p1")).unwrap();
        assert_eq!(
            c.oir(b"LAMINA 640 100 2"),
            Ok(Respuesta::Lamina(lamina::Cabecera { ancho: 640, alto: 100, elementos: 2 }))
        );
        assert!(matches!(c.fase(), Fase::Lamina(_)));
        assert!(matches!(c.oir(b"CAJA 0 0 640 100 ffffff"), Ok(Respuesta::Elemento(lamina::Paso::Elemento(_)))));
        // Latin-1 dentro del texto: no pasa por `leer`, pasa por el lector.
        assert!(matches!(c.oir(b"TEXTO 8 8 1 000000 ma\xF1ana"), Ok(Respuesta::Elemento(_))));
        assert_eq!(c.fase(), Fase::Charla, "las dos llegaron: se vuelve a la charla");
        // Y se puede pedir otra: el clic.
        c.pedir(&Pedido::Pide(b"p2")).unwrap();
        assert_eq!(c.oir(b"NO no hay pagina con ese id"), Ok(Respuesta::No { motivo: b"no hay pagina con ese id" }));
        assert_eq!(c.fase(), Fase::Charla);
    }

    /// `PAGINA <url>` espera lo mismo que un `PIDE` de pagina: la antena navego
    /// y trae la lamina, o dice NO (sin navegador, sin red, o la url no carga).
    #[test]
    fn una_pagina_por_url_es_como_un_pide() {
        let mut c = charlando();
        c.pedir(&Pedido::Pagina(b"https://example.com")).unwrap();
        assert_eq!(c.fase(), Fase::Pidiendo);
        c.oir(b"LAMINA 640 100 1").unwrap();
        assert!(matches!(c.oir(b"CAJA 0 0 640 100 eeeeee"), Ok(Respuesta::Elemento(_))));
        assert_eq!(c.fase(), Fase::Charla);
        c.pedir(&Pedido::Pagina(b"https://no.existe")).unwrap();
        assert_eq!(c.oir(b"NO la antena no tiene navegador"), Ok(Respuesta::No { motivo: b"la antena no tiene navegador" }));
        assert_eq!(c.pedir(&Pedido::Pagina(b"javascript:alert(1)")), Err(Rechazo::Orden), "una url mala no se manda");
    }

    /// Las lineas de una lamina no cuentan para `LINEAS_MAX`: una pagina real
    /// son miles, y tiene su propio tope.
    #[test]
    fn una_lamina_larga_no_es_charla() {
        let mut c = charlando();
        c.pedir(&Pedido::Pide(b"p1")).unwrap();
        c.oir(b"LAMINA 640 4000 200").unwrap();
        for i in 0..200u32 {
            let linea = alloc_linea(i);
            assert!(c.oir(&linea).is_ok(), "linea {} de la lamina", i);
        }
        assert_eq!(c.fase(), Fase::Charla);
    }

    fn alloc_linea(i: u32) -> Vec<u8> {
        let mut v = b"CAJA 0 ".to_vec();
        v.extend_from_slice((i * 16).to_string().as_bytes());
        v.extend_from_slice(b" 8 8 000000");
        v
    }

    /// Una lamina rota cierra la conversacion ENTERA, como cualquier otra
    /// linea rota: una caja fuera de la pagina no se recorta en silencio.
    #[test]
    fn una_lamina_rota_cierra() {
        let mut c = charlando();
        c.pedir(&Pedido::Pide(b"p1")).unwrap();
        c.oir(b"LAMINA 640 100 2").unwrap();
        assert_eq!(c.oir(b"CAJA 700 0 10 10 000000"), Err(Rechazo::Fuera));
        assert_eq!(c.fase(), Fase::Cerrada);
        assert_eq!(c.oir(b"CAJA 0 0 10 10 000000"), Err(Rechazo::Cerrada));

        let mut c = charlando();
        assert_eq!(c.oir(b"LAMINA 640 100 1"), Err(Rechazo::Orden), "una lamina que nadie pidio");

        let mut c = charlando();
        c.pedir(&Pedido::Pide(b"p1")).unwrap();
        assert_eq!(c.oir(b"LAMINA 640 100 0"), Ok(Respuesta::Lamina(lamina::Cabecera { ancho: 640, alto: 100, elementos: 0 })));
        assert_eq!(c.fase(), Fase::Charla, "una lamina vacia es una lamina entera");
    }

    #[test]
    fn la_antena_que_no_para_de_hablar() {
        let mut c = charlando();
        let mut cerrada = false;
        for _ in 0..(LINEAS_MAX + 4) {
            if c.pedir(&Pedido::Pide(b"v1")).is_err() {
                cerrada = true;
                break;
            }
            if c.oir(b"NO otra vez no").is_err() {
                cerrada = true;
                break;
            }
        }
        assert!(cerrada, "mas de {LINEAS_MAX} lineas sin video cierran");
        assert_eq!(c.fase(), Fase::Cerrada);
    }

    #[test]
    fn un_video_en_vivo_tiene_techo() {
        let mut c = charlando();
        c.pedir(&Pedido::Pide(b"v1")).unwrap();
        c.oir(b"VIDEO 0 mpeg1 640x360").unwrap();
        c.bytes(usize::MAX >> 1).unwrap_err();
        assert_eq!(c.fase(), Fase::Cerrada);
    }
}
