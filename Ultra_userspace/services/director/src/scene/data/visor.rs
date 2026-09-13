//! **EL VISOR** -- ver lo que hay DENTRO de un fichero de ESTRATOS.
//!
//! [consumo] NADA      no corre en reposo por su cuenta: pinta cuando el
//!                     compositor se lo pide, y el compositor solo pinta si
//!                     algo cambio (L6h)
//!
//! === Por que esto es solo interfaz ===
//!
//! Porque leer ya funcionaba y nadie lo estaba usando. `Archivo::leer_de`
//! resuelve ESTRATOS antes que FAT32 desde `obj/file.rs`, asi que el contenido
//! de un fichero del volumen se pide con la MISMA llamada que uno de FAT32.
//! Cero lineas de kernel, cero operaciones nuevas en el ABI.
//!
//! Lo que faltaba era pintarlo. Hasta hoy ESTRATOS **escribia y nadie leia**:
//! `guarda` metia un fichero y no habia forma de mirarlo desde la ventana, asi
//! que ENTRAR sobre un fichero no hacia nada y el doble clic tampoco.
//!
//! === El tope, dicho con su numero ===
//!
//! [`TOPE`] son 64 KiB. Un fichero mas grande **no se abre a medias**: se dice
//! cuanto mide y cuanto cabe. Es la misma regla que `cursor::verify` con su
//! buffer de firma -- *un limite propio se confiesa, no se disfraza de fallo del
//! disco*, y ensenar los primeros 64 KiB de un fichero de cuatro MiB sin avisar
//! es contar una verdad recortada.
//!
//! ** Y el tope no es capricho: el kernel trae el fichero ENTERO a RAM
//! (`obj/estratos.rs` lo explica y acaba de re-decidirse el 20-08). Una pantalla
//! de texto son dos KiB; pedir cuatro MiB para ensenar dos es justo lo que ese
//! fichero deja anotado como la nota que vencera el dia que esto crezca.
//!
//! === Donde se ve ===
//!
//! **En el sitio de la rejilla**, y ESC vuelve. Entrar en un fichero es como
//! entrar en una carpeta: no pide ventana nueva, ni pestana, ni aprender un
//! gesto que no se usa en ningun otro sitio.

use bmo_userland as bmo;

use super::*;
use crate::scene::zonas::Zona;

/// Lo mas grande que este visor abre. Ver la cabecera.
pub(crate) const TOPE: u64 = 64 * 1024;

/// **El bloque donde aterriza el fichero que se esta mirando.**
///
/// Se pide UNA vez y se reusa. Es SUYO y no el de la consola a proposito: el de
/// `guarda` se llena y se vacia dentro de una orden, y este tiene que seguir
/// siendo valido mientras la ventana lo pinta. Compartirlo significaria que
/// volcar la consola te cambia bajo los pies el fichero que estas leyendo.
static mut CONTENIDO: Option<bmo::Memoria> = None;

fn bloque() -> Option<&'static bmo::Memoria> {
    let slot = core::ptr::addr_of_mut!(CONTENIDO);
    unsafe {
        if (*slot).is_none() {
            *slot = bmo::Memoria::request(TOPE);
        }
        (*slot).as_ref()
    }
}

// == ** LAS IMAGENES (2026-09-13) ============================================
//
// Eddi: *"dale con el visor de imagenes"*. BICO, BMP y QOI se descifran en
// `bmo-imagen` --probado en el anfitrion contra ficheros ROTOS-- y aqui solo se
// traen los bytes y se pintan. El visor de texto no cambia.
//
// Dos bloques mas, pedidos la PRIMERA vez que se abre una imagen y no antes:
// quien no mira imagenes no paga 8 MiB.

/// Lo mas grande que se abre como imagen.
const IMG_TOPE: u64 = 4 * 1024 * 1024;
/// Los pixeles: `LADO_MAX` al cuadrado, cuatro bytes cada uno.
const IMG_PIXELES: u64 = (bmo_imagen::LADO_MAX as u64) * (bmo_imagen::LADO_MAX as u64) * 4;

static mut IMG_FICHERO: Option<bmo::Memoria> = None;
static mut IMG_BUFER: Option<bmo::Memoria> = None;

fn pedido(slot: *mut Option<bmo::Memoria>, bytes: u64) -> Option<&'static bmo::Memoria> {
    unsafe {
        if (*slot).is_none() {
            *slot = bmo::Memoria::request(bytes);
        }
        (*slot).as_ref()
    }
}

