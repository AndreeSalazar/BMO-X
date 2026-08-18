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
//! identidad, nivel. `[explorador]` contesta *"que hay dentro?"*. Son preguntas
//! distintas y por eso son dos pestanas y no una pantalla: meter un arbol entre
//! la generacion y la ocupacion deja las dos ilegibles. `TAB` cambia.
//!
//! ** El explorador fue a su vez DOS pestanas --`nodos` y `carpetas`-- hasta el
//! 2026-08-18, y ahora son tres paneles de una sola vista: arbol, rejilla y
//! grafo. El porque, en [`View::Obra`]; el reparto del ancho, en
//! `scene::zonas`.
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

use super::arbol;
use super::chrome::Chrome;
use super::zonas::{Zona, Zonas, MIGA_H};
use super::*;
use crate::text::decimal;

// La ventana de Datos es VERDE porque es ESTRATOS, y eso se queda: el color
// dice de que ventana estas hablando antes de leer su titulo. Lo que cambia es
// el tono -- el verde de antes era de rotulador, escogido para verse en una foto
// de una pantalla que a lo mejor ni arrancaba.
pub(crate) const DATA_BG: u32 = 0x0013_1C18;
pub(crate) const DATA_TITLE_BG: u32 = 0x001B_2622;
/// El borde, discreto. Lo que separa la ventana del fondo es la sombra.
pub(crate) const DATA_EDGE: u32 = 0x002C_4038;
/// Y el acento verde, que si puede ser vivo: es una linea, no un marco.
pub(crate) const DATA_TITLE: u32 = 0x0034_D399;

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
    /// Primera fila visible DEL ARBOL, que es un desplazamiento distinto.
    ///
    /// * Y tiene que serlo: el arbol ensena los hermanos de todos los niveles y
    /// la rejilla los hijos de uno solo, asi que sus listas no miden lo mismo
    /// ni de lejos. Con un `from` compartido, bajar por la rejilla arrastraria
    /// el arbol a una fila que no tiene nada que ver.
    pub(crate) arbol_from: usize,
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
    /// ** LAS DOS LECTURAS A LA VEZ: el arbol, la rejilla y el grafo.
    ///
    /// Eran dos pestanas --`nodos` y `carpetas`-- y el dueno pidio juntarlas el
    /// 2026-08-18, con el argumento que las justifica:
    ///
    /// > *"en explorer es 2D y en nodos es 3D, asi mas facil de gestionar"*
    ///
    /// Y no es una preferencia de aspecto. En ESTRATOS **una carpeta no es una
    /// carpeta: es un nodo con atributos**. La rejilla contesta *que hay
    /// dentro* y el grafo contesta *que es esto y como se conecta* -- dos
    /// preguntas distintas sobre el mismo dato. Con una pestana detras de otra
    /// habia que elegir cual de las dos mirar, y elegir entre ellas es
    /// exactamente lo que no hace falta: caben las dos.
    ///
    /// El reparto del ancho vive en `scene::zonas`, no aqui.
    Obra,
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
            arbol_from: 0,
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
        if self.view != View::Obra || self.chrome.minimized {
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
    /// Donde caen las cajas del grafo: `(x, ancho, x de los hijos, primera y)`.
    ///
    /// ** Sale de `Zonas` y no de `chrome`, y ese es el cambio que permite que
    /// el grafo comparta ventana con la rejilla. Se recalcula en vez de
    /// guardarse porque es aritmetica pura -- guardarlo obligaria a acordarse
    /// de refrescarlo al mover la ventana, que es justo la clase de "acordarse"
    /// que aqui sale mal.
    fn graph_geometry(&self) -> (u32, u32, u32, u32) {
        const CHANNEL: u32 = 44;
        let z = Zonas::repartir(&self.chrome).grafo;
        let box_w = ((z.w.saturating_sub(CHANNEL)) / 2).max(NODE_MIN);
        let children_x = z.x + box_w + CHANNEL;
        (z.x, box_w, children_x, z.y + 4)
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

    /// **Cuantos hijos se ensenan de una vez, en LOS DOS paneles.**
    ///
    /// Mide con las cajas del grafo, que son las mas altas, y la rejilla usa
    /// este mismo numero aunque le cabrian mas filas. Es a proposito: las dos
    /// columnas tienen que ensenar el MISMO tramo de hijos. Con dos cuentas
    /// distintas, la lista ensenaria un archivo que el grafo de al lado no
    /// tiene -- y entonces dejan de ser la misma cosa vista de dos maneras,
    /// que es lo unico que justifica ponerlas juntas.
    fn fit_count(&self) -> usize {
        let z = Zonas::repartir(&self.chrome);
        // Si el grafo no cabe, manda la rejilla: sus filas son mas bajas y
        // caben mas. Preguntar por el panel que no se pinta daria un tope
        // inventado.
        let alto = if z.grafo.hay() { z.grafo.h } else { z.rejilla.h };
        let paso = if z.grafo.hay() { NODE_H + NODE_GAP } else { ROW_H };
        (alto.saturating_sub(28) / paso).max(1) as usize
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

// == ** EL EXPLORADOR: los tres paneles a la vez ============================
//
// Eran dos pestanas y ahora es una vista. El argumento del dueno, que es el
// que manda aqui:
//
//   > "en explorer es 2D y en nodos es 3D, asi mas facil de gestionar"
//
// Traducido a lo que ESTRATOS es de verdad: la rejilla contesta *que hay* y el
// grafo contesta *que es esto*. Un directorio de este volumen no es una
// carpeta, es un nodo con atributos -- y eso la rejilla no lo puede ensenar
// por mucho que se le anadan columnas.
//
// ** Y ninguno de los tres paneles sabe donde esta: cada uno recibe su
// rectangulo de `scene::zonas`. Es lo que permite que el grafo se retire
// cuando la ventana es estrecha sin que los otros dos se enteren.

fn obra(p: &bmo::Pantalla, c: &DataWindow) {
    let z = Zonas::repartir(&c.chrome);

    if bmo::info(bmo::INFO_ES_MONTADO) == 0 {
        p.texto(z.miga.x, z.miga.y, "ningun volumen ESTRATOS montado.", INK_BAD);
        return;
    }
    // La guarda solo PREGUNTA. Poner el cursor en la raiz es de quien entra en
    // la vista (`keys/panels.rs`) -- ver la nota larga que dejo ahi el fallo de
    // "pintar navegaba".
    if bmo::estratos::tipo() == bmo::estratos::NOTHING {
        p.texto(z.miga.x, z.miga.y, "el volumen monta pero no tiene raiz legible.", INK_BAD);
        p.texto(z.miga.x, z.miga.y + bmo::GLIFO_ALTO + 4, "el motivo esta en F11.", INK_DIM);
        return;
    }

    miga(p, &z.miga);
    arbol::paint(p, &z.arbol, c.arbol_from, DATA_TITLE, NODE_SEL);
    paint_folders(p, c, &z.rejilla);
    paint_nodes(p, c, &z.grafo);
    pie(p, c, &z.pie);

    // Los separadores. Una linea de un pixel entre paneles: sin ella, tres
    // columnas de texto sobre el mismo fondo se leen como una sola tabla mal
    // alineada.
    for x in [z.arbol.derecha(), z.grafo.x.wrapping_sub(6)] {
        if x > z.miga.x && x < z.miga.derecha() {
            p.rect(x + 5, z.rejilla.y, 1, z.rejilla.h, DATA_EDGE);
        }
    }
}

/// **LA MIGA DE PAN**: `/ > cobol > 10`, y a la derecha cuantos hijos hay.
///
/// Antes ponia `profundidad 2`, y eso no dice DONDE estas: dos carpetas
/// distintas con los mismos nombres dentro se veian identicas.
///
/// Los nombres los guarda el cursor AL BAJAR, porque despues ya no se saben: un
/// nodo no sabe como se llama -- el nombre vive en la entrada de su padre.
///
/// * Estaba ESCRITA DOS VECES, una en cada pestana, y por eso vive aqui ahora:
/// con las dos vistas a la vez habria pintado dos migas distintas del mismo
/// sitio.
fn miga(p: &bmo::Pantalla, z: &Zona) {
    let hondo = bmo::estratos::hondo();
    let ty = z.y + (MIGA_H - bmo::GLIFO_ALTO) / 2;
    let mut x = p.texto(z.x, ty, "/", DATA_TITLE);
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
    let n = decimal(bmo::estratos::hijos(), &mut b);
    let x = p.texto_bytes(x, ty, &b[..n], INK);
    if bmo::estratos::truncado() {
        // Se DICE. Un listado recortado en silencio se ve igual que un
        // directorio con pocos archivos, y esa confusion cuesta horas.
        p.texto(x, ty, "  (RECORTADO)", INK_BAD);
    }
    p.rect(z.x, z.abajo() - 2, z.w, 1, DATA_EDGE);
}

/// **EL PIE**: el detalle del nodo senalado, y la linea que anuncia las teclas.
///
/// Las dos lineas ya existian sueltas al fondo de la ventana, cada una midiendo
/// por su cuenta contra `chrome.height`. Aqui reciben su zona.
fn pie(p: &bmo::Pantalla, c: &DataWindow, z: &Zona) {
    let how_many = bmo::estratos::hijos() as usize;
    p.rect(z.x, z.y, z.w, 1, DATA_EDGE);

    // -- * EL DETALLE del nodo senalado --
    //
    // Un grafo que solo ensena nombres contesta *que hay*; no contesta *que es
    // esto*.
    let dy = z.y + 5;
    if c.sel < how_many {
        let mut b = [0u8; 10];
        let x = p.texto(z.x, dy, "sel: ", INK_DIM);
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
        let vx = x + 2 * bmo::GLIFO_ANCHO;
        match c.verified {
            None => { p.texto(vx, dy, "V comprueba", INK_DIM); }
            Some(bmo::estratos::FIRMA_CUADRA) => { p.texto(vx, dy, "CUADRA", INK_OK); }
            // El unico mensaje de esta ventana que significa "hay un problema
            // en el disco". Por eso es el unico en rojo.
            Some(bmo::estratos::FIRMA_NO_CUADRA) => { p.texto(vx, dy, "NO CUADRA", INK_BAD); }
            Some(bmo::estratos::FIRMA_AUSENTE) => { p.texto(vx, dy, "sin firma", INK_DIM); }
            // TENUE y no rojo: el archivo esta bien, lo que no cabe es nuestro
            // buffer de comprobacion. En rojo mandaba a buscar una corrupcion
            // que no existe.
            Some(bmo::estratos::FIRMA_NO_CABE) => { p.texto(vx, dy, "no cabe (>256 KiB)", INK_DIM); }
            _ => { p.texto(vx, dy, "no se pudo leer", INK_BAD); }
        }
    }

    // -- ** `S sella` DICHO EN LA BARRA, y esa es la mitad del arreglo --
    //
    // La orden de sellar existia desde hacia dias y **no estaba escrita en
    // ningun sitio que se vea**: el dueno la busco teniendola delante. Una
    // funcion que no se anuncia no es discreta, es una funcion que no esta.
    let y = z.y + 5 + bmo::GLIFO_ALTO + 5;
    match c.seal {
        Seal::Asking => p.texto(
            z.x, y,
            "S OTRA VEZ para SELLAR (escribe en el disco)   otra tecla cancela",
            0x00F0_D070,
        ),
        Seal::Done(g) => {
            let x = p.texto(z.x, y, "SELLADO. generacion ", INK_OK);
            let mut b = [0u8; 10];
            let n = decimal(g, &mut b);
            let x = p.texto_bytes(x, y, &b[..n], INK_OK);
            p.texto(x, y, "   reinicia y mirala otra vez: eso prueba la barrera", INK_DIM)
        }
        Seal::Failed => p.texto(
            z.x, y,
            "NO se sello. el volumen sigue igual; el motivo esta en F11.",
            INK_BAD,
        ),
        Seal::Idle => p.texto(
            z.x, y,
            "flechas mueven  ENTRAR baja  RETROCESO sube  clic en el arbol salta  V firma  S sella",
            INK_DIM,
        ),
    };
}

/// El alto de una fila del explorador. Una linea de texto y aire a los lados:
/// lo justo para que el realce de la seleccion no toque las letras.
const ROW_H: u32 = 22;

/// **LA REJILLA: los hijos del nodo actual, como los ensena un explorador.**
///
/// Comparte cursor con el grafo --literalmente el mismo, no una copia-- y desde
/// que los dos se pintan a la vez comparte tambien la VENTANA de scroll: las
/// dos columnas ensenan exactamente los mismos hijos, uno como lista y otro
/// como cajas. Es lo que las hace dos lecturas de una cosa y no dos listas que
/// hay que cuadrar con la vista.
///
/// Tres columnas y ni una mas: **nombre, que es, cuanto ocupa**. Un explorador
/// que ensena diez columnas por defecto obliga a leerlas todas para encontrar
/// la unica que importaba.
fn paint_folders(p: &bmo::Pantalla, c: &DataWindow, z: &Zona) {
    if !z.hay() {
        return;
    }
    let how_many = bmo::estratos::hijos() as usize;
    let mut ty = z.y;

    // La cabecera de columnas, y su linea. Las `x` salen del ancho de LA ZONA
    // --no del marco-- para que estirar la ventana de sitio a los nombres
    // largos sin invadir al panel de al lado.
    let col_kind = z.x + (z.w * 55) / 100;
    let col_size = z.x + (z.w * 78) / 100;
    p.texto(z.x + 22, ty, "nombre", INK_DIM);
    p.texto(col_kind, ty, "que es", INK_DIM);
    p.texto(col_size, ty, "bytes", INK_DIM);
    ty += bmo::GLIFO_ALTO + 3;
    p.rect(z.x, ty, z.w, 1, DATA_EDGE);
    ty += 4;

    if how_many == 0 {
        p.texto(z.x + 22, ty + 4, "esta vacio.", INK_DIM);
        return;
    }

    // ** El cuantas-caben sale de `fit_count`, que mide con las cajas del
    // GRAFO y no con estas filas. Es a proposito: las dos columnas tienen que
    // ensenar el mismo tramo de hijos, y el tramo lo manda el panel que menos
    // cosas mete. Con dos cuentas distintas, la lista ensenaria un archivo que
    // el grafo de al lado no tiene -- y entonces ya no son la misma cosa vista
    // de dos maneras.
    let last = (c.from + c.fit_count()).min(how_many);

    let mut i = c.from;
    while i < last {
        let (type_name, color) = class_color(bmo::estratos::hijo_tipo(i as u64));
        // El realce de la fila senalada. Va DEBAJO del texto y ocupa el ancho
        // entero: es como se lee "esta es la seleccionada" sin un cursor.
        if i == c.sel {
            p.rect(z.x, ty, z.w, ROW_H, NODE_SEL);
        }
        // El cuadrito de color: dice la clase antes de leer la columna. Es el
        // mismo color que su caja en el grafo, a proposito -- mirar el mismo
        // nodo en los dos paneles no puede darle dos colores.
        p.rect(z.x + 4, ty + (ROW_H - 8) / 2, 8, 8, color);

        let mut nom = [0u8; 64];
        let n = bmo::estratos::hijo_nombre(i as u64, &mut nom);
        let ty_texto = ty + (ROW_H - bmo::GLIFO_ALTO) / 2;
        p.texto_bytes(z.x + 22, ty_texto, &nom[..n], INK);
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
        let x = p.texto(z.x + 22, ty + 2, "y ", INK_DIM);
        let x = p.texto_bytes(x, ty + 2, &b[..nb], INK);
        p.texto(x, ty + 2, " mas abajo", INK_DIM);
    }
}

/// **EL GRAFO: el nodo actual y sus hijos, unidos por una curva cada uno.**
///
/// * La spec del dueno, cumplida: un grafo tipo n8n -- cajas con titulo y
/// nombre, unidas por lineas, con color por clase. No una lista con sangrias.
///
/// Y desde el 2026-08-18 **no es una pestana: es la columna de la derecha**,
/// junto a la rejilla. La razon esta en la cabecera de [`View::Obra`] y es la
/// que da sentido a tener las dos a la vez: la rejilla contesta *que hay
/// dentro* y esto contesta *que es eso y como se conecta*.
fn paint_nodes(p: &bmo::Pantalla, c: &DataWindow, z: &Zona) {
    if !z.hay() {
        return;
    }
    let how_many = bmo::estratos::hijos() as usize;
    let hondo = bmo::estratos::hondo();

    // -- * EL REPARTO DEL ANCHO --
    //
    // Las cajas no miden lo mismo pase lo que pase: el ancho de LA ZONA se
    // parte entre las dos columnas y el canal de las ramas. Estirar la ventana
    // hace que quepan nombres mas largos, que es para lo que uno la estira.
    //
    // ** Y sale de `graph_geometry`, **la misma que usa el acierto del raton**.
    // Tenerlo dos veces era garantizar que un dia se pulsara una caja y se
    // seleccionara otra: dos copias de una geometria se separan solas.
    let (tx, box_w, children_x, first_y) = c.graph_geometry();

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
    // Los tirantes van horizontales y a media distancia, que es lo que hace que
    // la curva salga y entre en horizontal aunque el hijo este muy abajo: la
    // clasica S. Ver `bmo::curve`.
    let last = (c.from + c.fit_count()).min(how_many);
    // El recorte: las aristas no pueden salirse de SU panel. Antes se recortaba
    // contra el marco de la ventana entera, que con una sola vista dentro venia
    // a ser lo mismo; ahora no lo es -- una curva que se saliera por la
    // izquierda se pintaria encima de la rejilla.
    let rec = bmo::Recorte::nuevo(z.x as i32, z.y as i32, z.w as i32, z.h as i32);
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
    let (c1, c2) = match c.view {
        View::Numbers => (INK, INK_DIM),
        View::Obra => (INK_DIM, INK),
    };
    let fin1 = p.texto(px, c.chrome.y + 8, "numeros", c1);
    let px2 = fin1 + 2 * bmo::GLIFO_ANCHO;
    let fin2 = p.texto(px2, c.chrome.y + 8, "explorador", c2);
    let (sx, sw) = match c.view {
        View::Numbers => (px, fin1 - px),
        View::Obra => (px2, fin2 - px2),
    };
    p.rect(sx, c.chrome.y + 8 + bmo::GLIFO_ALTO + 2, sw, 2, DATA_TITLE);

    if c.view == View::Obra {
        obra(p, c);
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
    // == LA VERDAD SOBRE LA ESCRITURA, y ahora la bandera SI la dice =========
    //
    // ** HASTA EL 2026-08-18 ESTE `if` ERA CODIGO MUERTO, y la rama de abajo la
    // unica que se veia.
    //
    // `INFO_ES_ESCRIBIBLE` contestaba **un cero constante** en el kernel, con un
    // comentario que decia que la transaccion existia pero que nadie la habia
    // cableado al dispositivo. Era cierto el dia que se escribio; dejo de serlo
    // cuando `sellar` empezo a escribir el superbloque de verdad -- y el disco
    // de esta casa va por la generacion 3, o sea que ha commiteado tres veces.
    //
    // Este panel ya se habia arreglado una vez por exactamente lo mismo, y el
    // arreglo fue prosa: se cambio lo que la rama DICE. El defecto no estaba
    // aqui -- estaba en que el campo no podia decir otra cosa.
    //
    // > Un valor fijo puesto por prudencia envejece hacia la MENTIRA, y no
    // > avisa: lo unico que cambia a su alrededor es el mundo.
    //
    // Ahora la bandera es la conjuncion de las condiciones que de verdad
    // deciden --hay volumen, es de este disco, cabe, y el gate armo la
    // escritura-- asi que las dos ramas significan algo.
    if bmo::info(bmo::INFO_ES_ESCRIBIBLE) != 0 {
        p.texto(tx, ty, "escritura: ABIERTA", INK_OK);
        ty += bmo::GLIFO_ALTO + 3;
        p.texto(tx, ty, "  sellar cierra un estrato y sube la generacion,", INK_DIM);
        ty += bmo::GLIFO_ALTO + 2;
        p.texto(tx, ty, "  con FLUSH CACHE de verdad.  TAB -> S.", INK_DIM);
    } else {
        // ** UN "NO" QUE NO DICE CUAL DE LAS CUATRO ES UN "NO" QUE NO SIRVE.
        //
        // La bandera es una Y de varias condiciones, y cada una manda a mirar
        // un sitio distinto: no hay volumen (se formatea), es de otro disco (se
        // clono), no cabe (hay que recoger), o el gate del disco no armo (eso
        // es del arranque, no de ESTRATOS). Ensenar solo "NO" obligaria a
        // adivinar entre cuatro -- que es lo que costo una vuelta al metal en
        // el recorte del 17-08.
        //
        // Y no hace falta un campo nuevo: las tres primeras ya se preguntan por
        // separado en esta misma ventana, asi que si las tres dicen que si, el
        // que queda es el gate.
        p.texto(tx, ty, "escritura: CERRADA", 0x00F0_D070);
        ty += bmo::GLIFO_ALTO + 3;
        let montado = bmo::info(bmo::INFO_ES_MONTADO) != 0;
        let mio = bmo::info(bmo::INFO_ES_IDENTIDAD) != 0;
        let cabe = bmo::info(bmo::INFO_ES_NIVEL) < 3;
        let porque: &str = if !montado {
            "  no hay volumen montado: se formatea con estratos-fmt."
        } else if !mio {
            "  el volumen NO nacio en este disco: no se le escribe."
        } else if !cabe {
            "  por encima del 95%: solo lectura hasta que se recoja."
        } else {
            "  el gate de identidad del disco no armo la escritura."
        };
        p.texto(tx, ty, porque, INK_DIM);
        ty += bmo::GLIFO_ALTO + 2;
        p.texto(tx, ty, "  sin esto, sellar no escribe y el recorte tampoco.", INK_DIM);
    }

    ty += bmo::GLIFO_ALTO + 10;
    p.texto(tx, ty, "F12 o ESC cierran.   TAB: el explorador.", INK_DIM);
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
