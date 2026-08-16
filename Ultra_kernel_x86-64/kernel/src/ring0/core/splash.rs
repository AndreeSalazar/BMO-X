//! El SPLASH de Ring 0 -- lo que se ve cuando UEFI termina.
//!
//! === Lo que hay ===
//!
//! - [`boot_intro`]: **el gato**, `BMO-X` y `BMO METAKERNEL`. Una pantalla.
//! - El PANEL persistente: el log del kernel con su marca de tiempo, la banda
//!   de CABINA y el prompt del shell de Ring 0.
//! - La barra de progreso interpolada.
//!
//! === Y lo que decia haber y no habia ===
//!
//! Esta cabecera anunciaba un *"animated concentric logo (inside-out
//! expansion)"*. Existia --`draw_logo_animated`, `draw_ring`, `fill_circle`,
//! `isqrt`, cuatro constantes `LOGO_*`-- y **no lo llamaba nadie**: unas 120
//! lineas de anillos concentricos que ningun arranque dibujo nunca. Se borro el
//! 2026-08-07 junto con `scene`, la carteleria de texto que sustituyo el logo.
//!
//! Un comentario que promete una funcion que no se ejecuta es peor que no tener
//! comentario: manda a buscar el bug en el sitio equivocado.

// ?????? Font: 8x16 bitmap, chars 32..126 (space through ~) ??????????????????????????????

use crate::ring0::core::gato;

/// Cuanto se queda el logo a la vista, en ms.
///
/// [!] **Es tiempo de arranque puro.** No hay trabajo con el que solaparlo --el
/// kernel esta operativo a los 52 ms-- y no se puede saltar con una tecla porque
/// el USB no esta enumerado todavia.
///
/// El dueno pidio tres segundos. Van 1.600 porque el mismo dia se quitaron 4,5 s
/// de espera artificial (los cuatro carteles de aqui y la siesta del compositor),
/// y 3.000 devolveria dos tercios de lo ganado. Es una linea: si se quieren los
/// tres segundos, se cambia el numero.
const GATO_MS: u64 = 1600;

const FONT_H: usize   = 16;
const FONT_W: usize   = 8;
const CHAR_W: usize   = 10;  // 2px spacing
const CHAR_H: usize   = 20;  // 4px line spacing

static FONT16: [[u8; 16]; 120] = include!("font16_data.rs");
/// Bytes Latin-1 de los glifos extra, en el mismo orden en que aparecen en
/// FONT16 a partir del indice 95. Generado junto al font: si crece la tabla
/// del generador crecen los dos archivos y aqui solo cambia el tamano.
static FONT_EXTRA: [u8; 25] = include!("font16_extra.rs");
/// Cuantos glifos ASCII (32..=126) van primero en FONT16.
const ASCII_GLYPHS: usize = 95;

/// Byte -> indice de glifo. ASCII directo; para el espanol (n~, a-acento, ,
/// ...) se busca el byte Latin-1 en la tabla de extras.
///
/// Latin-1 y no UTF-8 a proposito: en Ring 0 un caracter es UN byte, asi el
/// teclado, la linea del shell y el framebuffer hablan el mismo idioma sin
/// decodificador de por medio.
fn glyph_index(c: u8) -> Option<usize> {
    if (32..=126).contains(&c) {
        return Some(c as usize - 32);
    }
    let mut i = 0;
    while i < FONT_EXTRA.len() {
        if FONT_EXTRA[i] == c { return Some(ASCII_GLYPHS + i); }
        i += 1;
    }
    None
}

// ?????? Color palette ???????????????????????????????????????????????????????????????????????????????????????????????????????????????????????????????????????????????????
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
const BG: u32          = 0xFF000000; // Negro, como el logo
const WHITE: u32       = 0xFFF1F5F9; // Soft crisp white
const DIM: u32         = 0xFF64748B; // Slate-500 muted text
const ACCENT: u32      = 0xFF00E5FF; // Neon cyan highlight
const ACCENT2: u32     = 0xFF818CF8; // Indigo-400 accent for loading state

// Logo layers (inside ??? outside)

// ?????? State for smooth progress interpolation ?????????????????????????????????????????????????????????????????????
static mut LAST_PCT: u32 = 0;

// ?????? Primitive drawing ???????????????????????????????????????????????????????????????????????????????????????????????????????????????????????????????????????
//
// The GOP framebuffer is typically mapped as WC (write-combining)
// by UEFI. WC stores are batched into the WC buffer and NOT
// guaranteed to reach VRAM until a full memory barrier flushes
// the buffer. `sfence` only orders `movnti` non-temporal stores;
// for normal WC writes, `mfence` is required. Without `mfence`,
// the display hardware sees the old contents (black) for an
// unpredictable amount of time, and the screen appears blank.

#[inline]
fn wc_flush() {
    // `mfence` is the correct barrier for WC memory:
    // it serializes all load/store instructions AND drains
    // the WC buffer before any subsequent loads or stores.
    unsafe { core::arch::asm!("mfence", options(nostack, preserves_flags)); }
}

fn put_pix(x: u32, y: u32, color: u32) {
    let fb = unsafe { crate::info::FB_ADDR as *mut u32 };
    let st  = unsafe { crate::info::FB_STRIDE as usize };
    let h   = unsafe { crate::info::FB_HEIGHT };
    if y < h && (x as usize) < st {
        unsafe {
            fb.add((y as usize) * st + (x as usize)).write_volatile(color);
        }
    }
}

fn fill_rect(x: u32, y: u32, w: u32, h: u32, color: u32) {
    let fb = unsafe { crate::info::FB_ADDR as *mut u32 };
    let st = unsafe { crate::info::FB_STRIDE as usize };
    let H  = unsafe { crate::info::FB_HEIGHT };
    if fb.is_null() { return; }
    let mut any = false;
    for dy in 0..h {
        let py = y + dy;
        if py >= H { break; }
        for dx in 0..w {
            let px = x + dx;
            if (px as usize) >= st { break; }
            unsafe { fb.add((py as usize) * st + (px as usize)).write_volatile(color); }
            any = true;
        }
    }
    if any { wc_flush(); }
}

fn draw_rect_outline(x: u32, y: u32, w: u32, h: u32, color: u32) {
    if w == 0 || h == 0 { return; }
    for dx in 0..w {
        put_pix(x + dx, y, color);
        put_pix(x + dx, y + h - 1, color);
    }
    for dy in 0..h {
        put_pix(x, y + dy, color);
        put_pix(x + w - 1, y + dy, color);
    }
    wc_flush();
}

/// TSC-based busy-wait. Reads TSC directly.
#[inline]
fn tsc_read() -> u64 {
    let lo: u32;
    let hi: u32;
    unsafe { core::arch::asm!("rdtsc", out("eax") lo, out("edx") hi); }
    ((hi as u64) << 32) | lo as u64
}

#[inline]
fn tsc_wait(cycles: u64) {
    let start = tsc_read();
    while tsc_read() - start < cycles {
        core::hint::spin_loop();
    }
}

// ?????? Color blending ????????????????????????????????????????????????????????????????????????????????????????????????????????????????????????????????????????????????

/// Blend a foreground color over BG at a given alpha (0..255).
fn blend(fg: u32, alpha: u32) -> u32 {
    let a = alpha.min(255);
    let inv = 255 - a;
    let fg_r = (fg >> 16) & 0xFF;
    let fg_g = (fg >> 8) & 0xFF;
    let fg_b = fg & 0xFF;
    let bg_r = (BG >> 16) & 0xFF;
    let bg_g = (BG >> 8) & 0xFF;
    let bg_b = BG & 0xFF;
    let r = (fg_r * a + bg_r * inv) / 255;
    let g = (fg_g * a + bg_g * inv) / 255;
    let b = (fg_b * a + bg_b * inv) / 255;
    0xFF000000 | (r << 16) | (g << 8) | b
}

/// Create a gradient color along the progress bar (cyan ??? indigo).
fn bar_gradient(x_off: u32, total_w: u32) -> u32 {
    if total_w == 0 { return ACCENT; }
    let t = (x_off * 255 / total_w).min(255);
    let inv = 255 - t;
    // ACCENT=0xFF00E5FF ??? ACCENT2=0xFF818CF8
    let r = (0x00 * inv + 0x81 * t) / 255;
    let g = (0xE5 * inv + 0x8C * t) / 255;
    let b = (0xFF * inv + 0xF8 * t) / 255;
    0xFF000000 | (r << 16) | (g << 8) | b
}

// ?????? Animated Logo (smooth radius sweep) ?????????????????????????????????????????????????????????????????????????????????

// ?????? Text drawing ??????????????????????????????????????????????????????????????????????????????????????????????????????????????????????????????????????????????????????

