//! El SPLASH de Ring 0 -- lo que se ve cuando UEFI termina.
//!
//! ## ** ESTE FICHERO TENIA 1.503 LINEAS, Y ESO ERA EL PROBLEMA
//!
//! Dentro convivian cinco cosas que no se parecen en nada: como se escribe un
//! pixel en el framebuffer, que forma tiene una letra, que hora es, el guion de
//! la intro animada y el panel persistente de la CABINA. Cinco motivos
//! distintos para cambiar el mismo fichero.
//!
//! Y no era una molestia estetica. El fallo del video del arranque del
//! 2026-08-15 --una franja muerta de 191 px pegada al borde izquierdo, el 7,2%
//! de la pantalla sin escribir en cada fotograma-- era **una linea de recorte
//! escrita a mano** perdida entre las otras mil quinientas:
//!
//! ```text
//!     if cw > 0 && ch > 0 && x >= 0 && y >= 0 && ... { fill_rect(...) }
//! ```
//!
//! Eso no recorta: descarta. Nadie la miro nunca como lo que era --la regla de
//! recorte del kernel-- porque no estaba en ningun sitio que se llamara asi.
//!
//! ## El corte, por lo que cada pieza contesta
//!
//! ```text
//!    lienzo.rs    DONDE caen los pixeles. El framebuffer y el recorte.
//!    texto.rs     QUE pixeles tiene una letra. La fuente de 8x16.
//!    reloj.rs     QUE HORA es. El TSC, y por que la animacion lo consulta.
//!    escena.rs    LA INTRO: el gato, la ciudad, el destello, los cuatro actos.
//!    tablero.rs   LA CABINA: el panel que se queda cuando el arranque acaba.
//! ```
//!
//! Aqui no queda dibujo. Quedan **los colores de la marca** --que los comparten
//! los tres modulos que pintan-- y la fachada publica, que es lo unico que ve el
//! resto del kernel.
//!
//! ## Lo que decia haber y no habia
//!
//! Esta cabecera anunciaba un *"animated concentric logo (inside-out
//! expansion)"*. Existia --`draw_logo_animated`, `draw_ring`, `fill_circle`,
//! `isqrt`, cuatro constantes `LOGO_*`-- y **no lo llamaba nadie**: unas 120
//! lineas de anillos concentricos que ningun arranque dibujo nunca. Se borro el
//! 2026-08-07 junto con `scene`, la carteleria de texto que sustituyo el logo.
//!
//! Un comentario que promete una funcion que no se ejecuta es peor que no tener
//! comentario: manda a buscar el bug en el sitio equivocado.

mod arranque;
mod escena;
mod lienzo;
mod reloj;
mod tablero;
mod texto;

// ?????? Color palette ???????????????????????????????????????????????????????
// ** NEGRO, y no el azul pizarra que habia.
//
// El logo de BMO-X es negro puro --se midio al generar las mascaras del gato:
// 97% negro plano-- y la pantalla de arranque decia ser ese logo mientras lo
// pintaba sobre `0xFF0A0F1D`. A ojo la diferencia es poca; al lado del PNG del
// README es una pantalla que no es la marca.
//
// Y hay un motivo tecnico ademas del de identidad: el gato se guarda **sin
// fondo** (ver `gato/mod.rs`) porque el fondo del splash ya es el fondo del
// logo. Esa frase solo es cierta si son el mismo color.
//
// Viven en el orquestador y no en un modulo de color porque son de la MARCA, no
// del dibujo: los mismos cinco valores los usan la intro, el panel de arranque
// y la CABINA. Un modulo `paleta` de cinco constantes seria un fichero por
// simetria.
const BG: u32          = 0xFF000000; // Negro, como el logo
const WHITE: u32       = 0xFFF1F5F9; // Soft crisp white
const DIM: u32         = 0xFF64748B; // Slate-500 muted text
const ACCENT: u32      = 0xFF00E5FF; // Neon cyan highlight
const ACCENT2: u32     = 0xFF818CF8; // Indigo-400 accent for loading state

// ?????? La fachada ??????????????????????????????????????????????????????????
//
// Lo que el resto del kernel llama, con los mismos nombres de siempre. Partir el
// fichero por dentro no cambia una linea en `phase.rs`, en `cabina/` ni en
// `faults.rs`: los modulos son `pub(crate)` y lo que sale de aqui es esta lista.

pub use arranque::{splash_clear, splash_init, splash_progress};
pub use escena::{
    intro_cierra, intro_empieza, intro_en_curso, intro_latido, intro_paso, intro_progreso,
};
pub use tablero::{
    dash_rows, splash_dash_rule, splash_dashboard_init, splash_dashboard_log,
    splash_dashboard_log_color, splash_dashboard_prompt, splash_status_right,
    DASH_LOG_W,
};

// -- Pantalla de fallo ---------------------------------------------------
//
// El informe de un fault de Ring 0 se pintaba en las filas del panel, encima
// de lo que hubiera. Cuando la pantalla esta cedida a Ring 3 eso deja el
// informe flotando sobre el escritorio de otro; y aunque no lo estuviera, un
// kernel que se muere merece decirlo con todas las letras y no en tres
// renglones apretados.
//
// Estos cuatro son lo minimo para pintar una pantalla entera desde `faults.rs`
// sin exponer el resto del splash ni duplicar el dibujado de texto.

/// Alto de una linea de texto, en pixeles. Lo necesita quien decida el layout.
pub const ALTO_LINEA: u32 = texto::CHAR_H as u32;
/// Ancho de un caracter. La fuente es de paso fijo.
pub const ANCHO_CHAR: u32 = texto::CHAR_W as u32;

/// Pinta la pantalla ENTERA de un color.
pub fn fallo_fondo(color: u32) {
    let w = unsafe { crate::info::FB_WIDTH };
    let h = unsafe { crate::info::FB_HEIGHT };
    if w == 0 || h == 0 { return; }
    lienzo::fill_rect(0, 0, w, h, color);
}

/// Un rectangulo. Para la barra de la cuenta atras.
pub fn fallo_rect(x: u32, y: u32, w: u32, h: u32, color: u32) {
    lienzo::fill_rect(x, y, w, h, color);
}

/// Texto en una posicion exacta.
pub fn fallo_texto(x: u32, y: u32, s: &str, color: u32) {
    texto::draw_str(x, y, s, color);
}

/// Texto grande, para el titulo.
pub fn fallo_texto_grande(x: u32, y: u32, s: &str, color: u32, escala: u32) {
    texto::draw_str_scaled(x, y, s, color, escala);
}
