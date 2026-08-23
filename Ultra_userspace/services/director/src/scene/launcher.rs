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
//! `bmo_abi::bef::resources`). La app trae su propia cara: no hay `.lnk` que se
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

use super::double_click::DoubleClick;
use super::{ACCENT, BG_TOP, INK};

/// Cuantas apps caben en el escritorio. Doce es lo que entra en una fila y
/// media a 1080p; pasado eso hace falta una rejilla con scroll, y eso es otra
/// conversacion.
pub const MAX_APPS: usize = 12;
/// Lado del icono TAL COMO SE GUARDA.
pub const ICON_SIDE: u32 = 16;
/// A cuanto se pinta. Ver la cabecera: se guarda pequeno y se agranda.
pub const SCALE: u32 = 2;
const ICON_PX: u32 = ICON_SIDE * SCALE;

/// La celda de cada app: el icono arriba, el nombre debajo.
const CELL_W: u32 = 104;
const CELL_H: u32 = 72;
/// Donde empieza la rejilla. Debajo de la barra de titulo, con aire.
const GRID_X: u32 = 24;
const GRID_Y: u32 = 56;

const PIXELS: usize = (ICON_SIDE * ICON_SIDE) as usize;

/// Una app encontrada en `apps\`.
pub struct App {
    /// `apps/doom.bex`, que es lo que se le pasa a `ejecutar`.
    path: [u8; 24],
    path_len: usize,
    /// `doom.bex`, para pintar debajo.
    name: [u8; 12],
    name_len: usize,
    /// Los pixeles del icono, si el paquete traia uno.
    pixeles: [u32; PIXELS],
    tiene_icono: bool,
}

impl App {
    const EMPTY: Self = Self {
        path: [0; 24],
        path_len: 0,
        name: [0; 12],
        name_len: 0,
        pixeles: [0; PIXELS],
        tiene_icono: false,
    };

    pub fn name(&self) -> &[u8] {
        &self.name[..self.name_len]
    }

    pub fn path(&self) -> &[u8] {
        &self.path[..self.path_len]
    }
}

pub struct Launcher {
    apps: [App; MAX_APPS],
    count: usize,
    /// El icono SENALADO, si hay alguno.
    ///
    /// ** Antes no existia porque un clic LANZABA, y lo que se lanza no hace
    /// falta senalarlo. Pero un escritorio en el que pulsar un icono arranca un
    /// programa al primer toque no deja mirar sin ejecutar -- y es la unica
    /// rejilla de la casa donde pulsar no se podia deshacer.
    sel: Option<usize>,
    /// La mitad abierta de un doble clic. Vive en `scene::double_click` porque
    /// la rejilla de ESTRATOS hace exactamente lo mismo, y porque hasta hoy las
    /// dos lo contaban en VUELTAS DEL BUCLE -- que es por lo que un icono se
    /// podia senalar y no se abria nunca.
    doble: DoubleClick,
}

/// El realce de la celda senalada. Un relleno tenue, no un marco: un borde de
/// un pixel alrededor de un icono transparente se lee como suciedad.
const SEL_BG: u32 = 0x001E_3A5F;

impl Launcher {
    /// Recorre `apps\`, se queda con los `.bex` y le saca el icono a cada uno.
    ///
    /// Se hace UNA VEZ, al arrancar el escritorio: son varias lecturas de disco
    /// por app y ninguna cambia mientras la maquina esta encendida. Un
    /// escritorio que releyera el directorio en cada fotograma seria un
    /// escritorio que hace E/S sesenta veces por segundo para ensenar lo mismo.
    // [!] `#[inline(never)]` NO es por tamano de codigo: es por PILA. Inlineado,
    // el struct se construye en una ranura del marco del llamante y se copia
    // despues; como llamada aparte, LLVM le pasa la direccion de destino como
    // puntero de retorno (`sret`) y escribe directamente en `.bss`. Medido en
    // el Ryzen el 2026-08-14 -- ver la cabecera de `desktop::install`.
    #[inline(never)]
    pub fn new() -> Self {
        let mut me = Self {
            apps: [const { App::EMPTY }; MAX_APPS],
            count: 0,
            sel: None,
            doble: DoubleClick::new(),
        };
        let Ok(dir) = bmo::Directorio::open(b"apps") else {
            // No hay `apps\`: no es un fallo, es un disco sin aplicaciones.
            return me;
        };
        while me.count < MAX_APPS {
            let Some(e) = dir.next() else { break };
            if e.es_dir {
                continue;
            }
            let mut name = [0u8; 12];
            let n = e.legible(&mut name);
            if n < 5 || !ends_in_bex(&name[..n]) {
                continue;
            }
            let i = me.count;
            let app = &mut me.apps[i];
            app.name[..n].copy_from_slice(&name[..n]);
            app.name_len = n;
            // `apps/` + el nombre. Cabe siempre: 5 + 12 = 17 de 24.
            app.path[..5].copy_from_slice(b"apps/");
            app.path[5..5 + n].copy_from_slice(&name[..n]);
            app.path_len = 5 + n;
            app.tiene_icono = read_icon(&app.path[..app.path_len], &mut app.pixeles);
            me.count += 1;
        }
        me
    }