fn draw_char(x: u32, y: u32, c: u8, color: u32) {
    let idx = match glyph_index(c) { Some(i) => i, None => return };
    let glyph = &FONT16[idx];

    // Is the glyph pixel at (col,row) set? Out-of-bounds counts as empty.
    let lit = |col: i32, row: i32| -> bool {
        if col < 0 || col >= FONT_W as i32 || row < 0 || row >= FONT_H as i32 {
            return false;
        }
        glyph[row as usize] & (0x80u8 >> col) != 0
    };

    // NITIDO Y SIMPLE (pedido del usuario, monitor 74 Hz): solo el glifo
    // exacto, a color pleno. Antes habia un pase de "anti-alias" que rellenaba
    // las esquinas concavas con un tono tenue (blend 110) -- eso REDONDEA pero
    // DIFUMINA el texto. Sin ese pase, cada pixel es limpio: letras crujientes.
    for row in 0..FONT_H as i32 {
        for col in 0..FONT_W as i32 {
            if lit(col, row) {
                put_pix(x + col as u32, y + row as u32, color);
            }
        }
    }
}

fn draw_str(x: u32, y: u32, s: &str, color: u32) {
    let mut cx = x;
    for b in s.bytes() {
        draw_char(cx, y, b, color);
        cx += CHAR_W as u32;
    }
    // `draw_char` pinta con `put_pix`, que NO drena el buffer WC. Sin este
    // flush, las letras llegan a VRAM tarde/parciales y dejan estela
    // fantasma (el "ghosting" del log rodante y del prompt). Un solo flush
    // por linea -- barato -- mata el efecto en todos los que dibujan texto.
    wc_flush();
}

fn text_width(s: &str) -> u32 {
    s.len() as u32 * CHAR_W as u32
}

// == Boot cinematic: escenas escaladas con transiciones ===================
//
// La entrada de BMO-X deja de ser un volcado de texto: una secuencia de
// escenas centradas (logo -> preparando -> RING 0 -> RING 3) con fundido de
// entrada y una linea de acento que barre, al estilo de un arranque de SO
// moderno. Luego aterriza en el dashboard donde el trabajo real fluye.

/// Espera de `ms` milisegundos reales (usa la frecuencia TSC ya calibrada;
/// si aun no existe, aproxima a ~3 GHz).
/// Milisegundos desde `origen`, contados con el TSC.
///
/// El bucle de la animacion se guia por ESTO y no por contar fotogramas, y es la
/// diferencia entre una animacion y una secuencia de dibujos.
///
/// Un bucle que avanza un paso fijo por vuelta dura lo que tarde en pintar: en
/// un panel de 1080p la ciudad son ~8 MB por fotograma a memoria
/// write-combining, o sea decenas de milisegundos que **no son los mismos** en
/// 720p que en 4K. Preguntandole al reloj, la animacion dura lo que dice durar y
/// lo unico que cambia con el panel es cuantos fotogramas caben dentro.
fn ms_desde(origen: u64) -> u32 {
    let f = crate::ring0::task::scheduler::tsc_freq();
    if f == 0 {
        return 0;
    }
    (tsc_read().wrapping_sub(origen) / (f / 1000)) as u32
}

fn hold_ms(ms: u64) {
    let f = crate::ring0::task::scheduler::tsc_freq();
    let cycles = if f == 0 { ms * 3_000_000 } else { ms * (f / 1000) };
    let start = tsc_read();
    while tsc_read().wrapping_sub(start) < cycles {
        core::hint::spin_loop();
    }
}

fn text_width_scaled(s: &str, scale: u32) -> u32 {
    s.len() as u32 * CHAR_W as u32 * scale
}

/// Un glifo dibujado a `scale`x (cada pixel = un bloque scalexscale). Sin AA:
/// a escala >=3 los bloques ya leen limpios y con peso.
fn draw_char_scaled(x: u32, y: u32, c: u8, color: u32, scale: u32) {
    let idx = match glyph_index(c) { Some(i) => i, None => return };
    let glyph = &FONT16[idx];
    for row in 0..FONT_H {
        let bits = glyph[row];
        for col in 0..FONT_W {
            if bits & (0x80 >> col) != 0 {
                fill_rect(x + col as u32 * scale, y + row as u32 * scale, scale, scale, color);
            }
        }
    }
}

fn draw_str_scaled(x: u32, y: u32, s: &str, color: u32, scale: u32) {
    let mut cx = x;
    for b in s.bytes() {
        draw_char_scaled(cx, y, b, color, scale);
        cx += CHAR_W as u32 * scale;
    }
}

/// Reproduce la secuencia de arranque completa (4 escenas). Llamar una vez,
/// con framebuffer disponible, antes de montar el dashboard.
/// * Dibuja EL GATO desde sus dos mascaras de 1 bit. Ver `ring0::core::gato`.
///
/// El fondo no se pinta: la mascara no lo lleva, porque el fondo del splash ya
/// es negro. Son 1.346 pixeles de trazo y 276 de ojos de los 27.360 del
/// rectangulo -- dibujarlo cuesta menos que rellenarlo.
///
/// La escala multiplica en enteros a proposito: interpolar un dibujo de lineas
/// de un pixel de grosor lo convierte en una mancha gris.
fn draw_gato(x0: u32, y0: u32, escala: u32) {
    // Sobre el splash el fondo es negro liso, asi que el halo se mezcla contra
    // negro y sale como un neon en una habitacion a oscuras.
    draw_gato_encendido(x0, y0, escala, 255, 255, 0, |_| BG);
}

/// **El gato ENCENDIENDOSE**, que es lo que pidio el dueno: *"el gato en neon
/// que se prende al arrancar"*.
///
/// `trazo` y `ojos` van de 0 a 255; `apagado` mezcla el resultado hacia negro
/// para el fundido final.
///
/// # Por que se enciende con COLOR y no con transparencia
///
/// Lo natural seria pintar el gato con alfa creciente sobre la ciudad. Y no se
/// puede: mezclar con lo de debajo obliga a **leer el framebuffer**, que es
/// memoria write-combining y va lentisimo -- la misma trampa que ya costo cara
/// en el blit de DOOM.
///
/// Asi que no se mezcla con el fondo: se mezcla el **color del trazo**, de un
/// gris muy oscuro a blanco. El pixel siempre es opaco y siempre esta ahi; lo
/// que cambia es su brillo. Y ademas queda mejor de lo que quedaria un fundido:
/// el gato empieza como una silueta apagada en la ciudad y **se enciende**, en
/// vez de materializarse de la nada.
///
/// [!] Los ojos van por su cuenta y **suben despues**. Un gato que abre los ojos
/// a la vez que aparece no se prende: ya estaba encendido.
///
/// # ** Y AHORA DERRAMA LUZ, que es lo que faltaba para que fuera un neon
///
/// El video del 2026-08-15 lo enseno: un trazo blanco de un pixel sobre un cielo
/// violeta claro **no se despega de la escena**. Lo que hace que algo se lea como
/// tubo de gas no es que brille, es que **enciende lo que tiene alrededor**.
///
/// El halo sale de `gato::neon`, que mide la distancia de cada pixel al trazo.
/// Los tres conjuntos --nucleo, halo cercano, halo lejano-- son disjuntos, asi
/// que esto sigue siendo **un caso por pixel** y no se pinta nada dos veces.
///
/// `fondo(y)` dice de que color esta la pantalla en esa fila. El halo se mezcla
/// CONTRA ese color en vez de ser un tono plano, y por eso se funde con el cielo
/// en lugar de recortarse encima como una calcomania. Es la unica forma de
/// mezclar aqui: **leer el framebuffer esta prohibido**, asi que el fondo se
/// pregunta a quien lo pinto.
fn draw_gato_encendido(
    x0: u32,
    y0: u32,
    escala: u32,
    trazo: u32,
    ojos: u32,
    apagado: u32,
    fondo: impl Fn(u32) -> u32,
) {
    use bmo_ciudad::paleta::{mezcla, NEGRO};
    // El trazo apagado no es negro: es el gris al que quedaria una silueta con
    // la ciudad detras. Negro del todo lo haria desaparecer sobre el cielo.
    const TRAZO_APAGADO: u32 = 0xFF1A1730;
    const R: u32 = gato::neon::RADIO as u32;
    let c_trazo = mezcla(mezcla(TRAZO_APAGADO, WHITE, trazo, 255), NEGRO, apagado, 255);
    let c_ojos = mezcla(mezcla(TRAZO_APAGADO, ACCENT, ojos, 255), NEGRO, apagado, 255);
    for fy in 0..gato::ALTO {
        let y = y0 + fy * escala;
        // El fondo se pregunta una vez por FILA, no por pixel: es el mismo para
        // los 152 de la fila y preguntarlo 152 veces seria pagar 27.000
        // divisiones por fotograma para obtener el mismo numero.
        //
        // Y los colores del derrame tambien se calculan una vez por fila: son
        // cuatro mezclas contra las 152x4 que saldrian de hacerlo por pixel.
        let bg = fondo(y);
        let mut halo = [0u32; R as usize];
        for (n, c) in halo.iter_mut().enumerate() {
            // El derrame entra con el TRAZO, no con los ojos: es el tubo el que
            // derrama. Caida cuadratica con la distancia.
            let queda = R - n as u32;
            let f = HALO_MAX * queda * queda / (R * R) * trazo / 255;
            *c = mezcla(mezcla(bg, ACCENT, f, 255), NEGRO, apagado, 255);
        }
        for fx in 0..gato::ANCHO {
            let i = (fy * gato::ANCHO + fx) as usize;
            // Los ojos ganan al trazo: son el unico sitio con color propio.
            let d = gato::neon::distancia(i);
            let color = if d == 0 {
                if gato::bit_ojos(i) { c_ojos } else { c_trazo }
            } else if d <= gato::neon::RADIO {
                halo[d as usize - 1]
            } else {
                continue;
            };
            fill_rect(x0 + fx * escala, y, escala, escala, color);
        }
    }
}

