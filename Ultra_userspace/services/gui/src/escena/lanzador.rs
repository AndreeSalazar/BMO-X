//! **EL LANZADOR: dar clic y ya.**
//!
//! Una rejilla de iconos en el escritorio, uno por cada `.bex` que haya en
//! `apps\`. Se pulsa y el programa arranca.
//!
//! ## Por que esto NO es un acceso directo
//!
//! En Windows el icono del escritorio es un `.lnk`: **un fichero aparte que
//! APUNTA** al programa. En Linux un `.desktop` con el nombre, el icono y el
//! comando escritos dentro. Los dos son punteros, y los dos se rompen igual --
//! mueves el programa y te queda un icono que dice "no se encuentra el
//! destino".
//!
//! Aqui no hay nada que apuntar. **El icono vive DENTRO del `.bex`**, como un
//! recurso mas de su paquete (`SectionKind::Resources`, ver
//! `bmo_abi::bef::recursos`). La app trae su propia cara: no hay `.lnk` que se
//! despegue, ni cache de iconos que reconstruir, ni un fichero de escritorio
//! que quede huerfano. Copias el `.bex` y va con su icono; lo borras y no queda
//! rastro que limpiar.
//!
//! Lo mas cerca que hay de esto fuera es el *bundle* de macOS --un `.app` es una
//! carpeta que el Finder ensena como un solo objeto-- y aqui esta un paso mas
//! alla: no es una carpeta, es **un fichero**.
//!
//! ## Y si la app no trae icono
//!
//! Se le dibuja uno: un cuadro de color con su inicial. El color sale del
//! propio nombre, asi que es **estable** --la misma app tiene siempre el mismo
//! color-- y distinto entre apps sin que nadie los asigne.
//!
//! No es un parche mientras llegan los iconos de verdad: es lo correcto. Un
//! escritorio donde una app sin icono aparece como un cuadro vacio obliga a
//! todo el mundo a dibujar antes de poder lanzar nada.
//!
//! ## El formato `BICO`, y por que es tan pequeno
//!
//! ```text
//!   0..4   "BICO"
//!   4..6   ancho  (u16)
//!   6..8   alto   (u16)
//!   8..    ancho*alto pixeles BGRA, u32 little-endian
//! ```
//!
//! **16x16, y se pinta al doble.** Un icono de 32x32 en crudo son 4 KiB por
//! app; doce apps son 48 KiB **residentes en el compositor** para algo que se
//! mira de reojo. A 16x16 son 1 KiB por app y 12 KiB en total.
//!
//! Y el numero importa mas de lo que parece: la gracia de meter el icono en el
//! paquete es que **no cueste nada llevarlo**. Un icono que engorda la app es
//! un icono que alguien acabara quitando.

use bmo_userland as bmo;

use super::{FONDO_ARRIBA, TEXTO};

/// Cuantas apps caben en el escritorio. Doce es lo que entra en una fila y
/// media a 1080p; pasado eso hace falta una rejilla con scroll, y eso es otra
/// conversacion.
pub const MAX_APPS: usize = 12;
/// Lado del icono TAL COMO SE GUARDA.
pub const ICONO_LADO: u32 = 16;
/// A cuanto se pinta. Ver la cabecera: se guarda pequeno y se agranda.
pub const ESCALA: u32 = 2;
const ICONO_PX: u32 = ICONO_LADO * ESCALA;

/// La celda de cada app: el icono arriba, el nombre debajo.
const CELDA_ANCHO: u32 = 104;
const CELDA_ALTO: u32 = 72;
/// Donde empieza la rejilla. Debajo de la barra de titulo, con aire.
const REJILLA_X: u32 = 24;
const REJILLA_Y: u32 = 56;

const PIXELES: usize = (ICONO_LADO * ICONO_LADO) as usize;

/// Una app encontrada en `apps\`.
pub struct App {
    /// `apps/doom.bex`, que es lo que se le pasa a `ejecutar`.
    ruta: [u8; 24],
    ruta_len: usize,
    /// `doom.bex`, para pintar debajo.
    nombre: [u8; 12],
    nombre_len: usize,
    /// Los pixeles del icono, si el paquete traia uno.
    pixeles: [u32; PIXELES],
    tiene_icono: bool,
}

impl App {
    const VACIA: Self = Self {
        ruta: [0; 24],
        ruta_len: 0,
        nombre: [0; 12],
        nombre_len: 0,
        pixeles: [0; PIXELES],
        tiene_icono: false,
    };

    pub fn nombre(&self) -> &[u8] {
        &self.nombre[..self.nombre_len]
    }

