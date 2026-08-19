//! **EL REPARTO DE LA VENTANA**: donde cae cada panel, decidido en un sitio.
//!
//! === Por que existe ===
//!
//! Hasta hoy cada vista de la ventana de ESTRATOS se creia duena del interior
//! entero: `paint_nodes` y `paint_folders` empezaban las dos en
//! `chrome.x + 16` y median contra `chrome.width`. Con una vista a la vez eso
//! funciona; con tres a la vez, no -- dos paneles que calculan su sitio contra
//! el marco se pintan encima el uno del otro.
//!
//! Aqui se parte el interior UNA vez y cada panel recibe su rectangulo. Nadie
//! mas mira `chrome`.
//!
//! === Y por que importa mas de lo que parece ===
//!
//! Porque la geometria del grafo **ya la usaban dos sitios**: el que pinta las
//! cajas y el que decide sobre cual cayo el raton. El aviso estaba escrito en
//! `box_at`: *si una de las dos cambia y la otra no, se pulsa una caja y se
//! selecciona otra*. Con tres paneles ese riesgo se multiplica -- ahora hay
//! tres sitios donde acertar y tres donde pintar.
//!
//! * **Una sola funcion contesta las dos preguntas.** Pintar y acertar salen
//! del mismo `Zonas`, asi que no pueden discrepar.
//!
//! === El orden en que se rinden los paneles ===
//!
//! Una ventana estrecha no puede llevar tres columnas, y decidirlo por las
//! bravas --repartir a partes iguales y que salga lo que salga-- da tres
//! columnas ilegibles en vez de una util.
//!
//! ```text
//!   cabe todo          arbol | rejilla | grafo
//!   no cabe el grafo   arbol | rejilla
//!   no cabe el arbol           rejilla
//! ```
//!
//! ** El GRAFO se rinde primero, y no es por gusto: la rejilla es donde se
//! trabaja y el arbol es como se llega. El grafo contesta *como se conecta
//! esto*, que es la pregunta que se puede aplazar -- y sigue estando entera en
//! cuanto la ventana se estira. Es la misma clase de decision que el umbral del
//! 60% del compositor: se elige lo que se pierde, en vez de perderlo todo un
//! poco.

use super::TITLE_H;
use super::chrome::Chrome;

/// Un rectangulo. No hay uno en `bmo_userland` porque la pantalla habla en
/// cuatro numeros sueltos, y aqui hacen falta juntos para poder pasarlos.
#[derive(Clone, Copy)]
pub(crate) struct Zona {
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
}

impl Zona {
    /// Un panel que no se pinta. **No es un panel de ancho cero**: es la
    /// respuesta a "aqui no cabe", y quien la reciba tiene que preguntarlo con
    /// [`hay`](Zona::hay) antes de pintar nada.
    pub const NADA: Self = Self { x: 0, y: 0, w: 0, h: 0 };

    pub fn hay(&self) -> bool {
        self.w > 0 && self.h > 0
    }

    pub fn contiene(&self, px: u32, py: u32) -> bool {
        self.hay() && px >= self.x && px < self.x + self.w && py >= self.y && py < self.y + self.h
    }

    /// El borde derecho. Sale de aqui y no de una suma en cada uso: una suma
    /// repetida es una que un dia se escribe con el signo cambiado.
    pub fn derecha(&self) -> u32 {
        self.x + self.w
    }

    pub fn abajo(&self) -> u32 {
        self.y + self.h
    }
}

/// El aire alrededor del contenido y entre paneles.
const MARGEN: u32 = 16;
const CANAL: u32 = 12;

/// Alto de la miga de pan, que va debajo de las pestanas y encima de todo.
pub(crate) const MIGA_H: u32 = 26;
/// Alto de la barra de estado del pie.
///
/// Dos lineas, y las dos ya existian sueltas al fondo de la ventana: el DETALLE
/// del nodo senalado (cuanto mide, cuantos atributos, si va firmado) y la de
/// AYUDA, que es donde se anuncia `S sella`. Se juntan en una zona con nombre
/// para que dejen de calcularse cada una contra `chrome.height`.
pub(crate) const PIE_H: u32 = 40;

/// Ancho del panel de arbol. Fijo, y a proposito.
///
/// Un arbol que se estira con la ventana se lleva el sitio de la rejilla, que
/// es donde se mira. Ciento sesenta y ocho pixeles son veintiun caracteres:
/// caben los nombres de carpeta que se usan y los que no se recortan **y se
/// dice**, en vez de robarle ancho a lo que importa.
const ARBOL_W: u32 = 168;
/// Por debajo de esto la rejilla deja de ser una rejilla.
const REJILLA_MIN: u32 = 240;
/// Lo que el grafo necesita para ser un grafo: **dos columnas de cajas y el
/// canal entre ellas**.
///
/// ** NO ES UN NUMERO PUESTO A OJO, Y ANTES LO ERA.
///
/// Aqui habia un `260`. El grafo pinta dos cajas de `NODE_MIN` con `CHANNEL` en
/// medio, o sea `2*170 + 44 = 384`. Entre 260 y 383 este reparto daba el panel
/// por bueno **y las cajas de los hijos se pintaban fuera de la ventana**, sobre
/// el escritorio: `node_box` no recorta, solo las curvas llevan `Recorte`.
///
/// Se vio en el Ryzen el 2026-08-18, en una foto, y es la clase de fallo que
/// esta casa ya conoce: dos constantes en dos ficheros que tienen que cuadrar y
/// nadie las obliga. Ahora **la cuenta se hace aqui con los numeros del grafo**,
/// asi que cambiar el tamano de una caja mueve este minimo solo.
const GRAFO_MIN: u32 = 2 * super::data::NODE_MIN + super::data::CHANNEL;
/// Que parte del sitio sobrante se lleva el grafo cuando cabe.
const GRAFO_PCT: u32 = 42;

