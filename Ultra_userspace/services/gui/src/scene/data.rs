//! **La consola de DATOS** -- F12, el centro de control de ESTRATOS.
//!
//! === Por que una ventana aparte y no otro comando ===
//!
//! La caja de `Ejecutar` es de una linea: escribes una ruta y algo corre. Eso
//! sirve para lanzar; no sirve para **mirar un almacen**. Un volumen tiene
//! estado --generacion, ocupacion, identidad, nivel-- que se lee de un vistazo o
//! no se lee, y ponerlo detras de un comando obliga a teclearlo cada vez que
//! quieres saber si algo cambio.
//!
//! === Por que F12 y no un Ctrl+algo ===
//!
//! * **Una tecla de funcion no produce caracter en NINGUNA distribucion.** No
//! puede chocar con escribir, y eso es lo unico que importa en un atajo del
//! sistema. `Ctrl+Alt` ya lo tiene la ventana de Ejecutar **y es AltGr en
//! espanol** --lo que da `@ # [ ] \ | EUR`--, asi que ese atajo lleva una danza
//! entera para no romper el teclado: dispara al SOLTAR, y solo si no llego
//! ningun caracter mientras estaban pulsados. Encadenar otro combo encima
//! empeoraria justo lo que costo arreglar.
//!
//! Hasta hoy las teclas de funcion llegaban al kernel y morian ahi: la
//! distribucion no las resolvia a ningun byte. El hueco estaba limpio.
//!
//! === Lo que ENSENA, y lo que todavia no hace ===
//!
//! Ensena. Y dice, en alto, que todavia no escribe.
//!
//! La maquina de estados de la transaccion existe y esta probada
//! (`bmo_estratos::escritura`, 12 tests), pero **nadie la ha cableado al
//! dispositivo**: no hay `write` ni `FLUSH CACHE`. Esta ventana lo pone en
//! pantalla en vez de ofrecer un boton que no hace nada -- en un almacen, una
//! promesa de escritura que no ocurre es como se pierde el trabajo de alguien.

//! === * LAS DOS CARAS (spec del dueno, CUMPLIDA el 2026-08-03) ===
//!
//! `[numeros]` contesta *"como esta el almacen?"* -- generacion, espacio,
//! identidad, nivel. `[nodos]` contesta *"que hay dentro?"*. Son preguntas
//! distintas y por eso son dos pestanas y no una pantalla: meter un arbol entre
//! la generacion y la ocupacion deja las dos ilegibles. `TAB` cambia.
//!
//! La referencia que puso el dueno era buena y concreta: **un grafo tipo n8n** --
//! cajas con titulo y nombre, unidas por lineas, con color por clase. No una
//! lista con sangrias.
//!
//! El porque es el de siempre en este proyecto: **ESTRATOS no es un arbol de
//! carpetas, es un grafo de objetos** (nodos, atributos, flujos, estratos) que
//! se apuntan entre si y **nunca se sobreescriben**. Dibujarlo como una lista
//! indentada obliga a imaginarse las aristas; dibujarlo como lo que es se
//! entiende sin explicacion.
//!
//! Los cuatro puntos, y donde quedo cada uno:
//!
//! 1. [OK] Exponer a Ring 3 lo que el kernel ya sabia leer. Es el **cursor** de
//!    `ring0/fsys/estratos.rs`, dos operaciones de la superficie.
//! 2. [OK] Un color por clase, y el mismo en toda la ventana -- `class_color`.
//! 3. [OK] Caja con **titulo** (que es) y **nombre** (cual es), que es lo que el
//!    dueno pidio: *"con titulos y nombres para facilitar"*.
//! 4. o Teclado si: flechas, `ENTRAR` baja, `RETROCESO` sube, `RePag`/`AvPag`
//!    de cinco en cinco. **Con el raton, solo arrastrar la ventana**: pulsar una
//!    caja para seleccionarla todavia no esta.
//!
//! === Lo que sigue sin hacer, dicho ===
//!
//! - **Las versiones no se ven.** Cada commit deja los nodos viejos en pie, y
//!   eso en un grafo *se veria* -- es la historia del volumen dibujada. Hoy el
//!   cursor solo llega al estrato mas reciente.
//! - **Escribir sigue sin existir.** La maquina de estados de la transaccion
//!   esta probada y `sellar()` ya commitea, pero crear un objeto es otra cosa.
//!   Esta ventana lo dice en alto en vez de ofrecer un boton que no hace nada:
//!   en un almacen, una promesa de escritura que no ocurre es como se pierde el
//!   trabajo de alguien.

use bmo_userland as bmo;

use super::chrome::Chrome;
use super::*;
use crate::text::decimal;

// La ventana de Datos es VERDE porque es ESTRATOS, y eso se queda: el color
// dice de que ventana estas hablando antes de leer su titulo. Lo que cambia es
// el tono -- el verde de antes era de rotulador, escogido para verse en una foto
// de una pantalla que a lo mejor ni arrancaba.
const DATA_BG: u32 = 0x0013_1C18;
const DATA_TITLE_BG: u32 = 0x001B_2622;
/// El borde, discreto. Lo que separa la ventana del fondo es la sombra.
const DATA_EDGE: u32 = 0x002C_4038;
/// Y el acento verde, que si puede ser vivo: es una linea, no un marco.
const DATA_TITLE: u32 = 0x0034_D399;

/// Los cuatro niveles de `bmo_estratos::espacio`, con su color.
///
/// El orden es el del ABI (`INFO_ES_NIVEL`), no uno inventado aqui: si
/// divergieran, el panel pintaria en verde un volumen en solo lectura.
fn level_text(n: u64) -> (&'static str, u32) {
    match n {
        0 => ("holgado", INK_OK),
        1 => ("AVISO: por encima del 70%", 0x00F0_D070),
        2 => ("FAULT: por encima del 85%", INK_BAD),
        _ => ("SOLO LECTURA: por encima del 95%", INK_BAD),
    }
}