    pub fn ruta(&self) -> &[u8] {
        &self.ruta[..self.ruta_len]
    }
}

pub struct Lanzador {
    apps: [App; MAX_APPS],
    cuantas: usize,
}

impl Lanzador {
    /// Recorre `apps\`, se queda con los `.bex` y le saca el icono a cada uno.
    ///
    /// Se hace UNA VEZ, al arrancar el escritorio: son varias lecturas de disco
    /// por app y ninguna cambia mientras la maquina esta encendida. Un
    /// escritorio que releyera el directorio en cada fotograma seria un
    /// escritorio que hace E/S sesenta veces por segundo para ensenar lo mismo.
    pub fn nuevo() -> Self {
        let mut yo = Self {
            apps: [const { App::VACIA }; MAX_APPS],
            cuantas: 0,
        };
        let Ok(dir) = bmo::Directorio::open(b"apps") else {
            // No hay `apps\`: no es un fallo, es un disco sin aplicaciones.
            return yo;
        };
        while yo.cuantas < MAX_APPS {
            let Some(e) = dir.next() else { break };
            if e.es_dir {
                continue;
            }
            let mut nombre = [0u8; 12];
            let n = e.legible(&mut nombre);
            if n < 5 || !termina_en_bex(&nombre[..n]) {
                continue;
            }
            let i = yo.cuantas;
            let app = &mut yo.apps[i];
            app.nombre[..n].copy_from_slice(&nombre[..n]);
            app.nombre_len = n;
            // `apps/` + el nombre. Cabe siempre: 5 + 12 = 17 de 24.
            app.ruta[..5].copy_from_slice(b"apps/");
            app.ruta[5..5 + n].copy_from_slice(&nombre[..n]);
            app.ruta_len = 5 + n;
            app.tiene_icono = leer_icono(&app.ruta[..app.ruta_len], &mut app.pixeles);
            yo.cuantas += 1;
        }
        yo
    }

    pub fn cuantas(&self) -> usize {
        self.cuantas
    }

    pub fn app(&self, i: usize) -> Option<&App> {
        if i < self.cuantas {
            Some(&self.apps[i])
        } else {
            None
        }
    }

    /// Que app cae bajo `(x, y)`, si es que hay alguna.
    ///
    /// El area sensible es **la celda entera**, no solo los pixeles del icono:
    /// apuntar a un cuadro de 32x32 con un raton es mas dificil de lo que
    /// parece, y el nombre de debajo forma parte de lo que uno cree estar
    /// pulsando.
    pub fn app_en(&self, p: &bmo::Pantalla, x: u32, y: u32) -> Option<usize> {
        let por_fila = self.por_fila(p);
        if por_fila == 0 || y < REJILLA_Y || x < REJILLA_X {
            return None;
        }
        let col = (x - REJILLA_X) / CELDA_ANCHO;
        let fila = (y - REJILLA_Y) / CELDA_ALTO;
        if col >= por_fila {
            return None;
        }
        let i = (fila * por_fila + col) as usize;
        if i < self.cuantas {
            Some(i)
        } else {
            None
        }
    }

    fn por_fila(&self, p: &bmo::Pantalla) -> u32 {
        if p.ancho <= REJILLA_X * 2 {
            return 0;
        }
        ((p.ancho - REJILLA_X * 2) / CELDA_ANCHO).max(1)
    }

    fn celda(&self, p: &bmo::Pantalla, i: usize) -> (u32, u32) {
        let por_fila = self.por_fila(p).max(1);
        let col = (i as u32) % por_fila;
        let fila = (i as u32) / por_fila;
        (REJILLA_X + col * CELDA_ANCHO, REJILLA_Y + fila * CELDA_ALTO)
    }
}

/// Pinta la rejilla entera. Va DESPUES del fondo y antes de las ventanas.
pub fn pintar(p: &bmo::Pantalla, l: &Lanzador) {
    for i in 0..l.cuantas {
        let (cx, cy) = l.celda(p, i);
        let app = &l.apps[i];
        // El icono, centrado en la celda.
        let ix = cx + (CELDA_ANCHO - ICONO_PX) / 2;
        if app.tiene_icono {
            pintar_pixeles(p, ix, cy, &app.pixeles);
        } else {
            pintar_por_defecto(p, ix, cy, app.nombre());
        }
        // El nombre debajo, centrado y SIN el `.bex`: la extension es la misma
        // en todos, asi que ocupa sitio y no distingue nada.
        let visible = sin_extension(app.nombre());
        let ancho = visible.len() as u32 * 8;
        let tx = if ancho < CELDA_ANCHO {
            cx + (CELDA_ANCHO - ancho) / 2
        } else {
            cx
        };
        p.texto_bytes(tx, cy + ICONO_PX + 6, visible, TEXTO);
    }
}

