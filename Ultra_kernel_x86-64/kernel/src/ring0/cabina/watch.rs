//! **THE WATCHES** -- and yes, this is polling, with a reason.
//!
//! [carril]  AMARILLO  las vigilancias, y son sondeo con motivo escrito
//!
//! === Why this is a file of its own ===
//!
//! Because everything else in CABINA is passive: somebody calls `info` and an
//! event is recorded. These are the opposite -- they go and LOOK, on a timer,
//! at things that never announce themselves.
//!
//! ** A device that stops answering does not send an event saying so. That is
//! the whole argument for polling here, and it is worth having in one place
//! where it can be defended rather than scattered where it looks like an
//! oversight.

use super::*;

// -- Vigilancias (esto SI es polling, y con razon) ---------------------------

/// Condiciones que se sostienen en el tiempo y no tienen un "instante" en que
/// alguien pueda emitirlas: hay que mirarlas cada tanto. Cada una se narra UNA
/// vez (de-dup por flag). Llamado desde `render_hud`.
pub(crate) fn watch(s: &TelemetrySnapshot, mib_free: u64) {
    static mut W_KBD_MUDO: bool = false;
    static mut W_MEMLOW: bool = false;
    unsafe {
        // ** ESTO ERA UN `fault` Y ERA UNA FALSA ALARMA (2026-08-11).
        //
        // La condicion es `kev == 0`: **cero teclas pulsadas**. Y eso tambien es
        // cierto cuando nadie ha tocado el teclado todavia. La maquina arranca,
        // pasan dos segundos sin que el dueno escriba, y CABINA declaraba un
        // fallo de hardware que no existia.
        //
        // Las fotos del 2026-08-11 lo prueban solas: el FAULT sale en `t02002` y
        // despues llega `primera tecla recibida: el teclado WRITES`. El teclado
        // estaba perfecto. El que no habia escrito era el usuario.
        //
        // === Por que no se puede arreglar la condicion, solo el mensaje ===
        //
        // **Ninguna observacion pasiva distingue "roto" de "nadie ha escrito".**
        // Se miro si servia el contador de transferencias del xHC (`tev`): no
        // sirve, porque es global y el raton lo sube el solo. Y "entregaba y
        // dejo de hacerlo" tampoco vale: dejar de escribir es lo normal.
        //
        // Asi que se dice el HECHO y no el veredicto, que es la regla de toda
        // esta semana. Lo que se sabe es *"enumero y todavia no ha llegado
        // ninguna"*; lo que NO se sabe es por que.
        //
        // === Y por que importa mas de lo que parece ===
        //
        // Un FAULT falso no es un fallo de mas: **gasta el credito de todos los
        // demas**. Quien lee la caja negra y encuentra una linea roja que no era
        // verdad empieza a dudar de las que si lo son -- y esta salia en TODOS
        // los arranques, la primera de la lista.
        //
        // Una caja negra que grita lobo es peor que una callada.
        if !W_KBD_MUDO {
            let (kbd, _m, _ks, _ms, _mev, _x, _y, _b, kev) = crate::ring0::dev::usb::hid_stats();
            if kbd && kev == 0 && s.cpu.timer_ticks > 0x2000 {
                W_KBD_MUDO = true;
                let (kdci, _, _, _) = crate::ring0::dev::usb::kbd_debug();
                info("usb", "el teclado enumero y aun no ha llegado ninguna tecla", kdci as u64);
            }
        }
        if mib_free < 256 && !W_MEMLOW {
            W_MEMLOW = true;
            warn("mem", "RAM libre por debajo de 256MiB", mib_free);
        }

        // ** THE BUS THREAD, WATCHED -- and this one CAN be told apart.
        //
        // The keyboard watch above cannot distinguish "broken" from "nobody has
        // typed". This one can, and that difference is what makes it worth
        // having: the bus thread beats 250 times a second **whether or not
        // anybody touches anything**. So if its counter is the same as it was
        // last time this ran, there is no innocent explanation. It died, it never
        // started, or something is holding the CPU without ever yielding.
        //
        // And it matters more than it looks: the day that counter stops, the
        // keyboard goes back to depending on whoever holds the input asking for
        // keys -- which is the freeze this whole change exists to remove. Without
        // this line the system would fall back into the old bug **silently**, and
        // the symptom would be blamed on the keyboard again.
        //
        // De-duplicated by flag: said once, not sixty times a second.
        static mut W_BUS_PARADO: bool = false;
        static mut ULTIMA_VUELTA: u64 = 0;
        static mut VISTO_EN_TICK: u64 = 0;
        if !W_BUS_PARADO {
            let (turns, _) = crate::ring0::dev::usb::bus_stats();
            let ahora = s.cpu.timer_ticks;
            // Only judged after a real span of ticks, and only once the thread
            // has been seen alive at least once: a zero at boot is "not started
            // yet", which is a different sentence and would be a false alarm --
            // the exact mistake the keyboard watch above already paid for.
            if turns > 0 && ULTIMA_VUELTA == 0 {
                ULTIMA_VUELTA = turns;
                VISTO_EN_TICK = ahora;
            } else if ULTIMA_VUELTA > 0 && ahora.wrapping_sub(VISTO_EN_TICK) > 0x400 {
                if turns == ULTIMA_VUELTA {
                    W_BUS_PARADO = true;
                    fault("usb", "el hilo del bus DEJO DE LATIR: vuelta", turns);
                } else {
                    ULTIMA_VUELTA = turns;
                    VISTO_EN_TICK = ahora;
                }
            }
        }
    }
}
