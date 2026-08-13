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
//! 2. [OK] Un color por clase, y el mismo en toda la ventana -- `color_clase`.
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

use super::marco::Marco;
use super::*;
use crate::texto::decimal;

// La ventana de Datos es VERDE porque es ESTRATOS, y eso se queda: el color
// dice de que ventana estas hablando antes de leer su titulo. Lo que cambia es
// el tono -- el verde de antes era de rotulador, escogido para verse en una foto
// de una pantalla que a lo mejor ni arrancaba.
const DATOS_FONDO: u32 = 0x0013_1C18;
const DATOS_TITULO_FONDO: u32 = 0x001B_2622;
/// El borde, discreto. Lo que separa la ventana del fondo es la sombra.
const DATOS_BORDE: u32 = 0x002C_4038;
/// Y el acento verde, que si puede ser vivo: es una linea, no un marco.
const DATOS_TITULO: u32 = 0x0034_D399;

/// Los cuatro niveles de `bmo_estratos::espacio`, con su color.
///
/// El orden es el del ABI (`INFO_ES_NIVEL`), no uno inventado aqui: si
/// divergieran, el panel pintaria en verde un volumen en solo lectura.
fn nivel_texto(n: u64) -> (&'static str, u32) {
    match n {
        0 => ("holgado", TEXTO_BIEN),
        1 => ("AVISO: por encima del 70%", 0x00F0_D070),
        2 => ("FAULT: por encima del 85%", TEXTO_MAL),
        _ => ("SOLO LECTURA: por encima del 95%", TEXTO_MAL),
    }
}

/// La ventana de Datos: **un marco y lo que hay dentro**.
///
/// Todo lo de mover, estirar, maximizar y los tres botones vive en
/// [`super::marco::Marco`] y no aqui. Lo que queda en esta estructura es lo
/// unico que de verdad es de ESTRATOS: que se esta ensenando y por donde va la
/// vista del arbol.
pub(crate) struct CajaDatos {
    pub(crate) marco: Marco,
    /// Que se esta ensenando: los numeros o el arbol. Ver [`Vista`].
    pub(crate) vista: Vista,
    /// Que hijo esta senalado en la vista de nodos.
    pub(crate) sel: usize,
    /// Primer hijo visible: la lista es mas larga que la ventana.
    pub(crate) desde: usize,
    /// Lo que dijo la ultima verificacion de firma, si se pidio alguna.
    ///
    /// `None` es "no se ha preguntado", y **no es lo mismo que "sin firma"**:
    /// ensenar `sin firma` sin haber mirado seria contestar por el disco.
    /// Se borra al cambiar de nodo -- el resultado es de UN archivo.
    pub(crate) verificado: Option<u64>,
}

/// Lo que se puede encoger sin que deje de servir. Por debajo de esto el grafo
/// no cabe y los numeros se cortan -- una ventana que se puede dejar inservible
/// con el raton es una trampa, no una libertad.
pub(crate) const DATOS_MIN_ANCHO: u32 = 460;
pub(crate) const DATOS_MIN_ALTO: u32 = 260;
/// Y el tamano con el que nace, **en tantos por ciento de la pantalla**. Un
/// `640 x 330` en pixeles es correcto en una pantalla y en ninguna otra.
const DATOS_PCT_ANCHO: u32 = 46;
const DATOS_PCT_ALTO: u32 = 44;

/// Las dos caras de esta ventana.
///
/// La de numeros contesta *como esta el almacen?* y la de nodos *que hay
/// dentro?*. Son preguntas distintas y por eso no se mezclan en una pantalla:
/// meter un arbol entre la generacion y la ocupacion deja las dos ilegibles.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Vista {
    Numeros,
    Nodos,
}

// El alto de la barra de titulo --que es el asa-- sale de `super::TITULO_ALTO`:
// el mismo que la caja de Ejecutar. Dos ventanas del mismo sistema con barras
// de distinta altura se ven como dos programas de distinta epoca.