/// Lo que se esta mirando, o nada.
pub(crate) struct Visor {
    /// Si es una imagen que se pudo descifrar: sus medidas.
    imagen: Option<bmo_imagen::Medidas>,
    /// Si no se pudo abrir como imagen: por que.
    fallo: Option<&'static str>,
    pub(crate) abierto: bool,
    nombre: [u8; 64],
    nombre_len: usize,
    /// Lo que MIDE el fichero, que no siempre es lo que se trajo.
    mide: u64,
    leidos: usize,
    /// Primera linea visible. El scroll del visor.
    pub(crate) desde: usize,
}

impl Visor {
    pub(crate) const VACIO: Self = Self {
        imagen: None,
        fallo: None,
        abierto: false,
        nombre: [0; 64],
        nombre_len: 0,
        mide: 0,
        leidos: 0,
        desde: 0,
    };

    pub(crate) fn nombre(&self) -> &[u8] {
        &self.nombre[..self.nombre_len]
    }

    /// **Abre `ruta` y se trae su contenido.** `false` si no se pudo.
    ///
    /// El motivo del `false` no viaja: se pinta al abrir --el tope, o que no se
    /// pudo leer-- y quien llama solo necesita saber si hay algo que ensenar.
    pub(crate) fn abrir(&mut self, ruta: &[u8], nombre: &[u8]) -> bool {
        self.abierto = false;
        self.desde = 0;
        self.leidos = 0;
        self.mide = 0;
        self.imagen = None;
        self.fallo = None;
        let k = nombre.len().min(self.nombre.len());
        self.nombre[..k].copy_from_slice(&nombre[..k]);
        self.nombre_len = k;

        let Ok(a) = bmo::Archivo::leer_de(ruta) else {
            return false;
        };
        self.mide = a.tamano();
        // ** SE ABRE IGUAL cuando no cabe: el visor tiene que poder DECIR que no
        // cabe, y para eso hace falta que la vista exista. Lo que no hace es
        // leer ni pintar medio fichero.
        self.abierto = true;
        // Una imagen se reconoce por lo que la TABLA dice que es, no probando a
        // descifrar cualquier fichero: un texto que empezara por "BM" no es un
        // mapa de bits.
        use crate::scene::asociaciones::{de, Clase};
        if de(nombre).0 == Clase::Imagen {
            self.abrir_imagen(&a);
            return true;
        }
        if self.mide > TOPE {
            return true;
        }
        let Some(m) = bloque() else {
            return true;
        };
        // *** **DEL DISCO AL BLOQUE, DE UNA LLAMADA** (06-09).
        //
        // Esto era `a.read(dst)` sobre un slice fabricado con
        // `from_raw_parts_mut`, y `read` mueve **siete bytes por syscall**: un
        // fichero de 64 KiB --el TOPE de aqui-- son **9.363 puertas**, a 969
        // ciclos cada una. Ahora es UNA.
        //
        // ** Y el bloque YA ESTABA: `bloque()` lo pide al kernel desde el primer
        // dia. Lo unico que faltaba era decirle al kernel *"escribe ahi"* en vez
        // de traerse los bytes de siete en siete para copiarlos nosotros al
        // mismo sitio. La operacion existia en el ABI y la cara de Rust no la
        // llamaba -- ver `Archivo::leer_en`.
        //
        // [!] Y se lleva por delante un `unsafe`: ya no hay que fabricar un
        // slice sobre `m.base()` para leer. El kernel escribe en el bloque por
        // su HANDLE, que es justo lo que hace innecesario tocar el puntero.
        self.leidos = a.leer_en(m, 0, self.mide) as usize;
        true
    }

    /// Trae los bytes y los descifra. Lo que falle queda en `fallo`, con motivo.
    fn abrir_imagen(&mut self, a: &bmo::Archivo) {
        if self.mide > IMG_TOPE {
            self.fallo = Some("la imagen pasa de 4 MiB: no se abre a medias");
            return;
        }
        let (Some(fichero), Some(bufer)) = (
            pedido(core::ptr::addr_of_mut!(IMG_FICHERO), IMG_TOPE),
            pedido(core::ptr::addr_of_mut!(IMG_BUFER), IMG_PIXELES),
        ) else {
            self.fallo = Some("sin memoria para la imagen");
            return;
        };
        let n = a.leer_en(fichero, 0, self.mide) as usize;
        // SAFETY: `n` bytes que el kernel acaba de escribir en un bloque de este
        // proceso; el bufer de pixeles mide IMG_PIXELES y el base es de pagina,
        // o sea alineado a 4.
        let bytes = unsafe { core::slice::from_raw_parts(fichero.base() as *const u8, n) };
        let pixeles = unsafe {
            core::slice::from_raw_parts_mut(bufer.base() as *mut u32, (IMG_PIXELES / 4) as usize)
        };
        match bmo_imagen::decodificar(bytes, pixeles) {
            Ok(m) => self.imagen = Some(m),
            Err(e) => self.fallo = Some(e.motivo()),
        }
    }