/// La ventana de Datos: **un marco y lo que hay dentro**.
///
/// Todo lo de mover, estirar, maximizar y los tres botones vive en
/// [`super::chrome::Chrome`] y no aqui. Lo que queda en esta estructura es lo
/// unico que de verdad es de ESTRATOS: que se esta ensenando y por donde va la
/// vista del arbol.
pub(crate) struct DataWindow {
    pub(crate) chrome: Chrome,
    /// Que se esta ensenando: los numeros o el arbol. Ver [`View`].
    pub(crate) view: View,
    /// Que hijo esta senalado en la vista de nodos.
    pub(crate) sel: usize,
    /// Primer hijo visible: la lista es mas larga que la ventana.
    pub(crate) from: usize,
    /// Lo que dijo la ultima verificacion de firma, si se pidio alguna.
    ///
    /// `None` es "no se ha preguntado", y **no es lo mismo que "sin firma"**:
    /// ensenar `sin firma` sin haber mirado seria contestar por el disco.
    /// Se borra al cambiar de nodo -- el resultado es de UN archivo.
    pub(crate) verified: Option<u64>,
    /// Por donde va el sellado. Ver [`Seal`].
    pub(crate) seal: Seal,
}

/// **El estado del sellado, que es lo unico de esta ventana que ESCRIBE.**
///
/// ## Por que hay un estado y no una tecla a secas
///
/// La orden vivia en el terminal principal como `estratos sellar` -- dos
/// palabras, para que hiciera falta escribirlo queriendo. El dueno la pidio el
/// 2026-08-13 y no la encontro, teniendola delante.
///
/// Se mudo aqui, que es su sitio: **el verbo vive donde vive el objeto**. Pero
/// una tecla suelta en una ventana donde se pulsan flechas seria peor que las
/// dos palabras que se quitaron. De ahi los dos tiempos:
///
///   `S` -> la barra del pie pregunta -> `S` otra vez -> se sella
///
/// Cualquier otra tecla lo cancela, y cambiar de pestana tambien. Es la misma
/// idea que las dos palabras --que haga falta quererlo-- pero **dicha en
/// pantalla en vez de escondida en un parser**.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Seal {
    /// Nadie ha pedido nada.
    Idle,
    /// Se pulso `S` una vez: falta confirmar.
    Asking,
    /// Se sello, y esta es la generacion que contesto el kernel.
    Done(u64),
    /// Se intento y el kernel dijo que no. El motivo esta en F11.
    Failed,
}

/// Lo que se puede encoger sin que deje de servir. Por debajo de esto el grafo
/// no cabe y los numeros se cortan -- una ventana que se puede dejar inservible
/// con el raton es una trampa, no una libertad.
pub(crate) const DATA_MIN_W: u32 = 460;
pub(crate) const DATA_MIN_H: u32 = 260;
/// Y el tamano con el que nace, **en tantos por ciento de la pantalla**. Un
/// `640 x 330` en pixeles es correcto en una pantalla y en ninguna otra.
const DATA_PCT_W: u32 = 46;
const DATA_PCT_H: u32 = 44;

/// Las dos caras de esta ventana.
///
/// La de numeros contesta *como esta el almacen?* y la de nodos *que hay
/// dentro?*. Son preguntas distintas y por eso no se mezclan en una pantalla:
/// meter un arbol entre la generacion y la ocupacion deja las dos ilegibles.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum View {
    Numbers,
    Nodes,
    /// ** LA TERCERA CARA: el mismo volumen como CARPETAS.
    ///
    /// Peticion del dueno el 2026-08-13: *"el TAB ese mismo es EXPLORADOR como
    /// Windows 11 para ver carpetas"*.
    ///
    /// Y sale casi gratis, que es lo que la hace buena idea: **no lee otra
    /// cosa**. Es el MISMO cursor de ESTRATOS que alimenta el grafo --
    /// `hijo_nombre`, `hijo_tipo`, `hijo_bytes`, `entrar`, `subir`-- pintado
    /// como una lista en vez de como cajas con aristas. Las teclas tampoco
    /// cambian: el despacho de `main.rs` da la navegacion a todo lo que no sea
    /// `Numbers`, asi que esta vista la hereda entera.
    ///
    /// Y esa es exactamente la idea que el dueno lleva describiendo desde el
    /// principio: **un explorador con dos modos sobre el mismo dato**. El
    /// familiar para trabajar y el de grafo para ver como se conecta. No son dos
    /// programas: son dos `paint`.
    Folders,
}

// El alto de la barra de titulo --que es el asa-- sale de `super::TITLE_H`:
// el mismo que la caja de Ejecutar. Dos ventanas del mismo sistema con barras
// de distinta altura se ven como dos programas de distinta epoca.

impl DataWindow {
    pub(crate) fn new(p: &bmo::Pantalla) -> Self {
        Self {
            chrome: Chrome::new(
                p,
                DATA_PCT_W,
                DATA_PCT_H,
                DATA_MIN_W,
                DATA_MIN_H,
            ),
            view: View::Numbers,
            sel: 0,
            from: 0,
            verified: None,
            seal: Seal::Idle,
        }
    }

    /// * **Sobre que caja del grafo esta el puntero**, si sobre alguna.
    ///
    /// `None` es "en ninguna" y `Some(usize::MAX)` es **la caja del padre**, la
    /// de la izquierda: pulsarla sube un nivel, que es el gesto que la mano
    /// busca sola cuando ya se ha bajado.
    ///
    /// La geometria se calcula IGUAL que en `paint_nodes` y ese es el riesgo
    /// de este metodo: si una de las dos cambia y la otra no, se pulsa una caja
    /// y se selecciona otra. Las dos usan las mismas constantes y el mismo
    /// reparto del ancho a proposito.
    pub(crate) fn box_at(&self, px: u32, py: u32, how_many: usize) -> Option<usize> {
        if self.view != View::Nodes || self.chrome.minimized {
            return None;
        }
        let (tx, box_w, children_x, first_y) = self.graph_geometry();
        // La del padre?
        if px >= tx && px < tx + box_w && py >= first_y && py < first_y + NODE_H {
            return Some(usize::MAX);
        }
        if px < children_x || px >= children_x + box_w || py < first_y {
            return None;
        }
        let step = NODE_H + NODE_GAP;
        let row = ((py - first_y) / step) as usize;
        // El hueco ENTRE dos cajas no es ninguna de las dos. Sin esta guarda,
        // pulsar en el aire seleccionaria la de arriba.
        if (py - first_y) % step >= NODE_H {
            return None;
        }
        let i = self.from + row;
        if i < how_many && row < self.fit_count() { Some(i) } else { None }
    }

