//! **LA BIBLIOTECA** -- todo lo que hay en DATOS, por lo que ES (2026-09-13).
//!
//! [consumo] NADA      recorre el disco al ENTRAR en la vista o con `R`; pintar
//!                     solo mira lo ya leido (L6h)
//!
//! ## Por que existe
//!
//! Eddi: *"que tenga biblioteca mi BMO-X ... una app que encuentre TODO: imagenes,
//! audio, otros"*. El explorador contesta *donde esta*; esto contesta *que
//! tengo*: para encontrar tus musicas no deberias saber en que carpeta estan.
//!
//! ## ** Tres paneles, como un lanzador de Hyprland (segunda version, 13-09)
//!
//! La primera fue una rejilla de tarjetas. En el Ryzen salio llena y
//! DESORDENADA --Eddi: *"funcionan pero todo eso es desordenado, vamos a
//! organizar como esto"*, con la captura de un lanzador: panel a la izquierda,
//! lista a la derecha y la fila elegida resaltada--. Una rejilla de 12x13
//! tarjetas iguales obliga a leerlas todas; una lista con UNA ficha grande de la
//! elegida se lee de arriba abajo.
//!
//! ```text
//!    +----------+----------------+---------------------------+
//!    | todo 160 |                | [A] cubo.bex     c  47168 |
//!    | apps 112 |   LA ELEGIDA   | [A] guia.bex     c  41818 |
//!    | imag.  3 |   en grande:   |#[I] foto.bmp  datos   822#|
//!    | audio  1 |   que es y con | [T] leeme.txt    /    120 |
//!    | texto 20 |   que se abre  |                           |
//!    +----------+----------------+---------------------------+
//! ```
//!
//! ## ** Lo que NO es biblioteca, y salio en la foto
//!
//! `$RECYCLE.BIN` --la papelera que Windows deja en cualquier disco que toca--
//! metia una copia de cada fichero borrado: la mitad de las 160 tarjetas eran
//! esa carpeta, y el tope decia RECORTADA. Se saltan las carpetas que empiezan
//! por `$` y `SYSTEM~1` (System Volume Information), a cualquier hondo. Y `sys`
//! y `efi` en la raiz, que son el sistema.
//!
//! Por ANCHURA sobre `DIR_ABRIR`, UNA carpeta abierta a la vez (el kernel tiene
//! ocho ranuras). Cuatro niveles, 48 carpetas y 160 ficheros; lo que no cabe se
//! DICE.

use bmo_userland as bmo;
use core::ptr::addr_of_mut;

use super::*;
use crate::scene::asociaciones::{self, Abre, Clase};
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

