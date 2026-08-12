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

use super::marco::Marco;
use super::*;
use crate::texto::decimal;

const VIT_PCT_ANCHO: u32 = 52;
const VIT_PCT_ALTO: u32 = 46;
const VIT_MIN_ANCHO: u32 = 460;
const VIT_MIN_ALTO: u32 = 240;

const VIT_FONDO: u32 = 0x0008_0C10;
const VIT_TITULO_FONDO: u32 = 0x000E_161B;
const VIT_BORDE: u32 = 0x001B_3340;
/// El mismo cian del gato que usa CABINA. Se repite el valor en vez de
/// importarlo porque el de `cabina.rs` es privado suyo, y hacerlo publico solo
/// para esto ataria dos ventanas que no tienen nada que ver.
const VIT_CIAN: u32 = 0x0034_E2E4;

/// Cual de las dos vistas.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Cual {
    /// F7 -- el CPU: a que va, que gasta, quien esta despierto.
    Cpu,
    /// F8 -- la memoria: cuanta hay, y **quien se la esta comiendo**.
    Memoria,
}

pub(crate) struct CajaVitales {
    pub(crate) marco: Marco,
    pub(crate) cual: Cual,
}

impl CajaVitales {
    pub(crate) fn nueva(p: &bmo::Pantalla, cual: Cual) -> Self {
        Self {
            marco: Marco::nuevo(p, VIT_PCT_ANCHO, VIT_PCT_ALTO, VIT_MIN_ANCHO, VIT_MIN_ALTO),
            cual,
        }
    }
}

fn poner(s: &[u8], dst: &mut [u8], n: &mut usize) {
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
    poner(&d[..k], dst, n);
}

/// Un numero con un decimal: `59.5`. Sin coma flotante, que aqui no hay.
fn con_decimal(entero: u64, milesimas: u64, dst: &mut [u8], n: &mut usize) {
    num(entero, dst, n);
    poner(b".", dst, n);
    num(milesimas, dst, n);
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
        con_decimal(bytes / MIB, (bytes % MIB) * 10 / MIB, dst, n);
        poner(b" MiB", dst, n);
    } else if bytes >= KIB {
        con_decimal(bytes / KIB, (bytes % KIB) * 10 / KIB, dst, n);
        poner(b" KiB", dst, n);
    } else {
        num(bytes, dst, n);
        poner(b" B", dst, n);
    }
}

/// Una barra `[####----]` de `ancho` casillas.
fn barra(usado: u64, total: u64, ancho: usize, dst: &mut [u8], n: &mut usize) {
    poner(b"[", dst, n);
    let llenas = if total == 0 { 0 } else { (usado * ancho as u64 / total) as usize };
    for i in 0..ancho {
        poner(if i < llenas { b"#" } else { b"-" }, dst, n);
    }
    poner(b"]", dst, n);
}

pub(crate) fn pintar(p: &bmo::Pantalla, c: &CajaVitales) {
    if c.marco.minimizada {
        return;
    }
    c.marco.pintar_cromo(p, VIT_BORDE, VIT_FONDO, VIT_TITULO_FONDO, VIT_CIAN);
    c.marco.pintar_botones(p, VIT_TITULO_FONDO);

    let tx = c.marco.x + 16;
    p.texto(
        tx,
        c.marco.y + 8,
        match c.cual {
            Cual::Cpu => "CPU",
            Cual::Memoria => "MEMORIA",
        },
        VIT_CIAN,
    );
    p.texto(
        tx + 90,
        c.marco.y + 8,
        match c.cual {
            Cual::Cpu => "a que va y que gasta",
            Cual::Memoria => "quien se la esta comiendo",
        },
        TEXTO_TENUE,
    );

    let mut y = c.marco.y + TITULO_ALTO + 10;
    let paso = bmo::GLIFO_ALTO + 4;
    let mut b = [0u8; 96];

    match c.cual {
        Cual::Cpu => pintar_cpu(p, c, tx, &mut y, paso, &mut b),
        Cual::Memoria => pintar_memoria(p, c, tx, &mut y, paso, &mut b),
    }

    // La barra de abajo dice lo unico que hay que saber para usarla.
    p.texto(
        tx,
        c.marco.y + c.marco.alto - bmo::GLIFO_ALTO - 8,
        "se repinta sola   ESC cierra   arrastra el titulo",
        TEXTO_TENUE,
    );
}