    /// El reparto del grafo: `(x del padre, ancho de caja, x de los hijos, y de
    /// la primera fila)`. **Lo comparten quien pinta y quien acierta.**
    fn graph_geometry(&self) -> (u32, u32, u32, u32) {
        const CHANNEL: u32 = 44;
        let tx = self.chrome.x + 16;
        let usable = self.chrome.width.saturating_sub(32);
        let box_w = ((usable.saturating_sub(CHANNEL)) / 2).max(NODE_MIN);
        let children_x = tx + box_w + CHANNEL;
        let first_y = self.chrome.y + TITLE_H + 6 + bmo::GLIFO_ALTO + 10 + 4;
        (tx, box_w, children_x, first_y)
    }

    // Los atajos de siempre, para no escribir `.chrome.` en cada uso. Son
    // reenvios y nada mas: la logica vive en `Chrome` y aqui no se repite.
    pub(crate) fn x(&self) -> u32 { self.chrome.x }
    pub(crate) fn y(&self) -> u32 { self.chrome.y }
    pub(crate) fn width(&self) -> u32 { self.chrome.width }
    pub(crate) fn height(&self) -> u32 { self.chrome.height }

    /// Este pixel cae dentro? Lo necesita el borrado para saber que repintar.
    pub(crate) fn contains(&self, px: u32, py: u32) -> bool {
        self.chrome.contains(px, py)
    }

    /// Tras cambiar de tamano, la seleccion puede haber quedado fuera de lo que
    /// se pinta. Se recoloca la ventana de scroll -- si no, encoger dejaria el
    /// cursor senalando una caja que ya no esta en pantalla.
    pub(crate) fn relayout(&mut self) {
        let fit_count = self.fit_count();
        if self.sel >= self.from + fit_count {
            self.from = self.sel + 1 - fit_count;
        }
    }

    /// Cuantas cajas de hijo caben de una vez en la vista de nodos.
    fn fit_count(&self) -> usize {
        let usable = self.chrome.height.saturating_sub(TITLE_H + 56);
        (usable / (NODE_H + NODE_GAP)).max(1) as usize
    }

    /// Mueve la seleccion y arrastra la ventana de scroll con ella.
    pub(crate) fn move_sel(&mut self, delta: i32, how_many: usize) {
        if how_many == 0 {
            self.sel = 0;
            self.from = 0;
            return;
        }
        let limit = how_many - 1;
        self.sel = (self.sel as i32 + delta).clamp(0, limit as i32) as usize;
        let fit_count = self.fit_count();
        if self.sel < self.from {
            self.from = self.sel;
        } else if self.sel >= self.from + fit_count {
            self.from = self.sel + 1 - fit_count;
        }
    }

    /// Vuelve al principio de la lista. Se llama al cambiar de nodo: dejar la
    /// seleccion donde estaba haria que entrar en un directorio de dos hijos
    /// senalara al septimo, que no existe.
    pub(crate) fn to_top(&mut self) {
        self.sel = 0;
        self.from = 0;
    }
}

// -- El GRAFO ----------------------------------------------------------------
//
// * La spec del dueno, cumplida: **un grafo tipo n8n** -- cajas con titulo y
// nombre, unidas por lineas, con color por clase. No una lista con sangrias.
//
// El porque es el de siempre en este proyecto: **ESTRATOS no es un arbol de
// carpetas, es un grafo de objetos** que se apuntan entre si y nunca se
// sobreescriben. Dibujarlo como una lista indentada obliga a imaginarse las
// aristas; dibujarlo como lo que es se entiende sin explicacion.

/// Ancho MINIMO de una caja. El de verdad sale del ancho de la ventana: al
/// estirarla, las cajas crecen y caben nombres mas largos. Una caja de tamano
/// fijo dentro de una ventana que se estira deja un desierto a la derecha.
const NODE_MIN: u32 = 170;
const NODE_H: u32 = 40;
const NODE_GAP: u32 = 12;
const SHADOW_NODE: u32 = 0x000B_100E;
/// Las aristas del grafo. Mas claras que el borde de la ventana **a proposito**:
/// son lo que hay que seguir con la vista, y una linea del mismo tono que el
/// marco se pierde entre los marcos.
const DATA_EDGE_LINE: u32 = 0x0045_6B5C;
/// El cuerpo de una caja del grafo: un peldano por encima de la ventana, que es
/// la misma regla que separa la ventana del escritorio.
const NODE_BG: u32 = 0x001B_2622;
/// Y la senalada, otro peldano mas. La profundidad se lee sola.
const NODE_SEL: u32 = 0x0024_332C;

/// **Un color por clase, y el mismo en toda la ventana.** Es el punto 2 de la
/// spec: si el verde significara una cosa en el padre y otra en los hijos, el
/// color dejaria de informar y solo decoraria.
fn class_color(kind: u64) -> (&'static str, u32) {
    match kind {
        bmo::estratos::DIRECTORIO => ("directorio", 0x0057_C8F0),
        bmo::estratos::ARCHIVO => ("archivo", 0x007E_E787),
        _ => ("ilegible", INK_BAD),
    }
}

