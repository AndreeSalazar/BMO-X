//! **Asking the USB audio device how it wants its samples.** Nothing else.
//!
//! Step 0 of `docs/AUDIO_MAESTRO.md`, kernel side. The decision --reading the
//! descriptor-- lives in `bmo_uaudio::stream`, where it is tested; here there is
//! only what no test can cover: touching the bus and printing.
//!
//! # Why its own file, next to `bus` and `rescate`
//!
//! Because it is a fourth job and it does not belong to any of the other three.
//! `mod.rs` is the bridge to the xHC for HID; this is a different device class
//! that happens to hang off the same controller. Putting it in `mod.rs` would be
//! re-doing exactly what was undone on 2026-08-12.
//!
//! # ** IT IS A TYPED COMMAND, NOT A BOOT STEP, AND THAT IS THE WHOLE SAFETY
//!
//! Enumerating a port RESETS it. Doing that to an already-working keyboard kills
//! it -- the mine is documented in `bmo_uhid::puertos`: *"resetear un puerto ES
//! un cambio de puerto, asi que el aviso que la dispara lo genera ella misma,
//! para siempre. Peor: el puerto que giraba era el del teclado ya enumerado, que
//! moria con el primer reset."*
//!
//! So this only ever touches ports **that `bmo_uhid` did not take**. A port with
//! a keyboard on it is never addressed from here, and that is checked and not
//! remembered.
//!
//! And it runs from the `audio` command and not from boot, same as `smp` and
//! `net rx`: if something goes wrong, what hangs is a command and not the
//! machine at power-on.

use crate::ring0::cabina;

/// **Walks the untaken ports and reports the first playback pipe it finds.**
///
/// Returns `true` if it found one. Everything it learns goes to CABINA, because
/// the point of this step is a photograph that can be compared against what the
/// other operating system on this machine says about the same headset -- the
/// same method that answered the July `#GP` and the NIC's MAC.
///
/// # Safety
/// Touches xHC MMIO: has to run with the kernel CR3 loaded. The `audio` command
/// goes through [`super::pump_bus`]'s wrapper for that reason.
pub unsafe fn censar() -> bool {
    let ctrl = match bmo_xhci::controller() {
        Some(c) => c,
        None => {
            cabina::warn("audio", "no hay controlador xHCI: no hay a quien preguntar", 0);
            return false;
        }
    };
    let max_ports = ctrl.max_ports;

    let mut mirados = 0u64;
    for port in 0..max_ports {
        // ** LOS PUERTOS DE OTRO NO SE TOCAN. Ver la cabecera.
        let ocupado = {
            let hid = &*core::ptr::addr_of!(super::HID);
            hid.puertos().tomado(port)
        };
        if ocupado {
            continue;
        }
        let slot = match bmo_uhid::enumera::direccionar_puerto(port) {
            Some(s) => s,
            None => continue,
        };
        mirados += 1;
        let mut cfg = [0u8; bmo_uhid::enumera::MAX_CFG];
        let largo = match bmo_uhid::enumera::leer_descriptores(slot, &mut cfg) {
            Some((_, n)) => n,
            None => {
                cabina::warn("audio", "un aparato no dejo leer su descriptor, puerto", port as u64);
                continue;
            }
        };
        let Some(p) = bmo_uaudio::stream::find_playback(&cfg[..largo]) else {
            continue;
        };

        // -- The four numbers. Each with its unit, so nobody converts by hand.
        cabina::count("audio", "interfaz AudioStreaming, alt", p.alt_setting as u64);
        cabina::count("audio", "canales", p.channels as u64);
        cabina::count("audio", "bits por muestra", p.bits as u64);
        cabina::bytes("audio", "bytes por trama (wMaxPacketSize)", p.max_packet as u64);
        for i in 0..p.rates().len() {
            cabina::count("audio", "frecuencia que acepta", p.rates()[i] as u64);
        }

        // ** Y EL NUMERO QUE DECIDE SI ALGO VA A SONAR.
        //
        // Una trama de la frecuencia elegida tiene que CABER en el paquete. Si
        // no cabe, no hay codigo correcto que lo arregle -- y decirlo aqui evita
        // buscar el fallo en el driver que todavia no existe.
        match p.best_rate(48000) {
            Some(r) => {
                cabina::count("audio", "frecuencia elegida", r as u64);
                cabina::bytes("audio", "y una trama suya ocupa", p.bytes_per_interval(r) as u64);
            }
            None => {
                cabina::fault("audio", "ninguna frecuencia suya cabe en su propio paquete", 0);
            }
        }
        cabina::count("audio", "el endpoint isocrono es el DCI", p.dci as u64);
        return true;
    }

    // Decir "no hay" y decir "no mire" son cosas distintas, y sin el numero de
    // puertos mirados se ven igual.
    cabina::count("audio", "puertos libres mirados, y ninguno reproduce", mirados);
    false
}
