//! **CABINA** -- F11, lo que el kernel ve, con su gravedad.
//!
//! === Que cambia respecto al klog ===
//!
//! El klog es la transcripcion del kernel en texto plano: 96 bytes por linea y
//! **sin severidad**. Servia, y por eso existio primero. Pero la linea que dice
//! si el SMP levanto los doce nucleos llegaba aqui igual que las veinte lineas
//! verdes que la rodean, y **un dato que existe y no se puede distinguir no
//! esta**.
//!
//! CABINA lleva por evento su severidad, su capa, su modulo y su valor. Esta
//! ventana los pinta: una columna de colores alineada se lee de un vistazo sin
//! tener que leer el texto.
//!
//! === * Esto NO es "ir a Ring 0" ===
//!
//! Y la distincion no es un tecnicismo, es el sistema entero.
//!
//! Aqui no se ejecuta nada privilegiado. El compositor sigue siendo un proceso
//! de Ring 3 con sus capabilities contadas, y lo unico que hace es **preguntar**
//! (`TASK_OP_CABINA_INFO` y `_TEXTO`). Ninguna de las dos escribe nada.
//!
//! En un sistema de capabilities, **ver y poder son cosas separadas**. Que se
//! pueda mirar TODO sin poder tocar nada es la mitad interesante de la
//! transparencia total que declara este proyecto.
//!
//! === Se mueve, como todas ===
//!
//! Lleva [`Chrome`], igual que la ventana de ESTRATOS: se arrastra por la barra
//! de titulo, se redimensiona por la esquina, y tiene sus tres botones. Una
//! ventana clavada en el centro tapa justo lo que uno quiere comparar con ella.

use bmo_userland as bmo;

use super::chrome::Chrome;
use super::*;
use crate::text::decimal;

// Proporcion de la pantalla, no un tamano fijo: ver `docs/identidad/LIDERES.md`.
const CAB_PCT_W: u32 = 70;
const CAB_PCT_H: u32 = 55;
const CAB_MIN_W: u32 = 520;
const CAB_MIN_H: u32 = 260;

// -- El CIAN del gato -----------------------------------------------------
//
// Es el color de BMO-X: los ojos del gato del logo y el kanji. Aqui hace de
// acento --titulo, subrayado y el bloque del icono-- sobre un fondo casi negro
// con un punto de azul, para que el cian resalte sin quemar.
//
// El neon se usa **solo en el acento**. Novecientos pixeles de borde de neon
// era lo que mas cansaba de mirar en la version anterior de esta ventana, y ya
// se rebajo una vez: la leccion esta pagada.
const CAB_BG: u32 = 0x0007_0B0E;
const CAB_TITLE_BG: u32 = 0x000D_1519;
const CAB_EDGE: u32 = 0x0019_3038;
const CYAN: u32 = 0x0034_E2E4;
const CYAN_DIM: u32 = 0x0017_6E70;

// -- Los colores de la GRAVEDAD -------------------------------------------
//
// El orden es el de `cabina_core::Severity`, y se pintan de frio a caliente
// porque asi no hay que aprenderse nada: lo que sube de temperatura importa
// mas. `Trace` es el mas apagado a proposito -- es ruido util, no una noticia.
const SEV_COLOR: [u32; 5] = [
    0x00C8_D4DC, // Info    -- gris claro
    0x0055_6673, // Trace   -- gris apagado
    0x00E8_B84B, // Warning -- ambar
    0x00F2_6B4B, // Fault   -- naranja rojo
    0x00FF_3355, // Panic   -- rojo
];

const SEV_NAME: [&str; 5] = ["info", "trace", " AVISO", " FALLO", " PANICO"];

/// Cuantos eventos caben, sabiendo el alto. Se calcula y no se fija: la ventana
/// se puede redimensionar, y una cuenta fija dejaria filas pintadas fuera o
/// hueco vacio dentro.
fn visible_rows(chrome: &Chrome) -> usize {
    let usable = chrome.height.saturating_sub(TITLE_H + 44);
    (usable / (bmo::GLIFO_ALTO + 3)) as usize
}