/// Una caja con **titulo** (que es) y **nombre** (cual es). Punto 3 de la spec.
///
/// El titulo va arriba y en el color de la clase; el nombre, debajo y en
/// blanco. Al reves se leeria el nombre y habria que buscar el tipo, que es lo
/// contrario de para que esta el color.
fn node_box(
    p: &bmo::Pantalla,
    x: u32,
    y: u32,
    width: u32,
    kind: u64,
    name: &[u8],
    pointed_at: bool,
) {
    let (title, color) = class_color(kind);
    // La senalada lleva el borde del acento y un cuerpo un punto mas claro. Un
    // borde blanco a secas se lee como "esto esta roto"; el realce de una
    // seleccion tiene que ser el color del sistema, no una alarma.
    let (edge, cuerpo) = if pointed_at {
        (color, NODE_SEL)
    } else {
        (DATA_EDGE, NODE_BG)
    };
    // Sombra propia. Es lo que separa las cajas del fondo de la ventana y lo
    // que hace que un grafo parezca un grafo y no una lista con marcos.
    rounded_rect(p, x + 2, y + 3, width, NODE_H, SHADOW_NODE);
    rounded_rect(p, x, y, width, NODE_H, edge);
    rounded_rect(p, x + 1, y + 1, width - 2, NODE_H - 2, cuerpo);

    // * El PUNTO de clase, no una pestana lateral.
    //
    // La barra pegada al borde peleaba con la curva de la esquina y se veia
    // como un defecto. Un punto delante del titulo es el mismo idioma que usan
    // la barra del sistema y las dos ventanas: se lee la clase de un vistazo y
    // no depende de que el titulo quepa.
    p.rect(x + 11, y + 7, 7, 7, color);
    p.texto(x + 24, y + 5, title, color);

    // El nombre, en su propia linea y en blanco. El titulo dice QUE es y el
    // nombre CUAL es; ponerlos del mismo color obliga a leer los dos para
    // saber cual es cual.
    let fits = ((width.saturating_sub(28)) / bmo::GLIFO_ANCHO) as usize;
    // Recortar por el final y no por el principio: los nombres de un volumen
    // se distinguen por delante (`maestro.bex`, `movim.txt`), no por detras.
    let n = name.len().min(fits);
    p.texto_bytes(x + 24, y + 5 + bmo::GLIFO_ALTO + 3, &name[..n], INK);
    // Si no cupo entero se dice con un punto, no cortando a lo bruto: una
    // ventana estrecha no puede hacer que dos archivos parezcan el mismo.
    if n < name.len() {
        p.texto(
            x + 24 + n as u32 * bmo::GLIFO_ANCHO,
            y + 5 + bmo::GLIFO_ALTO + 3,
            "~",
            INK_DIM,
        );
    }
}

/// La vista de NODOS: el nodo actual a la izquierda y sus hijos a la derecha,
/// unidos por una espina y sus ramas.
/// El alto de una fila del explorador. Una linea de texto y aire a los lados:
/// lo justo para que el realce de la seleccion no toque las letras.
const ROW_H: u32 = 22;

/// **La vista de CARPETAS: el mismo volumen, como lo ensena un explorador.**
///
/// Comparte cursor con el grafo --literalmente el mismo, no una copia-- asi que
/// entrar aqui y salir al grafo deja el sitio donde estaba. Es lo que hace que
/// las dos pestanas se sientan una sola cosa vista de dos maneras, que era la
/// peticion.
///
/// Tres columnas y ni una mas: **nombre, que es, cuanto ocupa**. Un explorador
/// que ensena diez columnas por defecto obliga a leerlas todas para encontrar la
/// unica que importaba.
fn paint_folders(p: &bmo::Pantalla, c: &DataWindow) {
    let tx = c.chrome.x + 16;
    let mut ty = c.chrome.y + TITLE_H + 6;

    if bmo::info(bmo::INFO_ES_MONTADO) == 0 {
        p.texto(tx, ty, "ningun volumen ESTRATOS montado.", INK_BAD);
        return;
    }
    let hondo = bmo::estratos::hondo();
    let how_many = bmo::estratos::hijos() as usize;

    // La miga de pan, igual que en el grafo: sin ella dos carpetas con los
    // mismos nombres dentro se ven identicas.
    {
        let mut x = p.texto(tx, ty, "/", DATA_TITLE);
        let mut level = 1u64;
        while level <= hondo {
            let mut nom = [0u8; 40];
            let n = bmo::estratos::nombre_nivel(level, &mut nom);
            x = p.texto(x + 2, ty, " > ", INK_DIM);
            let ink = if level == hondo { INK } else { INK_DIM };
            x = p.texto_bytes(x, ty, &nom[..n], ink);
            level += 1;
        }
    }
    ty += bmo::GLIFO_ALTO + 8;

    // La cabecera de columnas, y su linea. Las `x` salen del ancho del marco
    // para que estirar la ventana de sitio a los nombres largos, que es para lo
    // que se estira.
    let width = c.chrome.width.saturating_sub(32);
    let col_kind = tx + (width * 55) / 100;
    let col_size = tx + (width * 78) / 100;
    p.texto(tx + 22, ty, "nombre", INK_DIM);
    p.texto(col_kind, ty, "que es", INK_DIM);
    p.texto(col_size, ty, "bytes", INK_DIM);
    ty += bmo::GLIFO_ALTO + 3;
    p.rect(tx, ty, width, 1, DATA_EDGE);
    ty += 4;

    if how_many == 0 {
        p.texto(tx + 22, ty + 4, "esta vacio.", INK_DIM);
        return;
    }

    // Cuantas filas caben. Se descuenta la barra de teclas del pie: una lista
    // que se pinta por debajo de su propia leyenda es una lista que miente
    // sobre cuanto hay.
    let to = c.chrome.y + c.chrome.height - bmo::GLIFO_ALTO - 14;
    let fit_count = ((to.saturating_sub(ty)) / ROW_H).max(1) as usize;
    let last = (c.from + fit_count).min(how_many);

    let mut i = c.from;
    while i < last {
        let (type_name, color) = class_color(bmo::estratos::hijo_tipo(i as u64));
        // El realce de la fila senalada. Va DEBAJO del texto y ocupa el ancho
        // entero: es como se lee "esta es la seleccionada" sin un cursor.
        if i == c.sel {
            p.rect(tx, ty, width, ROW_H, NODE_SEL);
        }
        // El cuadrito de color: dice la clase antes de leer la columna. Es el
        // mismo color que su caja en el grafo, a proposito -- cambiar de
        // pestana no puede cambiarle el color a un nodo.
        p.rect(tx + 4, ty + (ROW_H - 8) / 2, 8, 8, color);

        let mut nom = [0u8; 64];
        let n = bmo::estratos::hijo_nombre(i as u64, &mut nom);
        let ty_texto = ty + (ROW_H - bmo::GLIFO_ALTO) / 2;
        p.texto_bytes(tx + 22, ty_texto, &nom[..n], INK);
        p.texto(col_kind, ty_texto, type_name, INK_DIM);
        let mut b = [0u8; 10];
        let nb = decimal(bmo::estratos::hijo_bytes(i as u64), &mut b);
        p.texto_bytes(col_size, ty_texto, &b[..nb], INK_DIM);

        ty += ROW_H;
        i += 1;
    }

    // Y si hay mas de los que caben, se DICE. Una lista recortada en silencio
    // se ve igual que una carpeta con pocos archivos.
    if last < how_many {
        let mut b = [0u8; 10];
        let nb = decimal((how_many - last) as u64, &mut b);
        let x = p.texto(tx + 22, ty + 2, "y ", INK_DIM);
        let x = p.texto_bytes(x, ty + 2, &b[..nb], INK);
        p.texto(x, ty + 2, " mas abajo", INK_DIM);
    }
}

