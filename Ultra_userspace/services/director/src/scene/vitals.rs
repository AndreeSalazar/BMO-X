//! **F7 y F8: lo que la maquina esta haciendo AHORA.** Su propia ventana.
//!
//! Pedido por el dueno el 2026-08-12: *"el F7 y F8, pero al presionar no veo mi
//! terminal la caja para ver -- no es por terminal sino SU PROPIO terminal para
//! facilitar las vistas... y falta el mem, que estan comiendo, inspirado en
//! administrador de tareas"*.
//!
//! # Por que no son un comando de la caja de Ejecutar
//!
//! Porque son **dos formas distintas de mirar**, y meterlas en la misma caja las
//! rompe a las dos:
//!
//! | | |
//! |---|---|
//! | `info` en la caja | una FOTO. Se escribe, sale, y se queda ahi mientras subes por el historial |
//! | F7 / F8 | una VISTA. Se repinta sola, y lo que se mira es **como cambia** |
//!
//! Un numero que cambia dentro de un historial es ruido: cada refresco empujaria
//! hacia arriba lo que estabas leyendo. Y una foto que no se repinta no sirve
//! para ver quien esta comiendo memoria AHORA.
//!
//! Es la misma frontera que separa CABINA (F11) de `cabina` como orden: la
//! primera es una ventana viva, la segunda un volcado.
//!
//! # Y por que dos ventanas y no una con pestanas
//!
//! Porque se miran en momentos distintos. F7 se abre cuando algo va lento; F8
//! cuando algo se come la RAM. Juntarlas obligaria a cambiar de pestana justo
//! cuando se tiene prisa.

use bmo_userland as bmo;

use super::chrome::Chrome;
use super::*;
use crate::text::decimal;

const VIT_PCT_W: u32 = 52;
const VIT_PCT_H: u32 = 46;
const VIT_MIN_W: u32 = 460;
const VIT_MIN_H: u32 = 240;

const VIT_BG: u32 = 0x0008_0C10;
const VIT_TITLE_BG: u32 = 0x000E_161B;
const VIT_EDGE: u32 = 0x001B_3340;
/// El mismo cian del gato que usa CABINA. Se repite el valor en vez de
/// importarlo porque el de `cabina.rs` es privado suyo, y hacerlo publico solo
/// para esto ataria dos ventanas que no tienen nada que ver.
const VIT_CYAN: u32 = 0x0034_E2E4;

/// Cual de las dos vistas.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Which {
    /// F7 -- el CPU: a que va, que gasta, quien esta despierto.
    Cpu,
    /// F8 -- la memoria: cuanta hay, y **quien se la esta comiendo**.
    Memoria,
}

pub(crate) struct VitalsWindow {
    pub(crate) chrome: Chrome,
    pub(crate) which: Which,
}