    pub fn count(&self) -> usize {
        self.count
    }

    pub fn app(&self, i: usize) -> Option<&App> {
        if i < self.count {
            Some(&self.apps[i])
        } else {
            None
        }
    }

    /// Cual esta senalado.
    pub fn sel(&self) -> Option<usize> {
        self.sel
    }

    /// **Un clic en el icono `i`.** `true` si es el SEGUNDO de un doble clic.
    ///
    /// Es la misma regla que la rejilla de ESTRATOS, y desde el 2026-08-23 es
    /// literalmente el mismo codigo --`scene::double_click`-- y no dos copias
    /// que se parecen: el primero SENALA, el segundo ABRE. Dos rejillas con dos
    /// costumbres distintas en el mismo escritorio serian dos cosas que
    /// aprender donde deberia haber una.
    pub fn clic(&mut self, i: usize) -> bool {
        self.sel = Some(i);
        self.doble.hit(i)
    }

    /// Quita el realce. Lo llama quien pulsa FUERA de la rejilla.
    pub fn soltar(&mut self) -> bool {
        let habia = self.sel.is_some();
        self.sel = None;
        self.doble.clear();
        habia
    }

    /// El rectangulo de la celda `i`, para borrarla o realzarla.
    pub fn celda(&self, p: &bmo::Pantalla, i: usize) -> (u32, u32, u32, u32) {
        let (cx, cy) = self.cell(p, i);
        (cx, cy, CELL_W, CELL_H)
    }

    /// Que app cae bajo `(x, y)`, si es que hay alguna.
    ///
    /// El area sensible es **la celda entera**, no solo los pixeles del icono:
    /// apuntar a un cuadro de 32x32 con un raton es mas dificil de lo que
    /// parece, y el nombre de debajo forma parte de lo que uno cree estar
    /// pulsando.
    pub fn app_at(&self, p: &bmo::Pantalla, x: u32, y: u32) -> Option<usize> {
        let per_row = self.per_row(p);
        if per_row == 0 || y < GRID_Y || x < GRID_X {
            return None;
        }
        let col = (x - GRID_X) / CELL_W;
        let row = (y - GRID_Y) / CELL_H;
        if col >= per_row {
            return None;
        }
        let i = (row * per_row + col) as usize;
        if i < self.count {
            Some(i)
        } else {
            None
        }
    }

    fn per_row(&self, p: &bmo::Pantalla) -> u32 {
        if p.ancho <= GRID_X * 2 {
            return 0;
        }
        ((p.ancho - GRID_X * 2) / CELL_W).max(1)
    }

    fn cell(&self, p: &bmo::Pantalla, i: usize) -> (u32, u32) {
        let per_row = self.per_row(p).max(1);
        let col = (i as u32) % per_row;
        let row = (i as u32) / per_row;
        (GRID_X + col * CELL_W, GRID_Y + row * CELL_H)
    }
}

/// Pinta la rejilla entera. Va DESPUES del fondo y antes de las ventanas.
pub fn paint(p: &bmo::Pantalla, l: &Launcher) {
    for i in 0..l.count {
        paint_una(p, l, i);
    }
}

/// **Pinta UNA celda**: su realce si esta senalada, el icono y el nombre.
///
/// Existe aparte para que cambiar de icono senalado no cueste repintar los
/// doce. Con tres apps la diferencia no se nota; con doce y un fondo en
/// degradado, si -- y la regla no deberia depender de cuantas haya.
///
/// ** El que la llama para BORRAR un realce tiene que devolver el fondo antes
/// (`scene::erase_window` sobre [`Launcher::celda`]): aqui no se pinta fondo,
/// porque el fondo del escritorio es un degradado y esto no lo sabe calcular.
pub fn paint_una(p: &bmo::Pantalla, l: &Launcher, i: usize) {
    if i >= l.count {
        return;
    }
    let (cx, cy) = l.cell(p, i);
    let app = &l.apps[i];
    if l.sel == Some(i) {
        p.rect(cx + 2, cy - 4, CELL_W - 4, CELL_H - 8, SEL_BG);
        p.rect(cx + 2, cy - 4, CELL_W - 4, 1, ACCENT);
    }
    // El icono, centrado en la celda.
    let ix = cx + (CELL_W - ICON_PX) / 2;
    if app.tiene_icono {
        paint_pixels(p, ix, cy, &app.pixeles);
    } else {
        paint_default(p, ix, cy, app.name());
    }
    // El nombre debajo, centrado y SIN el `.bex`: la extension es la misma
    // en todos, asi que ocupa sitio y no distingue nada.
    let visible = without_extension(app.name());
    let width = visible.len() as u32 * 8;
    let tx = if width < CELL_W {
        cx + (CELL_W - width) / 2
    } else {
        cx
    };
    p.texto_bytes(tx, cy + ICON_PX + 6, visible, INK);
}