fn paint_nodes(p: &bmo::Pantalla, c: &DataWindow) {
    let tx = c.chrome.x + 16;
    let mut ty = c.chrome.y + TITLE_H + 6;

    if bmo::info(bmo::INFO_ES_MONTADO) == 0 {
        p.texto(tx, ty, "ningun volumen ESTRATOS montado.", INK_BAD);
        return;
    }
    if !bmo::estratos::a_la_raiz() && bmo::estratos::tipo() == bmo::estratos::NOTHING {
        p.texto(tx, ty, "el volumen monta pero no tiene raiz legible.", INK_BAD);
        ty += bmo::GLIFO_ALTO + 4;
        p.texto(tx, ty, "el motivo esta en F11.", INK_DIM);
        return;
    }

    let hondo = bmo::estratos::hondo();
    let how_many = bmo::estratos::hijos() as usize;

    // -- * LA MIGA DE PAN, con nombres de verdad --
    //
    // Antes ponia `profundidad 2`, y eso no dice DONDE estas: dos carpetas
    // distintas con los mismos nombres dentro se veian identicas. Ahora es
    // `/ > cobol > 10`, que es la unica forma de saber que estas mirando.
    //
    // Los nombres los guarda el cursor AL BAJAR, porque despues ya no se
    // saben: un nodo no sabe como se llama -- el nombre vive en la entrada de
    // su padre.
    {
        let mut x = p.texto(tx, ty, "/", DATA_TITLE);
        let mut level = 1u64;
        while level <= hondo {
            let mut nom = [0u8; 40];
            let n = bmo::estratos::nombre_nivel(level, &mut nom);
            x = p.texto(x + 2, ty, " > ", INK_DIM);
            // El ultimo tramo en blanco y los de antes apagados: se lee de un
            // vistazo donde estas sin perder de donde vienes.
            let ink = if level == hondo { INK } else { INK_DIM };
            x = p.texto_bytes(x, ty, &nom[..n], ink);
            level += 1;
        }
        let mut b = [0u8; 10];
        let x = p.texto(x + 3 * bmo::GLIFO_ANCHO, ty, "hijos ", INK_DIM);
        let n = decimal(how_many as u64, &mut b);
        let x = p.texto_bytes(x, ty, &b[..n], INK);
        if bmo::estratos::truncado() {
            // Se DICE. Un listado recortado en silencio se ve igual que un
            // directorio con pocos archivos, y esa confusion cuesta horas.
            p.texto(x, ty, "  (RECORTADO)", INK_BAD);
        }
    }
    ty += bmo::GLIFO_ALTO + 10;

    // -- * EL REPARTO DEL ANCHO --
    //
    // Las cajas ya no miden lo mismo pase lo que pase: el ancho util se parte
    // entre las dos columnas y el canal de las ramas. Estirar la ventana hace
    // que quepan nombres mas largos, que es para lo que uno la estira.
    //
    // ** Y sale de `graph_geometry`, **la misma que usa el acierto del
    // raton**. Tenerlo dos veces era garantizar que un dia se pulsara una caja
    // y se seleccionara otra: dos copias de una geometria se separan solas.
    let (_, box_w, children_x, first_y) = c.graph_geometry();

    // -- El nodo actual, a la izquierda --
    let parent_y = first_y;
    // El nombre del padre: en la raiz `/`, y si no, el ultimo tramo de la ruta.
    let mut parent_name = [0u8; 40];
    let np = if hondo == 0 {
        parent_name[0] = b'/';
        1
    } else {
        bmo::estratos::nombre_nivel(hondo, &mut parent_name)
    };
    node_box(p, tx, parent_y, box_w, bmo::estratos::tipo(), &parent_name[..np], false);
    if hondo > 0 {
        // Se dice que se puede subir, y como. Un gesto que existe y no esta
        // escrito lo conoce solo quien lo programo.
        p.texto(tx, parent_y + NODE_H + 6, "clic aqui SUBE", INK_DIM);
    }

    if how_many == 0 {
        p.texto(tx, parent_y + NODE_H + 22, "esta vacio.", INK_DIM);
        return;
    }

    // -- Las aristas: UNA CURVA POR HIJO, del padre a cada caja --
    //
    // ** ESTO ERA UNA ESPINA CON CODOS, Y EL COMENTARIO QUE LO JUSTIFICABA
    // DECIA UNA COSA FALSA.
    //
    // Decia: *"sin primitiva de linea: un rectangulo de un pixel de ancho ES
    // una linea, y para un grafo de codos --que es como pinta n8n-- no hace
    // falta mas"*. Lo primero es cierto y lo segundo no: **n8n une sus nodos
    // con curvas Bezier**, con las tangentes horizontales en los dos extremos.
    //
    // Y la diferencia no es estetica. En una espina con codos, todas las ramas
    // salen del MISMO tramo vertical: mirando una caja no se sabe por donde
    // llego, porque su rama es identica a las otras. Con una curva por hijo,
    // cada arista tiene su propio recorrido de la salida del padre a la entrada
    // del hijo, **y se puede seguir con el dedo**. Eso es lo que convierte un
    // cuadro de tuberias en un grafo.
    //
    // Los tirantes van horizontales y a media distancia (`CHANNEL/2`), que es lo
    // que hace que la curva salga y entre en horizontal aunque el hijo este
    // muy abajo: la clasica S. Ver `bmo::curve`.
    let fit_count = c.fit_count();
    let last = (c.from + fit_count).min(how_many);
    // El recorte, que hasta ahora no existia: las aristas no pueden salirse del
    // marco de la ventana. Con codos calculados a mano no hacia falta --nunca
    // se salian por construccion--; una curva se sale en cuanto el marco se
    // encoge, y sin esto pintaria por encima de lo que haya al lado.
    let rec = bmo::Recorte::nuevo(
        c.chrome.x as i32 + 1,
        (c.chrome.y + TITLE_H) as i32,
        c.chrome.width as i32 - 2,
        c.chrome.height as i32 - TITLE_H as i32 - 1,
    );
    let out_y = parent_y + NODE_H / 2;
    let out_x = tx + box_w;
    // El tirante: la mitad del canal. Sale de la geometria y no de un numero a
    // ojo, asi que estirar la ventana no descuadra las curvas.
    let taut = ((children_x - out_x) / 2) as i32;

    let mut hy = first_y;
    for i in c.from..last {
        let center = hy + NODE_H / 2;
        p.curva(
            &rec,
            (out_x as i32, out_y as i32),
            (out_x as i32 + taut, out_y as i32),
            (children_x as i32 - taut, center as i32),
            (children_x as i32, center as i32),
            DATA_EDGE_LINE,
        );
        // La punta de flecha, que es el escalon 2 ganandose el sitio: dice
        // hacia DONDE va la arista, que con una curva ya no es obvio si se mira
        // solo un tramo. Tres vertices y entra en la caja por su borde.
        p.triangulo(
            &rec,
            (children_x as i32, center as i32),
            (children_x as i32 - 7, center as i32 - 4),
            (children_x as i32 - 7, center as i32 + 4),
            DATA_EDGE_LINE,
        );
        let kind = bmo::estratos::hijo_tipo(i as u64);
        let mut name = [0u8; 64];
        let n = bmo::estratos::hijo_nombre(i as u64, &mut name);
        node_box(p, children_x, hy, box_w, kind, &name[..n], i == c.sel);
        hy += NODE_H + NODE_GAP;
    }
    // El punto de salida en el padre: cierra las aristas en su origen en vez de
    // dejarlas naciendo de un borde. Con una sola espina hacia falta uno; con
    // una curva por hijo, todas salen de aqui y por eso se nota mas.
    p.rect(out_x - 1, out_y - 2, 5, 5, DATA_EDGE_LINE);

    // -- * EL PANEL DE DETALLE del nodo senalado --
    //
    // Un grafo que solo ensena nombres contesta *que hay*; no contesta *que es
    // esto*. Va al PIE y en una linea: es informacion de apoyo, y un panel
    // lateral se comeria el ancho que las cajas necesitan para sus nombres.
    if c.sel < how_many {
        let dy = c.chrome.y + c.chrome.height - TITLE_H - bmo::GLIFO_ALTO - 2;
        p.rect(c.chrome.x + 1, dy - 6, c.chrome.width - 2, 1, DATA_EDGE);
        let mut b = [0u8; 10];
        let x = p.texto(tx, dy, "sel: ", INK_DIM);
        let n = decimal(bmo::estratos::hijo_bytes(c.sel as u64), &mut b);
        let x = p.texto_bytes(x, dy, &b[..n], INK);
        let x = p.texto(x, dy, " B   atributos ", INK_DIM);
        let n = decimal(bmo::estratos::hijo_atributos(c.sel as u64), &mut b);
        let x = p.texto_bytes(x, dy, &b[..n], INK);
        // La firma. **Se dice si la LLEVA; que CUADRE se pide con V** -- leer el
        // archivo entero y hacerle el BLAKE3 en cada repintado convertiria un
        // panel en un martillo sobre el disco.
        let x = p.texto(x, dy, "   firma ", INK_DIM);
        let x = if bmo::estratos::hijo_firmado(c.sel as u64) {
            p.texto(x, dy, "SI", INK_OK)
        } else {
            p.texto(x, dy, "no", INK_DIM)
        };
        // Y el resultado de la ultima verificacion, si se pidio.
        match c.verified {
            None => {
                p.texto(x + 2 * bmo::GLIFO_ANCHO, dy, "V comprueba", INK_DIM);
            }
            Some(bmo::estratos::FIRMA_CUADRA) => {
                p.texto(x + 2 * bmo::GLIFO_ANCHO, dy, "CUADRA", INK_OK);
            }
            Some(bmo::estratos::FIRMA_NO_CUADRA) => {
                // El unico mensaje de esta ventana que significa "hay un
                // problema en el disco". Por eso es el unico en rojo.
                p.texto(x + 2 * bmo::GLIFO_ANCHO, dy, "NO CUADRA", INK_BAD);
            }
            Some(bmo::estratos::FIRMA_AUSENTE) => {
                p.texto(x + 2 * bmo::GLIFO_ANCHO, dy, "sin firma", INK_DIM);
            }
            Some(bmo::estratos::FIRMA_NO_CABE) => {
                // TENUE y no rojo: el archivo esta bien, lo que no cabe es
                // nuestro buffer de comprobacion. Pintarlo en rojo mandaba a
                // buscar una corrupcion que no existe.
                p.texto(x + 2 * bmo::GLIFO_ANCHO, dy, "no cabe (>256 KiB)", INK_DIM);
            }
            _ => {
                p.texto(x + 2 * bmo::GLIFO_ANCHO, dy, "no se pudo leer", INK_BAD);
            }
        }
    }

    // Si la lista no cabe entera, decirlo con numeros y no con puntos
    // suspensivos: "3-8 de 40" se lee; "..." no dice cuanto falta.
    if how_many > fit_count {
        let mut b = [0u8; 10];
        let y = c.chrome.y + c.chrome.height - TITLE_H - bmo::GLIFO_ALTO;
        let n = decimal(c.from as u64 + 1, &mut b);
        let x = p.texto_bytes(children_x, y, &b[..n], INK_DIM);
        let x = p.texto(x, y, "-", INK_DIM);
        let n = decimal(last as u64, &mut b);
        let x = p.texto_bytes(x, y, &b[..n], INK_DIM);
        let x = p.texto(x, y, " de ", INK_DIM);
        let n = decimal(how_many as u64, &mut b);
        p.texto_bytes(x, y, &b[..n], INK_DIM);
    }
}