impl VitalsWindow {
    pub(crate) fn new(p: &bmo::Pantalla, which: Which) -> Self {
        Self {
            chrome: Chrome::new(p, VIT_PCT_W, VIT_PCT_H, VIT_MIN_W, VIT_MIN_H),
            which,
        }
    }
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

/// Un numero con un decimal: `59.5`. Sin coma flotante, que aqui no hay.
fn with_decimal(whole: u64, thousandths: u64, dst: &mut [u8], n: &mut usize) {
    num(whole, dst, n);
    place(b".", dst, n);
    num(thousandths, dst, n);
}

/// Bytes a la escala que se lea, **con el numero exacto detras**.
///
/// La misma regla que `cabina_core::legible::size`, y por el mismo motivo: la
/// escala se lee y el numero exacto se COMPARA. Aqui la comparacion es contra lo
/// que el propio programa cree haber pedido.
fn tam(bytes: u64, dst: &mut [u8], n: &mut usize) {
    const MIB: u64 = 1024 * 1024;
    const KIB: u64 = 1024;
    if bytes >= MIB {
        with_decimal(bytes / MIB, (bytes % MIB) * 10 / MIB, dst, n);
        place(b" MiB", dst, n);
    } else if bytes >= KIB {
        with_decimal(bytes / KIB, (bytes % KIB) * 10 / KIB, dst, n);
        place(b" KiB", dst, n);
    } else {
        num(bytes, dst, n);
        place(b" B", dst, n);
    }
}

/// Una barra `[####----]` de `width` casillas.
fn bar(used_one: u64, total: u64, width: usize, dst: &mut [u8], n: &mut usize) {
    place(b"[", dst, n);
    let full = if total == 0 { 0 } else { (used_one * width as u64 / total) as usize };
    for i in 0..width {
        place(if i < full { b"#" } else { b"-" }, dst, n);
    }
    place(b"]", dst, n);
}

/// `vueltas` son las del bucle del escritorio en el ultimo segundo entero
/// (`desktop::Tick::loops_per_second`). Llega como argumento y no se calcula
/// aqui porque una VISTA no mide: mide quien da las vueltas.
pub(crate) fn paint(p: &bmo::Pantalla, c: &VitalsWindow, vueltas: u32) {
    if c.chrome.minimized {
        return;
    }
    c.chrome.paint_chrome(p, VIT_EDGE, VIT_BG, VIT_TITLE_BG, VIT_CYAN);
    c.chrome.paint_buttons(p, VIT_TITLE_BG);

    let tx = c.chrome.x + 16;
    p.texto(
        tx,
        c.chrome.y + 8,
        match c.which {
            Which::Cpu => "CPU",
            Which::Memoria => "MEMORIA",
        },
        VIT_CYAN,
    );
    p.texto(
        tx + 90,
        c.chrome.y + 8,
        match c.which {
            Which::Cpu => "a que va y que gasta",
            Which::Memoria => "quien se la esta comiendo",
        },
        INK_DIM,
    );

    let mut y = c.chrome.y + TITLE_H + 10;
    let step = bmo::GLIFO_ALTO + 4;
    let mut b = [0u8; 96];

    match c.which {
        Which::Cpu => paint_cpu(p, c, tx, &mut y, step, &mut b, vueltas),
        Which::Memoria => paint_memory(p, c, tx, &mut y, step, &mut b),
    }

    // La barra de abajo dice lo unico que hay que saber para usarla.
    p.texto(
        tx,
        c.chrome.y + c.chrome.height - bmo::GLIFO_ALTO - 8,
        "se repinta sola   ESC cierra   arrastra el titulo",
        INK_DIM,
    );
}

/// Una fila `label   value`, con la etiqueta a ancho fijo.
fn row(p: &bmo::Pantalla, x: u32, y: u32, etiq: &str, b: &[u8], ink: u32) {
    p.texto(x, y, etiq, INK_DIM);
    if let Ok(s) = core::str::from_utf8(b) {
        p.texto(x + 130, y, s, ink);
    }
}

fn paint_cpu(
    p: &bmo::Pantalla,
    c: &VitalsWindow,
    tx: u32,
    y: &mut u32,
    step: u32,
    b: &mut [u8; 96],
    vueltas: u32,
) {
    let _ = c;

    // ** LO PRIMERO ES QUE SABE MEDIR, y no una medida.
    //
    // Igual que en `info`: las filas de abajo pueden salir vacias por dos
    // motivos que se ven igual, y sin esta no se distinguen.
    let sensors = bmo::info(bmo::INFO_CPU_SENSORES);
    let mut n = 0usize;
    if sensors == 0 {
        place(b"nada: el perfil no declara sensores", b, &mut n);
    } else {
        if sensors & 1 != 0 {
            place(b"frecuencia", b, &mut n);
        }
        if sensors & 3 == 3 {
            place(b" + ", b, &mut n);
        }
        if sensors & 2 != 0 {
            place(b"consumo", b, &mut n);
        }
    }
    row(p, tx, *y, "mide", &b[..n], INK_DIM);
    *y += step;

    let hz = bmo::info(bmo::INFO_TSC_HZ);
    let actual = bmo::info(bmo::INFO_CPU_HZ_REAL);
    let mut n = 0usize;
    if actual == 0 {
        place(b"-- (aun sin dos lecturas)", b, &mut n);
    } else {
        with_decimal(actual / 1_000_000_000, (actual % 1_000_000_000) / 100_000_000, b, &mut n);
        place(b" GHz   de ", b, &mut n);
        with_decimal(hz / 1_000_000_000, (hz % 1_000_000_000) / 100_000_000, b, &mut n);
        place(b" base", b, &mut n);
    }
    row(p, tx, *y, "va a", &b[..n], if actual > hz { INK_OK } else { INK });
    *y += step;

    let mw = bmo::info(bmo::INFO_CPU_MW_PAQUETE);
    let mut n = 0usize;
    if mw == 0 {
        place(b"-- (sin RAPL)", b, &mut n);
    } else {
        with_decimal(mw / 1000, (mw % 1000) / 100, b, &mut n);
        place(b" W el paquete entero", b, &mut n);
    }
    row(p, tx, *y, "gasta", &b[..n], INK);
    *y += step;

    // [!] Y el de ESTE nucleo va aparte y con su nombre completo. Ver el
    // comentario de `INFO_CPU_MW_NUCLEO_ACTUAL`: llamarlo "nucleos" hizo que un
    // numero correcto se leyera como una mentira.
    let mwn = bmo::info(bmo::INFO_CPU_MW_NUCLEO_ACTUAL);
    let mut n = 0usize;
    if mwn == 0 {
        place(b"--", b, &mut n);
    } else {
        with_decimal(mwn / 1000, (mwn % 1000) / 100, b, &mut n);
        place(b" W   (solo el que lee: los otros no se ven)", b, &mut n);
    }
    row(p, tx, *y, "este nucleo", &b[..n], INK_DIM);
    *y += step + 6;

    let alive_count = bmo::info(bmo::INFO_SMP_VIVOS);
    let mut n = 0usize;
    if alive_count == 0 {
        place(b"solo el BSP    (`smp all` levanta los demas)", b, &mut n);
    } else {
        num(alive_count + 1, b, &mut n);
        place(b" en pie de ", b, &mut n);
        num(bmo::info(bmo::INFO_CPU_HILOS), b, &mut n);
    }
    row(p, tx, *y, "nucleos", &b[..n], if alive_count == 0 { INK_DIM } else { INK_OK });
    *y += step;

    let mut n = 0usize;
    num(bmo::info(bmo::INFO_TAREAS_TOTAL), b, &mut n);
    place(b" tareas, ", b, &mut n);
    num(bmo::info(bmo::INFO_TAREAS_LISTAS), b, &mut n);
    place(b" listas", b, &mut n);
    row(p, tx, *y, "planificador", &b[..n], INK);
    *y += step;

    // ** LA VUELTA DEL ESCRITORIO, que hasta hoy nadie habia contado.
    //
    // Tres sitios calibraron un ritmo contra "unos 60 por segundo" sin que
    // ninguno lo midiera, y el doble clic de los iconos era uno de ellos: con el
    // bucle corriendo mucho mas rapido, la ventana del gesto se encogia sola y
    // un icono se podia senalar sin abrirse nunca. Ver `scene::double_click`.
    //
    // Va en F7 porque F7 es la ventana de "algo va lento", y esta es la cifra
    // que dice si el lento es el escritorio.
    let mut n = 0usize;
    if vueltas == 0 && hz == 0 {
        // Sin reloj de referencia no hay ritmo que medir, y decir "aun" seria
        // prometer un numero que no va a llegar nunca.
        place(b"-- (sin reloj de referencia)", b, &mut n);
    } else if vueltas == 0 {
        place(b"-- (aun sin un segundo entero)", b, &mut n);
    } else {
        num(vueltas as u64, b, &mut n);
        place(b" vueltas/s   del bucle, no fotogramas", b, &mut n);
    }
    row(p, tx, *y, "escritorio", &b[..n], if vueltas == 0 { INK_DIM } else { INK });
}

fn paint_memory(p: &bmo::Pantalla, c: &VitalsWindow, tx: u32, y: &mut u32, step: u32, b: &mut [u8; 96]) {
    let total = bmo::info(bmo::INFO_RAM_TOTAL);
    let free_one = bmo::info(bmo::INFO_RAM_LIBRE);
    let used = total.saturating_sub(free_one);

    let mut n = 0usize;
    tam(used, b, &mut n);
    place(b" de ", b, &mut n);
    tam(total, b, &mut n);
    place(b"  ", b, &mut n);
    bar(used, total, 20, b, &mut n);
    row(p, tx, *y, "usada", &b[..n], INK);
    *y += step + 8;

    // ** LA TABLA: QUIEN SE LA ESTA COMIENDO.
    //
    // Es la vista que el dueno pidio, y la que hasta hoy no se podia hacer: el
    // kernel sabia cuanto come el proceso N desde julio, y no habia forma de
    // preguntar QUIENES son sin saber sus pids de antemano. Ver
    // `obj::memoria::ranura`.
    p.texto(tx, *y, "pid", INK_DIM);
    p.texto(tx + 70, *y, "pedido", INK_DIM);
    p.texto(tx + 210, *y, "veces", INK_DIM);
    *y += step;

    let mut row_n = 0usize;
    let mut sum = 0u64;
    // El tope es el de la tabla del kernel: pedir mas seria preguntar por
    // ranuras que no existen, y el kernel contestaria 0 -- que aqui se lee
    // igual que "no hay mas".
    while row_n < 16 {
        let campo = |base: u64| base | ((row_n as u64) << 8);
        let pid = bmo::info(campo(bmo::INFO_MEM_QUIEN_PID));
        if pid == 0 {
            break;
        }
        let bytes = bmo::info(campo(bmo::INFO_MEM_QUIEN_BYTES));
        let times = bmo::info(campo(bmo::INFO_MEM_QUIEN_PETICIONES));
        sum += bytes;

        let mut n = 0usize;
        num(pid, b, &mut n);
        if let Ok(s) = core::str::from_utf8(&b[..n]) {
            p.texto(tx, *y, s, INK);
        }
        let mut n = 0usize;
        tam(bytes, b, &mut n);
        if let Ok(s) = core::str::from_utf8(&b[..n]) {
            p.texto(tx + 70, *y, s, INK);
        }
        let mut n = 0usize;
        num(times, b, &mut n);
        // Las peticiones se pintan en ambar al acercarse al tope: cuatro es el
        // maximo y la quinta la niega el kernel. Un programa en 3 no esta roto,
        // pero es el que hay que mirar si algo falla al pedir.
        if let Ok(s) = core::str::from_utf8(&b[..n]) {
            p.texto(tx + 210, *y, s, if times >= 3 { INK_BAD } else { INK_DIM });
        }
        *y += step;
        row_n += 1;
    }

    if row_n == 0 {
        // Y esto NO es un fallo: significa que ningun programa de Ring 3 ha
        // pedido memoria con `KIND_MEMORIA`. Decirlo con palabras evita leer una
        // tabla vacia como una tabla rota.
        p.texto(tx, *y, "nadie ha pedido memoria todavia", INK_DIM);
        *y += step;
    } else {
        *y += 6;
        let mut n = 0usize;
        tam(sum, b, &mut n);
        place(b" entre ", b, &mut n);
        num(row_n as u64, b, &mut n);
        place(b" procesos", b, &mut n);
        row(p, tx, *y, "suman", &b[..n], INK_OK);
    }

    let _ = c;
}
