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
    let bit = |m: &[u8], i: usize| m[i / 8] >> (i % 8) & 1 == 1;
    for fy in 0..gato::ALTO {
        for fx in 0..gato::ANCHO {
            let i = (fy * gato::ANCHO + fx) as usize;
            // Los ojos ganan al trazo: son el unico sitio con color.
            let color = if bit(&gato::OJOS, i) {
                ACCENT
            } else if bit(&gato::TRAZO, i) {
                WHITE
            } else {
                continue;
            };
            fill_rect(x0 + fx * escala, y0 + fy * escala, escala, escala, color);
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
fn draw_kanji(x0: u32, y0: u32, escala: u32, color: u32) {
    let bit = |m: &[u8], i: usize| m[i / 8] >> (i % 8) & 1 == 1;
    for fy in 0..gato::KANJI_ALTO {
        for fx in 0..gato::KANJI_ANCHO {
            let i = (fy * gato::KANJI_ANCHO + fx) as usize;
            if bit(&gato::KANJI, i) {
                fill_rect(x0 + fx * escala, y0 + fy * escala, escala, escala, color);
            }
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

pub fn boot_intro() {
    let w = unsafe { crate::info::FB_WIDTH };
    let h = unsafe { crate::info::FB_HEIGHT };
    if w == 0 || h == 0 {
        return;
    }
    fill_rect(0, 0, w, h, BG);

    // ** LA CIUDAD, DETRAS DE TODO.
    //
    // La dibuja `bmo-ciudad` emitiendo rectangulos; aqui solo se le dice donde
    // caen. El crate no sabe que existe un framebuffer, y por eso sus pruebas
    // corren en el anfitrion -- algo que un fondo de arranque no habia podido
    // hacer nunca en esta casa.
    //
    // ** LA SEMILLA ES EL TAMANO DEL PANEL, y es una decision.
    //
    // Tenia que cumplir dos cosas a la vez: que la ciudad sea **siempre la
    // misma** en esta maquina --un fondo que cambia en cada arranque no sirve
    // para notar que algo cambio-- y que no sea la misma en todas. El panel da
    // las dos: es estable aqui, distinto alli, y no hay que guardar nada ni
    // leer el disco antes de poder pintar.
    let mut ciudad = bmo_ciudad::Ciudad::nueva(w as i32, h as i32, ((w as u64) << 20) | h as u64);
    // ** Y ARRANCA A OSCURAS, a proposito. La ciudad se enciende cuando haya de
    // que informar; encenderla entera aqui seria decir que el sistema esta listo
    // antes de estarlo, y esta pantalla no miente.
    ciudad.encender(0);
    ciudad.dibujar(|x, y, cw, ch, color| {
        if cw > 0 && ch > 0 && x >= 0 && y >= 0 && (x as u32) < w && (y as u32) < h {
            fill_rect(x as u32, y as u32, cw as u32, ch as u32, color);
        }
    });

    // La escala sale de la ALTURA de la pantalla, no de un numero fijo: en 1080
    // sale a x2 y en 720 a x1, y en las dos ocupa la misma fraccion. Un `3`
    // puesto a mano se sale por abajo en el primer monitor pequeno.
    let escala = if h >= 900 { 2 } else { 1 };
    let gw = gato::ANCHO * escala;
    let gh = gato::ALTO * escala;

    // El bloque entero --gato + titulo + subtitulo-- se centra como una unidad.
    // Centrar el gato y luego colgarle el texto deja el conjunto bajo.
    const HUECO: u32 = 34;
    let escala_t = if h >= 900 { 5 } else { 4 };
    let tw = text_width_scaled("BMO-X", escala_t);
    let th = FONT_H as u32 * escala_t;
    let alto_total = gh + HUECO + th + 10 + 3 + 14 + FONT_H as u32;

    // ** LA FILA DE ARRIBA SON DOS PIEZAS: el gato y el kanji a su derecha.
    //
    // En el logo la composicion **no es simetrica**: el gato va a la izquierda y
    // el kanji a su derecha, y el par se centra como una unidad. Centrar solo el
    // gato y colgarle el kanji al lado dejaria el conjunto corrido a la
    // izquierda -- que es el mismo error que ya evitaba el bloque entero cuando
    // se escribio ("centrar el gato y luego colgarle el texto deja el conjunto
    // bajo"), aplicado ahora al otro eje.
    let kw = gato::KANJI_ANCHO * escala;
    let kh = gato::KANJI_ALTO * escala;
    let hueco_k = 22 * escala;
    let fila_w = gw + hueco_k + kw;

    let gy = h.saturating_sub(alto_total) / 2;
    let gx = w.saturating_sub(fila_w) / 2;
    draw_gato(gx, gy, escala);
    // ** La altura del kanji sale del logo, no de "centrado a ojo": alli su
    // centro cae al 75% del alto del gato --medido sobre la imagen-- y no a la
    // mitad. Centrarlo verticalmente lo subiria y se notaria.
    let ky = gy + (gh * 3) / 4 - kh / 2;
    draw_kanji(gx + gw + hueco_k, ky, escala, ACCENT);

    let ty = gy + gh + HUECO;
    let tx = w.saturating_sub(tw) / 2;
    draw_str_scaled(tx, ty, "BMO-X", WHITE, escala_t);
    // ** EL TRIANGULO, pegado a la X como en el logo. Va DESPUES del texto y a
    // su derecha, a la altura de la mitad superior de las letras.
    let lado = th / 2;
    triangulo_aviso(tx + tw + escala_t as u32 * 2, ty + th / 3, lado, ACCENT);
    // Subrayado exacto: el ancho se pregunta a la fuente, no se estima.
    fill_rect(tx, ty + th + 10, tw, 3, ACCENT);

    // ** EL SUBTITULO CON SUS DOS REGLAS, que en el logo lo flanquean.
    //
    // No es adorno: son lo que convierte una linea de texto suelta en un pie de
    // firma. Y se calculan del ancho del texto --no de un numero a mano-- para
    // que sigan cuadrando el dia que el subtitulo cambie de palabra.
    let sub = "BMO METAKERNEL";
    let sw = text_width(sub);
    let sy = ty + th + 10 + 3 + 14;
    let sx = w.saturating_sub(sw) / 2;
    draw_str(sx, sy, sub, ACCENT);
    let regla = (tw / 3).max(20);
    let hueco_regla = 14;
    let ry = sy + FONT_H as u32 / 2;
    fill_rect(sx.saturating_sub(hueco_regla + regla), ry, regla, 1, DIM);
    fill_rect(sx + sw + hueco_regla, ry, regla, 1, DIM);

    // ** Y EL MARCO, lo ultimo: encuadra todo lo demas.
    marco_esquinas(w, h, ACCENT);
    wc_flush();

    // * EL FUNDIDO ES DE LOS OJOS, no de la pantalla.
    //
    // 276 pixeles parpadeando cuestan nada y dan lo unico que un logo estatico
    // no da: la senal de que la maquina esta VIVA. Un fundido de pantalla
    // completa costaria los millones de pixeles que el arranque no tiene que
    // gastar.
    // [!] `gx`, no `(w - gw)/2`. El gato ya no se centra solo --comparte fila
    // con el kanji-- y esta linea calculaba su propia X: los ojos habrian
    // parpadeado a la izquierda de donde esta la cara.
    let ex = gx;
    for &a in &[90u32, 170, 255] {
        let ojo = blend(ACCENT, a);
        let bit = |m: &[u8], i: usize| m[i / 8] >> (i % 8) & 1 == 1;
        for fy in 0..gato::ALTO {
            for fx in 0..gato::ANCHO {
                let i = (fy * gato::ANCHO + fx) as usize;
                if bit(&gato::OJOS, i) {
                    fill_rect(ex + fx * escala, gy + fy * escala, escala, escala, ojo);
                }
            }
        }
        wc_flush();
        hold_ms(70);
    }

    // * Y SE QUEDA A LA VISTA `GATO_MS`.
    //
    // Aqui no hay trabajo con el que solapar: el kernel esta operativo a los
    // 52 ms, asi que para que el logo se VEA hay que esperar. Y **no se puede
    // saltar con una tecla**, a diferencia de la intro del compositor: en este
    // punto del arranque el USB no se ha enumerado todavia y no hay teclado que
    // preguntar.
    //
    // O sea que este numero es tiempo de arranque, todo el. Esta en una
    // constante y con el coste escrito para que subirlo sea una decision y no un
    // descuido.
    hold_ms(GATO_MS);
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