/// Pinta la consola de datos entera.
///
/// Se redibuja completa en cada invocacion y no por fotograma: los numeros de
/// un volumen no cambian solos mientras nadie escriba, y repintar 200k pixeles
/// sobre memoria de video sin cache sesenta veces por segundo para ensenar los
/// mismos digitos es tirar el fotograma.
pub(crate) fn paint(p: &bmo::Pantalla, c: &DataWindow) {
    if c.chrome.minimized {
        return;
    }
    // * El cromo entero --sombra, borde, cuerpo, barra, los tres botones y el
    // asa de la esquina-- lo pinta el MARCO. Aqui solo van los colores, que si
    // son de esta ventana: el verde dice ESTRATOS antes de que nadie lea el
    // titulo.
    c.chrome.paint_chrome(p, DATA_EDGE, DATA_BG, DATA_TITLE_BG, DATA_TITLE);

    let tx = c.chrome.x + 16;
    p.rect(tx, c.chrome.y + 9, 8, 8, DATA_TITLE);
    let px = p.texto(tx + 16, c.chrome.y + 8, "ESTRATOS", INK);
    let px = px + 2 * bmo::GLIFO_ANCHO;
    // Las pestanas: la activa lleva su subrayado. Un corchete pintado de otro
    // color se pierde en una foto; una linea debajo no.
    let (c1, c2, c3) = match c.view {
        View::Numbers => (INK, INK_DIM, INK_DIM),
        View::Nodes => (INK_DIM, INK, INK_DIM),
        View::Folders => (INK_DIM, INK_DIM, INK),
    };
    let fin1 = p.texto(px, c.chrome.y + 8, "numeros", c1);
    let px2 = fin1 + 2 * bmo::GLIFO_ANCHO;
    let fin2 = p.texto(px2, c.chrome.y + 8, "nodos", c2);
    let px3 = fin2 + 2 * bmo::GLIFO_ANCHO;
    let fin3 = p.texto(px3, c.chrome.y + 8, "carpetas", c3);
    let (sx, sw) = match c.view {
        View::Numbers => (px, fin1 - px),
        View::Nodes => (px2, fin2 - px2),
        View::Folders => (px3, fin3 - px3),
    };
    p.rect(sx, c.chrome.y + 8 + bmo::GLIFO_ALTO + 2, sw, 2, DATA_TITLE);

    if c.view == View::Nodes || c.view == View::Folders {
        if c.view == View::Nodes {
            paint_nodes(p, c);
        } else {
            paint_folders(p, c);
        }
        let y = c.chrome.y + c.chrome.height - bmo::GLIFO_ALTO - 8;
        // ** `S sella` DICHO EN LA BARRA, y esa es la mitad del arreglo.
        //
        // La orden de sellar existia desde hacia dias y **no estaba escrita en
        // ningun sitio que se vea**: el dueno la busco teniendola delante. Una
        // funcion que no se anuncia no es discreta, es una funcion que no esta.
        match c.seal {
            Seal::Asking => p.texto(
                tx, y,
                "S OTRA VEZ para SELLAR (escribe en el disco)   otra tecla cancela",
                0x00F0_D070,
            ),
            Seal::Done(g) => {
                let x = p.texto(tx, y, "SELLADO. generacion ", INK_OK);
                let mut b = [0u8; 10];
                let n = decimal(g, &mut b);
                let x = p.texto_bytes(x, y, &b[..n], INK_OK);
                p.texto(x, y, "   reinicia y mirala otra vez: eso prueba la barrera", INK_DIM)
            }
            Seal::Failed => p.texto(
                tx, y,
                "NO se sello. el volumen sigue igual; el motivo esta en F11.",
                INK_BAD,
            ),
            Seal::Idle => p.texto(
                tx, y,
                "flechas mueven  ENTRAR baja  RETROCESO sube  V firma  S sella  F12 cierra",
                INK_DIM,
            ),
        };
        return;
    }

    let mut ty = c.chrome.y + TITLE_H + 6;

    if bmo::info(bmo::INFO_ES_MONTADO) == 0 {
        p.texto(tx, ty, "ningun volumen ESTRATOS montado.", INK_BAD);
        ty += bmo::GLIFO_ALTO + 4;
        p.texto(tx, ty, "se formatea desde el anfitrion con estratos-fmt.", INK_DIM);
        return;
    }

    let bloques = bmo::info(bmo::INFO_ES_BLOQUES);
    let used_count = bmo::info(bmo::INFO_ES_USADOS);
    let tam = bmo::info(bmo::INFO_ES_BLOQUE_TAM).max(1);
    let level = bmo::info(bmo::INFO_ES_NIVEL);

    let row = |label: &str, y: &mut u32, pinta: &dyn Fn(u32, u32)| {
        p.texto(tx, *y, label, INK_DIM);
        pinta(tx + 13 * bmo::GLIFO_ANCHO, *y);
        *y += bmo::GLIFO_ALTO + 3;
    };

    row("generacion", &mut ty, &|x, y| {
        let g = bmo::info(bmo::INFO_ES_GENERACION);
        let mut b = [0u8; 10];
        let n = decimal(g, &mut b);
        let x = p.texto_bytes(x, y, &b[..n], INK);
        p.texto(x, y, "  transacciones desde el formateo", INK_DIM);
    });

    row("espacio", &mut ty, &|x, y| {
        let x = magnitude(p, x, y, used_count * tam, INK);
        let x = p.texto(x, y, " de ", INK_DIM);
        let x = magnitude(p, x, y, bloques * tam, INK);
        let pct = if bloques == 0 { 0 } else { used_count * 100 / bloques };
        let x = p.texto(x, y, "   ", INK_DIM);
        let mut b = [0u8; 10];
        let n = decimal(pct, &mut b);
        let x = p.texto_bytes(x, y, &b[..n], INK);
        p.texto(x, y, "%", INK);
    });

    row("estado", &mut ty, &|x, y| {
        let (t, color) = level_text(level);
        p.texto(x, y, t, color);
    });

    row("identidad", &mut ty, &|x, y| {
        if bmo::info(bmo::INFO_ES_IDENTIDAD) != 0 {
            p.texto(x, y, "nacio en ESTE disco", INK_OK);
        } else {
            p.texto(x, y, "NO nacio aqui: clonado? no se escribira", INK_BAD);
        }
    });

    // * Cuantas VERSIONES mas caben. Es lo que de verdad contesta "cuando
    // hara falta el recolector?" -- un porcentaje no lo dice, y la respuesta
    // con 414 GiB son millones.
    row("caben", &mut ty, &|x, y| {
        let free = bloques.saturating_sub(used_count);
        let per_obj = (20 * 1024u64).div_ceil(tam).max(1);
        let mut b = [0u8; 10];
        let n = decimal(free / per_obj, &mut b);
        let x = p.texto_bytes(x, y, &b[..n], INK);
        p.texto(x, y, "  objetos mas de 20 KiB", INK_DIM);
    });

    ty += 8;
    // -- La verdad sobre la escritura --
    if bmo::info(bmo::INFO_ES_ESCRIBIBLE) != 0 {
        p.texto(tx, ty, "escritura: ABIERTA", INK_OK);
    } else {
        // ** ESTE PANEL MINTIO, Y SE VIO EN UNA FOTO.
        //
        // Decia *"la transaccion existe y esta probada (12 tests), pero nadie la
        // ha cableado al dispositivo todavia: falta el write y el FLUSH CACHE de
        // verdad"* -- y el 2026-08-13 el dueno sello desde esta misma ventana y
        // la generacion subio a 3. O sea que el write y el FLUSH CACHE **si
        // estaban cableados**, y el panel seguia contando el estado de hace dos
        // semanas.
        //
        // Un panel de diagnostico que se queda viejo es peor que no tenerlo:
        // este decia que no se podia escribir mientras el disco se escribia.
        //
        // Lo que la bandera dice de verdad es que la ventana de escritura de
        // SECTORES SUELTOS esta cerrada -- el gate de identidad y el rango
        // permitido--, no que la transaccion no exista. Son dos cosas y ahora se
        // dicen por separado.
        p.texto(tx, ty, "escritura: por TRANSACCION", 0x00F0_D070);
        ty += bmo::GLIFO_ALTO + 3;
        p.texto(tx, ty, "  sellar SI escribe: cierra un estrato y sube la", INK_DIM);
        ty += bmo::GLIFO_ALTO + 2;
        p.texto(tx, ty, "  generacion, con FLUSH CACHE de verdad.  TAB -> S.", INK_DIM);
        ty += bmo::GLIFO_ALTO + 2;
        p.texto(tx, ty, "  lo que NO hay es escritura de sectores sueltos:", INK_DIM);
        ty += bmo::GLIFO_ALTO + 2;
        p.texto(tx, ty, "  aqui no se toca un bloque sin una transaccion.", INK_DIM);
    }

    ty += bmo::GLIFO_ALTO + 10;
    p.texto(tx, ty, "F12 o ESC cierran.   TAB: nodos y carpetas.", INK_DIM);
    ty += bmo::GLIFO_ALTO + 2;
    // * Decirlo aqui evita el susto: con esta ventana delante el teclado es
    // SUYO, asi que teclear no escribe en la caja de abajo. Antes si escribia
    // --en una ventana tapada, sin verlo--, y eso era el fallo.
    p.texto(tx, ty, "mientras este abierta, el teclado es de esta ventana.", INK_DIM);
}

