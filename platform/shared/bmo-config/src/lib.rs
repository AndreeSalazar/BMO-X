//! **bmo-config** -- el aspecto del DIRECTOR, en un fichero que se edita a mano.
//!
//! generacion: nieto -- no sabe quien la llama ni de donde salio el texto;
//! recibe bytes y un `Estilo` con los valores de partida.
//!
//! ## Por que existe (2026-09-13)
//!
//! Eddi: *"puede haber su propia configuracion o modo editor?"*. Hasta hoy los
//! colores salian de `tema.maqueta` y se fijaban AL COMPILAR: cambiar el acento
//! era reconstruir el sistema. Esto es la mitad de leer: `sys/director.cfg`.
//!
//! ## El formato, entero
//!
//! ```text
//!    # comentario (tambien ;)
//!    acento         = #60A5FA
//!    barra_flotante = si
//!    barra_hueco    = 6          # de 0 a 12
//! ```
//!
//! ## ** Lo que NO hace un fichero roto
//!
//! No deja el escritorio sin arrancar ni a medio pintar. Cada linea que no se
//! entiende se APUNTA --con su numero y su motivo-- y la clave se queda con el
//! valor que tenia. Un escritorio que se niega a arrancar porque alguien se dejo
//! un `#` es un escritorio que no arranca el dia que mas falta hace.

#![cfg_attr(not(test), no_std)]

/// Lo que se puede cambiar. Quien llama pone los valores de partida (los de
/// `tema.maqueta`) y el fichero pisa los que diga.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Estilo {
    pub acento: u32,
    pub fondo_arriba: u32,
    pub fondo_abajo: u32,
    pub barra_fondo: u32,
    pub barra_borde: u32,
    /// Barra en forma de pastilla, separada de los bordes. `no` = la de siempre.
    pub barra_flotante: bool,
    /// Pixeles de hueco entre la pastilla y el borde de la pantalla.
    pub barra_hueco: u32,
    pub reloj: bool,
    pub vatios: bool,
    pub memoria: bool,
    pub cpu: bool,
    /// Una FOTO de fondo (BICO, BMP o QOI) que tapa el degradado. Vacia = el
    /// degradado de siempre.
    pub fondo_imagen: Ruta,
}

/// Lo mas larga que puede ser una ruta del fichero.
pub const RUTA_MAX: usize = 40;

/// Una ruta corta y sin espacios. Sin montones: el DIRECTOR es `no_std`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Ruta {
    b: [u8; RUTA_MAX],
    n: u8,
}

impl Ruta {
    pub const VACIA: Self = Self { b: [0; RUTA_MAX], n: 0 };

    pub fn vacia(&self) -> bool {
        self.n == 0
    }

    pub fn bytes(&self) -> &[u8] {
        &self.b[..self.n as usize]
    }

