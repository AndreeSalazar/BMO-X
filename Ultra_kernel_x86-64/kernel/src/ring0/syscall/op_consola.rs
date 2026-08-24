//! **LA CONSOLA**: escribir en ella y leer de ella.
//!
//! ## Por que sale del despachador (L6b)
//!
//! Porque es la unica pareja del `match` que habla con una PANTALLA, y porque
//! `CONSOLE_WRITE` es ademas la operacion mas caliente del sistema: tiene su
//! propia via rapida en `despachar`, antes de resolver ningun handle.
//!
//! ** Tenerla aqui deja el despachador con lo que es --una tabla de a quien le
//! toca-- y pone el cuerpo donde se puede leer entero sin bajar por cuarenta
//! brazos mas.
//!
//! ## [!] Esto NO es un reparto puro de L6d, y se dice
//!
//! El cuerpo se movio tal cual; el brazo paso a ser una llamada.

use super::*;

//// Bootstrap console: render up to 8 packed bytes (LE, NUL-stop) to
//// the kernel's on-screen log + serial. This is how the first Ring 3
//// program draws -- the whole point of the CPL3->CPL0 demo. It writes
//// nothing but text and cannot escalate; the caller only ever paints
//// into the kernel-owned console surface.
//// La salida va a la consola ASIGNADA al proceso, si tiene una -- o al
//// panel del kernel si no, exactamente como antes. Lo nuevo rodea a lo
//// viejo en vez de romperlo: los cinco demos embebidos siguen hablando
//// por el panel sin cambiar una linea.
pub(super) fn console_write(arg0: u64, arg1: u64) -> BmoStatus {
        let pid = scheduler::current_pid();
        match crate::ring0::obj::console::output_of(pid) {
            Some(idx) => {
                // Desempaquetar aqui: el anillo guarda bytes, no palabras.
                // El cero corta, igual que en la consola del kernel.
                let w = arg0.to_le_bytes();
                let n = w.iter().position(|&b| b == 0).unwrap_or(8);
                crate::ring0::obj::console::write(idx, &w[..n]);
                // ** Y TAMBIEN AL ANILLO DEL KERNEL, que es la caja negra.
                //
                // Esto es una bifurcacion de ENTREGA, no de registro: la
                // consola asignada decide **quien lo lee en vivo**; el
                // anillo de `uconsole` es lo que el kernel se acuerda de que
                // dijo cada proceso, y eso no puede depender de a quien se
                // lo estuviera diciendo.
                //
                // Se cobro el 2026-08-14: DOOM murio con `#GP` tras imprimir
                // VEINTE lineas --hasta `I_Init: Setting up machine state.`--
                // y su autopsia decia:
                //
                //     ultimo    (no escribio nada)
                //
                // Falso, y de la peor clase: no callaba, **afirmaba**. Su
                // salida iba a la consola hija que le creo el escritorio, y
                // el anillo del kernel no la veia pasar. Un informe que dice
                // "no dijo nada" manda a mirar donde no es.
                crate::ring0::uconsole::write_packed(arg0);
            }
            None => crate::ring0::uconsole::write_packed(arg0),
        }
        BmoStatus::ok_value(0)
}

pub(super) fn console_read(arg0: u64, arg1: u64) -> BmoStatus {
        let _ = arg0;
        let pid = scheduler::current_pid();
        match crate::ring0::obj::console::output_of(pid) {
            Some(idx) => BmoStatus::ok_value(crate::ring0::obj::console::read_entry(idx)),
            // Sin consola asignada no hay de donde leer. Cero = "nada", no
            // error: un programa que sondea no debe morir por preguntar.
            None => BmoStatus::ok_value(0),
        }
}