/// El area que ocupa la rejilla, para que quien repinte el fondo sepa que
/// tiene que volver a pintar esto encima.
pub fn area(p: &bmo::Pantalla, l: &Lanzador) -> (u32, u32, u32, u32) {
    if l.cuantas == 0 {
        return (0, 0, 0, 0);
    }
    let por_fila = l.por_fila(p).max(1);
    let filas = ((l.cuantas as u32) + por_fila - 1) / por_fila;
    let cols = (l.cuantas as u32).min(por_fila);
    (
        REJILLA_X,
        REJILLA_Y,
        cols * CELDA_ANCHO,
        filas * CELDA_ALTO,
    )
}

fn pintar_pixeles(p: &bmo::Pantalla, x: u32, y: u32, px: &[u32; PIXELES]) {
    for fy in 0..ICONO_LADO {
        for fx in 0..ICONO_LADO {
            let c = px[(fy * ICONO_LADO + fx) as usize];
            // ** El canal alto a cero = TRANSPARENTE, y se salta.
            //
            // Sin esto un icono redondo se pinta dentro de su cuadro negro y el
            // escritorio se llena de sellos. Es un test de bit, no un
            // compositor: no hay mezcla, o esta o no esta.
            if c >> 24 == 0 {
                continue;
            }
            p.rect(x + fx * ESCALA, y + fy * ESCALA, ESCALA, ESCALA, c & 0x00FF_FFFF);
        }
    }
}

/// El icono de quien no trae icono: un cuadro de color con su inicial.
fn pintar_por_defecto(p: &bmo::Pantalla, x: u32, y: u32, nombre: &[u8]) {
    let color = color_de(nombre);
    p.rect(x, y, ICONO_PX, ICONO_PX, color);
    // Un borde mas claro arriba y mas oscuro abajo: dos rectangulos y el cuadro
    // deja de parecer un agujero.
    p.rect(x, y, ICONO_PX, 1, aclarar(color));
    p.rect(x, y + ICONO_PX - 1, ICONO_PX, 1, FONDO_ARRIBA);
    let inicial = mayuscula(nombre.first().copied().unwrap_or(b'?'));
    // `glifo_escala` a 2 mide 16x16; centrarlo es restar la mitad.
    p.glifo_escala(x + ICONO_PX / 2 - 8, y + ICONO_PX / 2 - 8, inicial, 0x00FF_FFFF, 2);
}

/// Un color estable a partir del nombre.
///
/// La misma app tiene siempre el mismo, y dos apps distintas casi nunca el
/// mismo -- que es todo lo que se le pide. Se fija el brillo para que la
/// inicial blanca se lea encima: un hash suelto produce amarillos donde no se
/// ve nada.
fn color_de(nombre: &[u8]) -> u32 {
    let mut h: u32 = 2166136261;
    for &b in nombre {
        h ^= b as u32;
        h = h.wrapping_mul(16777619);
    }
    // Tres canales en 0x40..0xC0: ni tan oscuro que parezca un hueco ni tan
    // claro que se coma el texto de encima.
    let r = 0x40 + (h & 0x7F);
    let g = 0x40 + ((h >> 8) & 0x7F);
    let b = 0x40 + ((h >> 16) & 0x7F);
    (r << 16) | (g << 8) | b
}

fn aclarar(c: u32) -> u32 {
    let r = (((c >> 16) & 0xFF) + 0x30).min(0xFF);
    let g = (((c >> 8) & 0xFF) + 0x30).min(0xFF);
    let b = ((c & 0xFF) + 0x30).min(0xFF);
    (r << 16) | (g << 8) | b
}

fn mayuscula(b: u8) -> u8 {
    if b.is_ascii_lowercase() {
        b - 32
    } else {
        b
    }
}

fn termina_en_bex(n: &[u8]) -> bool {
    let l = n.len();
    l >= 4 && n[l - 4..].eq_ignore_ascii_case(b".bex")
}

fn sin_extension(n: &[u8]) -> &[u8] {
    match n.iter().rposition(|&b| b == b'.') {
        Some(i) => &n[..i],
        None => n,
    }
}