/// **El kanji del logo** -- el que significa "gato", y por eso esta en la marca.
///
/// Una sola mascara porque en el logo es de un solo color -- medido al
/// generarla: 1.440 pixeles, todos cian, ni uno blanco. 666 bytes.
///
/// Se DIBUJA y no se escribe, igual que el triangulo de aviso: la fuente del
/// kernel es ASCII de 16 px, y meter un glifo CJK seria arrastrar una tabla de
/// simbolos entera para un caracter. Y dibujarlo a mano tampoco valia: son once
/// trazos, y un kanji torcido en la pantalla de arranque es peor que no ponerlo.
/// Sale del PNG con el mismo guion que saco al gato.
/// ** Y DERRAMA IGUAL QUE EL GATO. El kanji y el gato son **dos piezas del mismo
/// letrero**: uno con halo y el otro plano se leen como dos dibujos pegados en
/// vez de como una marca. Se vio en la primera imagen de `bmo-vista-ciudad`, que
/// es justo el fallo que antes habria hecho falta reiniciar para encontrar.
fn draw_kanji(
    x0: u32,
    y0: u32,
    escala: u32,
    color: u32,
    alfa: u32,
    apagado: u32,
    fondo: impl Fn(u32) -> u32,
) {
    use bmo_ciudad::paleta::{mezcla, NEGRO};
    const R: u32 = gato::neon::RADIO as u32;
    for fy in 0..gato::KANJI_ALTO {
        let y = y0 + fy * escala;
        let bg = fondo(y);
        let mut halo = [0u32; R as usize];
        for (n, c) in halo.iter_mut().enumerate() {
            // El derrame sube CON el trazo. Sin este `alfa` el kanji llegaba
            // con el halo a plena potencia mientras su propio trazo aun estaba
            // a medias: un contorno tenue dentro de un resplandor entero, que se
            // ve como si el halo fuera otra cosa. Lo destapo el previsualizador
            // en el fotograma del ms 900.
            let queda = R - n as u32;
            let f = HALO_MAX * queda * queda / (R * R) * alfa / 255;
            *c = mezcla(mezcla(bg, ACCENT, f, 255), NEGRO, apagado, 255);
        }
        for fx in 0..gato::KANJI_ANCHO {
            let i = (fy * gato::KANJI_ANCHO + fx) as usize;
            let d = gato::neon::distancia_kanji(i);
            let c = if d == 0 {
                color
            } else if d <= gato::neon::RADIO {
                halo[d as usize - 1]
            } else {
                continue;
            };
            fill_rect(x0 + fx * escala, y, escala, escala, c);
        }
    }
}

/// ** LA INTRO DEL ARRANQUE -- **el logo, y nada mas**.
///
/// === Que habia aqui, y por que se fue ===
///
/// Cuatro carteles de texto a pantalla completa:
///
/// ```text
///   scene("BMO-X", ...)       hold_ms(700)
///   scene("Preparando", ...)  hold_ms(350)
///   scene("RING 0", ...)      hold_ms(550)
///   scene("RING 3", ...)      hold_ms(550)
/// ```
///
/// **2.150 ms de esperas explicitas**, y `scene` trae dentro otros ~405 ms de
/// fundidos cada una: en total **casi cuatro segundos de carteleria antes de que
/// el kernel empiece a hacer nada**. Y encima decia "RING 3 : userspace listo"
/// cuando el userspace no habia arrancado todavia -- un cartel que anuncia un
/// estado que aun no existe.
///
/// El dueno lo dijo claro: *"eso ya se ve un poco feo"*. Tenia razon dos veces,
/// porque ademas de feo era lento.
///
/// === Lo que hay ahora ===
///
/// Una pantalla: el gato, `BMO-X`, y `BMO METAKERNEL` debajo. El fundido es
/// **el de los ojos**, que son 276 pixeles -- no un barrido de pantalla completa.
///
/// Coste total: unos 240 ms contra 3.700. Y el logo se queda puesto hasta que
/// `phase1_ui` aterriza en el panel, asi que **es lo que se ve mientras carga**,
/// que es exactamente lo que se pedia.
///
/// === Y no se anuncia lo que no ha pasado ===
///
/// Los estados --RING 0 despierto, RING 3 arrancado-- ya los cuenta el log del
/// panel **cuando ocurren de verdad**, con su marca de tiempo. Un cartel que los
/// promete antes es una mentira con animacion.
/// **Las cuatro esquinas del marco.** Cuatro angulos, nada mas.
///
/// Es el vocabulario visual que el dueno pidio --el de una interfaz de sala de
/// operaciones-- y es el mas barato que existe: **ocho rectangulos**. Un marco
/// entero seria una linea de 1920 px por lado que compite con el contenido; una
/// esquina insinua el marco y deja el centro limpio.
///
/// Y hace un trabajo real ademas del de adorno: dice **donde acaba la pantalla**.
/// En un monitor sin bordes visibles, con el fondo negro del logo y la sala a
/// oscuras, el panel y la pared son el mismo color.
fn marco_esquinas(w: u32, h: u32, color: u32) {
    // Proporcionales a la pantalla, no fijos: en 4K un angulo de 24 px no se ve.
    let largo = (w / 26).clamp(24, 90);
    let grosor = if h >= 900 { 2 } else { 1 };
    let m = (w / 60).clamp(12, 48); // margen desde el borde
    for &(x, y, hx, hy) in &[
        (m, m, 1i32, 1i32),                                   // arriba izquierda
        (w.saturating_sub(m), m, -1, 1),                      // arriba derecha
        (m, h.saturating_sub(m), 1, -1),                      // abajo izquierda
        (w.saturating_sub(m), h.saturating_sub(m), -1, -1),   // abajo derecha
    ] {
        // El brazo horizontal y el vertical de cada angulo. `hx`/`hy` dicen
        // hacia donde crece cada uno, asi que las cuatro esquinas salen del
        // mismo par de lineas en vez de cuatro casos escritos a mano.
        let x0 = if hx > 0 { x } else { x.saturating_sub(largo) };
        let y0 = if hy > 0 { y } else { y.saturating_sub(grosor) };
        fill_rect(x0, y0, largo, grosor, color);
        let x1 = if hx > 0 { x } else { x.saturating_sub(grosor) };
        let y1 = if hy > 0 { y } else { y.saturating_sub(largo) };
        fill_rect(x1, y1, grosor, largo, color);
    }
}

/// **El triangulo de aviso**, el que va detras de la X en el logo.
///
/// Se dibuja y no se escribe porque **la fuente es ASCII de 16 px y no tiene ese
/// glifo** -- y meter uno seria abrir la puerta a que la pantalla de arranque
/// dependa de una tabla de simbolos que hoy no existe. Son tres lados y una
/// barra: geometria exacta, sin inventar nada del logo.
///
/// [!] El contorno se calcula por fila (`media = i * lado / (2 * alto)`), que es
/// lo unico que se puede hacer sin un trazador de lineas -- y `splash.rs` no
/// tiene uno porque hasta hoy no lo habia necesitado nadie.
fn triangulo_aviso(x: u32, y: u32, lado: u32, color: u32) {
    let alto = lado * 7 / 8;
    if alto == 0 {
        return;
    }
    let cx = x + lado / 2;
    let grosor = (lado / 12).max(1);
    let mut i = 0;
    while i < alto {
        let media = i * lado / (2 * alto);
        fill_rect(cx.saturating_sub(media), y + i, grosor, 1, color);
        fill_rect(cx + media, y + i, grosor, 1, color);
        i += 1;
    }
    fill_rect(x, y + alto, lado + grosor, grosor, color);
    // La admiracion: barra y punto, con el hueco que la separa.
    let bh = alto / 2;
    fill_rect(cx, y + alto / 4, grosor, bh, color);
    fill_rect(cx, y + alto / 4 + bh + grosor, grosor, grosor, color);
}