/// Una fila `etiqueta   valor`, con la etiqueta a ancho fijo.
fn fila(p: &bmo::Pantalla, x: u32, y: u32, etiq: &str, b: &[u8], tinta: u32) {
    p.texto(x, y, etiq, TEXTO_TENUE);
    if let Ok(s) = core::str::from_utf8(b) {
        p.texto(x + 130, y, s, tinta);
    }
}

fn pintar_cpu(p: &bmo::Pantalla, c: &CajaVitales, tx: u32, y: &mut u32, paso: u32, b: &mut [u8; 96]) {
    let _ = c;

    // ** LO PRIMERO ES QUE SABE MEDIR, y no una medida.
    //
    // Igual que en `info`: las filas de abajo pueden salir vacias por dos
    // motivos que se ven igual, y sin esta no se distinguen.
    let sensores = bmo::info(bmo::INFO_CPU_SENSORES);
    let mut n = 0usize;
    if sensores == 0 {
        poner(b"nada: el perfil no declara sensores", b, &mut n);
    } else {
        if sensores & 1 != 0 {
            poner(b"frecuencia", b, &mut n);
        }
        if sensores & 3 == 3 {
            poner(b" + ", b, &mut n);
        }
        if sensores & 2 != 0 {
            poner(b"consumo", b, &mut n);
        }
    }
    fila(p, tx, *y, "mide", &b[..n], TEXTO_TENUE);
    *y += paso;

    let hz = bmo::info(bmo::INFO_TSC_HZ);
    let real = bmo::info(bmo::INFO_CPU_HZ_REAL);
    let mut n = 0usize;
    if real == 0 {
        poner(b"-- (aun sin dos lecturas)", b, &mut n);
    } else {
        con_decimal(real / 1_000_000_000, (real % 1_000_000_000) / 100_000_000, b, &mut n);
        poner(b" GHz   de ", b, &mut n);
        con_decimal(hz / 1_000_000_000, (hz % 1_000_000_000) / 100_000_000, b, &mut n);
        poner(b" base", b, &mut n);
    }
    fila(p, tx, *y, "va a", &b[..n], if real > hz { TEXTO_BIEN } else { TEXTO });
    *y += paso;

    let mw = bmo::info(bmo::INFO_CPU_MW_PAQUETE);
    let mut n = 0usize;
    if mw == 0 {
        poner(b"-- (sin RAPL)", b, &mut n);
    } else {
        con_decimal(mw / 1000, (mw % 1000) / 100, b, &mut n);
        poner(b" W el paquete entero", b, &mut n);
    }
    fila(p, tx, *y, "gasta", &b[..n], TEXTO);
    *y += paso;

    // [!] Y el de ESTE nucleo va aparte y con su nombre completo. Ver el
    // comentario de `INFO_CPU_MW_NUCLEO_ACTUAL`: llamarlo "nucleos" hizo que un
    // numero correcto se leyera como una mentira.
    let mwn = bmo::info(bmo::INFO_CPU_MW_NUCLEO_ACTUAL);
    let mut n = 0usize;
    if mwn == 0 {
        poner(b"--", b, &mut n);
    } else {
        con_decimal(mwn / 1000, (mwn % 1000) / 100, b, &mut n);
        poner(b" W   (solo el que lee: los otros no se ven)", b, &mut n);
    }
    fila(p, tx, *y, "este nucleo", &b[..n], TEXTO_TENUE);
    *y += paso + 6;

    let vivos = bmo::info(bmo::INFO_SMP_VIVOS);
    let mut n = 0usize;
    if vivos == 0 {
        poner(b"solo el BSP    (`smp all` levanta los demas)", b, &mut n);
    } else {
        num(vivos + 1, b, &mut n);
        poner(b" en pie de ", b, &mut n);
        num(bmo::info(bmo::INFO_CPU_HILOS), b, &mut n);
    }
    fila(p, tx, *y, "nucleos", &b[..n], if vivos == 0 { TEXTO_TENUE } else { TEXTO_BIEN });
    *y += paso;

    let mut n = 0usize;
    num(bmo::info(bmo::INFO_TAREAS_TOTAL), b, &mut n);
    poner(b" tareas, ", b, &mut n);
    num(bmo::info(bmo::INFO_TAREAS_LISTAS), b, &mut n);
    poner(b" listas", b, &mut n);
    fila(p, tx, *y, "planificador", &b[..n], TEXTO);
}