impl CajaDatos {
    pub(crate) fn nueva(p: &bmo::Pantalla) -> Self {
        Self {
            marco: Marco::nuevo(
                p,
                DATOS_PCT_ANCHO,
                DATOS_PCT_ALTO,
                DATOS_MIN_ANCHO,
                DATOS_MIN_ALTO,
            ),
            vista: Vista::Numeros,
            sel: 0,
            desde: 0,
            verificado: None,
        }
    }

    /// * **Sobre que caja del grafo esta el puntero**, si sobre alguna.
    ///
    /// `None` es "en ninguna" y `Some(usize::MAX)` es **la caja del padre**, la
    /// de la izquierda: pulsarla sube un nivel, que es el gesto que la mano
    /// busca sola cuando ya se ha bajado.
    ///
    /// La geometria se calcula IGUAL que en `pintar_nodos` y ese es el riesgo
    /// de este metodo: si una de las dos cambia y la otra no, se pulsa una caja
    /// y se selecciona otra. Las dos usan las mismas constantes y el mismo
    /// reparto del ancho a proposito.
    pub(crate) fn caja_en(&self, px: u32, py: u32, cuantos: usize) -> Option<usize> {
        if self.vista != Vista::Nodos || self.marco.minimizada {
            return None;
        }
        let (tx, ancho_caja, hijos_x, primera_y) = self.geometria_grafo();
        // La del padre?
        if px >= tx && px < tx + ancho_caja && py >= primera_y && py < primera_y + CAJA_NODO_ALTO {
            return Some(usize::MAX);
        }
        if px < hijos_x || px >= hijos_x + ancho_caja || py < primera_y {
            return None;
        }
        let paso = CAJA_NODO_ALTO + CAJA_NODO_HUECO;
        let fila = ((py - primera_y) / paso) as usize;
        // El hueco ENTRE dos cajas no es ninguna de las dos. Sin esta guarda,
        // pulsar en el aire seleccionaria la de arriba.
        if (py - primera_y) % paso >= CAJA_NODO_ALTO {
            return None;
        }
        let i = self.desde + fila;
        if i < cuantos && fila < self.caben() { Some(i) } else { None }
    }

    /// El reparto del grafo: `(x del padre, ancho de caja, x de los hijos, y de
    /// la primera fila)`. **Lo comparten quien pinta y quien acierta.**
    fn geometria_grafo(&self) -> (u32, u32, u32, u32) {
        const CANAL: u32 = 44;
        let tx = self.marco.x + 16;
        let util = self.marco.ancho.saturating_sub(32);
        let ancho_caja = ((util.saturating_sub(CANAL)) / 2).max(CAJA_NODO_MIN);
        let hijos_x = tx + ancho_caja + CANAL;
        let primera_y = self.marco.y + TITULO_ALTO + 6 + bmo::GLIFO_ALTO + 10 + 4;
        (tx, ancho_caja, hijos_x, primera_y)
    }

    // Los atajos de siempre, para no escribir `.marco.` en cada uso. Son
    // reenvios y nada mas: la logica vive en `Marco` y aqui no se repite.
    pub(crate) fn x(&self) -> u32 { self.marco.x }
    pub(crate) fn y(&self) -> u32 { self.marco.y }
    pub(crate) fn ancho(&self) -> u32 { self.marco.ancho }
    pub(crate) fn alto(&self) -> u32 { self.marco.alto }

    /// Este pixel cae dentro? Lo necesita el borrado para saber que repintar.
    pub(crate) fn contiene(&self, px: u32, py: u32) -> bool {
        self.marco.contiene(px, py)
    }

    /// Tras cambiar de tamano, la seleccion puede haber quedado fuera de lo que
    /// se pinta. Se recoloca la ventana de scroll -- si no, encoger dejaria el
    /// cursor senalando una caja que ya no esta en pantalla.
    pub(crate) fn recolocar(&mut self) {
        let caben = self.caben();
        if self.sel >= self.desde + caben {
            self.desde = self.sel + 1 - caben;
        }
    }