/// Cuanto tine el aura el cielo en su centro, de 0 a 255.
///
/// [!] **Estaba en 150 y el previsualizador lo tumbo.** A esa fuerza el aura
/// salia como un globo turquesa detras del gato: una forma que competia con el
/// gato en vez de sostenerlo.
///
/// El reparto correcto es otro y se ve en cuanto se puede mirar la imagen: **el
/// resplandor que se nota es el que sigue la silueta** (`gato::neon`, que es lo
/// que hace un tubo de neon de verdad), y el aura es solo un lavado que levanta
/// el cielo un punto para que el conjunto no este pegado sobre el degradado.
/// Cincuenta se ve; ciento cincuenta se mira.
const FUERZA_AURA: u32 = 50;

/// Cuanto tine el derrame el color del fondo en su primer nivel, de 0 a 255. De
/// ahi cae con el cuadrado de la distancia: un derrame que no cae se ve como un
/// borde grueso, que es lo contrario de un resplandor.
///
/// Lo comparten el gato y el kanji a proposito: son el mismo letrero y tienen
/// que brillar igual.
const HALO_MAX: u32 = 150;

/// Cuando empezo la intro, en ciclos de TSC. `0` = no ha empezado.
static mut INTRO_T0: u64 = 0;

/// **Esta la intro en pantalla?**
///
/// Existe para una sola cosa: que el log del arranque **no pinte encima**. Ver
/// [`splash_dashboard_log_color`].
pub fn intro_en_curso() -> bool {
    unsafe { INTRO_T0 != 0 }
}
/// La ciudad, compuesta una vez. Vive aqui y no en la pila porque ahora se
/// dibuja **desde muchos sitios** del arranque, no de una sentada.
static mut INTRO_CIUDAD: Option<bmo_ciudad::Ciudad> = None;

/// **Empieza la intro y NO se queda esperando.**
///
/// Ver [`intro_paso`], que es donde esta explicado el cambio entero.
pub fn intro_empieza() {
    let w = unsafe { crate::info::FB_WIDTH };
    let h = unsafe { crate::info::FB_HEIGHT };
    if w == 0 || h == 0 {
        return;
    }
    // El halo del gato se calcula UNA vez: es una dilatacion de la mascara, y la
    // mascara no cambia nunca. Aqui y no en el primer fotograma para que ese
    // coste no caiga dentro de la animacion. Ver `gato::neon`.
    gato::neon::preparar();
    unsafe {
        INTRO_T0 = tsc_read();
        INTRO_CIUDAD = Some(bmo_ciudad::Ciudad::nueva(
            w as i32,
            h as i32,
            ((w as u64) << 20) | h as u64,
        ));
    }
    intro_paso(0);
}

/// **Un fotograma de la intro, con el progreso REAL del arranque.**
///
/// # El truco de Santa Monica, y por que el modelo de antes estaba mal
///
/// Lo dijo el dueno viendo el log de arranque pasar: *"BMO-X esta preparando
/// todo eso, los datos se ejecutan en tiempo real... tiene que esconder con
/// truco inspirado como hicieron Santa Monica en God of War"*.
///
/// God of War 2018 no tiene pantallas de carga: el trabajo se hace **debajo** de
/// una camara que no corta. La carga no se elimina -- se tapa con algo que el
/// jugador queria ver de todas formas.
///
/// Aqui era al reves, y el comentario de `phase.rs` lo decia con todas las
/// letras: *"la animacion juega, luego apareces en el escritorio"*, el modelo de
/// Windows. O sea **2.400 ms de animacion MAS el tiempo real de arrancar**. Y el
/// coste ya estaba medido y confesado en otro sitio: `boot_timeline` tiene una
/// fila propia para el `GATO_MS` porque, sin ella, ese segundo y medio se
/// achacaba a la enumeracion del bus PCI.
///
/// ** Ahora la intro no espera a nada: se llama a esto entre paso y paso del
/// arranque de verdad --USB, xHCI, AHCI, el censo de PCI-- y cada llamada pinta
/// UN fotograma con el reloj que haya. La animacion dura **lo que dure el
/// trabajo**, y no cuesta ni un milisegundo de mas.
///
/// # Y `pct` no es una barra: es la ciudad
///
/// El progreso enciende las torres. Con lo cual se cierra la idea que el dueno
/// tuvo dos dias antes --*"en el fondo se ve el sistema de ciudad con TODO"*--:
/// **la ciudad encendiendose ES el arranque ocurriendo**, no una animacion que
/// finge acompanarlo. Un subsistema que tarda deja su tramo de ciudad a oscuras
/// mas tiempo, y eso es informacion de verdad.
pub fn intro_paso(pct: u32) {
    let w = unsafe { crate::info::FB_WIDTH };
    let h = unsafe { crate::info::FB_HEIGHT };
    if w == 0 || h == 0 {
        return;
    }
    let t0 = unsafe { INTRO_T0 };
    if t0 == 0 {
        return;
    }
    // El tiempo manda en la camara y en el gato; el PROGRESO manda en la ciudad.
    // Son dos relojes distintos a proposito: uno cuenta lo que se ve, el otro
    // cuenta lo que pasa.
    let ms = ms_desde(t0).min(bmo_ciudad::DURACION_MS);
    let mut f = bmo_ciudad::fotograma(ms);
    f.ciudad_pct = pct.min(100);
    // Los dos ultimos actos los dispara `intro_cierra`, no el reloj: mientras
    // haya trabajo, el gato se queda mirando.
    f.destello = 0;
    f.negro = 0;
    pintar_escena(w, h, &f);
}

/// **Cierra la intro: los ojos toman el control y todo se va a negro.**
///
/// Esto SI espera, y es el unico sitio donde se puede: el trabajo ya termino, no
/// hay nada debajo que tapar. Son los ultimos 500 ms del guion.
pub fn intro_cierra() {
    let w = unsafe { crate::info::FB_WIDTH };
    let h = unsafe { crate::info::FB_HEIGHT };
    if w == 0 || h == 0 || unsafe { INTRO_T0 } == 0 {
        return;
    }
    let t0 = tsc_read();
    let dur = bmo_ciudad::DURACION_MS - bmo_ciudad::acto::FIN_GATO;
    loop {
        let d = ms_desde(t0);
        if d >= dur {
            break;
        }
        let f = bmo_ciudad::fotograma(bmo_ciudad::acto::FIN_GATO + d);
        pintar_escena(w, h, &f);
    }
    fill_rect(0, 0, w, h, BG);
    wc_flush();
    unsafe {
        INTRO_T0 = 0;
        INTRO_CIUDAD = None;
    }
}

