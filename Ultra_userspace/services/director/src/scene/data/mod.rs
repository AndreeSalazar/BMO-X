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
use super::consola::{self, Consola};
use super::iconos;
use super::menu::{self, Menu};
use super::numeros;
use super::chrome::Chrome;
use super::zonas::Zonas;
use super::*;

// La ventana de Datos es VERDE porque es ESTRATOS, y eso se queda: el color
// dice de que ventana estas hablando antes de leer su titulo. Lo que cambia es
// el tono -- el verde de antes era de rotulador, escogido para verse en una foto
// de una pantalla que a lo mejor ni arrancaba.
/// Cuantos fotogramas caben entre los dos clics de un doble clic.
///
/// Veinticuatro, que a los ~60 por segundo del escritorio son unos 400 ms -- la
/// cifra que usan todos y la que tiene la mano acostumbrada. Ver `clic_frame`
/// en [`DataWindow`] para por que esto se mide en fotogramas.
pub(crate) const DOBLE_CLIC: u32 = 24;

pub(crate) const DATA_BG: u32 = 0x0013_1C18;
pub(crate) const DATA_TITLE_BG: u32 = 0x001B_2622;
/// El borde, discreto. Lo que separa la ventana del fondo es la sombra.
pub(crate) const DATA_EDGE: u32 = 0x002C_4038;
/// Y el acento verde, que si puede ser vivo: es una linea, no un marco.
pub(crate) const DATA_TITLE: u32 = 0x0034_D399;

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
    /// Lo que se esta mirando dentro de un fichero, si hay algo.
    pub(crate) visor: Visor,
    /// El fotograma del ultimo clic en la rejilla, y sobre que fila cayo.
    ///
    /// ** SE CUENTA EN FOTOGRAMAS Y NO EN MILISEGUNDOS, y no es por gusto: en
    /// Ring 3 el unico reloj que hay es `INFO_FECHA`, que da la hora de la
    /// placa **al segundo**. Con esa resolucion no se puede distinguir un doble
    /// clic de dos clics seguidos.
    ///
    /// La consecuencia, dicha: si el bucle del escritorio corre mas rapido, la
    /// ventana del doble clic se acorta sola. Es el precio de no tener un
    /// contador fino, y se paga sabiendolo.
    clic_frame: u32,
    clic_fila: usize,
    /// Por donde va el sellado. Ver [`Seal`].
    pub(crate) seal: Seal,
    /// El terminal del pie, `Ctrl+n`. Ver [`super::consola`].
    pub(crate) consola: Consola,
    /// El menu del clic derecho. Ver [`super::menu`].
    pub(crate) menu: Menu,
    /// Primera version visible en el historial, y cual esta senalada.
    ///
    /// Aparte de las del explorador por lo mismo que el scroll del arbol: son
    /// listas de cosas distintas y compartir el indice haria que moverse por
    /// una arrastrara la otra a una fila sin relacion.
    pub(crate) hist_from: usize,
    pub(crate) hist_sel: usize,
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
    /// ** LA HISTORIA del volumen: la cadena de versiones hacia atras.
    ///
    /// Es la tercera pregunta, y distinta de las otras dos: `[numeros]` dice
    /// COMO ESTA el almacen, `[explorador]` QUE HAY dentro, y esta **QUE HA
    /// PASADO**. Un volumen que nunca sobreescribe tiene esa tercera respuesta
    /// y ningun sistema de ficheros clasico la tiene.
    Historial,
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
            visor: Visor::VACIO,
            clic_frame: 0,
            clic_fila: 0,
            seal: Seal::Idle,
            consola: Consola::nueva(),
            menu: Menu::nuevo(),
            hist_from: 0,
            hist_sel: 0,
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
        let z = Zonas::repartir(&self.chrome, self.consola.abierta).grafo;
        // ** SIN `.max(NODE_MIN)`, y esa llamada era el fallo.
        //
        // Clampar el ancho por abajo suena prudente y es justo lo contrario:
        // obliga a la caja a medir 170 aunque en el panel no quepan dos de 170
        // mas el canal, y entonces la segunda columna se sale. Quien decide si
        // cabe es `zonas`, que ya exige `2*NODE_MIN + CHANNEL` antes de dar el
        // panel; aqui solo se reparte lo que hay.
        let box_w = z.w.saturating_sub(CHANNEL) / 2;
        let children_x = z.x + box_w + CHANNEL;
        (z.x, box_w, children_x, z.y + 4)
    }

    /// **Sobre que hijo cayo el puntero EN LA REJILLA.**
    ///
    /// Faltaba: se navegaba con las flechas o por el arbol, y pulsar una fila no
    /// hacia nada. Una lista que parece pulsable y no lo es ensena a no pulsar.
    ///
    /// La geometria sale de `Zonas` y de `REJILLA_CABECERA`, las mismas que usa
    /// el pintado.
    /// **Un clic en la fila `i`.** Devuelve `true` si es el SEGUNDO de un
    /// doble clic.
    ///
    /// Siempre selecciona: pulsar una fila la senala, que es lo que la mano
    /// espera y lo que hace que senalar salga gratis. Lo que el doble anade es
    /// ABRIR, y por eso el primero nunca abre nada.
    ///
    /// ** El segundo clic CIERRA el gesto (`clic_frame = 0`). Sin eso, tres
    /// clics seguidos serian dos aperturas: la tercera pulsacion volveria a
    /// caer dentro de la ventana de la segunda y abriria otra vez, que es
    /// justo lo que no hace ningun escritorio.
    pub(crate) fn clic_rejilla(&mut self, i: usize, frame: u32) -> bool {
        let doble = self.clic_frame != 0
            && self.clic_fila == i
            && frame.wrapping_sub(self.clic_frame) <= DOBLE_CLIC;
        self.sel = i;
        // El veredicto de una firma es de UN archivo: al cambiar de fila se
        // borra, o un `CUADRA` viejo se quedaria debajo del nombre de otro.
        self.verified = None;
        self.clic_frame = if doble { 0 } else { frame };
        self.clic_fila = i;
        doble
    }

    /// **La ruta ENTERA del hijo `i`**, desde la raiz. `0` si no cabe.
    ///
    /// Hermana de `Consola::ruta_de`, y viven separadas mientras cada una use
    /// su propio ancho: la de la consola escribe en su renglon de `COLS` y esta
    /// en el buffer del que abre. El dia que las dos quieran lo mismo, esta es
    /// la que se queda -- aqui es donde vive el cursor.
    pub(crate) fn ruta_del_hijo(&self, i: usize, dst: &mut [u8; 128]) -> usize {
        let mut k = 0usize;
        let hondo = bmo::estratos::hondo();
        let mut nivel = 1u64;
        while nivel <= hondo {
            let mut nom = [0u8; 64];
            let m = bmo::estratos::nombre_nivel(nivel, &mut nom);
            if k + m + 1 >= dst.len() {
                return 0;
            }
            dst[k..k + m].copy_from_slice(&nom[..m]);
            k += m;
            dst[k] = b'/';
            k += 1;
            nivel += 1;
        }
        let mut nom = [0u8; 64];
        let m = bmo::estratos::hijo_nombre(i as u64, &mut nom);
        if m == 0 || k + m > dst.len() {
            return 0;
        }
        dst[k..k + m].copy_from_slice(&nom[..m]);
        k + m
    }

    /// **Cuantas lineas del visor caben en el panel.**
    ///
    /// La cuenta vive aqui y no en `keys/`: el reparto de zonas es de esta
    /// ventana. Quien mueve el scroll solo tiene que saber CUANTO, no donde
    /// empieza la rejilla ni cuanto ocupa una cabecera.
    pub(crate) fn visor_caben(&self) -> usize {
        let z = Zonas::repartir(&self.chrome, self.consola.abierta).rejilla;
        if !z.hay() {
            return 1;
        }
        // La cabecera del visor se lleva un renglon y su raya.
        let util = z.h.saturating_sub(bmo::GLIFO_ALTO + 12);
        ((util / bmo::GLIFO_ALTO) as usize).max(1)
    }

    /// **Abre el fichero senalado en el visor.** `false` si no era un fichero.
    ///
    /// ** Se pregunta por el TIPO y no se prueba a abrir: un directorio se abre
    /// igual como ruta, y el visor ensenaria sus entradas crudas como si fueran
    /// texto. Eso no es ver un fichero, es ensenar el formato por dentro.
    pub(crate) fn ver_senalado(&mut self) -> bool {
        if self.sel >= bmo::estratos::hijos() as usize {
            return false;
        }
        if bmo::estratos::hijo_tipo(self.sel as u64) != bmo::estratos::ARCHIVO {
            return false;
        }
        let mut ruta = [0u8; 128];
        let k = self.ruta_del_hijo(self.sel, &mut ruta);
        if k == 0 {
            return false;
        }
        let mut nom = [0u8; 64];
        let m = bmo::estratos::hijo_nombre(self.sel as u64, &mut nom);
        self.visor.abrir(&ruta[..k], &nom[..m])
    }

    pub(crate) fn fila_rejilla_en(&self, px: u32, py: u32) -> Option<usize> {
        if self.view != View::Obra || self.chrome.minimized {
            return None;
        }
        let z = Zonas::repartir(&self.chrome, self.consola.abierta).rejilla;
        if !z.contiene(px, py) {
            return None;
        }
        let y0 = z.y + REJILLA_CABECERA;
        if py < y0 {
            return None;
        }
        let k = ((py - y0) / ROW_H) as usize;
        let i = self.from + k;
        // Por debajo de la ultima fila es el PANEL, no la ultima. Pulsar el
        // hueco de abajo no puede seleccionar lo que hay mas arriba.
        if i < bmo::estratos::hijos() as usize && k < self.fit_count() {
            Some(i)
        } else {
            None
        }
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
        let z = Zonas::repartir(&self.chrome, self.consola.abierta);
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
pub(crate) const NODE_MIN: u32 = 170;

/// El canal entre las dos columnas de cajas: por donde van las curvas.
///
/// * Estaba DENTRO de `graph_geometry` y por eso `zonas` no podia verlo. Y
/// tenia que verlo: el ancho minimo que le da al panel del grafo se calcula con
/// estos dos numeros. Con el canal escondido aqui, alli habia un `260` puesto a
/// ojo -- y `2*170 + 44` son **384**, o sea que entre 260 y 383 el panel se
/// aceptaba y las cajas se salian por el borde derecho de la ventana.
///
/// [!] Visto en el Ryzen el 2026-08-18, en una foto: las tres cajas de los
/// hijos pintadas FUERA del marco, sobre el escritorio. `node_box` no recorta
/// --solo las curvas llevan `Recorte`--, asi que la geometria no puede
/// permitirse mentir: lo que diga, se pinta.
pub(crate) const CHANNEL: u32 = 44;
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

/// **LO SELECCIONADO VA EN AZUL, y el azul no me lo he inventado.**
///
/// `tema/tema.maqueta` lo tiene escrito desde que existe la paleta:
///
/// ```text
///   .accent  #60A5FA   22 usos -- esto se puede tocar
/// ```
///
/// Seleccionar ES decir "esto se puede tocar": lo que este realzado es sobre lo
/// que van a actuar el menu, `ENTRAR` y `V`. Asi que el color ya estaba
/// decidido y aqui solo se usa.
///
/// ** Y va en AZUL sobre una ventana VERDE a proposito. El verde dice de que
/// ventana estas hablando --es la identidad de ESTRATOS-- y el azul dice sobre
/// que vas a actuar. Son dos preguntas distintas: si la seleccion fuera otro
/// verde habria que compararla con el fondo para verla.
const SEL_FONDO: u32 = 0x0015_2A45;
/// El filo de neon. Un pixel del acento entero, sin apagar.
///
/// El relleno solo no basta: sobre un fondo oscuro un azul apagado se lee como
/// una sombra. Lo que hace que se vea SELECCIONADO es el borde vivo, igual que
/// el subrayado de la pestana activa -- una linea de color se ve en una foto y
/// un relleno de color no.
const SEL_NEON: u32 = ACCENT;

/// Pinta el realce de lo seleccionado: relleno y filo.
fn realce(p: &bmo::Pantalla, x: u32, y: u32, w: u32, h: u32) {
    p.rect(x, y, w, h, SEL_FONDO);
    p.rect(x, y, w, 1, SEL_NEON);
    p.rect(x, y + h - 1, w, 1, SEL_NEON);
    p.rect(x, y, 1, h, SEL_NEON);
    p.rect(x + w - 1, y, 1, h, SEL_NEON);
}

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
    // ** El filo de la seleccionada es el ACENTO, no el color de su clase.
    //
    // Antes era el color de clase, y eso mezclaba dos preguntas en un pixel:
    // "que es esto" y "es esto lo senalado". Un directorio seleccionado y uno
    // sin seleccionar se diferenciaban en el TONO del mismo azul celeste.
    //
    // Ahora la clase la sigue diciendo el punto de dentro, y el filo dice
    // seleccion. El mismo azul que la rejilla, para que mirar el mismo nodo en
    // los dos paneles no de dos respuestas.
    let (edge, cuerpo) = if pointed_at {
        (SEL_NEON, SEL_FONDO)
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
/// **El EXPLORADOR**: la miga, el arbol de carpetas, la rejilla y el pie.
///
/// Salio de aqui el 19-08 al cruzar L6a, y el corte no hubo que elegirlo: el
/// fichero ya llevaba escrita su propia raya de separacion justo ahi. Es texto
/// MOVIDO.
///
/// ** Las dos medidas de la rejilla se reexportan porque las usa la ventana
/// para saber en que fila cayo un clic. Viven con quien las dibuja.
mod obra;
pub(crate) use obra::{obra, REJILLA_CABECERA, ROW_H};

/// **El VISOR**: lo que hay DENTRO de un fichero, donde iria la rejilla.
///
/// Fichero propio y no una funcion mas aqui: `data.rs` ya se partio una vez por
/// L6a, y lo que se anade a una ventana crece por su cuenta.
pub(crate) mod visor;
pub(crate) use visor::Visor;

/// **Repinta SOLO la consola del pie.**
///
/// El hermano barato de [`paint`], y existe por un numero: teclear una letra
/// repintaba la ventana ENTERA. El reparto de zonas vive aqui --es esta ventana
/// la que sabe donde cae su consola-- para que quien la llama no tenga que
/// saberselo, que es como se acaba con dos repartos que no dicen lo mismo.
pub(crate) fn paint_consola(p: &bmo::Pantalla, c: &DataWindow) {
    if c.chrome.minimized {
        return;
    }
    let z = Zonas::repartir(&c.chrome, c.consola.abierta);
    consola::paint(p, &z.consola, &c.consola, DATA_BG, DATA_EDGE, DATA_TITLE);
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
        View::Obra => (INK_DIM, INK, INK_DIM),
        View::Historial => (INK_DIM, INK_DIM, INK),
    };
    let fin1 = p.texto(px, c.chrome.y + 8, "numeros", c1);
    let px2 = fin1 + 2 * bmo::GLIFO_ANCHO;
    let fin2 = p.texto(px2, c.chrome.y + 8, "explorador", c2);
    let px3 = fin2 + 2 * bmo::GLIFO_ANCHO;
    let fin3 = p.texto(px3, c.chrome.y + 8, "historial", c3);
    let (sx, sw) = match c.view {
        View::Numbers => (px, fin1 - px),
        View::Obra => (px2, fin2 - px2),
        View::Historial => (px3, fin3 - px3),
    };
    p.rect(sx, c.chrome.y + 8 + bmo::GLIFO_ALTO + 2, sw, 2, DATA_TITLE);

    if c.view == View::Obra {
        obra(p, c);
        return;
    }
    if c.view == View::Historial {
        // El panel ocupa el cuerpo entero: aqui no hay arbol ni rejilla que
        // repartir, hay una sola columna de versiones.
        let z = Zonas::repartir(&c.chrome, false);
        historial::paint(
            p,
            &z.rejilla,
            c.hist_from,
            c.hist_sel,
            DATA_EDGE,
            NODE_BG,
            SEL_NEON,
            SEL_FONDO,
        );
        let y = c.chrome.y + c.chrome.height - bmo::GLIFO_ALTO - 8;
        p.texto(
            tx, y,
            "mirar y ya: volver a una version todavia no esta. TAB sigue.",
            INK_DIM,
        );
        return;
    }

    numeros::paint(p, c, tx);
}