    /// Cuantas cajas de hijo caben de una vez en la vista de nodos.
    fn caben(&self) -> usize {
        let util = self.marco.alto.saturating_sub(TITULO_ALTO + 56);
        (util / (CAJA_NODO_ALTO + CAJA_NODO_HUECO)).max(1) as usize
    }

    /// Mueve la seleccion y arrastra la ventana de scroll con ella.
    pub(crate) fn mover_sel(&mut self, delta: i32, cuantos: usize) {
        if cuantos == 0 {
            self.sel = 0;
            self.desde = 0;
            return;
        }
        let tope = cuantos - 1;
        self.sel = (self.sel as i32 + delta).clamp(0, tope as i32) as usize;
        let caben = self.caben();
        if self.sel < self.desde {
            self.desde = self.sel;
        } else if self.sel >= self.desde + caben {
            self.desde = self.sel + 1 - caben;
        }
    }

    /// Vuelve al principio de la lista. Se llama al cambiar de nodo: dejar la
    /// seleccion donde estaba haria que entrar en un directorio de dos hijos
    /// senalara al septimo, que no existe.
    pub(crate) fn al_principio(&mut self) {
        self.sel = 0;
        self.desde = 0;
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
const CAJA_NODO_MIN: u32 = 170;
const CAJA_NODO_ALTO: u32 = 40;
const CAJA_NODO_HUECO: u32 = 12;
const SOMBRA_NODO: u32 = 0x000B_100E;
/// Las aristas del grafo. Mas claras que el borde de la ventana **a proposito**:
/// son lo que hay que seguir con la vista, y una linea del mismo tono que el
/// marco se pierde entre los marcos.
const DATOS_ARISTA: u32 = 0x0045_6B5C;
/// El cuerpo de una caja del grafo: un peldano por encima de la ventana, que es
/// la misma regla que separa la ventana del escritorio.
const CAJA_NODO_FONDO: u32 = 0x001B_2622;
/// Y la senalada, otro peldano mas. La profundidad se lee sola.
const CAJA_NODO_SEL: u32 = 0x0024_332C;

/// **Un color por clase, y el mismo en toda la ventana.** Es el punto 2 de la
/// spec: si el verde significara una cosa en el padre y otra en los hijos, el
/// color dejaria de informar y solo decoraria.
fn color_clase(tipo: u64) -> (&'static str, u32) {
    match tipo {
        bmo::estratos::DIRECTORIO => ("directorio", 0x0057_C8F0),
        bmo::estratos::ARCHIVO => ("archivo", 0x007E_E787),
        _ => ("ilegible", TEXTO_MAL),
    }
}

/// Una caja con **titulo** (que es) y **nombre** (cual es). Punto 3 de la spec.
///
/// El titulo va arriba y en el color de la clase; el nombre, debajo y en
/// blanco. Al reves se leeria el nombre y habria que buscar el tipo, que es lo
/// contrario de para que esta el color.
fn caja_nodo(
    p: &bmo::Pantalla,
    x: u32,
    y: u32,
    ancho: u32,
    tipo: u64,
    name: &[u8],
    pointed_at: bool,
) {
    let (titulo, color) = color_clase(tipo);
    // La senalada lleva el borde del acento y un cuerpo un punto mas claro. Un
    // borde blanco a secas se lee como "esto esta roto"; el realce de una
    // seleccion tiene que ser el color del sistema, no una alarma.
    let (borde, cuerpo) = if pointed_at {
        (color, CAJA_NODO_SEL)
    } else {
        (DATOS_BORDE, CAJA_NODO_FONDO)
    };
    // Sombra propia. Es lo que separa las cajas del fondo de la ventana y lo
    // que hace que un grafo parezca un grafo y no una lista con marcos.
    rect_redondeado(p, x + 2, y + 3, ancho, CAJA_NODO_ALTO, SOMBRA_NODO);
    rect_redondeado(p, x, y, ancho, CAJA_NODO_ALTO, borde);
    rect_redondeado(p, x + 1, y + 1, ancho - 2, CAJA_NODO_ALTO - 2, cuerpo);

    // * El PUNTO de clase, no una pestana lateral.
    //
    // La barra pegada al borde peleaba con la curva de la esquina y se veia
    // como un defecto. Un punto delante del titulo es el mismo idioma que usan
    // la barra del sistema y las dos ventanas: se lee la clase de un vistazo y
    // no depende de que el titulo quepa.
    p.rect(x + 11, y + 7, 7, 7, color);
    p.texto(x + 24, y + 5, titulo, color);

    // El nombre, en su propia linea y en blanco. El titulo dice QUE es y el
    // nombre CUAL es; ponerlos del mismo color obliga a leer los dos para
    // saber cual es cual.
    let cabe = ((ancho.saturating_sub(28)) / bmo::GLIFO_ANCHO) as usize;
    // Recortar por el final y no por el principio: los nombres de un volumen
    // se distinguen por delante (`maestro.bex`, `movim.txt`), no por detras.
    let n = name.len().min(cabe);
    p.texto_bytes(x + 24, y + 5 + bmo::GLIFO_ALTO + 3, &name[..n], TEXTO);
    // Si no cupo entero se dice con un punto, no cortando a lo bruto: una
    // ventana estrecha no puede hacer que dos archivos parezcan el mismo.
    if n < name.len() {
        p.texto(
            x + 24 + n as u32 * bmo::GLIFO_ANCHO,
            y + 5 + bmo::GLIFO_ALTO + 3,
            "~",
            TEXTO_TENUE,
        );
    }
}

/// La vista de NODOS: el nodo actual a la izquierda y sus hijos a la derecha,
/// unidos por una espina y sus ramas.
fn pintar_nodos(p: &bmo::Pantalla, c: &CajaDatos) {
    let tx = c.marco.x + 16;
    let mut ty = c.marco.y + TITULO_ALTO + 6;

    if bmo::info(bmo::INFO_ES_MONTADO) == 0 {
        p.texto(tx, ty, "ningun volumen ESTRATOS montado.", TEXTO_MAL);
        return;
    }
    if !bmo::estratos::a_la_raiz() && bmo::estratos::tipo() == bmo::estratos::NOTHING {
        p.texto(tx, ty, "el volumen monta pero no tiene raiz legible.", TEXTO_MAL);
        ty += bmo::GLIFO_ALTO + 4;
        p.texto(tx, ty, "el motivo esta en F11.", TEXTO_TENUE);
        return;
    }

    let hondo = bmo::estratos::hondo();
    let cuantos = bmo::estratos::hijos() as usize;

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
        let mut x = p.texto(tx, ty, "/", DATOS_TITULO);
        let mut nivel = 1u64;
        while nivel <= hondo {
            let mut nom = [0u8; 40];
            let n = bmo::estratos::nombre_nivel(nivel, &mut nom);
            x = p.texto(x + 2, ty, " > ", TEXTO_TENUE);
            // El ultimo tramo en blanco y los de antes apagados: se lee de un
            // vistazo donde estas sin perder de donde vienes.
            let tinta = if nivel == hondo { TEXTO } else { TEXTO_TENUE };
            x = p.texto_bytes(x, ty, &nom[..n], tinta);
            nivel += 1;
        }
        let mut b = [0u8; 10];
        let x = p.texto(x + 3 * bmo::GLIFO_ANCHO, ty, "hijos ", TEXTO_TENUE);
        let n = decimal(cuantos as u64, &mut b);
        let x = p.texto_bytes(x, ty, &b[..n], TEXTO);
        if bmo::estratos::truncado() {
            // Se DICE. Un listado recortado en silencio se ve igual que un
            // directorio con pocos archivos, y esa confusion cuesta horas.
            p.texto(x, ty, "  (RECORTADO)", TEXTO_MAL);
        }
    }
    ty += bmo::GLIFO_ALTO + 10;

    // -- * EL REPARTO DEL ANCHO --
    //
    // Las cajas ya no miden lo mismo pase lo que pase: el ancho util se parte
    // entre las dos columnas y el canal de las ramas. Estirar la ventana hace
    // que quepan nombres mas largos, que es para lo que uno la estira.
    //
    // ** Y sale de `geometria_grafo`, **la misma que usa el acierto del
    // raton**. Tenerlo dos veces era garantizar que un dia se pulsara una caja
    // y se seleccionara otra: dos copias de una geometria se separan solas.
    let (_, ancho_caja, hijos_x, primera_y) = c.geometria_grafo();

    // -- El nodo actual, a la izquierda --
    let padre_y = primera_y;
    // El nombre del padre: en la raiz `/`, y si no, el ultimo tramo de la ruta.
    let mut nom_padre = [0u8; 40];
    let np = if hondo == 0 {
        nom_padre[0] = b'/';
        1
    } else {
        bmo::estratos::nombre_nivel(hondo, &mut nom_padre)
    };
    caja_nodo(p, tx, padre_y, ancho_caja, bmo::estratos::tipo(), &nom_padre[..np], false);
    if hondo > 0 {
        // Se dice que se puede subir, y como. Un gesto que existe y no esta
        // escrito lo conoce solo quien lo programo.
        p.texto(tx, padre_y + CAJA_NODO_ALTO + 6, "clic aqui SUBE", TEXTO_TENUE);
    }

    if cuantos == 0 {
        p.texto(tx, padre_y + CAJA_NODO_ALTO + 22, "esta vacio.", TEXTO_TENUE);
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
    // Los tirantes van horizontales y a media distancia (`CANAL/2`), que es lo
    // que hace que la curva salga y entre en horizontal aunque el hijo este
    // muy abajo: la clasica S. Ver `bmo::curva`.
    let caben = c.caben();
    let ultimo = (c.desde + caben).min(cuantos);
    // El recorte, que hasta ahora no existia: las aristas no pueden salirse del
    // marco de la ventana. Con codos calculados a mano no hacia falta --nunca
    // se salian por construccion--; una curva se sale en cuanto el marco se
    // encoge, y sin esto pintaria por encima de lo que haya al lado.
    let rec = bmo::Recorte::nuevo(
        c.marco.x as i32 + 1,
        (c.marco.y + TITULO_ALTO) as i32,
        c.marco.ancho as i32 - 2,
        c.marco.alto as i32 - TITULO_ALTO as i32 - 1,
    );
    let salida_y = padre_y + CAJA_NODO_ALTO / 2;
    let salida_x = tx + ancho_caja;
    // El tirante: la mitad del canal. Sale de la geometria y no de un numero a
    // ojo, asi que estirar la ventana no descuadra las curvas.
    let tirante = ((hijos_x - salida_x) / 2) as i32;

    let mut hy = primera_y;
    for i in c.desde..ultimo {
        let centro = hy + CAJA_NODO_ALTO / 2;
        p.curva(
            &rec,
            (salida_x as i32, salida_y as i32),
            (salida_x as i32 + tirante, salida_y as i32),
            (hijos_x as i32 - tirante, centro as i32),
            (hijos_x as i32, centro as i32),
            DATOS_ARISTA,
        );
        // La punta de flecha, que es el escalon 2 ganandose el sitio: dice
        // hacia DONDE va la arista, que con una curva ya no es obvio si se mira
        // solo un tramo. Tres vertices y entra en la caja por su borde.
        p.triangulo(
            &rec,
            (hijos_x as i32, centro as i32),
            (hijos_x as i32 - 7, centro as i32 - 4),
            (hijos_x as i32 - 7, centro as i32 + 4),
            DATOS_ARISTA,
        );
        let tipo = bmo::estratos::hijo_tipo(i as u64);
        let mut name = [0u8; 64];
        let n = bmo::estratos::hijo_nombre(i as u64, &mut name);
        caja_nodo(p, hijos_x, hy, ancho_caja, tipo, &name[..n], i == c.sel);
        hy += CAJA_NODO_ALTO + CAJA_NODO_HUECO;
    }
    // El punto de salida en el padre: cierra las aristas en su origen en vez de
    // dejarlas naciendo de un borde. Con una sola espina hacia falta uno; con
    // una curva por hijo, todas salen de aqui y por eso se nota mas.
    p.rect(salida_x - 1, salida_y - 2, 5, 5, DATOS_ARISTA);

    // -- * EL PANEL DE DETALLE del nodo senalado --
    //
    // Un grafo que solo ensena nombres contesta *que hay*; no contesta *que es
    // esto*. Va al PIE y en una linea: es informacion de apoyo, y un panel
    // lateral se comeria el ancho que las cajas necesitan para sus nombres.
    if c.sel < cuantos {
        let dy = c.marco.y + c.marco.alto - TITULO_ALTO - bmo::GLIFO_ALTO - 2;
        p.rect(c.marco.x + 1, dy - 6, c.marco.ancho - 2, 1, DATOS_BORDE);
        let mut b = [0u8; 10];
        let x = p.texto(tx, dy, "sel: ", TEXTO_TENUE);
        let n = decimal(bmo::estratos::hijo_bytes(c.sel as u64), &mut b);
        let x = p.texto_bytes(x, dy, &b[..n], TEXTO);
        let x = p.texto(x, dy, " B   atributos ", TEXTO_TENUE);
        let n = decimal(bmo::estratos::hijo_atributos(c.sel as u64), &mut b);
        let x = p.texto_bytes(x, dy, &b[..n], TEXTO);
        // La firma. **Se dice si la LLEVA; que CUADRE se pide con V** -- leer el
        // archivo entero y hacerle el BLAKE3 en cada repintado convertiria un
        // panel en un martillo sobre el disco.
        let x = p.texto(x, dy, "   firma ", TEXTO_TENUE);
        let x = if bmo::estratos::hijo_firmado(c.sel as u64) {
            p.texto(x, dy, "SI", TEXTO_BIEN)
        } else {
            p.texto(x, dy, "no", TEXTO_TENUE)
        };
        // Y el resultado de la ultima verificacion, si se pidio.
        match c.verificado {
            None => {
                p.texto(x + 2 * bmo::GLIFO_ANCHO, dy, "V comprueba", TEXTO_TENUE);
            }
            Some(bmo::estratos::FIRMA_CUADRA) => {
                p.texto(x + 2 * bmo::GLIFO_ANCHO, dy, "CUADRA", TEXTO_BIEN);
            }
            Some(bmo::estratos::FIRMA_NO_CUADRA) => {
                // El unico mensaje de esta ventana que significa "hay un
                // problema en el disco". Por eso es el unico en rojo.
                p.texto(x + 2 * bmo::GLIFO_ANCHO, dy, "NO CUADRA", TEXTO_MAL);
            }
            Some(bmo::estratos::FIRMA_AUSENTE) => {
                p.texto(x + 2 * bmo::GLIFO_ANCHO, dy, "sin firma", TEXTO_TENUE);
            }
            Some(bmo::estratos::FIRMA_NO_CABE) => {
                // TENUE y no rojo: el archivo esta bien, lo que no cabe es
                // nuestro buffer de comprobacion. Pintarlo en rojo mandaba a
                // buscar una corrupcion que no existe.
                p.texto(x + 2 * bmo::GLIFO_ANCHO, dy, "no cabe (>256 KiB)", TEXTO_TENUE);
            }
            _ => {
                p.texto(x + 2 * bmo::GLIFO_ANCHO, dy, "no se pudo leer", TEXTO_MAL);
            }
        }
    }

    // Si la lista no cabe entera, decirlo con numeros y no con puntos
    // suspensivos: "3-8 de 40" se lee; "..." no dice cuanto falta.
    if cuantos > caben {
        let mut b = [0u8; 10];
        let y = c.marco.y + c.marco.alto - TITULO_ALTO - bmo::GLIFO_ALTO;
        let n = decimal(c.desde as u64 + 1, &mut b);
        let x = p.texto_bytes(hijos_x, y, &b[..n], TEXTO_TENUE);
        let x = p.texto(x, y, "-", TEXTO_TENUE);
        let n = decimal(ultimo as u64, &mut b);
        let x = p.texto_bytes(x, y, &b[..n], TEXTO_TENUE);
        let x = p.texto(x, y, " de ", TEXTO_TENUE);
        let n = decimal(cuantos as u64, &mut b);
        p.texto_bytes(x, y, &b[..n], TEXTO_TENUE);
    }
}

/// Pinta la consola de datos entera.
///
/// Se redibuja completa en cada invocacion y no por fotograma: los numeros de
/// un volumen no cambian solos mientras nadie escriba, y repintar 200k pixeles
/// sobre memoria de video sin cache sesenta veces por segundo para ensenar los
/// mismos digitos es tirar el fotograma.
pub(crate) fn pintar(p: &bmo::Pantalla, c: &CajaDatos) {
    if c.marco.minimizada {
        return;
    }
    // * El cromo entero --sombra, borde, cuerpo, barra, los tres botones y el
    // asa de la esquina-- lo pinta el MARCO. Aqui solo van los colores, que si
    // son de esta ventana: el verde dice ESTRATOS antes de que nadie lea el
    // titulo.
    c.marco.pintar_cromo(p, DATOS_BORDE, DATOS_FONDO, DATOS_TITULO_FONDO, DATOS_TITULO);

    let tx = c.marco.x + 16;
    p.rect(tx, c.marco.y + 9, 8, 8, DATOS_TITULO);
    let px = p.texto(tx + 16, c.marco.y + 8, "ESTRATOS", TEXTO);
    let px = px + 2 * bmo::GLIFO_ANCHO;
    // Las pestanas: la activa lleva su subrayado. Un corchete pintado de otro
    // color se pierde en una foto; una linea debajo no.
    let (c1, c2) = match c.vista {
        Vista::Numeros => (TEXTO, TEXTO_TENUE),
        Vista::Nodos => (TEXTO_TENUE, TEXTO),
    };
    let fin1 = p.texto(px, c.marco.y + 8, "numeros", c1);
    let px2 = fin1 + 2 * bmo::GLIFO_ANCHO;
    let fin2 = p.texto(px2, c.marco.y + 8, "nodos", c2);
    let (sx, sw) = match c.vista {
        Vista::Numeros => (px, fin1 - px),
        Vista::Nodos => (px2, fin2 - px2),
    };
    p.rect(sx, c.marco.y + 8 + bmo::GLIFO_ALTO + 2, sw, 2, DATOS_TITULO);

    if c.vista == Vista::Nodos {
        pintar_nodos(p, c);
        let y = c.marco.y + c.marco.alto - bmo::GLIFO_ALTO - 8;
        p.texto(tx, y, "flechas mueven   ENTRAR baja   RETROCESO sube   F12 cierra", TEXTO_TENUE);
        return;
    }

    let mut ty = c.marco.y + TITULO_ALTO + 6;

    if bmo::info(bmo::INFO_ES_MONTADO) == 0 {
        p.texto(tx, ty, "ningun volumen ESTRATOS montado.", TEXTO_MAL);
        ty += bmo::GLIFO_ALTO + 4;
        p.texto(tx, ty, "se formatea desde el anfitrion con estratos-fmt.", TEXTO_TENUE);
        return;
    }

    let bloques = bmo::info(bmo::INFO_ES_BLOQUES);
    let usados = bmo::info(bmo::INFO_ES_USADOS);
    let tam = bmo::info(bmo::INFO_ES_BLOQUE_TAM).max(1);
    let nivel = bmo::info(bmo::INFO_ES_NIVEL);

    let mut fila = |etiqueta: &str, y: &mut u32, pinta: &dyn Fn(u32, u32)| {
        p.texto(tx, *y, etiqueta, TEXTO_TENUE);
        pinta(tx + 13 * bmo::GLIFO_ANCHO, *y);
        *y += bmo::GLIFO_ALTO + 3;
    };

    fila("generacion", &mut ty, &|x, y| {
        let g = bmo::info(bmo::INFO_ES_GENERACION);
        let mut b = [0u8; 10];
        let n = decimal(g, &mut b);
        let x = p.texto_bytes(x, y, &b[..n], TEXTO);
        p.texto(x, y, "  transacciones desde el formateo", TEXTO_TENUE);
    });

    fila("espacio", &mut ty, &|x, y| {
        let x = magnitud(p, x, y, usados * tam, TEXTO);
        let x = p.texto(x, y, " de ", TEXTO_TENUE);
        let x = magnitud(p, x, y, bloques * tam, TEXTO);
        let pct = if bloques == 0 { 0 } else { usados * 100 / bloques };
        let x = p.texto(x, y, "   ", TEXTO_TENUE);
        let mut b = [0u8; 10];
        let n = decimal(pct, &mut b);
        let x = p.texto_bytes(x, y, &b[..n], TEXTO);
        p.texto(x, y, "%", TEXTO);
    });

    fila("estado", &mut ty, &|x, y| {
        let (t, color) = nivel_texto(nivel);
        p.texto(x, y, t, color);
    });

    fila("identidad", &mut ty, &|x, y| {
        if bmo::info(bmo::INFO_ES_IDENTIDAD) != 0 {
            p.texto(x, y, "nacio en ESTE disco", TEXTO_BIEN);
        } else {
            p.texto(x, y, "NO nacio aqui: clonado? no se escribira", TEXTO_MAL);
        }
    });

    // * Cuantas VERSIONES mas caben. Es lo que de verdad contesta "cuando
    // hara falta el recolector?" -- un porcentaje no lo dice, y la respuesta
    // con 414 GiB son millones.
    fila("caben", &mut ty, &|x, y| {
        let libres = bloques.saturating_sub(usados);
        let por_obj = (20 * 1024u64).div_ceil(tam).max(1);
        let mut b = [0u8; 10];
        let n = decimal(libres / por_obj, &mut b);
        let x = p.texto_bytes(x, y, &b[..n], TEXTO);
        p.texto(x, y, "  objetos mas de 20 KiB", TEXTO_TENUE);
    });

    ty += 8;
    // -- La verdad sobre la escritura --
    if bmo::info(bmo::INFO_ES_ESCRIBIBLE) != 0 {
        p.texto(tx, ty, "escritura: ABIERTA", TEXTO_BIEN);
    } else {
        p.texto(tx, ty, "escritura: CERRADA", TEXTO_MAL);
        ty += bmo::GLIFO_ALTO + 3;
        p.texto(tx, ty, "  la transaccion existe y esta probada (12 tests),", TEXTO_TENUE);
        ty += bmo::GLIFO_ALTO + 2;
        p.texto(tx, ty, "  pero nadie la ha cableado al dispositivo todavia:", TEXTO_TENUE);
        ty += bmo::GLIFO_ALTO + 2;
        p.texto(tx, ty, "  falta el write y el FLUSH CACHE de verdad.", TEXTO_TENUE);
    }

    ty += bmo::GLIFO_ALTO + 10;
    p.texto(tx, ty, "F12 o ESC cierran.   TAB ensena el ARBOL de nodos.", TEXTO_TENUE);
    ty += bmo::GLIFO_ALTO + 2;
    // * Decirlo aqui evita el susto: con esta ventana delante el teclado es
    // SUYO, asi que teclear no escribe en la caja de abajo. Antes si escribia
    // --en una ventana tapada, sin verlo--, y eso era el fallo.
    p.texto(tx, ty, "mientras este abierta, el teclado es de esta ventana.", TEXTO_TENUE);
}

/// Un numero de bytes con su unidad. Devuelve la x donde acabo.
///
/// Sin coma flotante: la parte fraccionaria sale de multiplicar el resto por
/// cien antes de dividir. Es la misma cuenta que hace el panel del kernel, y
/// esta aqui duplicada a proposito -- cruzar el anillo para formatear un numero
/// seria exactamente lo que un library OS no hace.
fn magnitud(p: &bmo::Pantalla, x: u32, y: u32, bytes: u64, color: u32) -> u32 {
    const K: u64 = 1024;
    const M: u64 = K * 1024;
    const G: u64 = M * 1024;
    let (unidad, div) = if bytes >= G {
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
    p.texto(x, y, unidad, color)
}
