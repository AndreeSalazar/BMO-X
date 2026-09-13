//! **LA BIBLIOTECA** -- todo lo que hay en DATOS, por lo que ES (2026-09-13).
//!
//! [consumo] NADA      recorre el disco al ENTRAR en la vista o con `R`; pintar
//!                     solo mira lo ya leido (L6h)
//!
//! ## Por que existe
//!
//! Eddi: *"que tenga biblioteca mi BMO-X ... una app que encuentre TODO: imagenes,
//! audio, otros ... inspirado en Hyprland"*.
//!
//! El explorador contesta *donde esta*; esto contesta *que tengo*. Son dos
//! preguntas, y por eso es otra vista y no otra columna: para encontrar tus
//! musicas no deberias tener que saber en que carpeta las dejaste.
//!
//! ## Lo de Hyprland que SI se toma, y lo que no
//!
//! ```text
//!    se toma    huecos entre tarjetas, bordes redondeados, el borde de ACENTO
//!               en la elegida, y la rejilla que se reparte sola al estirar
//!    no         animaciones (pintar por pintar gasta: L6h) ni transparencias
//!               (no hay compositor con canal alfa por ventana todavia)
//! ```
//!
//! ## El recorrido, y sus topes
//!
//! Por ANCHURA sobre `DIR_ABRIR`, UNA carpeta abierta a la vez (el kernel tiene
//! ocho ranuras y `Directorio` suelta la suya al acabar). Cuatro niveles de hondo,
//! 48 carpetas y 160 ficheros. Lo que no cabe se DICE (`recortada`).
//!
//! [!] Se saltan `sys` --es el sistema, no tu biblioteca-- y `efi`, que es de
//! arranque. Y los tipos que `asociaciones` no conoce no salen: una biblioteca
//! que lista lo que no sabe abrir es un `ls`.

use bmo_userland as bmo;
use core::ptr::addr_of_mut;

use super::*;
use crate::scene::asociaciones::{self, Clase};
use crate::scene::zonas::Zona;
use crate::text::decimal;

const MAX: usize = 160;
const RUTA: usize = 48;
const CARPETAS: usize = 48;
const HONDO: u8 = 4;

#[derive(Clone, Copy)]
struct Item {
    ruta: [u8; RUTA],
    largo: u8,
    /// Donde empieza el nombre dentro de la ruta. Lo de antes es la carpeta.
    nombre: u8,
    clase: Clase,
    bytes: u32,
}

const ITEM_VACIO: Item = Item { ruta: [0; RUTA], largo: 0, nombre: 0, clase: Clase::Otro, bytes: 0 };

static mut ITEMS: [Item; MAX] = [ITEM_VACIO; MAX];
static mut CUANTOS: usize = 0;
static mut RECORTADA: bool = false;
static mut COLA: [[u8; RUTA]; CARPETAS] = [[0; RUTA]; CARPETAS];
static mut COLA_LARGO: [u8; CARPETAS] = [0; CARPETAS];
static mut COLA_HONDO: [u8; CARPETAS] = [0; CARPETAS];
/// `None` = todas las clases.
static mut FILTRO: Option<Clase> = None;

fn items() -> &'static mut [Item; MAX] {
    unsafe { &mut *addr_of_mut!(ITEMS) }
}

/// **Recorre DATOS y apunta lo que sabe abrir.** Lo llama quien ENTRA en la vista.
pub(crate) fn releer() {
    unsafe {
        CUANTOS = 0;
        RECORTADA = false;
        let cola = &mut *addr_of_mut!(COLA);
        let largo = &mut *addr_of_mut!(COLA_LARGO);
        let hondo = &mut *addr_of_mut!(COLA_HONDO);
        largo[0] = 0;
        hondo[0] = 0;
        let mut en_cola = 1usize;
        let mut i = 0usize;
        while i < en_cola {
            let base = cola[i];
            let bl = largo[i] as usize;
            let d = hondo[i];
            i += 1;
            let Ok(dir) = bmo::Directorio::open(&base[..bl]) else { continue };
            let mut vistas = 0u32;
            while vistas < 256 {
                let Some(e) = dir.next() else { break };
                vistas += 1;
                let mut nom = [0u8; 12];
                let n = e.legible(&mut nom);
                if crate::text::is_dot_entry(&nom[..n]) {
                    continue;
                }
                // La ruta: `carpeta/nombre`, o `nombre` en la raiz.
                let hueco = if bl > 0 { 1 } else { 0 };
                if bl + hueco + n > RUTA {
                    RECORTADA = true;
                    continue;
                }
                let mut r = [0u8; RUTA];
                r[..bl].copy_from_slice(&base[..bl]);
                if hueco == 1 {
                    r[bl] = b'/';
                }
                r[bl + hueco..bl + hueco + n].copy_from_slice(&nom[..n]);
                let rl = bl + hueco + n;
                if e.es_dir {
                    if d == 0 && (&nom[..n] == b"sys" || &nom[..n] == b"efi") {
                        continue;
                    }
                    if d + 1 >= HONDO || en_cola == CARPETAS {
                        RECORTADA = true;
                        continue;
                    }
                    cola[en_cola] = r;
                    largo[en_cola] = rl as u8;
                    hondo[en_cola] = d + 1;
                    en_cola += 1;
                } else {
                    let (clase, _) = asociaciones::de(&nom[..n]);
                    if clase == Clase::Otro {
                        continue;
                    }
                    if CUANTOS == MAX {
                        RECORTADA = true;
                        break;
                    }
                    items()[CUANTOS] = Item {
                        ruta: r,
                        largo: rl as u8,
                        nombre: (bl + hueco) as u8,
                        clase,
                        bytes: e.bytes,
                    };
                    CUANTOS += 1;
                }
            }
        }
    }
}