pub(crate) struct CabinaWindow {
    pub(crate) chrome: Chrome,
    /// Cuantos eventos hacia atras empieza la ventana. `0` = lo ultimo.
    pub(crate) from: u64,
    /// Gravedad minima que deja pasar. `0` = todas.
    ///
    /// Vive aqui y no en el modulo que pinta porque es estado de la SESION.
    /// Filtrar por gravedad --y no por texto-- se puede **porque CABINA la
    /// lleva**: el klog no, y por eso su filtro tenia que adivinar por el
    /// prefijo de la linea.
    pub(crate) minima: u64,
    /// **Ensenar SOLO lo que produjo la ultima accion.**
    ///
    /// === Por que este filtro es distinto de los otros ===
    ///
    /// El de gravedad contesta *"que fue grave"*, y trae lo grave de esta accion
    /// mezclado con lo grave de las diez anteriores. Un lanzamiento emite
    /// eventos desde cuatro modulos --`lanzar`, `proc`, `bex`, `disk`-- asi que
    /// para leer QUE paso al pulsar hay que juntarlos a ojo por el `#N`.
    ///
    /// Este contesta la otra pregunta, que es la que se hace de verdad delante
    /// de la pantalla: **"ensename todo lo que hizo esto que acabo de pulsar"**.
    /// Lo bueno y lo malo, en orden, sin nada de antes.
    ///
    /// ** No hay nada que deducir: el kernel ya agrupa (`cabina::intento`) y
    /// desde hoy entrega el numero (`CABINA_INTENTO`). Esto solo lo lee.
    pub(crate) last_only: bool,
}

impl CabinaWindow {
    pub(crate) fn new(p: &bmo::Pantalla) -> Self {
        Self {
            chrome: Chrome::new(p, CAB_PCT_W, CAB_PCT_H, CAB_MIN_W, CAB_MIN_H),
            from: 0,
            minima: 0,
            last_only: false,
        }
    }
}

/// **El numero del intento mas reciente que haya en el anillo.** `0` = ninguno.
///
/// Se busca hacia atras desde lo ultimo, que es donde esta: mirar los 48 en
/// orden costaria lo mismo, pero empezar por el final permite parar en cuanto
/// aparece uno -- y en la practica es el primero o el segundo.
fn last_try(any: u64) -> u64 {
    let mut i = 0u64;
    while i < any {
        let n = bmo::cabina_intento(i);
        if n != 0 {
            return n;
        }
        i += 1;
    }
    0
}

fn place(s: &[u8], dst: &mut [u8], n: &mut usize) {
    for &b in s {
        if *n < dst.len() {
            dst[*n] = b;
            *n += 1;
        }
    }
}

fn num(v: u64, dst: &mut [u8], n: &mut usize) {
    let mut d = [0u8; 10];
    let k = decimal(v, &mut d);
    place(&d[..k], dst, n);
}

