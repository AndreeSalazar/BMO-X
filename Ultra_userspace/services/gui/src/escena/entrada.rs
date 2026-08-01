//! **La entrada a Ring 3** — lo que se ve cuando el userspace toma la máquina.
//!
//! ═══ Por qué existe ═══
//!
//! Hasta ahora el paso de Ring 0 a Ring 3 era **invisible**: el kernel dejaba
//! de pintar, el compositor limpiaba la pantalla y aparecía un escritorio. Si
//! algo fallaba en medio, lo que quedaba era un shell — y nadie podía decir si
//! el compositor no había arrancado, si había arrancado y muerto, o si estaba
//! vivo y no pintaba.
//!
//! Esta pantalla es el momento **dicho en voz alta**: quién toma la máquina,
//! qué le acaban de ceder, y sobre qué corre. No es adorno: cada línea es un
//! dato que, cuando falta, es exactamente lo que hay que preguntar.
//!
//! ═══ Está CRONOMETRADA, no contada en vueltas ═══
//!
//! Ring 3 no tiene reloj en los tres syscalls… pero `RDTSC` no es privilegiada
//! y el kernel publica la frecuencia medida (`INFO_TSC_HZ`). Con eso, una
//! espera de 900 ms es de 900 ms **en esta máquina y en la siguiente**. Contar
//! vueltas del bucle habría dado una intro de dos segundos en un Ryzen y de
//! veinte en algo más lento — que es como se hacían las cosas cuando no había
//! forma de saber la hora, y aquí sí la hay.

use bmo_userland as bmo;

use super::*;
use crate::texto::decimal;

const ENT_FONDO: u32 = 0x000A_0E17;
const ENT_TENUE: u32 = 0x0059_6B8A;

/// Lee el contador de ciclos del CPU. No es privilegiada: Ring 3 puede.
#[inline]
fn ciclos() -> u64 {
    let (hi, lo): (u32, u32);
    unsafe {
        core::arch::asm!("rdtsc", out("edx") hi, out("eax") lo, options(nomem, nostack));
    }
    ((hi as u64) << 32) | lo as u64
}

/// Espera exacta, cediendo el CPU mientras tanto.
///
/// ★ Cede en el bucle a propósito: un `spin` de 900 ms en un sistema
/// preemptivo es 900 ms robados al resto de las tareas. Aquí la espera es del
/// que mira, no del que calcula.
fn esperar_ms(ms: u64) {
    let hz = bmo::info(bmo::INFO_TSC_HZ);
    if hz == 0 {
        // Sin frecuencia medida no se inventa una: se sigue. Una intro que no
        // se ve es infinitamente mejor que una espera de duración desconocida.
        return;
    }
    let objetivo = ciclos() + hz / 1000 * ms;
    while ciclos() < objetivo {
        bmo::ceder();
    }
}

/// Una fila del informe: etiqueta a la izquierda, valor a la derecha.
fn fila(p: &bmo::Pantalla, x: u32, y: u32, etiqueta: &str, valor: &str, color: u32) {
    p.texto(x, y, etiqueta, ENT_TENUE);
    p.texto(x + 13 * bmo::GLIFO_ANCHO, y, valor, color);
}

