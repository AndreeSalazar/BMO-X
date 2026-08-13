//! **Lo que NO SE DESHACE.** `ktest`, `panic`, `reboot` y `halt`.
//!
//! # Por que estas cuatro tienen fichero propio siendo veintiseis lineas
//!
//! Porque el tamano no es el criterio. Estas cuatro comparten la unica
//! propiedad que de verdad importa al ordenar un shell: **despues de ellas no
//! se sigue**. Una reinicia, otra para la maquina, otra provoca un fault a
//! proposito y la cuarta lanza un hilo que no muere.
//!
//! Tenerlas juntas y al final hace visible una regla que antes habia que
//! recordar: si estas anadiendo algo aqui, para y pregunta si de verdad va
//! aqui. Un fichero de veintiseis lineas que evita eso una sola vez ya se pago.
//!
//! # Y por que `panic` existe
//!
//! Porque un manejador de faults que nadie ha disparado a proposito es un
//! manejador sin probar. `panic` es la unica forma de saber que el camino de
//! la autopsia funciona **antes** de necesitarlo.

use super::super::phase::s_log;

// El CUERPO del hilo de prueba viaja con la orden que lo lanza: era su unico
// llamante, y una funcion que solo existe para un comando pertenece al fichero
// de ese comando.

pub(crate) fn shell_ktest() {
    match crate::ring0::task::scheduler::spawn_kernel(ktest_main as usize as u64, 0xB0, 1) {
        Some(tid) => {
            crate::ring0::dev::console::serial_write("[ktest] spawned tid=");
            crate::ring0::dev::console::serial_write_u64(tid as u64, 10);
            crate::ring0::dev::console::serial_write("\n");
        }
        None => s_log("[ktest] spawn failed (no frames or task slots)"),
    }
}

pub(crate) fn shell_panic() -> ! {
    s_log("[shell] triggering test panic...");
    panic!("intentional panic from serial shell");
}

pub(crate) fn shell_reboot() -> ! {
    // El pulso del 8042 a secas no reiniciaba nada en esta placa --su i8042
    // solo entrega ruido-- asi que el comando se quedaba colgado en el `hlt`
    // de despues. `reinicio::ahora` prueba 0xCF9, luego el 8042, y si nada
    // funciona provoca un triple fault, que no depende de ningun chipset.
    s_log("[shell] reboot");
    crate::ring0::plat::reinicio::ahora();
}

pub(crate) fn shell_halt() -> ! {
    s_log("[shell] halting");
    loop { unsafe { core::arch::asm!("sti; hlt"); } }
}

extern "C" fn ktest_main(arg: u64) -> ! {
    use crate::ring0::dev::console::{serial_write, serial_write_u64};
    serial_write("[ktest] start tid=");
    serial_write_u64(crate::ring0::task::scheduler::current_tid() as u64, 10);
    serial_write(" arg=");
    serial_write_u64(arg, 10);
    serial_write("\n");
    for i in 0..3u64 {
        serial_write("[ktest] window ");
        serial_write_u64(i, 10);
        serial_write("\n");
        // Busy window ~250 ms so the timer preempts us several times and
        // the shell task runs in between (look for the '>' echoes).
        let start = crate::ring0::task::scheduler::rdtsc();
        let span = crate::ring0::task::scheduler::tsc_freq() / 4;
        while crate::ring0::task::scheduler::rdtsc().wrapping_sub(start) < span {
            core::hint::spin_loop();
        }
    }
    serial_write("[ktest] park 2000 ms (WAIT deadline)\n");
    let deadline = crate::ring0::task::scheduler::rdtsc()
        + crate::ring0::task::scheduler::ns_to_tsc(2_000_000_000);
    crate::ring0::task::scheduler::park_until(deadline);
    serial_write("[ktest] woke; exit via reaper\n");
    crate::ring0::task::scheduler::exit_and_park();
}