//! **EL EXPLORADOR DE ESTRATOS** -- los tres paneles a la vez.
//!
//! === De donde salio ===
//!
//! De `data.rs`, que cruzo las 1.000 lineas el 2026-08-19 al ganar
//! `paint_consola` -- la funcion que arregla que teclear en la consola
//! repintara la ventana entera. **Aqui no se cambio una linea de logica: solo
//! se movio**, y el corte estaba ya marcado a mano en el fichero.
//!
//! ```text
//!   data/mod.rs    la ventana: su estado, el grafo, el cromo y el reparto
//!   data/obra.rs   ESTO: la miga, las carpetas, la rejilla y el pie
//! ```

use bmo_userland as bmo;

use super::*;
use crate::scene::iconos;
use crate::scene::zonas::{Zona, MIGA_H};
use crate::text::decimal;

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

pub(crate) fn obra(p: &bmo::Pantalla, c: &DataWindow) {
    let z = Zonas::repartir(&c.chrome, c.consola.abierta);

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
    consola::paint(p, &z.consola, &c.consola, DATA_BG, DATA_EDGE, DATA_TITLE);
    pie(p, c, &z.pie);
    // EL ULTIMO de todo: es lo unico que puede taparlo todo, y si se pintara
    // antes lo taparia cualquier panel que venga detras.
    menu::paint(p, &c.menu, NODE_BG, DATA_TITLE);

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
            "flechas mueven  ENTRAR baja  F2 renombra  V firma  S sella  Ctrl+n consola",
            INK_DIM,
        ),
    };
}

/// Lo que ocupa la cabecera de la rejilla antes de la primera fila.
///
/// ** Sale de aqui y lo usan LOS DOS: el que pinta las filas y el que decide
/// sobre cual cayo el raton. Es el mismo aviso que lleva `box_at` desde que se
/// escribio -- dos copias de una geometria se separan solas, y el sintoma es
/// pulsar un fichero y que se seleccione el de al lado.
pub(crate) const REJILLA_CABECERA: u32 = bmo::GLIFO_ALTO + 7;

/// El alto de una fila del explorador. Una linea de texto y aire a los lados:
/// lo justo para que el realce de la seleccion no toque las letras.
pub(crate) const ROW_H: u32 = 22;

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
    p.rect(z.x, z.y + bmo::GLIFO_ALTO + 3, z.w, 1, DATA_EDGE);
    ty = z.y + REJILLA_CABECERA;

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
        let kind = bmo::estratos::hijo_tipo(i as u64);
        let (type_name, color) = class_color(kind);

        // El realce de la fila senalada. Va DEBAJO del texto y ocupa el ancho
        // entero: es como se lee "esta es la seleccionada" sin un cursor.
        if i == c.sel {
            realce(p, z.x, ty, z.w, ROW_H);
        }
        // ** EL ICONO. Aqui habia un cuadrito de color de ocho pixeles.
        //
        // El cuadrito decia la clase --y sigue diciendola, porque el icono va
        // del mismo color-- pero habia que HABERLO APRENDIDO: azul es
        // directorio, verde es archivo. Una carpeta se reconoce sin que nadie
        // te la explique, y eso es lo unico que un icono tiene que hacer.
        //
        // La forma dice QUE ES y el color dice lo mismo, asi que se refuerzan
        // en vez de competir. Y el color sigue siendo el de su caja en el
        // grafo de al lado: mirar el mismo nodo en los dos paneles no puede
        // darle dos colores.
        super::iconos::pintar(p, z.x + 2, ty + (ROW_H - iconos::LADO) / 2, kind, color, 1);

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