/// **Pinta UN fotograma de la escena entera**: ciudad, gato, kanji y destello.
///
/// El encuadre no se calcula aqui: se le pide a `bmo_ciudad::encuadre`, que es
/// aritmetica pura y **se prueba sin encender la maquina**. Mientras estuvo
/// dentro de esta funcion solo se podia juzgar reiniciando el Ryzen, y asi se
/// colo el fallo del video del 08-15: el titulo escrito sobre los tejados.
///
/// Lo que queda aqui es pintar. Esta funcion sigue **sin estado**: se puede
/// llamar desde cualquier punto del arranque sin que nadie haya guardado nada
/// antes.
fn pintar_escena(w: u32, h: u32, f: &bmo_ciudad::Fotograma) {
    use bmo_ciudad::paleta::{mezcla, NEGRO};

    // La escala sale de la ALTURA de la pantalla, no de un numero fijo: en 1080
    // sale a x2 y en 720 a x1, y en las dos ocupa la misma fraccion.
    let escala = if h >= 900 { 2 } else { 1 };
    let escala_t = if h >= 900 { 5 } else { 4 };
    let gw = gato::ANCHO * escala;
    let gh = gato::ALTO * escala;
    let kw = gato::KANJI_ANCHO * escala;
    let kh = gato::KANJI_ALTO * escala;
    let tw = text_width_scaled("BMO-X", escala_t);

    // El techo y el canto del marco se le preguntan a la ciudad en vez de copiar
    // aqui unos porcentajes. Si manana alguien sube las torres o ensancha el
    // marco, el logo se aparta solo.
    let (techo, marco_interior) = unsafe {
        let ciudad = &*core::ptr::addr_of!(INTRO_CIUDAD);
        match ciudad.as_ref() {
            Some(c) => (c.techo().max(0) as u32, c.marco().interior().max(0) as u32),
            None => (h, 0),
        }
    };
    let medidas = bmo_ciudad::Medidas {
        pantalla_w: w,
        pantalla_h: h,
        techo,
        marco_interior,
        gato_w: gw,
        gato_h: gh,
        kanji_w: kw,
        kanji_h: kh,
        hueco_kanji: 22 * escala,
        titulo_w: tw,
        titulo_h: FONT_H as u32 * escala_t,
        linea_h: FONT_H as u32,
    };
    let enc = bmo_ciudad::componer(&medidas);
    let (gx, gy, ky) = (enc.gato_x, enc.gato_y, enc.kanji_y);
    let th = medidas.titulo_h;

    // -- LA CIUDAD, detras de todo.
    unsafe {
        let ciudad = &mut *core::ptr::addr_of_mut!(INTRO_CIUDAD);
        if let Some(c) = ciudad.as_mut() {
            c.encender(f.ciudad_pct);
            let cam = bmo_ciudad::Camara::nueva(f.avance);
            c.dibujar(cam, |x, y, cw, ch, color| {
                if cw > 0 && ch > 0 && x >= 0 && y >= 0 && (x as u32) < w && (y as u32) < h {
                    let c = mezcla(color, NEGRO, f.negro, 255);
                    fill_rect(x as u32, y as u32, cw as u32, ch as u32, c);
                }
            });
        }
    }

    // -- ** EL AURA: el cielo ENCENDIDO detras del logo.
    //
    // La otra mitad de "las capas estan mezcladas". La escalera de valores de la
    // paleta separo el cielo de las torres, pero el logo no tenia separacion de
    // NADA: estaba estampado sobre el degradado, y cuando el cielo llegaba a su
    // parte clara el gato casi desaparecia.
    //
    // Un neon de verdad enciende el aire que tiene detras. Eso es esto, y va
    // ENTRE la ciudad y el gato porque es cielo respondiendo a una luz -- no es
    // parte del gato. Ver `bmo_ciudad::halo`.
    //
    // [!] Es OPACA (no se puede leer el framebuffer para mezclar), asi que tiene
    // que caber en el cielo despejado o borraria las torres. De ahi el recorte
    // contra `techo`: la caja del aura nunca baja de ahi.
    let fuerza_aura = FUERZA_AURA * f.gato_alfa / 255;
    if f.gato_alfa > 0 {
        unsafe {
            let ciudad = &*core::ptr::addr_of!(INTRO_CIUDAD);
            if let Some(c) = ciudad.as_ref() {
                bmo_ciudad::aura(
                    |y| mezcla(c.color_cielo(y), NEGRO, f.negro, 255),
                    enc.aura_cx,
                    enc.aura_cy,
                    enc.aura_rx,
                    enc.aura_ry,
                    ACCENT,
                    fuerza_aura,
                    |x, y, aw, ah, color| {
                        if aw > 0 && ah > 0 && x >= 0 && y >= 0 && (x as u32) < w && (y as u32) < h {
                            fill_rect(x as u32, y as u32, aw as u32, ah as u32, color);
                        }
                    },
                );
            }
        }
    }

    // -- EL GATO, encendiendose por encima Y CON SU PROPIO RITMO.
    //
    // ** El flote es lo que lo separa del fondo de verdad. Dos planos quietos
    // uno sobre otro se leen como un collage por mucho que tengan brillos
    // distintos; en cuanto uno se mueve **a su ritmo**, el ojo los separa solo.
    // Es la misma pista que el paralaje, aplicada al primer plano -- y por eso
    // el periodo (2,6 s) no es multiplo de nada de la camara: si coincidieran,
    // el gato y la ciudad se moverian a compas y volverian a parecer lo mismo.
    //
    // El kanji flota con el, y el titulo NO: el titulo es tipografia, y una
    // tipografia que se mueve se lee como un fallo de sincronia, no como vida.
    if f.gato_alfa > 0 {
        let gy = (gy as i32 + f.gato_flote).max(0) as u32;
        let ky = (ky as i32 + f.gato_flote).max(0) as u32;
        // El latido del neon va SOBRE el brillo de los ojos, con tope: un neon
        // perfectamente estable no parece neon, parece un LED.
        let ojos = (f.ojos_alfa + f.ojos_pulso).min(255);
        // El fondo contra el que se mezcla el halo es el AURA, no el cielo
        // pelado: el gato se dibuja encima de ella. Se reconstruye con la misma
        // aritmetica en vez de leerla de la pantalla -- leer el framebuffer
        // esta prohibido aqui, y el numero se sabe.
        let bajo_el_logo = |y: u32| {
            let cielo = unsafe {
                let ciudad = &*core::ptr::addr_of!(INTRO_CIUDAD);
                ciudad.as_ref().map_or(NEGRO, |c| c.color_cielo(y as i32))
            };
            // Cuanto tine el aura a esta altura: cae con el cuadrado de la
            // distancia al centro, igual que en `bmo_ciudad::halo`.
            let ry = enc.aura_ry as u32;
            let dy = (y as i32 - enc.aura_cy).unsigned_abs().min(ry);
            let cerca = ry - dy;
            let f_aura = fuerza_aura * cerca * cerca / (ry * ry).max(1);
            mezcla(mezcla(cielo, ACCENT, f_aura, 255), NEGRO, f.negro, 255)
        };
        draw_gato_encendido(gx, gy, escala, f.gato_alfa, ojos, f.negro, bajo_el_logo);
        // El kanji flota con el gato, asi que su `x` sale del encuadre y su `y`
        // lleva el mismo desplazamiento que el trazo. Y derrama igual que el
        // gato: son el mismo letrero.
        draw_kanji(
            enc.kanji_x,
            ky,
            escala,
            mezcla(mezcla(0xFF1A1730, ACCENT, ojos, 255), NEGRO, f.negro, 255),
            ojos,
            f.negro,
            bajo_el_logo,
        );
        // El titulo entra con el trazo: es parte del gato, no de la ciudad. Y NO
        // flota: una tipografia que se mueve se lee como un fallo de sincronia.
        let ty = enc.titulo_y;
        let tx = enc.titulo_x;
        let c_txt = mezcla(mezcla(NEGRO, WHITE, f.gato_alfa, 255), NEGRO, f.negro, 255);
        let c_ac = mezcla(mezcla(NEGRO, ACCENT, f.gato_alfa, 255), NEGRO, f.negro, 255);
        draw_str_scaled(tx, ty, "BMO-X", c_txt, escala_t);
        triangulo_aviso(tx + tw + escala_t * 2, ty + th / 3, th / 2, c_ac);
        fill_rect(tx, ty + th + 10, tw, 3, c_ac);
        let sub = "BMO METAKERNEL";
        let sw = text_width(sub);
        let sy = ty + th + 10 + 3 + 14;
        let sx = w.saturating_sub(sw) / 2;
        draw_str(sx, sy, sub, c_ac);
        let regla = (tw / 3).max(20);
        let ry = sy + FONT_H as u32 / 2;
        let c_dim = mezcla(mezcla(NEGRO, DIM, f.gato_alfa, 255), NEGRO, f.negro, 255);
        fill_rect(sx.saturating_sub(14 + regla), ry, regla, 1, c_dim);
        fill_rect(sx + sw + 14, ry, regla, 1, c_dim);
    }

    // -- EL DESTELLO: los ojos tomando el control.
    //
    // Una caja de cian que crece desde la cara del gato hasta comerse la
    // pantalla. Se pinta ENCIMA de todo y se apaga hacia negro con el mismo
    // `f.negro` que la ciudad, asi que no se "quita": se lo traga el negro.
    if f.destello > 0 {
        let cara_x = (gx + gw / 2) as i32;
        let cara_y = (gy + gh / 3) as i32;
        let radio = (f.destello * (w.max(h)) / 255) as i32;
        let c = mezcla(mezcla(NEGRO, ACCENT, f.destello, 255), NEGRO, f.negro, 255);
        let x0 = (cara_x - radio).max(0) as u32;
        let y0 = (cara_y - radio).max(0) as u32;
        let x1 = ((cara_x + radio).max(0) as u32).min(w);
        let y1 = ((cara_y + radio).max(0) as u32).min(h);
        if x1 > x0 && y1 > y0 {
            fill_rect(x0, y0, x1 - x0, y1 - y0, c);
        }
    }

    // El marco, lo ultimo: encuadra todo lo demas.
    marco_esquinas(w, h, mezcla(ACCENT, NEGRO, f.negro, 255));
    wc_flush();
}

// ?????? Smooth progress bar ?????????????????????????????????????????????????????????????????????????????????????????????????????????????????????????????????

/// Animate the progress bar smoothly from `LAST_PCT` to `target_pct`.
/// Uses sub-percentage pixel-level interpolation for ultra-smooth fill.
fn smooth_progress(bx: u32, by: u32, bar_w: u32, bar_h: u32, target_pct: u32) {
    let start_pix = unsafe { (bar_w as u64 * LAST_PCT as u64 / 100) as u32 };
    let end_pix = (bar_w as u64 * target_pct.min(100) as u64 / 100) as u32;

    if start_pix >= end_pix {
        unsafe { LAST_PCT = target_pct.min(100); }
        return;
    }

    // Animate pixel-by-pixel for maximum smoothness
    let mut px = start_pix;
    while px < end_pix {
        // Draw the new column with gradient color
        let col_color = bar_gradient(px, bar_w);
        fill_rect(bx + px, by, 1, bar_h, col_color);
        px += 1;

        // Adaptive speed: fast start, smooth middle, slow finish
        let progress_ratio = px * 100 / bar_w;
        let delay = if progress_ratio < 30 {
            800_000u64
        } else if progress_ratio < 70 {
            1_200_000u64
        } else {
            1_800_000u64
        };
        tsc_wait(delay);
    }

    unsafe { LAST_PCT = target_pct.min(100); }
}