/// Donde cae cada cosa dentro de la ventana de ESTRATOS.
pub(crate) struct Zonas {
    /// La miga de pan: `/ > datos > notas`, y a la derecha la cuenta.
    pub miga: Zona,
    /// El panel de arbol. [`Zona::NADA`] si la ventana es estrecha.
    pub arbol: Zona,
    /// Donde se trabaja: los hijos del nodo actual.
    pub rejilla: Zona,
    /// El grafo. [`Zona::NADA`] si no cabe.
    pub grafo: Zona,
    /// El terminal del pie. [`Zona::NADA`] mientras esta cerrado.
    ///
    /// * Le quita alto a los paneles y no se pinta encima de ellos, que es la
    /// diferencia entre un panel y un cartel: abrirlo ENCOGE lo de arriba, asi
    /// que nada queda tapado y nada hay que recordar donde estaba.
    pub consola: Zona,
    /// La barra de estado.
    pub pie: Zona,
}

impl Zonas {
    /// Reparte el interior de `c`.
    ///
    /// [!] Todas las restas son `saturating_sub`. Una ventana en su tamano
    /// minimo con la miga y el pie descontados puede dejar menos de cero de
    /// alto util, y un `u32` que baja de cero no da error: **da cuatro mil
    /// millones**, y el panel se pinta por toda la pantalla. Ya paso una vez en
    /// esta casa con `&ruta[..n]` y un `usize::MAX`.
    pub fn repartir(c: &Chrome, consola: bool) -> Self {
        let x0 = c.x + MARGEN;
        let y0 = c.y + TITLE_H + 6;
        let ancho = c.width.saturating_sub(MARGEN * 2);

        let miga = Zona { x: x0, y: y0, w: ancho, h: MIGA_H };

        // Lo que queda entre la miga y el pie.
        let cuerpo_y = miga.abajo();
        let cuerpo_h = c
            .height
            .saturating_sub(TITLE_H + 6 + MIGA_H + PIE_H + MARGEN / 2);

        // La consola sale del alto del CUERPO, no del pie: lo que encoge es lo
        // que se mira, y el estado de abajo tiene que seguir estando.
        let consola_h = if consola { super::consola::ALTO } else { 0 };
        let cuerpo_h = cuerpo_h.saturating_sub(consola_h);

        let consola = if consola_h > 0 {
            Zona { x: x0, y: cuerpo_y + cuerpo_h, w: ancho, h: consola_h }
        } else {
            Zona::NADA
        };

        let pie = Zona {
            x: x0,
            y: cuerpo_y + cuerpo_h + consola_h,
            w: ancho,
            h: PIE_H,
        };

        // -- Quien cabe --
        //
        // Se pregunta de mas a menos y se para en el primero que entra. Escrito
        // como una escalera y no como una formula porque la regla ES una
        // escalera: cual se cae primero es una decision, no un resultado.
        let con_arbol = ancho >= ARBOL_W + CANAL + REJILLA_MIN;
        let con_grafo = ancho >= ARBOL_W + CANAL + REJILLA_MIN + CANAL + GRAFO_MIN;

        let (arbol, resto_x, resto_w) = if con_arbol {
            (
                Zona { x: x0, y: cuerpo_y, w: ARBOL_W, h: cuerpo_h },
                x0 + ARBOL_W + CANAL,
                ancho.saturating_sub(ARBOL_W + CANAL),
            )
        } else {
            (Zona::NADA, x0, ancho)
        };

        let (rejilla, grafo) = if con_grafo {
            let g = (resto_w.saturating_sub(CANAL) * GRAFO_PCT / 100).max(GRAFO_MIN);
            let r = resto_w.saturating_sub(g + CANAL);
            (
                Zona { x: resto_x, y: cuerpo_y, w: r, h: cuerpo_h },
                Zona { x: resto_x + r + CANAL, y: cuerpo_y, w: g, h: cuerpo_h },
            )
        } else {
            (
                Zona { x: resto_x, y: cuerpo_y, w: resto_w, h: cuerpo_h },
                Zona::NADA,
            )
        };

        Self { miga, arbol, rejilla, grafo, consola, pie }
    }
}

/// [!] Y que no se pueda volver a torcer: si alguien sube `NODE_MIN` sin mirar,
/// esto no compila en vez de pintar por encima del escritorio.
const _: () = assert!(
    GRAFO_MIN >= 2 * super::data::NODE_MIN + super::data::CHANNEL,
    "el panel del grafo tiene que caber sus dos columnas y el canal"
);