pub(crate) fn filtro() -> Option<Clase> {
    unsafe { FILTRO }
}

pub(crate) fn poner_filtro(f: Option<Clase>) {
    unsafe { FILTRO = f };
}

fn pasa(it: &Item) -> bool {
    filtro().map_or(true, |f| f == it.clase)
}

/// Cuantas de una clase (o todas, con `None`), sin mirar el filtro.
pub(crate) fn de_clase(c: Option<Clase>) -> usize {
    let n = unsafe { CUANTOS };
    items()[..n].iter().filter(|it| c.map_or(true, |c| c == it.clase)).count()
}

/// Cuantas pasan el filtro.
pub(crate) fn visibles() -> usize {
    let n = unsafe { CUANTOS };
    items()[..n].iter().filter(|it| pasa(it)).count()
}

/// La `k`-esima que pasa el filtro.
fn item(k: usize) -> Option<Item> {
    let n = unsafe { CUANTOS };
    items()[..n].iter().filter(|it| pasa(it)).nth(k).copied()
}

/// La ruta y el nombre de la `k`-esima: `(ruta, desde_donde_empieza_el_nombre)`.
pub(crate) fn ruta(k: usize, dst: &mut [u8; 128]) -> (usize, usize) {
    match item(k) {
        Some(it) => {
            let n = it.largo as usize;
            dst[..n].copy_from_slice(&it.ruta[..n]);
            (n, it.nombre as usize)
        }
        None => (0, 0),
    }
}

pub(crate) fn recortada() -> bool {
    unsafe { RECORTADA }
}

// ===================================================================
//  La rejilla -- UNA geometria para pintar y para acertar con el raton
// ===================================================================

const CARD_W: u32 = 184;
const CARD_H: u32 = 58;
/// Los huecos de Hyprland: lo que hace que las tarjetas se lean sueltas.
const GAP: u32 = 10;
/// La fila de filtros de arriba.
const CABECERA: u32 = 34;

pub(crate) struct Rejilla {
    pub x: u32,
    pub y: u32,
    pub cols: usize,
    pub filas: usize,
}

pub(crate) fn rejilla(z: &Zona) -> Rejilla {
    let util_w = z.w.saturating_sub(GAP);
    let util_h = z.h.saturating_sub(CABECERA + GAP);
    let cols = ((util_w / (CARD_W + GAP)) as usize).max(1);
    let filas = ((util_h / (CARD_H + GAP)) as usize).max(1);
    // Centrada: el sobrante se reparte a los dos lados, como un tiling que no
    // deja todo el aire pegado a la derecha.
    let usado = cols as u32 * (CARD_W + GAP) - GAP;
    let x = z.x + z.w.saturating_sub(usado) / 2;
    Rejilla { x, y: z.y + CABECERA, cols, filas }
}

/// Sobre que tarjeta cayo el puntero. `from` es la primera FILA visible.
pub(crate) fn en(z: &Zona, from: usize, px: u32, py: u32) -> Option<usize> {
    if !z.contiene(px, py) {
        return None;
    }
    let r = rejilla(z);
    if px < r.x || py < r.y {
        return None;
    }
    let (cx, cy) = ((px - r.x) / (CARD_W + GAP), (py - r.y) / (CARD_H + GAP));
    // El hueco no es de ninguna tarjeta.
    if (px - r.x) % (CARD_W + GAP) >= CARD_W || (py - r.y) % (CARD_H + GAP) >= CARD_H {
        return None;
    }
    if cx as usize >= r.cols || cy as usize >= r.filas {
        return None;
    }
    let k = (from + cy as usize) * r.cols + cx as usize;
    if k < visibles() { Some(k) } else { None }
}

// ===================================================================
//  Pintar
// ===================================================================