// ?????? Public API ????????????????????????????????????????????????????????????????????????????????????????????????????????????????????????????????????????????????????????????

pub fn splash_init() {
    let w = unsafe { crate::info::FB_WIDTH };
    let h = unsafe { crate::info::FB_HEIGHT };
    let fb_addr = unsafe { crate::info::FB_ADDR };
    let fb_stride = unsafe { crate::info::FB_STRIDE };
    let fb_fmt = unsafe { crate::info::FB_PIXEL_FORMAT };

    // Log to serial (even if the user can't see it, it's useful later)
    crate::ring0::dev::console::serial_write("[splash] fb=0x");
    crate::ring0::dev::console::serial_write_u64(fb_addr, 16);
    crate::ring0::dev::console::serial_write(" ");
    crate::ring0::dev::console::serial_write_u64_dec(w as u64);
    crate::ring0::dev::console::serial_write("x");
    crate::ring0::dev::console::serial_write_u64_dec(h as u64);
    crate::ring0::dev::console::serial_write(" stride=");
    crate::ring0::dev::console::serial_write_u64_dec(fb_stride as u64);
    crate::ring0::dev::console::serial_write(" fmt=");
    crate::ring0::dev::console::serial_write_u64_dec(fb_fmt as u64);
    crate::ring0::dev::console::serial_write("\n");

    if w == 0 || h == 0 || fb_addr == 0 {
        crate::ring0::dev::console::serial_write("[splash] FB not available\n");
        return;
    }

    // -- Try filling the whole screen using rep stosd ---------------
    //    This is the fastest, most reliable way to write a GPU
    //    framebuffer: the CPU's string-store engine does 64-byte
    //    bursts internally and handles WC buffering correctly.
    //    After the fill, we use mfence to flush the WC buffer.
    let total = (fb_stride as usize) * (h as usize);
    crate::ring0::dev::console::serial_write("[splash] filling ");
    crate::ring0::dev::console::serial_write_u64_dec(total as u64);
    crate::ring0::dev::console::serial_write(" px\n");

    unsafe {
        let di = fb_addr;
        let color: u32 = 0xFFFFFF00u32;
        core::arch::asm!(
            "cld",
            "mov rdi, {di}",
            "mov eax, {color:e}",
            "mov ecx, {count:e}",
            "rep stosd",
            "mfence",
            di = in(reg) di,
            color = in(reg) color,
            count = in(reg) total,
            options(nostack, preserves_flags),
        );
    }

    crate::ring0::dev::console::serial_write("[splash] fill done -- screen should be yellow\n");

    // Wait a moment so the user can see the fill
    tsc_wait(300_000_000); // ~100 ms @ 3.7 GHz

    // Draw centered text over the fill
    let txt = "BMO-X";
    let tx = (w as u32).saturating_sub(text_width(txt)) / 2;
    let cy = h / 2;
    draw_str(tx, cy - 10, txt, 0xFF000000u32);
    wc_flush();
    crate::ring0::dev::console::serial_write("[splash] text drawn\n");

    // Skip the animated splash for now -- the fill test is priority
}

pub fn splash_progress(pct: u32, label: &str) {
    let w = unsafe { crate::info::FB_WIDTH };
    let h = unsafe { crate::info::FB_HEIGHT };
    if w == 0 || h == 0 { return; }

    let cy = h / 2;
    let bar_w = 320u32;
    let bar_h = 6u32;
    let bx = (w as u32).saturating_sub(bar_w) / 2;
    let bar_y = cy + 50;

    // Smooth pixel-level interpolated progress bar
    smooth_progress(bx, bar_y, bar_w, bar_h, pct);

    // Update label (clear old, draw new centered)
    let label_y = bar_y + bar_h + 12;
    fill_rect(0, label_y, w, CHAR_H as u32, BG);
    let lx = (w as u32).saturating_sub(text_width(label)) / 2;
    draw_str(lx, label_y, label, ACCENT2);
}

pub fn splash_clear() {
    let w = unsafe { crate::info::FB_WIDTH };
    let h = unsafe { crate::info::FB_HEIGHT };
    if w == 0 { return; }
    fill_rect(0, 0, w, h, BG);
}

// ===================================================================
//  Persistent Dashboard
// ===================================================================
//
// Once the boot splash finishes, the kernel switches to a
// persistent dashboard on the framebuffer. This is the visual
// equivalent of the serial shell: it shows the system status,
// the latest kernel log lines, and a prompt. Anything typed on
// the serial (COM1) is echoed on the screen so the user can
// interact even without a serial terminal attached.

const DASH_HEADER_H:  u32 = 44;  // top bar height
const DASH_FOOTER_H:  u32 = 36;  // bottom prompt bar height
const DASH_LOG_TOP:   u32 = 78;  // y of first log line
const DASH_LOG_W:     u32 = 80;  // max chars per line
const DASH_ROWS_MAX:  usize = 64; // tope duro (protege los buffers de filas)

/// Filas de log que CABEN de verdad en el panel, segun el alto REAL del
/// framebuffer.
///
/// Antes esto era una constante de 14. En 1080p (CHAR_H=20) caben ~49: se
/// desperdiciaban dos tercios del panel y, peor, obligaba al log rodante y a
/// CABINA a pelearse las mismas filas 2-13 borrandose mutuamente. El reparto
/// ahora lo decide el hardware, no un numero magico: preguntale al hardware
/// los HECHOS, hardcodea solo los CONTRATOS.
pub fn dash_rows() -> usize {
    let h = unsafe { crate::info::FB_HEIGHT };
    if h == 0 { return 0; }
    let avail = h.saturating_sub(DASH_FOOTER_H + DASH_LOG_TOP + 4);
    ((avail as usize) / CHAR_H).min(DASH_ROWS_MAX)
}

// -- PALETA: neon sobre negro ------------------------------------------------
//
// El fondo baja casi a negro puro a proposito: un neon solo brilla si lo que
// tiene alrededor esta apagado. El slate azulado anterior le robaba fuerza a
// todos los acentos porque ya era luminoso de por si.
//
// La familia son tres luces frias (cian, jade, violeta) contra tres calidas
// (ambar, oro, magenta), con el rojo lacado reservado EXCLUSIVAMENTE para lo
// que va mal. Que el rojo no se use de adorno es lo que hace que, cuando
// aparece, la vista vaya sola.

const VOID:           u32 = 0xFF04060C; // fuera del panel -- negro con tinte
const PANEL:          u32 = 0xFF080B14; // fondo del area de log
const CHROME:         u32 = 0xFF10151F; // barras superior e inferior
const EDGE:           u32 = 0xFF1E2738; // bordes apagados

const NEON_CYAN:      u32 = 0xFF00F0FF;
const NEON_MAGENTA:   u32 = 0xFFFF2D9B;
const NEON_AMBER:     u32 = 0xFFF6C445; // el amarillo de firma
const NEON_GOLD:      u32 = 0xFFFFB300;
const NEON_RED:       u32 = 0xFFFF3355; // solo para faults
const NEON_GREEN:     u32 = 0xFF39FF88;
const NEON_VIOLET:    u32 = 0xFFA78BFA;
const NEON_JADE:      u32 = 0xFF2DE2C5;

const DASH_BG:        u32 = PANEL;
const DASH_BAR:       u32 = CHROME;
const DASH_ACCENT:    u32 = NEON_CYAN;
const DASH_TEXT:      u32 = 0xFFE6EDF7;
const DASH_DIM:       u32 = 0xFF55647E;

// Colores-filtro por origen de linea (pedido del usuario): quien emite se
// reconoce por color sin leer el prefijo.
const DASH_RING3:     u32 = NEON_GREEN;   // salida de Ring 3
const DASH_TELEMETRY: u32 = NEON_AMBER;   // heartbeat r3hb (tablero)
const DASH_KBD:       u32 = NEON_VIOLET;  // entrada -- teclado y raton
const DASH_FAULT:     u32 = NEON_RED;     // reporter de CPU faults
const DASH_STORAGE:   u32 = NEON_JADE;    // disco y sistema de ficheros
const DASH_LANG_C:    u32 = NEON_CYAN;    // programas C
const DASH_LANG_COB:  u32 = NEON_GOLD;    // programas COBOL
const DASH_LANG_ASM:  u32 = NEON_MAGENTA; // programas en ensamblador
const DASH_STAGE:     u32 = NEON_AMBER;   // encabezados de acto