fn pintar_memoria(p: &bmo::Pantalla, c: &CajaVitales, tx: u32, y: &mut u32, paso: u32, b: &mut [u8; 96]) {
    let total = bmo::info(bmo::INFO_RAM_TOTAL);
    let libre = bmo::info(bmo::INFO_RAM_LIBRE);
    let usada = total.saturating_sub(libre);

    let mut n = 0usize;
    tam(usada, b, &mut n);
    poner(b" de ", b, &mut n);
    tam(total, b, &mut n);
    poner(b"  ", b, &mut n);
    barra(usada, total, 20, b, &mut n);
    fila(p, tx, *y, "usada", &b[..n], TEXTO);
    *y += paso + 8;

    // ** LA TABLA: QUIEN SE LA ESTA COMIENDO.
    //
    // Es la vista que el dueno pidio, y la que hasta hoy no se podia hacer: el
    // kernel sabia cuanto come el proceso N desde julio, y no habia forma de
    // preguntar QUIENES son sin saber sus pids de antemano. Ver
    // `obj::memoria::ranura`.
    p.texto(tx, *y, "pid", TEXTO_TENUE);
    p.texto(tx + 70, *y, "pedido", TEXTO_TENUE);
    p.texto(tx + 210, *y, "veces", TEXTO_TENUE);
    *y += paso;

    let mut fila_n = 0usize;
    let mut suma = 0u64;
    // El tope es el de la tabla del kernel: pedir mas seria preguntar por
    // ranuras que no existen, y el kernel contestaria 0 -- que aqui se lee
    // igual que "no hay mas".
    while fila_n < 16 {
        let campo = |base: u64| base | ((fila_n as u64) << 8);
        let pid = bmo::info(campo(bmo::INFO_MEM_QUIEN_PID));
        if pid == 0 {
            break;
        }
        let bytes = bmo::info(campo(bmo::INFO_MEM_QUIEN_BYTES));
        let veces = bmo::info(campo(bmo::INFO_MEM_QUIEN_PETICIONES));
        suma += bytes;

        let mut n = 0usize;
        num(pid, b, &mut n);
        if let Ok(s) = core::str::from_utf8(&b[..n]) {
            p.texto(tx, *y, s, TEXTO);
        }
        let mut n = 0usize;
        tam(bytes, b, &mut n);
        if let Ok(s) = core::str::from_utf8(&b[..n]) {
            p.texto(tx + 70, *y, s, TEXTO);
        }
        let mut n = 0usize;
        num(veces, b, &mut n);
        // Las peticiones se pintan en ambar al acercarse al tope: cuatro es el
        // maximo y la quinta la niega el kernel. Un programa en 3 no esta roto,
        // pero es el que hay que mirar si algo falla al pedir.
        if let Ok(s) = core::str::from_utf8(&b[..n]) {
            p.texto(tx + 210, *y, s, if veces >= 3 { TEXTO_MAL } else { TEXTO_TENUE });
        }
        *y += paso;
        fila_n += 1;
    }

    if fila_n == 0 {
        // Y esto NO es un fallo: significa que ningun programa de Ring 3 ha
        // pedido memoria con `KIND_MEMORIA`. Decirlo con palabras evita leer una
        // tabla vacia como una tabla rota.
        p.texto(tx, *y, "nadie ha pedido memoria todavia", TEXTO_TENUE);
        *y += paso;
    } else {
        *y += 6;
        let mut n = 0usize;
        tam(suma, b, &mut n);
        poner(b" entre ", b, &mut n);
        num(fila_n as u64, b, &mut n);
        poner(b" procesos", b, &mut n);
        fila(p, tx, *y, "suman", &b[..n], TEXTO_BIEN);
    }

    let _ = c;
}