    pub(crate) fn cerrar(&mut self) {
        self.abierto = false;
        self.desde = 0;
    }

    /// Mueve el scroll. `true` si algo cambio y hay que repintar.
    pub(crate) fn mover(&mut self, delta: isize, caben: usize) -> bool {
        let total = self.lineas();
        // Nunca se puede bajar mas alla de la ultima pantalla: si no, el scroll
        // sigue contando y la vista se queda en blanco sin decir por que.
        let tope = total.saturating_sub(caben);
        let antes = self.desde;
        self.desde = if delta < 0 {
            self.desde.saturating_sub((-delta) as usize)
        } else {
            (self.desde + delta as usize).min(tope)
        };
        self.desde != antes
    }

    /// Cuantas lineas tiene lo que se trajo.
    fn lineas(&self) -> usize {
        let Some(datos) = self.datos() else { return 0 };
        let mut n = 1usize;
        for b in datos {
            if *b == b'\n' {
                n += 1;
            }
        }
        n
    }

    /// Lo leido, o nada si no cabia.
    fn datos(&self) -> Option<&'static [u8]> {
        if self.leidos == 0 {
            return None;
        }
        let m = bloque()?;
        // SAFETY: `leidos` son bytes que este mismo proceso acaba de escribir
        // dentro del bloque, y el bloque vive hasta que el proceso muere.
        Some(unsafe { core::slice::from_raw_parts(m.base() as *const u8, self.leidos) })
    }
}

/// **La imagen, ajustada y centrada**, sobre un tablero que deja ver lo
/// transparente.
///
/// ```text
///    cabe         se AMPLIA por un entero (hasta x8): pixeles nitidos, sin
///                 inventar colores que la imagen no tiene
///    no cabe      se REDUCE tomando una muestra cada `d`: la vista entera,
///                 y la cabecera sigue diciendo las medidas de verdad
/// ```
///
/// ** Se marca la caja UNA vez y los puntos van sin marcar: marcar por pixel
/// copia 272 bytes de contabilidad cada vez (ver `Pantalla::punto_ya_marcado`).
fn pintar_imagen(p: &bmo::Pantalla, z: &Zona, y: u32, m: &bmo_imagen::Medidas) {
    let Some(bufer) = (unsafe { (*core::ptr::addr_of!(IMG_BUFER)).as_ref() }) else { return };
    let (w, h) = (m.ancho, m.alto);
    let area_w = z.w.saturating_sub(16).max(1);
    let area_h = z.abajo().saturating_sub(y + 8).max(1);
    let (num, den) = if w <= area_w && h <= area_h {
        ((area_w / w).min(area_h / h).clamp(1, 8), 1)
    } else {
        (1, ((w + area_w - 1) / area_w).max((h + area_h - 1) / area_h))
    };
    let (dw, dh) = (w * num / den, h * num / den);
    let x0 = z.x + (z.w - dw) / 2;
    let y0 = y + (area_h - dh) / 2;

    // El tablero: lo que es transparente tiene que VERSE transparente. Sobre un
    // fondo liso un pixel saltado y uno negro son iguales.
    const CUADRO: u32 = 8;
    let mut ty = 0;
    while ty < dh {
        let mut tx = 0;
        while tx < dw {
            let claro = ((tx / CUADRO) + (ty / CUADRO)) % 2 == 0;
            p.rect(x0 + tx, y0 + ty, CUADRO.min(dw - tx), CUADRO.min(dh - ty), if claro { 0x0030_3438 } else { 0x0024_272B });
            tx += CUADRO;
        }
        ty += CUADRO;
    }

    p.marcar(x0, y0, dw, dh);
    // SAFETY: el bufer mide IMG_PIXELES y `decodificar` escribio `w*h` en el.
    let px = unsafe { core::slice::from_raw_parts(bufer.base() as *const u32, (w * h) as usize) };
    for dy in 0..dh {
        let fila = ((dy * den / num) * w) as usize;
        for dx in 0..dw {
            let c = px[fila + (dx * den / num) as usize];
            if c >> 24 != 0 {
                p.punto_ya_marcado(x0 + dx, y0 + dy, c & 0x00FF_FFFF);
            }
        }
    }
    // Un marco fino: separa la imagen del tablero del fondo de la ventana.
    p.rect(x0.saturating_sub(1), y0.saturating_sub(1), dw + 2, 1, DATA_EDGE);
    p.rect(x0.saturating_sub(1), y0 + dh, dw + 2, 1, DATA_EDGE);
}