/// Los filtros: `0 todo  A apps 12  I imagenes 3 ...`, el activo con fondo.
fn cabecera(p: &bmo::Pantalla, z: &Zona) {
    let ty = z.y + (CABECERA - bmo::GLIFO_ALTO) / 2 - 2;
    let mut x = z.x + GAP;
    let chip = |p: &bmo::Pantalla, x: u32, tecla: u8, nombre: &str, cuantas: usize, activo: bool, color: u32| -> u32 {
        let mut b = [0u8; 10];
        let nb = decimal(cuantas as u64, &mut b);
        let w = (nombre.len() as u32 + nb as u32 + 5) * bmo::GLIFO_ANCHO;
        rounded_rect(p, x, ty - 5, w, bmo::GLIFO_ALTO + 10, if activo { sel_neon() } else { DATA_EDGE });
        rounded_rect(p, x + 1, ty - 4, w - 2, bmo::GLIFO_ALTO + 8, if activo { SEL_FONDO } else { NODE_BG });
        let cx = p.texto_bytes(x + bmo::GLIFO_ANCHO, ty, &[tecla], INK_DIM);
        p.rect(cx + 4, ty + bmo::GLIFO_ALTO / 2 - 3, 6, 6, color);
        let cx = p.texto(cx + 14, ty, nombre, if activo { INK } else { INK_DIM });
        p.texto_bytes(cx + bmo::GLIFO_ANCHO, ty, &b[..nb], INK);
        x + w + GAP
    };
    let f = filtro();
    x = chip(p, x, b'0', "todo", de_clase(None), f.is_none(), DATA_TITLE);
    for c in Clase::VISIBLES {
        x = chip(p, x, c.tecla(), c.nombre(), de_clase(Some(c)), f == Some(c), c.color());
    }
    if recortada() {
        p.texto(x, ty, "(RECORTADA)", INK_BAD);
    }
}

fn tarjeta(p: &bmo::Pantalla, x: u32, y: u32, it: &Item, elegida: bool) {
    rounded_rect(p, x + 2, y + 3, CARD_W, CARD_H, SHADOW_NODE);
    rounded_rect(p, x, y, CARD_W, CARD_H, if elegida { sel_neon() } else { DATA_EDGE });
    // ** El borde de la elegida es de DOS pixeles, como el de la ventana activa
    // de Hyprland: uno solo se pierde en una foto de la pantalla.
    let g = if elegida { 2 } else { 1 };
    rounded_rect(p, x + g, y + g, CARD_W - 2 * g, CARD_H - 2 * g, if elegida { SEL_FONDO } else { NODE_BG });

    let cabe = ((CARD_W - 36) / bmo::GLIFO_ANCHO) as usize;
    let nombre = &it.ruta[it.nombre as usize..it.largo as usize];
    p.rect(x + 12, y + 12, 8, 8, it.clase.color());
    let n = nombre.len().min(cabe);
    let fin = p.texto_bytes(x + 28, y + 8, &nombre[..n], INK);
    if n < nombre.len() {
        p.texto(fin, y + 8, "~", INK_DIM);
    }
    // Debajo: la carpeta, y el tamano a la derecha.
    let carpeta: &[u8] = if it.nombre == 0 { b"/" } else { &it.ruta[..it.nombre as usize - 1] };
    let mut b = [0u8; 10];
    let nb = decimal(it.bytes as u64, &mut b);
    let ty = y + 8 + bmo::GLIFO_ALTO + 6;
    let hueco = ((CARD_W - 40) / bmo::GLIFO_ANCHO) as usize;
    let cabe_carpeta = hueco.saturating_sub(nb + 2);
    let nc = carpeta.len().min(cabe_carpeta);
    p.texto_bytes(x + 12, ty, &carpeta[..nc], INK_DIM);
    p.texto_bytes(x + CARD_W - 12 - (nb as u32 + 1) * bmo::GLIFO_ANCHO, ty, &b[..nb], INK_DIM);
}

/// Pinta la biblioteca en `z`. `from` es la primera fila visible; `sel`, la
/// tarjeta elegida.
pub(crate) fn paint(p: &bmo::Pantalla, z: &Zona, from: usize, sel: usize) {
    if !z.hay() {
        return;
    }
    cabecera(p, z);
    let r = rejilla(z);
    let total = visibles();
    if total == 0 {
        let msg = if de_clase(None) == 0 {
            "no hay nada que yo sepa abrir en DATOS. R vuelve a mirar."
        } else {
            "nada de esta clase. 0 ensena todo."
        };
        p.texto(r.x, r.y + 8, msg, INK_DIM);
        return;
    }
    let n = unsafe { CUANTOS };
    let mut k = 0usize;
    for it in items()[..n].iter().filter(|it| pasa(it)) {
        let fila = k / r.cols;
        if fila >= from + r.filas {
            break;
        }
        if fila >= from {
            let col = (k % r.cols) as u32;
            let x = r.x + col * (CARD_W + GAP);
            let y = r.y + (fila - from) as u32 * (CARD_H + GAP);
            tarjeta(p, x, y, it, k == sel);
        }
        k += 1;
    }
    let ultima = (from + r.filas) * r.cols;
    if total > ultima {
        let mut b = [0u8; 10];
        let nb = decimal((total - ultima) as u64, &mut b);
        let y = r.y + r.filas as u32 * (CARD_H + GAP);
        let x = p.texto(r.x, y, "y ", INK_DIM);
        let x = p.texto_bytes(x, y, &b[..nb], INK);
        p.texto(x, y, " mas abajo", INK_DIM);
    }
}
