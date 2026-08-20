//! **EL VISOR** -- ver lo que hay DENTRO de un fichero de ESTRATOS.
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

/// Lo que se esta mirando, o nada.
pub(crate) struct Visor {
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
        if self.mide > TOPE {
            return true;
        }
        let Some(m) = bloque() else {
            return true;
        };
        // SAFETY: el bloque son TOPE bytes y `mide` no lo pasa -- se acaba de
        // comprobar. Es memoria de este proceso y esta mapeada.
        let dst = unsafe { core::slice::from_raw_parts_mut(m.base(), self.mide as usize) };
        self.leidos = a.read(dst);
        true
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
    p.texto(x, y, " B   ESC vuelve", INK_DIM);
    y += alto + 4;
    p.rect(z.x, y, z.w, 1, DATA_EDGE);
    y += 4;

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