/// Color de una linea del log segun su prefijo. Un solo punto de decision:
/// TODOS los caminos que pintan al panel (rolling log, CABINA, faults) pasan
/// por aqui.
///
/// La tabla crecio con los emisores que ya existian y salian todos en blanco:
/// los tres lenguajes tenian el mismo color que un mensaje del kernel, asi que
/// la pantalla mas impresionante del proyecto --tres programas propios
/// entrelazandose-- se leia como un parrafo plano. Ahora cada voz tiene la suya.
fn dash_line_color(msg: &str) -> u32 {
    let b = msg.as_bytes();
    // Programas de Ring 3, por lenguaje: cada uno con su luz.
    if b.starts_with(b"C> ") {
        DASH_LANG_C
    } else if b.starts_with(b"COBOL>") {
        DASH_LANG_COB
    } else if b.starts_with(b"asm>") {
        DASH_LANG_ASM
    } else if b.starts_with(b"ring3>") || b.starts_with(b"[ring3]") {
        DASH_RING3
    } else if b.starts_with(b"==") {
        // Encabezados de etapa del boot ("== RING 0 ... ==") y del shell.
        DASH_STAGE
    } else if b.starts_with(b"r3hb") {
        DASH_TELEMETRY
    } else if b.starts_with(b"kbd ") || b.starts_with(b"[usb]") || b.starts_with(b"[xhci]")
        || b.starts_with(b"[uhid]") {
        DASH_KBD
    } else if b.starts_with(b"[disk]") || b.starts_with(b"[ahci]") || b.starts_with(b"[fs]")
        || b.starts_with(b"[cabina]") {
        DASH_STORAGE
    } else if b.starts_with(b"[ring0]") || b.starts_with(b"[bex]") {
        DASH_ACCENT
    } else if b.starts_with(b"***") || b.starts_with(b"vec ") || b.starts_with(b"flt") {
        DASH_FAULT
    } else {
        DASH_TEXT
    }
}

// -- Cromo: las piezas que dan el look ---------------------------------------

/// Linea horizontal de 1 px con degradado entre dos colores.
///
/// Es el truco mas barato que existe para que una interfaz deje de parecer un
/// terminal: una sola fila de pixeles interpolada cuesta un bucle y cambia por
/// completo la sensacion de la barra que subraya.
fn hline_gradient(x: u32, y: u32, w: u32, c1: u32, c2: u32, scale: u32) {
    if w == 0 { return; }
    let (r1, g1, b1) = ((c1 >> 16) & 0xFF, (c1 >> 8) & 0xFF, c1 & 0xFF);
    let (r2, g2, b2) = ((c2 >> 16) & 0xFF, (c2 >> 8) & 0xFF, c2 & 0xFF);
    for i in 0..w {
        // Media ponderada: multiplicar ANTES de dividir. Interpolar por canal
        // con una resta encadenada se rompe en cuanto el color destino es mas
        // oscuro que el de origen, y el degradado se queda plano sin avisar.
        let r = (r1 * (w - i) + r2 * i) / w * scale / 255;
        let g = (g1 * (w - i) + g2 * i) / w * scale / 255;
        let b = (b1 * (w - i) + b2 * i) / w * scale / 255;
        put_pix(x + i, y, 0xFF00_0000 | (r << 16) | (g << 8) | b);
    }
}

/// Regla de neon: un pixel encendido y otro apagandose debajo.
///
/// Dos filas al 100 % se leen como una barra blanca --asi salia en la foto del
/// hardware, porque el brillo satura la camara y tambien el ojo--. La caida
/// abajo es lo que hace que se lea como una LUZ y no como un separador.
fn neon_rule(x: u32, y: u32, w: u32, c1: u32, c2: u32) {
    hline_gradient(x, y, w, c1, c2, 255);
    hline_gradient(x, y + 1, w, c1, c2, 90);
}

/// Esquinas en L en vez de un marco cerrado.
///
/// Es la firma visual del genero: el ojo cierra el rectangulo solo y el panel
/// respira. Un borde continuo encajona; cuatro corchetes sugieren.
fn corner_brackets(x: u32, y: u32, w: u32, h: u32, len: u32, thick: u32, color: u32) {
    if w < len * 2 || h < len * 2 { return; }
    // Superior izquierda
    fill_rect(x, y, len, thick, color);
    fill_rect(x, y, thick, len, color);
    // Superior derecha
    fill_rect(x + w - len, y, len, thick, color);
    fill_rect(x + w - thick, y, thick, len, color);
    // Inferior izquierda
    fill_rect(x, y + h - thick, len, thick, color);
    fill_rect(x, y + h - len, thick, len, color);
    // Inferior derecha
    fill_rect(x + w - len, y + h - thick, len, thick, color);
    fill_rect(x + w - thick, y + h - len, thick, len, color);
}

/// Etiqueta de seccion con su bloque de acento delante: `| TEXTO`.
///
/// El bloque es un rectangulo, no un glifo: la fuente es de 95 caracteres
/// ASCII mas 25 de Latin-1 y no tiene caracteres de dibujo. Pintar el adorno
/// en vez de escribirlo evita inventar glifos que no existen.
fn section_label(x: u32, y: u32, text: &str, accent: u32) {
    fill_rect(x, y + 2, 4, FONT_H as u32 - 4, accent);
    draw_str(x + 12, y, text, DASH_DIM);
}

/// Draw the persistent dashboard frame. Called once after the
/// splash finishes -- replaces the cleared screen with a UI that
/// stays visible for the rest of the kernel's lifetime.
pub fn splash_dashboard_init() {
    let w = unsafe { crate::info::FB_WIDTH };
    let h = unsafe { crate::info::FB_HEIGHT };
    if w == 0 || h == 0 { return; }

    // 1. El vacio. Todo lo que no es panel ni barra queda casi negro para que
    //    el neon tenga contra que brillar.
    fill_rect(0, 0, w, h, VOID);

    // 2. Barra superior: identidad del sistema.
    fill_rect(0, 0, w, DASH_HEADER_H, CHROME);
    // Marca de acento a la izquierda -- el bloque vertical que ancla el titulo.
    fill_rect(0, 0, 5, DASH_HEADER_H, NEON_MAGENTA);
    // El nombre en dos pesos: la marca en ambar, el subsistema en magenta.
    // Separarlos dice de un vistazo QUE es y DONDE esta corriendo.
    draw_str(22, 14, "BMO-X", NEON_AMBER);
    let x_after = 22 + text_width("BMO-X") + 12;
    draw_str(x_after, 14, "// RING 0", NEON_MAGENTA);
    let x_sub = x_after + text_width("// RING 0") + 16;
    draw_str(x_sub, 14, "bare metal orchestrator", DASH_DIM);
    // Subrayado de neon que recorre la barra: cian a la izquierda, magenta a
    // la derecha. Es la pieza que mas cambia la sensacion por menos pixeles.
    neon_rule(0, DASH_HEADER_H - 2, w, NEON_CYAN, NEON_MAGENTA);

    // 3. Barra inferior: el prompt.
    let fy = h - DASH_FOOTER_H;
    fill_rect(0, fy, w, DASH_FOOTER_H, CHROME);
    fill_rect(0, fy, 5, DASH_FOOTER_H, NEON_CYAN);
    neon_rule(0, fy, w, NEON_MAGENTA, NEON_CYAN);

    // 4. El panel del log: fondo propio, un punto mas claro que el vacio, para
    //    que se lea como una superficie y no como un agujero.
    let log_y = DASH_LOG_TOP;
    let log_h = h - DASH_FOOTER_H - log_y - 4;
    fill_rect(8, log_y - 6, w - 16, log_h, PANEL);
    // Bordes tenues + esquinas en L encendidas.
    draw_rect_outline(8, log_y - 6, w - 16, log_h, EDGE);
    corner_brackets(8, log_y - 6, w - 16, log_h, 22, 2, NEON_CYAN);

    // 5. Etiqueta de seccion. Va anclada al BORDE DE LA CABECERA, no restando
    //    del log: calculada hacia atras desde el log caia justo sobre la regla
    //    de neon y en el hardware el texto salia montado en la linea.
    section_label(14, DASH_HEADER_H + 8, "KERNEL LOG", NEON_CYAN);
}

/// Write a single log line into the dashboard's log area at
/// line `row` (0 = top, growing downward). Newer lines overwrite
/// older ones on the same row, so callers can manage a ring of
/// `dash_rows()` rows.
pub fn splash_dashboard_log(row: usize, msg: &str) {
    let c = dash_line_color(msg);
    splash_dashboard_log_color(row, msg, c);
}

/// Regla de separacion con etiqueta, a la altura de una fila del panel.
///
/// Es lo que separa el log rodante del cockpit de CABINA. Antes las dos zonas
/// se tocaban y la unica pista de donde acababa una era leer el contenido;
/// ahora hay una frontera que se ve sin leer. La linea se apaga hacia la
/// derecha para no competir con el texto que viene debajo.
///
/// El texto tiene que ser ASCII: la consola es Latin-1 de un byte por caracter
/// y un literal Rust con acentos viajaria en UTF-8, o sea dos glifos raros
/// donde deberia haber uno.
pub fn splash_dash_rule(row: usize, label: &str, accent: u32) {
    let w = unsafe { crate::info::FB_WIDTH };
    if w == 0 || row >= dash_rows() { return; }
    let y = DASH_LOG_TOP + (row as u32) * CHAR_H as u32;
    fill_rect(14, y, w - 28, CHAR_H as u32, PANEL);
    fill_rect(14, y + 3, 4, CHAR_H as u32 - 8, accent);
    draw_str(28, y + 1, label, accent);
    let lx = 28 + text_width(label) + 14;
    let right = w.saturating_sub(20);
    if right > lx {
        hline_gradient(lx, y + (CHAR_H as u32) / 2, right - lx, accent, PANEL, 255);
    }
}