/// **Repinta la rejilla entera devolviendo antes el fondo de cada celda.**
///
/// Es lo que hay que hacer cuando cambia QUIEN esta senalado: el realce viejo
/// solo se va devolviendo el degradado que habia debajo, y eso [`paint_una`] no
/// lo sabe hacer -- ni debe, porque el fondo es del escritorio y no de la
/// rejilla.
///
/// ** Vive aqui y no en el manejador del raton porque hacia falta en DOS
/// sitios --al senalar y al soltar-- y dos copias del mismo bucle es como se
/// acaba teniendo dos que no borran lo mismo.
pub(crate) fn repintar(p: &bmo::Pantalla, c: &super::RunBox, l: &Launcher, visible: bool) {
    for k in 0..l.count() {
        let (cx, cy, cw, ch) = l.celda(p, k);
        super::erase_window(p, c, cx, cy - 4, cw, ch, visible);
        paint_una(p, l, k);
    }
}

/// El area que ocupa la rejilla, para que quien repinte el fondo sepa que
/// tiene que volver a pintar esto encima.
pub fn area(p: &bmo::Pantalla, l: &Launcher) -> (u32, u32, u32, u32) {
    if l.count == 0 {
        return (0, 0, 0, 0);
    }
    let per_row = l.per_row(p).max(1);
    let rows = ((l.count as u32) + per_row - 1) / per_row;
    let cols = (l.count as u32).min(per_row);
    (
        GRID_X,
        GRID_Y,
        cols * CELL_W,
        rows * CELL_H,
    )
}

fn paint_pixels(p: &bmo::Pantalla, x: u32, y: u32, px: &[u32; PIXELS]) {
    for fy in 0..ICON_SIDE {
        for fx in 0..ICON_SIDE {
            let c = px[(fy * ICON_SIDE + fx) as usize];
            // ** El canal alto a cero = TRANSPARENTE, y se salta.
            //
            // Sin esto un icono redondo se pinta dentro de su cuadro negro y el
            // escritorio se llena de sellos. Es un test de bit, no un
            // compositor: no hay mezcla, o esta o no esta.
            if c >> 24 == 0 {
                continue;
            }
            p.rect(x + fx * SCALE, y + fy * SCALE, SCALE, SCALE, c & 0x00FF_FFFF);
        }
    }
}

/// El icono de quien no trae icono: un cuadro de color con su inicial.
fn paint_default(p: &bmo::Pantalla, x: u32, y: u32, name: &[u8]) {
    let color = color_from(name);
    p.rect(x, y, ICON_PX, ICON_PX, color);
    // Un borde mas claro arriba y mas oscuro abajo: dos rectangulos y el cuadro
    // deja de parecer un agujero.
    p.rect(x, y, ICON_PX, 1, lighten(color));
    p.rect(x, y + ICON_PX - 1, ICON_PX, 1, BG_TOP);
    let initial = upper(name.first().copied().unwrap_or(b'?'));
    // `glifo_escala` a 2 mide 16x16; centrarlo es restar la mitad.
    p.glifo_escala(x + ICON_PX / 2 - 8, y + ICON_PX / 2 - 8, initial, 0x00FF_FFFF, 2);
}