    /// `None` si es demasiado larga o lleva algo que no es ASCII visible.
    pub fn de(v: &[u8]) -> Option<Self> {
        if v.len() > RUTA_MAX || v.iter().any(|&c| c <= b' ' || c >= 0x7F) {
            return None;
        }
        let mut r = Self::VACIA;
        r.b[..v.len()].copy_from_slice(v);
        r.n = v.len() as u8;
        Some(r)
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Motivo {
    /// La linea no tiene `=`.
    SinIgual,
    /// La clave no es ninguna de las que existen.
    ClaveDesconocida,
    /// Se esperaba `#RRGGBB`.
    Color,
    /// Se esperaba un numero.
    Numero,
    /// Un numero fuera de su rango.
    FueraDeRango,
    /// Se esperaba `si` o `no`.
    SiNo,
    /// Una ruta de mas de `RUTA_MAX` o con caracteres raros.
    Ruta,
}

impl Motivo {
    pub fn texto(self) -> &'static str {
        match self {
            Motivo::SinIgual => "falta el `=`",
            Motivo::ClaveDesconocida => "no conozco esa clave",
            Motivo::Color => "un color va como #RRGGBB",
            Motivo::Numero => "ahi va un numero",
            Motivo::FueraDeRango => "numero fuera de su rango",
            Motivo::SiNo => "ahi va `si` o `no`",
            Motivo::Ruta => "una ruta va sin espacios y con 40 letras como mucho",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Fallo {
    pub linea: u32,
    pub motivo: Motivo,
}

/// Cuantos fallos se guardan. Un fichero con mas de ocho lineas malas no
/// necesita la novena para saber que hay que mirarlo.
pub const MAX_FALLOS: usize = 8;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Informe {
    pub fallos: [Fallo; MAX_FALLOS],
    pub n: usize,
    /// Hubo mas fallos de los que caben.
    pub recortado: bool,
    /// Cuantas claves se aplicaron bien.
    pub aplicadas: usize,
}

impl Informe {
    pub const VACIO: Self = Self {
        fallos: [Fallo { linea: 0, motivo: Motivo::SinIgual }; MAX_FALLOS],
        n: 0,
        recortado: false,
        aplicadas: 0,
    };

    fn apuntar(&mut self, linea: u32, motivo: Motivo) {
        if self.n == MAX_FALLOS {
            self.recortado = true;
        } else {
            self.fallos[self.n] = Fallo { linea, motivo };
            self.n += 1;
        }
    }

    pub fn fallos(&self) -> &[Fallo] {
        &self.fallos[..self.n]
    }
}

fn recortar(mut s: &[u8]) -> &[u8] {
    while let [b' ' | b'\t' | b'\r', resto @ ..] = s {
        s = resto;
    }
    while let [resto @ .., b' ' | b'\t' | b'\r'] = s {
        s = resto;
    }
    s
}

fn color(v: &[u8]) -> Option<u32> {
    let hex = match v {
        [b'#', h @ ..] => h,
        [b'0', b'x' | b'X', h @ ..] => h,
        _ => return None,
    };
    if hex.len() != 6 {
        return None;
    }
    let mut c = 0u32;
    for &b in hex {
        let d = (b as char).to_digit(16)?;
        c = c * 16 + d;
    }
    Some(c)
}

fn numero(v: &[u8]) -> Option<u32> {
    if v.is_empty() || v.len() > 6 {
        return None;
    }
    let mut n = 0u32;
    for &b in v {
        if !b.is_ascii_digit() {
            return None;
        }
        n = n * 10 + (b - b'0') as u32;
    }
    Some(n)
}

fn si_no(v: &[u8]) -> Option<bool> {
    match v {
        b"si" | b"SI" | b"Si" => Some(true),
        b"no" | b"NO" | b"No" => Some(false),
        _ => None,
    }
}

impl Estilo {
    /// **Aplica `texto` encima de lo que hay.** Lo que no se entiende se apunta y
    /// se salta.
    pub fn aplicar(&mut self, texto: &[u8]) -> Informe {
        let mut inf = Informe::VACIO;
        for (i, cruda) in texto.split(|&b| b == b'\n').enumerate() {
            let linea = i as u32 + 1;
            let l = recortar(cruda);
            // Una linea que EMPIEZA por `#` o `;` es un comentario entero.
            if l.is_empty() || l[0] == b'#' || l[0] == b';' {
                continue;
            }
            let Some(igual) = l.iter().position(|&b| b == b'=') else {
                inf.apuntar(linea, Motivo::SinIgual);
                continue;
            };
            let clave = recortar(&l[..igual]);
            // ** El valor es la PRIMERA PALABRA. Ningun valor de este formato
            // lleva espacios, asi que lo de detras es un comentario -- y por
            // eso `acento = #8B5CF6  # morado` no confunde el color con la nota,
            // que es justo lo que haria buscar el primer `#` de la linea.
            let resto = recortar(&l[igual + 1..]);
            let fin = resto.iter().position(|&b| b == b' ' || b == b'\t').unwrap_or(resto.len());
            let valor = &resto[..fin];
            match self.poner(clave, valor) {
                Ok(()) => inf.aplicadas += 1,
                Err(m) => inf.apuntar(linea, m),
            }
        }
        inf
    }

    /// **El mismo estilo, como texto que [`Estilo::aplicar`] vuelve a leer igual.**
    /// Lo usa el editor de aspecto para GUARDAR. Devuelve los bytes escritos; lo
    /// que no quepa en `dst` se corta, y quien llama tiene que darle sitio (con
    /// 1 KiB sobra).
    pub fn escribir(&self, dst: &mut [u8]) -> usize {
        struct W<'a> {
            d: &'a mut [u8],
            n: usize,
        }
        impl W<'_> {
            fn pega(&mut self, s: &[u8]) {
                for &b in s {
                    if self.n < self.d.len() {
                        self.d[self.n] = b;
                        self.n += 1;
                    }
                }
            }
            fn color(&mut self, clave: &[u8], c: u32) {
                self.pega(clave);
                self.pega(b" = #");
                for i in (0..6).rev() {
                    self.pega(&[b"0123456789ABCDEF"[((c >> (i * 4)) & 0xF) as usize]]);
                }
                self.pega(b"\n");
            }
            fn si_no(&mut self, clave: &[u8], v: bool) {
                self.pega(clave);
                self.pega(if v { b" = si\n" } else { b" = no\n" });
            }
        }
        let mut w = W { d: dst, n: 0 };
        w.pega(b"# director.cfg -- guardado por el editor de aspecto (`aspecto` en Ejecutar)\n");
        w.color(b"fondo_arriba", self.fondo_arriba);
        w.color(b"fondo_abajo", self.fondo_abajo);
        w.color(b"acento", self.acento);
        w.color(b"barra_fondo", self.barra_fondo);
        w.color(b"barra_borde", self.barra_borde);
        w.si_no(b"barra_flotante", self.barra_flotante);
        w.pega(b"barra_hueco = ");
        let h = self.barra_hueco.min(99);
        if h >= 10 {
            w.pega(&[b'0' + (h / 10) as u8]);
        }
        w.pega(&[b'0' + (h % 10) as u8, b'\n']);
        w.si_no(b"cpu", self.cpu);
        w.si_no(b"memoria", self.memoria);
        w.si_no(b"vatios", self.vatios);
        w.si_no(b"reloj", self.reloj);
        w.pega(b"fondo_imagen = ");
        w.pega(if self.fondo_imagen.vacia() { b"no" } else { self.fondo_imagen.bytes() });
        w.pega(b"\n");
        w.n
    }

    fn poner(&mut self, clave: &[u8], v: &[u8]) -> Result<(), Motivo> {
        let col = || color(v).ok_or(Motivo::Color);
        let sn = || si_no(v).ok_or(Motivo::SiNo);
        match clave {
            b"acento" => self.acento = col()?,
            b"fondo_arriba" => self.fondo_arriba = col()?,
            b"fondo_abajo" => self.fondo_abajo = col()?,
            b"barra_fondo" => self.barra_fondo = col()?,
            b"barra_borde" => self.barra_borde = col()?,
            b"barra_flotante" => self.barra_flotante = sn()?,
            b"barra_hueco" => {
                let n = numero(v).ok_or(Motivo::Numero)?;
                if n > 12 {
                    return Err(Motivo::FueraDeRango);
                }
                self.barra_hueco = n;
            }
            b"reloj" => self.reloj = sn()?,
            b"vatios" => self.vatios = sn()?,
            b"memoria" => self.memoria = sn()?,
            b"cpu" => self.cpu = sn()?,
            // `no` devuelve el degradado: es la forma de quitar la foto sin
            // borrar la linea.
            b"fondo_imagen" => {
                self.fondo_imagen = if v == b"no" { Ruta::VACIA } else { Ruta::de(v).ok_or(Motivo::Ruta)? }
            }
            _ => return Err(Motivo::ClaveDesconocida),
        }
        Ok(())
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    const BASE: Estilo = Estilo {
        acento: 0x0060_A5FA,
        fondo_arriba: 0x001B_2233,
        fondo_abajo: 0x000C_0F17,
        barra_fondo: 0x000F_131D,
        barra_borde: 0x0026_2F42,
        barra_flotante: true,
        barra_hueco: 6,
        reloj: true,
        vatios: true,
        memoria: true,
        cpu: true,
        fondo_imagen: Ruta::VACIA,
    };

    #[test]
    fn la_foto_de_fondo_se_pone_y_se_quita() {
        let mut e = BASE;
        let inf = e.aplicar(b"fondo_imagen = sys/fondo.qoi   # la del atardecer\n");
        assert_eq!(inf.fallos(), &[]);
        assert_eq!(e.fondo_imagen.bytes(), b"sys/fondo.qoi");
        e.aplicar(b"fondo_imagen = no\n");
        assert!(e.fondo_imagen.vacia());
        let largo = [b'a'; RUTA_MAX + 1];
        let mut linea = b"fondo_imagen = ".to_vec();
        linea.extend_from_slice(&largo);
        let inf = e.aplicar(&linea);
        assert_eq!(inf.fallos()[0].motivo, Motivo::Ruta);
        assert!(e.fondo_imagen.vacia(), "una ruta mala no pisa la que habia");
    }

    #[test]
    fn un_fichero_bueno_se_aplica_entero() {
        let mut e = BASE;
        let inf = e.aplicar(
            b"# tema morado\n\
              acento = #8B5CF6\r\n\
              barra_flotante = no\n\
              barra_hueco    = 10   # mas aire\n\
              reloj = si ; comentario tambien asi\n\
              \n",
        );
        assert_eq!(inf.fallos(), &[]);
        assert_eq!(inf.aplicadas, 4);
        assert_eq!(e.acento, 0x008B_5CF6);
        assert!(!e.barra_flotante);
        assert_eq!(e.barra_hueco, 10);
        assert!(e.reloj);
    }

    #[test]
    fn un_color_con_comentario_detras() {
        let mut e = BASE;
        let inf = e.aplicar(b"barra_fondo = #101018  # casi negro\n");
        assert_eq!(inf.fallos(), &[]);
        assert_eq!(e.barra_fondo, 0x0010_1018);
    }

    /// *** UN FICHERO ROTO NO ROMPE NADA: cada linea mala dice su numero y su
    /// motivo, y su clave se queda con lo que tenia.
    #[test]
    fn cada_linea_mala_dice_donde_y_por_que() {
        let mut e = BASE;
        let inf = e.aplicar(
            b"acento = azul\n\
              barra_hueco = 99\n\
              barra_hueco = seis\n\
              barra_flotante = quizas\n\
              brillo = 3\n\
              esto no lleva igual\n\
              vatios = no\n",
        );
        let f: Vec<(u32, Motivo)> = inf.fallos().iter().map(|f| (f.linea, f.motivo)).collect();
        assert_eq!(
            f,
            vec![
                (1, Motivo::Color),
                (2, Motivo::FueraDeRango),
                (3, Motivo::Numero),
                (4, Motivo::SiNo),
                (5, Motivo::ClaveDesconocida),
                (6, Motivo::SinIgual),
            ]
        );
        assert_eq!(e.acento, BASE.acento, "un valor malo NO pisa el bueno");
        assert_eq!(e.barra_hueco, 6);
        assert!(!e.vatios, "y lo bueno de despues SI se aplica");
        assert_eq!(inf.aplicadas, 1);
    }

    #[test]
    fn mas_de_ocho_fallos_se_dice_recortado() {
        let mut e = BASE;
        let inf = e.aplicar(&b"x\n".repeat(20));
        assert_eq!(inf.n, MAX_FALLOS);
        assert!(inf.recortado);
    }

    #[test]
    fn colores_mal_escritos() {
        for malo in [&b"#12345"[..], b"#1234567", b"#GG0000", b"123456", b"#"] {
            assert_eq!(color(malo), None, "{:?}", core::str::from_utf8(malo));
        }
        assert_eq!(color(b"0xFFaa00"), Some(0x00FF_AA00));
    }

    /// ** LO QUE GUARDA EL EDITOR SE VUELVE A LEER IGUAL: sin esto, guardar el
    /// aspecto y reiniciar podria devolver otro escritorio.
    #[test]
    fn lo_escrito_se_lee_igual() {
        let mut e = BASE;
        e.acento = 0x00C0_84FC;
        e.barra_flotante = false;
        e.barra_hueco = 11;
        e.vatios = false;
        e.fondo_imagen = Ruta::de(b"sys/fondo.qoi").unwrap();
        let mut buf = [0u8; 1024];
        let n = e.escribir(&mut buf);
        let mut leido = Estilo { acento: 0, barra_hueco: 0, ..BASE };
        let inf = leido.aplicar(&buf[..n]);
        assert_eq!(inf.fallos(), &[], "{}", String::from_utf8_lossy(&buf[..n]));
        assert_eq!(leido, e);
        assert_eq!(inf.aplicadas, 12, "las doce claves, todas");
    }

    #[test]
    fn un_fichero_vacio_o_binario_no_cambia_nada() {
        let mut e = BASE;
        assert_eq!(e.aplicar(b"").n, 0);
        let inf = e.aplicar(&[0u8, 0xFF, 0x10, b'\n', 0x80]);
        assert_eq!(e, BASE);
        assert!(inf.n > 0);
    }

    /// ** HOSTILE PASS (2026-09-17): the style file is text anyone can edit.
    /// Checked: nothing panics, whatever the file says.
    #[test]
    fn hostile_style_files_never_panic() {
        let good: &[u8] = b"fondo_imagen = sys/fondo.qoi   # la del atardecer";
        bmo_hostile::attack("estilo", bmo_hostile::DEFAULT_SEED, 30_000, &[good], 1024, |x| {
            let mut e = BASE;
            let _ = e.aplicar(x);
            let _ = Ruta::de(x);
        });
    }
}