// -- Leer el icono DE DENTRO del paquete --------------------------------
//
// Se leen los bytes a mano y no con `bmo_abi::bef`, por el mismo motivo que lo
// hace el kernel: **`bmo-abi` es el CONTRATO y aqui se implementa contra el**.
// Dos lectores del mismo formato es lo que obliga a que el formato este escrito
// y no solo implementado.
//
// Los offsets salen de `bef/header.rs`, `bef/sections.rs` y `bef/recursos.rs`.
// Si alguno cambia, esto deja de encontrar iconos -- y eso es visible al
// primer arranque, que es lo mejor que le puede pasar a una divergencia.

const SECCION_RESOURCES: u8 = 0x0B;
const ENTRADA_SECCION: usize = 48;
const CABECERA_BRES: usize = 16;
const ENTRADA_BRES: usize = 64;

/// Saca el recurso `icono` de un `.bex` y lo descifra como BICO.
/// `true` = habia icono y esta en `px`.
fn leer_icono(ruta: &[u8], px: &mut [u32; PIXELES]) -> bool {
    let Ok(f) = bmo::Archivo::leer_de(ruta) else {
        return false;
    };
    // -- La cabecera del BEF: cuantas secciones y donde esta su tabla --
    let mut cab = [0u8; 48];
    if f.read(&mut cab) < 48 {
        return false;
    }
    if &cab[0..4] != b"BEF1" {
        return false;
    }
    let tabla = leer_u64(&cab, 32) as u64;
    let cuantas = leer_u32(&cab, 40) as usize;
    if cuantas == 0 || cuantas > 255 {
        return false;
    }
    // -- Buscar la seccion de recursos --
    let mut sec_off = 0u64;
    let mut sec_len = 0u64;
    for i in 0..cuantas {
        f.saltar(tabla + (i * ENTRADA_SECCION) as u64);
        let mut e = [0u8; ENTRADA_SECCION];
        if f.read(&mut e) < ENTRADA_SECCION {
            return false;
        }
        if e[0] == SECCION_RESOURCES {
            sec_off = leer_u64(&e, 8);
            sec_len = leer_u64(&e, 16);
            break;
        }
    }
    if sec_len == 0 {
        return false;
    }
    // -- El indice BRES --
    f.saltar(sec_off);
    let mut bres = [0u8; CABECERA_BRES];
    if f.read(&mut bres) < CABECERA_BRES || &bres[0..4] != b"BRES" {
        return false;
    }
    let recursos = leer_u32(&bres, 4) as usize;
    for i in 0..recursos.min(64) {
        f.saltar(sec_off + (CABECERA_BRES + i * ENTRADA_BRES) as u64);
        let mut e = [0u8; ENTRADA_BRES];
        if f.read(&mut e) < ENTRADA_BRES {
            return false;
        }
        let largo = e[16] as usize;
        if largo > 47 || &e[17..17 + largo] != b"icono" {
            continue;
        }
        // El offset del recurso es RELATIVO A LA SECCION, no al fichero: por eso
        // se suma `sec_off`. Un offset absoluto habria que reescribirlo cada vez
        // que el `.bex` se vuelve a emitir con otra disposicion.
        let dato = sec_off + leer_u64(&e, 0);
        let tam = leer_u64(&e, 8) as usize;
        return leer_bico(&f, dato, tam, px);
    }
    false
}

fn leer_bico(f: &bmo::Archivo, off: u64, tam: usize, px: &mut [u32; PIXELES]) -> bool {
    const CAB: usize = 8;
    if tam < CAB + PIXELES * 4 {
        return false;
    }
    f.saltar(off);
    let mut cab = [0u8; CAB];
    if f.read(&mut cab) < CAB || &cab[0..4] != b"BICO" {
        return false;
    }
    // Solo se acepta el tamano que este escritorio sabe pintar. Escalar un
    // icono de otro tamano es una decision de aspecto que no toca aqui, y
    // aceptarlo a medias daria iconos deformes sin decir por que.
    if leer_u16(&cab, 4) as u32 != ICONO_LADO || leer_u16(&cab, 6) as u32 != ICONO_LADO {
        return false;
    }
    let mut bytes = [0u8; PIXELES * 4];
    if f.read(&mut bytes) < bytes.len() {
        return false;
    }
    for i in 0..PIXELES {
        px[i] = leer_u32(&bytes, i * 4);
    }
    true
}

fn leer_u16(b: &[u8], o: usize) -> u16 {
    u16::from_le_bytes([b[o], b[o + 1]])
}

fn leer_u32(b: &[u8], o: usize) -> u32 {
    u32::from_le_bytes([b[o], b[o + 1], b[o + 2], b[o + 3]])
}

fn leer_u64(b: &[u8], o: usize) -> u64 {
    let mut v = [0u8; 8];
    v.copy_from_slice(&b[o..o + 8]);
    u64::from_le_bytes(v)
}