/// Un numero de bytes con su unidad. Devuelve la x donde acabo.
///
/// Sin coma flotante: la parte fraccionaria sale de multiplicar el resto por
/// cien antes de dividir. Es la misma cuenta que hace el panel del kernel, y
/// esta aqui duplicada a proposito -- cruzar el anillo para formatear un numero
/// seria exactamente lo que un library OS no hace.
fn magnitude(p: &bmo::Pantalla, x: u32, y: u32, bytes: u64, color: u32) -> u32 {
    const K: u64 = 1024;
    const M: u64 = K * 1024;
    const G: u64 = M * 1024;
    let (unit, div) = if bytes >= G {
        ("GiB", G)
    } else if bytes >= M {
        ("MiB", M)
    } else if bytes >= K {
        ("KiB", K)
    } else {
        ("B", 1)
    };
    let mut b = [0u8; 10];
    let n = decimal(bytes / div, &mut b);
    let mut x = p.texto_bytes(x, y, &b[..n], color);
    if div > 1 {
        let frac = (bytes % div) * 100 / div;
        x = p.texto(x, y, ".", color);
        if frac < 10 {
            x = p.texto(x, y, "0", color);
        }
        let n = decimal(frac, &mut b);
        x = p.texto_bytes(x, y, &b[..n], color);
    }
    x = p.texto(x, y, " ", color);
    p.texto(x, y, unit, color)
}