/// Un color estable a partir del nombre.
///
/// La misma app tiene siempre el mismo, y dos apps distintas casi nunca el
/// mismo -- que es todo lo que se le pide. Se fija el brillo para que la
/// inicial blanca se lea encima: un hash suelto produce amarillos donde no se
/// ve nada.
fn color_from(name: &[u8]) -> u32 {
    let mut h: u32 = 2166136261;
    for &b in name {
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

fn lighten(c: u32) -> u32 {
    let r = (((c >> 16) & 0xFF) + 0x30).min(0xFF);
    let g = (((c >> 8) & 0xFF) + 0x30).min(0xFF);
    let b = ((c & 0xFF) + 0x30).min(0xFF);
    (r << 16) | (g << 8) | b
}

fn upper(b: u8) -> u8 {
    if b.is_ascii_lowercase() {
        b - 32
    } else {
        b
    }
}

/// **Lanzable**: `.bex`, y desde el 2026-08-22 tambien `.ibex`.
///
/// ## Por que son dos y no una
///
/// Son **el mismo formato**: un `.ibex` es un BEF, lo carga el mismo cargador y
/// lo lee el mismo gate. Lo que cambia es a que se ha comprometido quien lo
/// escribio -- declara su perfil, sus piezas y su mesa de katanas, y no llega al
/// disco si esa mesa no cuadra con sus bytes.
///
/// ** Asi que la extension no cambia como se lanza: cambia lo que se puede
/// afirmar de el sin abrirlo. Por eso aqui son dos nombres y una sola rama.
///
/// Se comprueba `.ibex` ANTES que `.bex` porque `.bex` no es sufijo de `.ibex`
/// --el punto lo impide-- pero un `ends_with` mal escrito lo seria, y de esos
/// hay uno en cada base de codigo.
fn ends_in_bex(n: &[u8]) -> bool {
    let l = n.len();
    (l >= 5 && n[l - 5..].eq_ignore_ascii_case(b".ibex"))
        || (l >= 4 && n[l - 4..].eq_ignore_ascii_case(b".bex"))
}

fn without_extension(n: &[u8]) -> &[u8] {
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
// Los offsets salen de `bef/header.rs`, `bef/sections.rs` y `bef/resources.rs`.
// Si alguno cambia, esto deja de encontrar iconos -- y eso es visible al
// primer arranque, que es lo mejor que le puede pasar a una divergencia.

const SECTION_RESOURCES: u8 = 0x0B;
const SPLASH_SECTION: usize = 48;
const HEADER_BRES: usize = 16;
const SPLASH_BRES: usize = 64;

/// Saca el recurso `icono` de un `.bex` y lo descifra como BICO.
/// `true` = habia icono y esta en `px`.
fn read_icon(path: &[u8], px: &mut [u32; PIXELS]) -> bool {
    let Ok(f) = bmo::Archivo::leer_de(path) else {
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
    let lookup = read_u64(&cab, 32) as u64;
    let count = read_u32(&cab, 40) as usize;
    if count == 0 || count > 255 {
        return false;
    }
    // -- Buscar la seccion de recursos --
    let mut sec_off = 0u64;
    let mut sec_len = 0u64;
    for i in 0..count {
        f.saltar(lookup + (i * SPLASH_SECTION) as u64);
        let mut e = [0u8; SPLASH_SECTION];
        if f.read(&mut e) < SPLASH_SECTION {
            return false;
        }
        if e[0] == SECTION_RESOURCES {
            sec_off = read_u64(&e, 8);
            sec_len = read_u64(&e, 16);
            break;
        }
    }
    if sec_len == 0 {
        return false;
    }
    // -- El indice BRES --
    f.saltar(sec_off);
    let mut bres = [0u8; HEADER_BRES];
    if f.read(&mut bres) < HEADER_BRES || &bres[0..4] != b"BRES" {
        return false;
    }
    let resources = read_u32(&bres, 4) as usize;
    for i in 0..resources.min(64) {
        f.saltar(sec_off + (HEADER_BRES + i * SPLASH_BRES) as u64);
        let mut e = [0u8; SPLASH_BRES];
        if f.read(&mut e) < SPLASH_BRES {
            return false;
        }
        let length = e[16] as usize;
        if length > 47 || &e[17..17 + length] != b"icono" {
            continue;
        }
        // El offset del recurso es RELATIVO A LA SECCION, no al fichero: por eso
        // se suma `sec_off`. Un offset absoluto habria que reescribirlo cada vez
        // que el `.bex` se vuelve a emitir con otra disposicion.
        let datum = sec_off + read_u64(&e, 0);
        let tam = read_u64(&e, 8) as usize;
        return read_bico(&f, datum, tam, px);
    }
    false
}

fn read_bico(f: &bmo::Archivo, off: u64, tam: usize, px: &mut [u32; PIXELS]) -> bool {
    const CAB: usize = 8;
    if tam < CAB + PIXELS * 4 {
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
    if read_u16(&cab, 4) as u32 != ICON_SIDE || read_u16(&cab, 6) as u32 != ICON_SIDE {
        return false;
    }
    let mut bytes = [0u8; PIXELS * 4];
    if f.read(&mut bytes) < bytes.len() {
        return false;
    }
    for i in 0..PIXELS {
        px[i] = read_u32(&bytes, i * 4);
    }
    true
}

fn read_u16(b: &[u8], o: usize) -> u16 {
    u16::from_le_bytes([b[o], b[o + 1]])
}

fn read_u32(b: &[u8], o: usize) -> u32 {
    u32::from_le_bytes([b[o], b[o + 1], b[o + 2], b[o + 3]])
}

fn read_u64(b: &[u8], o: usize) -> u64 {
    let mut v = [0u8; 8];
    v.copy_from_slice(&b[o..o + 8]);
    u64::from_le_bytes(v)
}