/// Igual que `splash_dashboard_log` pero con COLOR EXPLICITO -- para que CABINA
/// pinte cada fila segun su estado (verde=bien, ambar=atencion, rojo=problema)
/// en vez de un solo color plano.
pub fn splash_dashboard_log_color(row: usize, msg: &str, color: u32) {
    // ** MIENTRAS LA INTRO ESTA EN PANTALLA, ESTO NO PINTA (2026-08-15).
    //
    // Es la mitad que faltaba del truco de Santa Monica. La intro dejo de
    // SUMARSE al arranque y paso a TAPARLO... y el log siguio pintando **encima
    // de ella**. En el video del Ryzen se ve el resultado: un panel oscuro
    // comiendose los dos tercios de arriba de la pantalla con la ciudad
    // asomando por debajo. Dos capas peleandose por el mismo sitio, que es
    // literalmente lo que el dueno describio: *"la capa estan mezcladas"*.
    //
    // El dueno tambien dijo que hacer con eso, y sin ambiguedad: *"en codigos de
    // kernel en tiempo real esta en 0% a la vista, claro, porque eso no importa
    // sino la presentacion"*.
    //
    // [!] No se pierde NADA. Esta funcion solo PINTA: la linea ya viaja por
    // serie y ya esta guardada en el anillo de CABINA, que es de donde sale F11.
    // Lo unico que se apaga son los pixeles, y solo durante los dos segundos de
    // la intro. Si la intro no llegara a cerrarse, el arranque se veria mudo en
    // pantalla y seguiria hablando por el cable -- que es el canal del que
    // depura, y el que importa cuando algo va mal.
    if intro_en_curso() {
        return;
    }
    let w = unsafe { crate::info::FB_WIDTH };
    let h = unsafe { crate::info::FB_HEIGHT };
    if w == 0 || h == 0 { return; }
    if row >= dash_rows() { return; }
    let y = DASH_LOG_TOP + (row as u32) * CHAR_H as u32;
    // Clear the row (background)
    fill_rect(14, y, w - 28, CHAR_H as u32, DASH_BG);
    // Marca de canaleta: una barrita del color de la linea en el margen.
    //
    // El color del texto ya dice quien habla, pero hay que LEER la linea para
    // notarlo. Una columna de marcas alineadas se lee de un vistazo: se ve
    // cuantas voces distintas hay en pantalla y donde cambia el turno, sin
    // leer una sola palabra. Es lo que convierte el log en algo que se OJEA.
    //
    // El texto normal no lleva marca: si todo estuviera marcado, la columna no
    // diria nada. Marcar es distinguir.
    if color != DASH_TEXT {
        fill_rect(14, y + 4, 3, CHAR_H as u32 - 8, color);
    }
    // Draw up to DASH_LOG_W characters
    let mut buf = [0u8; DASH_LOG_W as usize];
    let bytes = msg.as_bytes();
    let n = bytes.len().min(buf.len());
    buf[..n].copy_from_slice(&bytes[..n]);
    if let Ok(s) = core::str::from_utf8(&buf[..n]) {
        draw_str(28, y, s, color);
    }
}

/// Update the bottom prompt area with the current command being
/// typed. The caller passes the in-progress line (up to a
/// reasonable limit). The prompt always starts with "serial > ".
pub fn splash_dashboard_prompt(line: &str, cursor: usize, blink: bool) {
    let w = unsafe { crate::info::FB_WIDTH };
    let h = unsafe { crate::info::FB_HEIGHT };
    if w == 0 || h == 0 { return; }
    let y = h - DASH_FOOTER_H + 10;
    fill_rect(20, y, w - 40, CHAR_H as u32, CHROME);
    // El prompt ya no dice "serial": el teclado USB escribe desde hace tiempo
    // y la etiqueta se habia quedado contando una etapa anterior del proyecto.
    // La marca en ambar, el signo en magenta -- los mismos dos colores del
    // titulo, para que cabecera y pie se lean como el mismo sistema.
    const PROMPT: &str = "bmo-x";
    draw_str(20, y, PROMPT, NEON_AMBER);
    let sign_x = 20 + text_width(PROMPT) + 8;
    draw_str(sign_x, y, ">", NEON_MAGENTA);
    let prefix_w = text_width(PROMPT) + 8 + text_width("> ") + 4;
    let max_chars = ((w - 40 - prefix_w) / CHAR_W as u32) as usize;
    let n = line.len().min(max_chars);
    let s = &line[..n];
    draw_str(20 + prefix_w, y, s, DASH_TEXT);
    // Cursor de bloque parpadeante EN SU POSICION dentro de la linea, no
    // siempre al final: con las flechas se edita en medio, y el cursor tiene
    // que estar donde va a caer la siguiente letra. Si tapa un caracter, se
    // redibuja encima en el color del fondo -- video inverso, como una terminal
    // de verdad.
    if blink {
        let cx = 20 + prefix_w + (cursor.min(n) as u32) * CHAR_W as u32;
        fill_rect(cx, y, (CHAR_W as u32) - 2, FONT_H as u32, NEON_MAGENTA);
        if cursor < n {
            let one = [line.as_bytes()[cursor]];
            if let Ok(ch) = core::str::from_utf8(&one) {
                draw_str(cx, y, ch, CHROME);
            }
        }
    }
    wc_flush();
}


/// Indicadores de la barra superior: distribucion de teclado activa y estado
/// de los bloqueos. Las lucecitas fisicas de un teclado pueden no responder
/// (firmware, emulacion); la pantalla no depende de eso.
pub fn splash_status_right(layout: &str, caps: bool, num: bool) {
    let w = unsafe { crate::info::FB_WIDTH };
    if w == 0 { return; }

    // La franja se limpia entera antes de escribir: al apagarse un indicador su
    // texto tiene que desaparecer, no quedarse pegado.
    let bar_x = w.saturating_sub(460);
    fill_rect(bar_x, 8, w.saturating_sub(bar_x + 16), DASH_HEADER_H - 12, CHROME);

    // Los bloqueos dejan de ser texto suelto y pasan a ser PASTILLAS: fondo
    // encendido y letra oscura. Un estado activo se ve encendido, no escrito --
    // que es justo lo que un teclado cuyas lucecitas no responden necesita.
    let caps_w = text_width("MAYUS") + 14;
    let num_w  = text_width("NUM") + 14;
    let mut kbd = [0u8; 32];
    let mut ko = 0usize;
    for &c in b"kbd ".iter() { if ko < kbd.len() { kbd[ko] = c; ko += 1; } }
    for &c in layout.as_bytes() { if ko < kbd.len() { kbd[ko] = c; ko += 1; } }
    let kbd_s = core::str::from_utf8(&kbd[..ko]).unwrap_or("");
    let kbd_w = text_width(kbd_s);

    let mut total = kbd_w;
    if caps { total += caps_w + 10; }
    if num  { total += num_w + 10; }
    let mut x = w.saturating_sub(total + 20);

    draw_str(x, 14, kbd_s, DASH_DIM);
    x += kbd_w + 10;
    if caps {
        fill_rect(x, 10, caps_w, FONT_H as u32 + 8, NEON_AMBER);
        draw_str(x + 7, 14, "MAYUS", CHROME);
        x += caps_w + 10;
    }
    if num {
        fill_rect(x, 10, num_w, FONT_H as u32 + 8, NEON_JADE);
        draw_str(x + 7, 14, "NUM", CHROME);
    }
}

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
pub const ALTO_LINEA: u32 = CHAR_H as u32;
/// Ancho de un caracter. La fuente es de paso fijo.
pub const ANCHO_CHAR: u32 = CHAR_W as u32;

/// Pinta la pantalla ENTERA de un color.
pub fn fallo_fondo(color: u32) {
    let w = unsafe { crate::info::FB_WIDTH };
    let h = unsafe { crate::info::FB_HEIGHT };
    if w == 0 || h == 0 { return; }
    fill_rect(0, 0, w, h, color);
}

/// Un rectangulo. Para la barra de la cuenta atras.
pub fn fallo_rect(x: u32, y: u32, w: u32, h: u32, color: u32) {
    fill_rect(x, y, w, h, color);
}

/// Texto en una posicion exacta.
pub fn fallo_texto(x: u32, y: u32, s: &str, color: u32) {
    draw_str(x, y, s, color);
}

/// Texto grande, para el titulo.
pub fn fallo_texto_grande(x: u32, y: u32, s: &str, color: u32, escala: u32) {
    draw_str_scaled(x, y, s, color, escala);
}