/// **Pinta el visor en `z`.** Va donde iria la rejilla.
pub(crate) fn paint(p: &bmo::Pantalla, z: &Zona, v: &Visor) {
    if !z.hay() || !v.abierto {
        return;
    }
    // La zona se deja ENTERA: la regla que costo el amasijo de letras del 20-08
    // en la consola. Quien pinta una zona la deja entera, fondo incluido.
    p.rect(z.x, z.y, z.w, z.h, DATA_BG);

    let alto = bmo::GLIFO_ALTO;
    let mut y = z.y + 4;
    let mut buf = [0u8; 96];
    // `decimal` pide un buffer de diez: el suyo, aparte del de las lineas.
    let mut num = [0u8; 10];

    // La cabecera: el nombre y lo que mide. Sin esto, un fichero vacio y uno
    // que no se pudo leer se ven igual.
    let x = p.texto(z.x + 4, y, "ver ", INK_DIM);
    let x = p.texto_bytes(x, y, v.nombre(), DATA_TITLE);
    let n = crate::text::decimal(v.mide, &mut num);
    let x = p.texto(x + bmo::GLIFO_ANCHO, y, "  ", INK_DIM);
    let x = p.texto_bytes(x, y, &num[..n], INK_DIM);
    let x = p.texto(x, y, " B", INK_DIM);
    // Una imagen dice sus medidas y su formato en la cabecera.
    let x = match v.imagen {
        Some(m) => {
            let x = p.texto(x, y, "   ", INK_DIM);
            let n = crate::text::decimal(m.ancho as u64, &mut num);
            let x = p.texto_bytes(x, y, &num[..n], INK);
            let x = p.texto(x, y, "x", INK_DIM);
            let n = crate::text::decimal(m.alto as u64, &mut num);
            let x = p.texto_bytes(x, y, &num[..n], INK);
            p.texto(x + bmo::GLIFO_ANCHO, y, m.formato.nombre(), DATA_TITLE)
        }
        None => x,
    };
    p.texto(x, y, "   ESC vuelve", INK_DIM);
    y += alto + 4;
    p.rect(z.x, y, z.w, 1, DATA_EDGE);
    y += 4;

    if let Some(motivo) = v.fallo {
        p.texto(z.x + 4, y, "no se pudo abrir como imagen:", INK_BAD);
        p.texto(z.x + 4, y + alto + 2, motivo, INK_DIM);
        return;
    }
    if let Some(m) = v.imagen {
        pintar_imagen(p, z, y, &m);
        return;
    }

    if v.mide > TOPE {
        p.texto(z.x + 4, y, "no cabe en el visor.", INK_BAD);
        let n = crate::text::decimal(TOPE, &mut num);
        let x = p.texto(z.x + 4, y + alto + 2, "el tope son ", INK_DIM);
        let x = p.texto_bytes(x, y + alto + 2, &num[..n], INK_DIM);
        p.texto(x, y + alto + 2, " bytes, y es NUESTRO:", INK_DIM);
        p.texto(z.x + 4, y + 2 * (alto + 2), "el kernel trae el fichero entero a RAM.", INK_DIM);
        return;
    }
    let Some(datos) = v.datos() else {
        p.texto(z.x + 4, y, "vacio, o no se pudo leer. el motivo esta en F11.", INK_DIM);
        return;
    };

    // Las columnas que caben, contando el margen de los dos lados.
    let cols = ((z.w.saturating_sub(8)) / bmo::GLIFO_ANCHO) as usize;
    let caben = ((z.abajo().saturating_sub(y)) / alto) as usize;

    let mut linea = 0usize;
    let mut pintadas = 0usize;
    let mut i = 0usize;
    while i < datos.len() && pintadas < caben {
        // El final de esta linea.
        let mut fin = i;
        while fin < datos.len() && datos[fin] != b'\n' {
            fin += 1;
        }
        if linea >= v.desde {
            let hasta = fin.min(i + cols.min(buf.len()));
            let mut k = 0usize;
            for b in &datos[i..hasta] {
                // ** LO QUE NO SE PUEDE PINTAR SE PINTA COMO UN PUNTO, y no se
                // manda tal cual: un byte de control convertido en glifo hace
                // que un fichero binario se vea como una explosion de simbolos
                // y que el que mira crea que el fichero esta roto.
                buf[k] = if *b >= 0x20 && *b < 0x7F { *b } else { b'.' };
                k += 1;
            }
            p.texto_bytes(z.x + 4, y, &buf[..k], INK);
            y += alto;
            pintadas += 1;
        }
        linea += 1;
        i = fin + 1;
    }
}