/// Una carpeta que no es de nadie: la papelera de Windows y su indice.
fn es_de_windows(nom: &[u8]) -> bool {
    nom.first() == Some(&b'$') || nom == b"system~1"
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
                    if es_de_windows(&nom[..n]) {
                        continue;
                    }
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

/// Las categorias del panel de la izquierda, en su orden.
pub(crate) const CATEGORIAS: [Option<Clase>; 5] =
    [None, Some(Clase::App), Some(Clase::Imagen), Some(Clase::Audio), Some(Clase::Texto)];

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

/// La ruta de la `k`-esima: `(bytes, donde empieza el nombre)`.
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
//  La geometria -- UNA para pintar y para acertar con el raton
// ===================================================================

/// Los huecos entre paneles: lo que hace que se lean sueltos.
const GAP: u32 = 10;
const LADO_W: u32 = 168;
/// Alto de una fila de la lista y de una categoria.
const FILA: u32 = 34;
/// La cabecera de cada panel (el titulo, o la cuenta de la lista).
const CAB: u32 = 32;

pub(crate) struct Partes {
    pub lado: Zona,
    pub vista: Zona,
    pub lista: Zona,
}

pub(crate) fn partes(z: &Zona) -> Partes {
    let y = z.y + GAP;
    let h = z.h.saturating_sub(2 * GAP);
    let lado = Zona { x: z.x + GAP, y, w: LADO_W, h };
    let resto_x = lado.x + LADO_W + GAP;
    let resto_w = (z.x + z.w).saturating_sub(resto_x + GAP);
    // La ficha grande solo si queda sitio para ella Y para una lista legible.
    let vista_w = if resto_w >= 700 { resto_w * 36 / 100 } else { 0 };
    let vista = if vista_w > 0 { Zona { x: resto_x, y, w: vista_w, h } } else { Zona::NADA };
    let lista_x = if vista_w > 0 { resto_x + vista_w + GAP } else { resto_x };
    let lista = Zona { x: lista_x, y, w: (z.x + z.w).saturating_sub(lista_x + GAP), h };
    Partes { lado, vista, lista }
}

/// Cuantas filas caben en la lista.
pub(crate) fn filas(z: &Zona) -> usize {
    (partes(z).lista.h.saturating_sub(CAB + 8) / FILA).max(1) as usize
}

pub(crate) enum Golpe {
    Categoria(Option<Clase>),
    Fila(usize),
}

/// Sobre que cayo el puntero. `from` es la primera fila visible de la lista.
pub(crate) fn en(z: &Zona, from: usize, px: u32, py: u32) -> Option<Golpe> {
    let pt = partes(z);
    if pt.lado.contiene(px, py) {
        let y0 = pt.lado.y + CAB;
        if py < y0 {
            return None;
        }
        let k = ((py - y0) / FILA) as usize;
        return CATEGORIAS.get(k).map(|c| Golpe::Categoria(*c));
    }
    if pt.lista.contiene(px, py) {
        let y0 = pt.lista.y + CAB;
        if py < y0 {
            return None;
        }
        let fila = ((py - y0) / FILA) as usize;
        let k = from + fila;
        if fila < filas(z) && k < visibles() {
            return Some(Golpe::Fila(k));
        }
    }
    None
}

// ===================================================================
//  Pintar
// ===================================================================

fn panel(p: &bmo::Pantalla, z: &Zona) {
    rounded_rect(p, z.x + 2, z.y + 3, z.w, z.h, SHADOW_NODE);
    rounded_rect(p, z.x, z.y, z.w, z.h, DATA_EDGE);
    rounded_rect(p, z.x + 1, z.y + 1, z.w - 2, z.h - 2, NODE_BG);
}

/// El realce de lo elegido: el acento de borde y un relleno oscuro. Es el
/// mismo en el panel de categorias y en la lista.
fn realzar(p: &bmo::Pantalla, x: u32, y: u32, w: u32, h: u32) {
    rounded_rect(p, x, y, w, h, sel_neon());
    rounded_rect(p, x + 1, y + 1, w - 2, h - 2, SEL_FONDO);
}

/// El icono de una clase: un DIBUJO desde el 2026-09-13, no una letra. Ver
/// `scene::pictos`.
fn icono(p: &bmo::Pantalla, x: u32, y: u32, lado: u32, c: Clase) {
    crate::scene::pictos::dibujar(p, x, y, lado, c);
}

fn lado(p: &bmo::Pantalla, z: &Zona) {
    panel(p, z);
    p.texto(z.x + 14, z.y + (CAB - bmo::GLIFO_ALTO) / 2, "biblioteca", DATA_TITLE);
    let f = filtro();
    for (k, c) in CATEGORIAS.iter().enumerate() {
        let y = z.y + CAB + k as u32 * FILA;
        let activa = *c == f;
        if activa {
            realzar(p, z.x + 6, y + 2, z.w - 12, FILA - 4);
        }
        let ty = y + (FILA - bmo::GLIFO_ALTO) / 2;
        let (nombre, color, tecla) = match c {
            None => ("todo", DATA_TITLE, b'0'),
            Some(c) => (c.nombre(), c.color(), c.tecla()),
        };
        p.rect(z.x + 16, y + FILA / 2 - 4, 8, 8, color);
        p.texto(z.x + 32, ty, nombre, if activa { INK } else { INK_DIM });
        let mut b = [0u8; 10];
        let nb = decimal(de_clase(*c) as u64, &mut b);
        let x = z.x + z.w - 14 - (nb as u32 + 2) * bmo::GLIFO_ANCHO;
        let x = p.texto_bytes(x, ty, &[tecla], INK_DIM);
        p.texto_bytes(x + bmo::GLIFO_ANCHO, ty, &b[..nb], INK);
    }
    if recortada() {
        let y = z.y + CAB + CATEGORIAS.len() as u32 * FILA + 8;
        p.texto(z.x + 14, y, "RECORTADA", INK_BAD);
    }
}

fn vista(p: &bmo::Pantalla, z: &Zona, sel: usize) {
    if !z.hay() {
        return;
    }
    panel(p, z);
    let Some(it) = item(sel) else {
        p.texto(z.x + 16, z.y + 16, "nada elegido", INK_DIM);
        return;
    };
    let x = z.x + 20;
    let mut y = z.y + 24;
    icono(p, x, y, 64, it.clase);
    y += 64 + 18;
    let nombre = &it.ruta[it.nombre as usize..it.largo as usize];
    let nombre_s = core::str::from_utf8(nombre).unwrap_or("?");
    // El nombre en grande, si cabe; si no, a su tamano. Recortado en grande no
    // se lee mejor, se lee peor.
    if nombre.len() as u32 * bmo::GLIFO_ANCHO * 2 <= z.w.saturating_sub(40) {
        p.texto_escala(x, y, nombre_s, INK, 2);
        y += bmo::GLIFO_ALTO * 2 + 14;
    } else {
        p.texto_bytes(x, y, nombre, INK);
        y += bmo::GLIFO_ALTO + 14;
    }
    let carpeta: &[u8] = if it.nombre == 0 { b"/" } else { &it.ruta[..it.nombre as usize - 1] };
    let fila = |p: &bmo::Pantalla, y: u32, clave: &str, valor: &[u8]| {
        let vx = p.texto(x, y, clave, INK_DIM);
        p.texto_bytes(vx.max(x + 9 * bmo::GLIFO_ANCHO), y, valor, INK);
    };
    fila(p, y, "carpeta", carpeta);
    y += bmo::GLIFO_ALTO + 6;
    let mut b = [0u8; 10];
    let nb = decimal(it.bytes as u64, &mut b);
    let mut t = [0u8; 12];
    t[..nb].copy_from_slice(&b[..nb]);
    t[nb..nb + 2].copy_from_slice(b" B");
    fila(p, y, "tamano", &t[..nb + 2]);
    y += bmo::GLIFO_ALTO + 6;
    fila(p, y, "tipo", it.clase.nombre().as_bytes());
    y += bmo::GLIFO_ALTO + 18;
    // Con que se abre: lo dice la MISMA tabla que decide al pulsar ENTRAR.
    p.rect(x, y, z.w.saturating_sub(40), 1, DATA_EDGE);
    y += 10;
    match asociaciones::de(nombre).1 {
        Abre::Programa => { p.texto(x, y, "ENTRAR la lanza", sel_neon()); }
        Abre::Visor => { p.texto(x, y, "ENTRAR la abre en el visor", sel_neon()); }
        Abre::Con(app) => {
            let ex = p.texto(x, y, "ENTRAR la abre con ", sel_neon());
            p.texto_bytes(ex, y, app, INK);
        }
        Abre::Falta(motivo) => {
            p.texto(x, y, "todavia no se abre:", INK_BAD);
            p.texto(x, y + bmo::GLIFO_ALTO + 4, motivo, INK_DIM);
        }
    }
}

fn lista(p: &bmo::Pantalla, z: &Zona, from: usize, sel: usize, caben: usize) {
    panel(p, z);
    let total = visibles();
    let ty = z.y + (CAB - bmo::GLIFO_ALTO) / 2;
    let mut b = [0u8; 10];
    let nb = decimal(total as u64, &mut b);
    let x = p.texto_bytes(z.x + 14, ty, &b[..nb], INK);
    p.texto(x + bmo::GLIFO_ANCHO, ty, if total == 1 { "fichero" } else { "ficheros" }, INK_DIM);
    if total == 0 {
        let msg = if de_clase(None) == 0 {
            "no hay nada que yo sepa abrir en DATOS. R vuelve a mirar."
        } else {
            "nada de esta clase. 0 ensena todo."
        };
        p.texto(z.x + 14, z.y + CAB + 8, msg, INK_DIM);
        return;
    }
    let n = unsafe { CUANTOS };
    let ancho = z.w.saturating_sub(12);
    for (k, it) in items()[..n].iter().filter(|it| pasa(it)).enumerate().skip(from).take(caben) {
        let y = z.y + CAB + (k - from) as u32 * FILA;
        if k == sel {
            realzar(p, z.x + 6, y + 2, ancho, FILA - 4);
        }
        icono(p, z.x + 16, y + 7, 20, it.clase);
        let ty = y + (FILA - bmo::GLIFO_ALTO) / 2;
        // Derecha: el tamano; delante, la carpeta. Lo que no cabe se come la
        // carpeta, nunca el nombre.
        let nb = decimal(it.bytes as u64, &mut b);
        let xs = z.x + z.w - 16 - nb as u32 * bmo::GLIFO_ANCHO;
        p.texto_bytes(xs, ty, &b[..nb], INK_DIM);
        let nombre = &it.ruta[it.nombre as usize..it.largo as usize];
        let x0 = z.x + 46;
        let cabe = ((xs.saturating_sub(x0 + 16)) / bmo::GLIFO_ANCHO) as usize;
        let nn = nombre.len().min(cabe);
        p.texto_bytes(x0, ty, &nombre[..nn], if k == sel { INK } else { INK });
        let carpeta: &[u8] = if it.nombre == 0 { b"/" } else { &it.ruta[..it.nombre as usize - 1] };
        let libre = cabe.saturating_sub(nn + 3);
        if libre >= 3 {
            let nc = carpeta.len().min(libre);
            let cx = xs.saturating_sub(16 + nc as u32 * bmo::GLIFO_ANCHO);
            p.texto_bytes(cx, ty, &carpeta[..nc], INK_DIM);
        }
    }
    if total > from + caben {
        let y = z.y + z.h - bmo::GLIFO_ALTO - 6;
        let nb = decimal((total - from - caben) as u64, &mut b);
        let x = p.texto(z.x + 14, y, "y ", INK_DIM);
        let x = p.texto_bytes(x, y, &b[..nb], INK);
        p.texto(x, y, " mas abajo", INK_DIM);
    }
}

/// Pinta la biblioteca en `z`. `from` es la primera fila visible; `sel`, la
/// elegida.
pub(crate) fn paint(p: &bmo::Pantalla, z: &Zona, from: usize, sel: usize) {
    if !z.hay() {
        return;
    }
    let pt = partes(z);
    lado(p, &pt.lado);
    vista(p, &pt.vista, sel);
    lista(p, &pt.lista, from, sel, filas(z));
}