pub(crate) fn paint(p: &bmo::Pantalla, c: &CabinaWindow) {
    if c.chrome.minimized {
        return;
    }
    c.chrome.paint_chrome(p, CAB_EDGE, CAB_BG, CAB_TITLE_BG, CYAN);
    c.chrome.paint_buttons(p, CAB_TITLE_BG);

    let tx = c.chrome.x + 16;
    // El bloque de acento del titulo: el ojo del gato, en pequeno.
    p.rect(tx, c.chrome.y + 9, 8, 8, CYAN);
    let px = p.texto(tx + 16, c.chrome.y + 8, "CABINA", INK);
    let px = p.texto(
        px + 2 * bmo::GLIFO_ANCHO,
        c.chrome.y + 8,
        "lo que el kernel ve",
        CYAN_DIM,
    );

    // ** LA FECHA Y LA HORA, en el titulo.
    //
    // Cada evento lleva su sello del arranque (`t34A93`), y eso ordena lo que
    // paso **en esta sesion** y nada mas: dos arranques no se pueden comparar y
    // un log no se puede cruzar con nada de fuera. La hora de la placa --que
    // lleva su pila desde antes de que existieramos-- convierte la bitacora en
    // algo que se puede archivar.
    //
    // Si no hay reloj no se pone nada. **No se inventa una fecha**: un log
    // fechado en 1970 miente con mas convicion que uno sin fechar.
    let mut seal = [0u8; 24];
    let sn = date_at(&mut seal);
    if sn > 0 {
        p.texto_bytes(px + 2 * bmo::GLIFO_ANCHO, c.chrome.y + 8, &seal[..sn], CYAN_DIM);
    }

    let mut ty = c.chrome.y + TITLE_H + 8;

    let any = bmo::cabina_disponibles();
    let total = bmo::cabina_total();
    let lost = bmo::cabina_perdidos();

    if any == 0 {
        p.texto(tx, ty, "el kernel no ha dicho nada todavia.", INK_DIM);
        return;
    }

    // La cabecera dice **cuantos se cayeron del anillo**, y eso vale tanto como
    // los que se ven: un anillo que dio la vuelta y no lo dice hace creer que
    // el arranque empezo donde empieza el primero que sobrevive.
    let mut cab = [0u8; 96];
    let mut n = 0usize;
    place(b"vivos ", &mut cab, &mut n);
    num(any, &mut cab, &mut n);
    place(b" de ", &mut cab, &mut n);
    num(total, &mut cab, &mut n);
    if lost > 0 {
        place(b"   (se cayeron ", &mut cab, &mut n);
        num(lost, &mut cab, &mut n);
        place(b")", &mut cab, &mut n);
    }
    if c.minima > 0 {
        place(b"   filtro: >= ", &mut cab, &mut n);
        place(
            SEV_NAME[(c.minima as usize).min(4)].trim_start().as_bytes(),
            &mut cab,
            &mut n,
        );
    }
    p.texto_bytes(tx, ty, &cab[..n], INK_DIM);
    ty += bmo::GLIFO_ALTO + 6;

    // -- LA GUIA DEL FILTRO, cada opcion de su propio color -------------
    //
    // Se pinta siempre, tambien sin filtro: un atajo que solo se descubre
    // pulsandolo no existe. Y cada nombre va del color de sus lineas, asi que
    // la guia se explica sola y no hay que memorizar nada.
    let mut gx = tx;
    gx = p.texto(gx, ty, "G:", INK_DIM) + bmo::GLIFO_ANCHO;
    for s in 0..5usize {
        let sel = c.minima as usize == s;
        let color = if sel { SEV_COLOR[s] } else { CYAN_DIM };
        let nom = SEV_NAME[s].trim_start();
        let end = p.texto(gx, ty, nom, color);
        if sel {
            p.rect(gx, ty + bmo::GLIFO_ALTO + 1, end - gx, 1, color);
        }
        gx = end + bmo::GLIFO_ANCHO;
    }
    // ** El filtro por ACCION, al lado del de gravedad y con su tecla a la
    // vista. Un atajo que solo se descubre pulsandolo no existe.
    gx += bmo::GLIFO_ANCHO * 2;
    let color_to = if c.last_only { SEV_COLOR[2] } else { CYAN_DIM };
    let end = p.texto(gx, ty, "A: esta accion", color_to);
    if c.last_only {
        p.rect(gx, ty + bmo::GLIFO_ALTO + 1, end - gx, 1, color_to);
    }
    ty += bmo::GLIFO_ALTO + 8;

    // -- LOS EVENTOS ----------------------------------------------------
    let count = visible_rows(&c.chrome);
    let mut painted_count = 0usize;
    let mut i = c.from;

    // ** El intento a seguir, resuelto UNA vez y no por evento: preguntarlo
    // dentro del bucle serian dos syscalls por linea para contestar siempre lo
    // mismo.
    let run_of = if c.last_only { last_try(any) } else { 0 };

    while painted_count < count && i < any {
        // El filtro por ACCION va ANTES que el de gravedad, y el orden importa:
        // dentro de una accion se quiere ver TODO --el `info` que dice de que
        // sector se leyo vale tanto como el `FALLO`-- asi que la gravedad se
        // aplica dentro de lo que la accion ya dejo pasar.
        if run_of != 0 && bmo::cabina_intento(i) != run_of {
            i += 1;
            continue;
        }
        let sev = bmo::cabina_severidad(i);
        // El filtro es por GRAVEDAD y no por texto. Solo se puede porque CABINA
        // la lleva: buscar la palabra "error" dentro de la linea pinta de rojo
        // un mensaje que dice "sin errores", que es la clase de ayuda que
        // estorba.
        if sev < c.minima {
            i += 1;
            continue;
        }
        let s = (sev as usize).min(4);
        let color = SEV_COLOR[s];

        // La barra de gravedad, a la izquierda. Es lo que hace que la columna
        // se lea sin leer: un ojo encuentra un cambio de color mucho antes que
        // una palabra.
        p.rect(tx, ty + 2, 3, bmo::GLIFO_ALTO - 2, color);

        let mut line = [0u8; 120];
        let mut n = 0usize;
        place(SEV_NAME[s].as_bytes(), &mut line, &mut n);
        place(b"  ", &mut line, &mut n);

        let mut module_name = [0u8; 16];
        let m = bmo::cabina_texto(i, bmo::CABINA_TXT_MODULO, &mut module_name);
        place(&module_name[..m], &mut line, &mut n);
        // El modulo se alinea a ocho para que los mensajes empiecen todos en la
        // misma columna. Con anchos distintos, la vista no encuentra el texto.
        for _ in m..8 {
            place(b" ", &mut line, &mut n);
        }
        place(b" ", &mut line, &mut n);

        let mut message_text = [0u8; 72];
        let k = bmo::cabina_texto(i, bmo::CABINA_TXT_MENSAJE, &mut message_text);
        place(&message_text[..k], &mut line, &mut n);

        // El VALOR solo si dice algo. Un `=0` detras de cada linea es ruido en
        // la mayoria, y justo en las que importa --fugas, choques de cerrojo--
        // el cero ES la respuesta correcta y ya se ve en `info`.
        if let Some(v) = bmo::cabina_campo(bmo::CABINA_VALOR, i) {
            if v != 0 {
                place(b" =", &mut line, &mut n);
                num(v, &mut line, &mut n);
            }
        }

        p.texto_bytes(tx + 10, ty, &line[..n.min(line.len())], color);
        ty += bmo::GLIFO_ALTO + 3;
        painted_count += 1;
        i += 1;
    }

    // -- La barra de atajos, del mismo estilo que las demas -------------
    let by = c.chrome.y + c.chrome.height - bmo::GLIFO_ALTO - 8;
    p.texto(
        tx,
        by,
        "G gravedad   RePag/AvPag historia   arrastra el titulo   ESC cierra",
        CYAN_DIM,
    );
}

/// `AAAA-MM-DD HH:MM` de la placa. Devuelve 0 si la maquina no sabe que dia es.
///
/// Los segundos se dejan fuera a proposito: en un titulo no se leen, y un
/// numero que cambia solo hace parpadear una linea que se mira quieta.
fn date_at(out: &mut [u8; 24]) -> usize {
    let v = bmo::info(bmo::INFO_FECHA);
    let Some(f) = bmo_rtc::desempaquetar(v) else {
        return 0;
    };
    let mut b = [0u8; 24];
    let n = bmo_rtc::escribir(&f, &mut b);
    if n < 16 {
        return 0;
    }
    // Hasta el minuto: `AAAA-MM-DD HH:MM` son 16.
    out[..16].copy_from_slice(&b[..16]);
    16
}