/// **La entrada.** Se pinta entera, se lee, y se va.
///
/// `hay_entrada` y `hay_consola` no son decoración: son las dos capabilities
/// que el compositor puede no recibir, y sin las cuales el escritorio arranca
/// igual pero **quieto y mudo**. Que se digan aquí es lo que distingue "no
/// funciona" de "no me la dieron".
pub(crate) fn pintar(p: &bmo::Pantalla, hay_entrada: bool, hay_consola: bool) {
    p.limpiar(ENT_FONDO);

    // Una banda de acento a la izquierda, de arriba abajo. Sujeta la
    // composición y cuesta un rectángulo.
    p.rect(0, 0, 6, p.alto, ACENTO);

    let x = 120;
    let mut y = p.alto / 2 - 190;

    // ── El nombre, grande ──
    let ancho = bmo::Pantalla::ancho_escala("BMO-X", 6);
    p.texto_escala(x, y, "BMO-X", TEXTO, 6);
    // Subrayado exacto bajo el título: el ancho se pregunta, no se estima.
    p.rect(x, y + 16 * 6 + 8, ancho, 3, ACENTO);
    y += 16 * 6 + 30;

    p.texto(x, y, "RING 3   ·   el userspace toma la maquina", ACENTO);
    y += bmo::GLIFO_ALTO + 34;

    // ── Lo que se acaba de ceder ──
    p.texto(x, y, "SE ME HA CEDIDO", ENT_TENUE);
    y += bmo::GLIFO_ALTO + 10;

    // La pantalla: siempre está, porque sin ella no habría nada que leer.
    let mut b = [0u8; 10];
    let mut n = decimal(p.ancho as u64, &mut b);
    p.texto(x, y, "la pantalla", ENT_TENUE);
    let mut px = p.texto_bytes(x + 13 * bmo::GLIFO_ANCHO, y, &b[..n], TEXTO);
    px = p.texto(px, y, " x ", ENT_TENUE);
    n = decimal(p.alto as u64, &mut b);
    px = p.texto_bytes(px, y, &b[..n], TEXTO);
    p.texto(px, y, "   y el kernel deja de pintar", ENT_TENUE);
    y += bmo::GLIFO_ALTO + 6;

    if hay_entrada {
        fila(p, x, y, "la entrada", "teclado y raton son mios", TEXTO_BIEN);
    } else {
        fila(p, x, y, "la entrada", "NO: el escritorio sera mudo", TEXTO_MAL);
    }
    y += bmo::GLIFO_ALTO + 6;

    if hay_consola {
        fila(p, x, y, "una consola", "lo que yo lance escribe AQUI", TEXTO_BIEN);
    } else {
        fila(p, x, y, "una consola", "NO: los hijos escribiran en el kernel", TEXTO_MAL);
    }
    y += bmo::GLIFO_ALTO + 28;

    // ── Sobre qué corre ──
    p.texto(x, y, "SOBRE", ENT_TENUE);
    y += bmo::GLIFO_ALTO + 10;

    let mut cpu = [0u8; 48];
    let ncpu = bmo::info_texto(bmo::INFO_TXT_CPU_NOMBRE, &mut cpu);
    if ncpu > 0 {
        p.texto(x, y, "cpu", ENT_TENUE);
        let mut cx = p.texto_bytes(x + 13 * bmo::GLIFO_ANCHO, y, &cpu[..ncpu], TEXTO);
        let hilos = bmo::info(bmo::INFO_CPU_HILOS);
        if hilos > 0 {
            cx = p.texto(cx, y, "   ", ENT_TENUE);
            n = decimal(hilos, &mut b);
            cx = p.texto_bytes(cx, y, &b[..n], TEXTO);
            cx = p.texto(cx, y, " hilos", ENT_TENUE);
        }
        let hz = bmo::info(bmo::INFO_TSC_HZ);
        if hz > 0 {
            cx = p.texto(cx, y, " a ", ENT_TENUE);
            // Dos decimales de GHz sin coma flotante, igual que el resto.
            n = decimal(hz / 1_000_000_000, &mut b);
            cx = p.texto_bytes(cx, y, &b[..n], TEXTO);
            cx = p.texto(cx, y, ".", TEXTO);
            let cent = (hz % 1_000_000_000) / 10_000_000;
            if cent < 10 {
                cx = p.texto(cx, y, "0", TEXTO);
            }
            n = decimal(cent, &mut b);
            cx = p.texto_bytes(cx, y, &b[..n], TEXTO);
            p.texto(cx, y, " GHz medidos", ENT_TENUE);
        }
        y += bmo::GLIFO_ALTO + 6;
    }

    // La memoria, con el número que a esta máquina le gusta enseñar: cuánto
    // ocupa el sistema entero.
    let total = bmo::info(bmo::INFO_RAM_TOTAL);
    let libre = bmo::info(bmo::INFO_RAM_LIBRE);
    if total > 0 {
        p.texto(x, y, "memoria", ENT_TENUE);
        let mut cx = x + 13 * bmo::GLIFO_ANCHO;
        n = decimal(total.saturating_sub(libre) / (1024 * 1024), &mut b);
        cx = p.texto_bytes(cx, y, &b[..n], TEXTO);
        cx = p.texto(cx, y, " MiB en uso de ", ENT_TENUE);
        n = decimal(total / (1024 * 1024 * 1024), &mut b);
        cx = p.texto_bytes(cx, y, &b[..n], TEXTO);
        p.texto(cx, y, " GiB", ENT_TENUE);
        y += bmo::GLIFO_ALTO + 6;
    }

    if bmo::info(bmo::INFO_DATOS_MONTADO) != 0 {
        fila(p, x, y, "disco", "volumen de datos montado", TEXTO_BIEN);
    } else {
        fila(p, x, y, "disco", "SIN volumen de datos", TEXTO_MAL);
    }
    y += bmo::GLIFO_ALTO + 34;

    // ── La frase, que es la tesis del proyecto ──
    p.rect(x, y, 560, 1, 0x0022_3040);
    y += 14;
    p.texto(x, y, "tres syscalls congelados.  todo lo demas son capabilities.", ENT_TENUE);
    y += bmo::GLIFO_ALTO + 4;
    p.texto(x, y, "esto no es una API prestada: es la maquina obedeciendo.", ENT_TENUE);

    // Se deja leer. Ver la cabecera: es tiempo REAL, no vueltas de bucle.
    esperar_ms(1100);
}
