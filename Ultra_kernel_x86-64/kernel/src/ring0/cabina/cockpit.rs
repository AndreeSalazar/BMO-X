//! **THE COCKPIT** -- what CABINA looks like on the screen.
//!
//! === Why this is a file of its own, and it is the biggest ===
//!
//! Because it is the only part that is about PRESENTATION: severity colours,
//! filters, the layout of the bands, which rows belong to the panel and which
//! to the rolling log. None of it changes what is recorded, and all of it
//! changes what a person sees at 3am with a broken machine.
//!
//! Keeping it apart from the recorder is what makes the recorder auditable: a
//! change here can be judged by eye, and a change there cannot.

use super::*;

// -- Cockpit -----------------------------------------------------------------
//
// CABINA vive en la BANDA INFERIOR del panel; el log rodante del kernel/shell
// se queda con la banda de arriba. Antes ambos escribian en las filas 2-13 y
// se borraban mutuamente (el panel estaba fijado en 14 filas aunque en 1080p
// caben ~49). Ahora el reparto se calcula del alto real del framebuffer.

/// Filas que ocupa el cockpit dentro de un panel de `total` filas: cabecera +
/// bitacora + 3 de telemetria. Un tercio del panel, acotado.
pub fn band_rows(total: usize) -> usize {
    // Panel diminuto: dejar SIEMPRE 2 filas al log rodante. Nunca devolver mas
    // que `total` -- quien llama resta esto y una resta negativa en `usize` da
    // la vuelta (bucle de millones de filas en release, no un panic honesto).
    if total < 12 { return total.saturating_sub(2); }
    (total / 3).clamp(6, 20)
}

/// Pinta el cockpit omnisciente. Llamado always-on desde el loop del shell.
pub fn render_hud() {
    if !crate::info::has_fb() { return; }
    let total = crate::ring0::core::splash::dash_rows();
    if total == 0 { return; }
    let rows = band_rows(total);
    // El cockpit necesita cabecera + al menos 1 linea de bitacora + CINCO de
    // telemetria (sys, ring3, usb, kbd y raton). Menos que eso no es un
    // cockpit, es ruido: mejor no pintar.
    if rows < 7 { return; }
    let top = total - rows;

    let s = snapshot();
    // * `mx`, `my` y `btn` llegaban aqui y se tiraban con un guion bajo, igual
    // que el `_info` del panico del compositor. CABINA sabia donde estaba el
    // raton y no lo decia, asi que "el raton no va" no se podia repartir entre
    // tres culpables muy distintos. Ahora se ensenan.
    let (kbd, mouse, ks, ms, mev, mx, my, btn, kev) = crate::ring0::dev::usb::hid_stats();
    let (tev, rev, hev) = crate::ring0::dev::usb::xfer_stats();
    let (kdci, es, ee, ec) = crate::ring0::dev::usb::kbd_debug();
    let (kst, kbi, kiv, _ksp, ksts) = crate::ring0::dev::usb::kbd_ep_debug();
    let st = crate::ring0::task::scheduler::tid_state(2);
    let (rx, ln) = crate::ring0::uconsole::stats();
    let tid = crate::ring0::task::scheduler::current_tid();
    let mib_free = s.memory.free_pages / 256; // 4096 B/pagina -> /256 = MiB

    watch(&s, mib_free);

    // Firma de cambio: ticks en bucket grueso (>>8) para no parpadear; el
    // resto son eventos reales. + generacion de pantalla para repintar tras clear.
    static mut LAST: u64 = u64::MAX;
    static mut LAST_GEN: u32 = u32::MAX;
    let sig = (s.cpu.timer_ticks >> 8)
        ^ (s.scheduler.context_switches << 8)
        ^ ((s.memory.free_pages & 0xFFFF) << 16)
        ^ ((kev as u64) << 24) ^ ((tev as u64) << 28) ^ ((mev as u64) << 32)
        ^ ((st as u64) << 40) ^ ((kbd as u64) << 48) ^ ((mouse as u64) << 49)
        ^ ((s.scheduler.processes) << 50) ^ ((rx as u64) << 54)
        ^ (event_total() << 58)
        // El estado del endpoint puede pasar a Halted en caliente: si cambia,
        // hay que repintar aunque no se mueva ningun contador.
        ^ ((kst as u64) << 20) ^ ((rev as u64) << 36);
    let gen = crate::ring0::core::shell::ui::screen_gen();
    unsafe {
        if LAST == sig && LAST_GEN == gen { return; }
        LAST = sig; LAST_GEN = gen;
    }

    // CABINA se pinta tambien desde contextos cuya CR3 (la del usuario) no
    // mapea el framebuffer: pintar bajo la CR3 del kernel y restaurar. Mismo
    // patron que uconsole::flush.
    let saved_cr3 = crate::ring0::mm::vmm::read_cr3();
    let kpml4 = crate::ring0::mm::vmm::kernel_pml4();
    if saved_cr3 != kpml4 { crate::ring0::mm::vmm::switch_to(kpml4); }

    // == Cabecera de la banda: identidad + salud del propio registrador.
    // Va como REGLA, no como una linea mas de texto: es la frontera entre el
    // log rodante y el cockpit, y tiene que verse sin leerla.
    // ** LA BARRA DE TECLAS VA AQUI, y desplaza a la telemetria del propio
    // registrador.
    //
    // Este es el unico renglon que esta SIEMPRE en pantalla, asi que es el
    // sitio mas caro del sistema y hay que gastarlo en lo que mas falta hace.
    // Hasta hoy decia `eventos=N perdidos=N tk=0x...`: la salud de CABINA, que
    // es un dato de segundo orden y que ademas casi siempre es el mismo.
    //
    // Lo que de verdad hace falta es **que teclas hay**. Este terminal es el
    // suelo al que caes cuando el escritorio se murio, y un atajo que solo se
    // descubre leyendo el codigo no existe. Con la fila puesta, el que llega no
    // tiene que saber nada: pulsa y ve. `perdidos` se queda --pero solo cuando
    // NO es cero, que es cuando significa algo--, y el resto se mira con `cabina`.
    let mut r = Buf::new();
    r.txt("F1 ayuda  F2 consumo  F3 apps  F4 fallo  F5 info  F6 tareas  F7 mem  F8 cabina");
    if event_lost() > 0 {
        r.txt("   PERDIDOS="); r.dec(event_lost());
    }
    // ** Y AQUI MUERE EL CIAN, que era lo que el dueno miraba y no le cuadraba.
    //
    // El cian era "titulo" y no significaba nada mas: un color gastado en
    // decir que un renglon es un renglon. En una banda donde el resto de los
    // colores SI dicen algo --verde bien, ambar atencion, rojo problema-- uno
    // que no dice nada compite con los que si. Ahora la cabecera es **verde
    // cuando CABINA esta grabando entero y ambar cuando ha perdido eventos**,
    // o sea que el renglon que siempre miras vale por si solo como semaforo.
    let hdr_color = if event_lost() > 0 { C_WARN } else { C_OK };
    crate::ring0::core::splash::splash_dash_rule(top, r.as_str(), hdr_color);

    // == BITACORA EN TIEMPO REAL: el historial, el mas nuevo abajo, cada linea
    // con seq y tick (orden y distancia entre hechos = la mitad del valor
    // forense) y el color de su severidad/capa. Esto es la caja negra -- aun
    // en RAM, pendiente el volcado a disco.
    let log_rows = rows - 5; // cabecera + 4 filas de telemetria (sys/ring3/usb/kbd)
    for slot in 0..log_rows {
        let row = top + 1 + slot;
        let n = log_rows - 1 - slot; // arriba = mas viejo; abajo = mas nuevo
        match event_back(n) {
            Some(ev) => {
                let mut r = Buf::new();
                r.dec_pad(ev.seq, 4);
                // ** EL INTENTO, delante y pegado al numero de evento. Es lo que
                // deja ver a ojo que cuatro renglones son UNA historia, sin leer
                // ni una palabra de ellos. Los eventos sueltos --arranque, USB--
                // llevan un hueco: no pertenecen a ningun intento y decir #0
                // seria inventarles uno.
                if ev.intento != 0 { r.txt(" #"); r.dec(ev.intento as u64); } else { r.txt("   "); }
                r.txt(" t"); r.hex(ev.tick_ns, 5);
                r.txt(" "); r.pad(ev.severity.name(), 5);
                r.txt(" "); r.txt(ev.module_str()); r.txt(": ");
                r.txt(ev.msg_str());
                // El `value` del evento se guardaba y se TIRABA al pintar. Es
                // justo el dato duro (direccion MMIO, slot, codigo de estado)
                // que convierte una frase en una pista.
                //
                // ** Y desde el 2026-08-12 se pinta CON SU UNIDAD, que el emisor
                // ya sabia y tiraba en la puerta. `4.0 MiB (4196020)` en vez de
                // `400DF4`, `100` en vez de `64`, y una direccion con su offset
                // dentro de la pagina -- que es literalmente el bug de la
                // relocation partida, dicho por la propia linea.
                if ev.value != 0 { r.txt(" ="); r.value_of(&ev); }
                // ** Y DE DONDE SALIO, pero SOLO cuando duele.
                //
                // Un `INFO` que sale sesenta veces por segundo no necesita
                // decir su linea: nadie lo va a ir a buscar. Un `FAULT` si, y
                // es lo primero que se hace -- el 2026-08-10 se busco la frase
                // `cabecera invalida` por todo el arbol para dar con el sitio.
                //
                // Ponerlo en todos gastaria el ancho de pantalla en los
                // eventos que menos falta hacen, y la pantalla es de 80
                // columnas: cada caracter que se gasta en ruido es uno que le
                // falta al mensaje.
                if matches!(ev.severity, Severity::Fault | Severity::Panic) {
                    let f = ev.fichero_str();
                    if !f.is_empty() {
                        r.txt("  <"); r.txt(f); r.txt(":"); r.dec(ev.linea as u64); r.txt(">");
                    }
                }
                splash_dashboard_log_color(row, r.as_str(), ev_color(&ev));
            }
            None => splash_dashboard_log_color(row, "", C_DIM),
        }
    }

    // == TELEMETRIA COMPACTA (3 ultimas filas): salud del sistema de un vistazo.
    // Sistema -- verde = sano, ambar = RAM baja.
    let mut r = Buf::new();
    r.txt("sys  mem="); r.dec(mib_free); r.txt("MiB");
    r.txt(" sw="); r.dec(s.scheduler.context_switches);
    r.txt(" task="); r.dec(s.scheduler.processes); r.txt("/"); r.dec(s.scheduler.threads);
    r.txt(" tid="); r.dec(tid as u64);
    let health = if mib_free < 256 { C_WARN } else { C_OK };
    splash_dashboard_log_color(total - 5, r.as_str(), health);

    // Ring 3 -- verde = corriendo, gris = termino, ambar = bloqueado.
    let mut r = Buf::new();
    r.txt("ring3 st="); r.hex(st as u64, 2);
    r.txt(" rx="); r.dec(rx as u64); r.txt(" ln="); r.dec(ln as u64);
    r.txt("  (01Rdy 02Run 03Blk 04Exit FFdone)");
    let r3_color = match st { 0x02 => C_OK, 0xFF | 0x04 => C_DIM, 0x03 => C_WARN, _ => C_INFO };
    splash_dashboard_log_color(total - 4, r.as_str(), r3_color);

    // USB -- EL AVISO: verde si escribe, ROJO si enumero sin teclas.
    let mut r = Buf::new();
    r.txt("usb  k="); r.txt(if kbd { "OK" } else { "--" });
    r.txt("(s"); r.dec(ks as u64); r.txt(")");
    r.txt(" m="); r.txt(if mouse { "OK" } else { "--" });
    r.txt("(s"); r.dec(ms as u64); r.txt(")");
    r.txt(" kev="); r.dec(kev as u64);
    r.txt(" tev="); r.dec(tev as u64);
    // rev = eventos CRUDOS del xHC (de cualquier tipo). Se calculaba y no se
    // mostraba: es el que distingue "el controlador esta mudo" de "habla pero
    // no de este endpoint".
    r.txt(" rev="); r.dec(rev as u64);
    r.txt(" hev="); r.dec(hev as u64);
    r.txt(" dci="); r.dec(kdci as u64);
    r.txt(" lev="); r.dec(es as u64); r.txt(":"); r.dec(ee as u64); r.txt(":"); r.dec(ec as u64);
    // El APARCADERO de eventos: `total:dropped:ahora`.
    //
    // Un evento que llega mientras la enumeracion espera otra cosa ya no se
    // tira -- se aparca. `dropped` tiene que ser **0**: si sube, el aparcadero
    // se lleno y se perdio un informe, que es como enmudece un endpoint. Es el
    // numero que antes no existia y por el que el teclado se apago sin decir
    // nada.
    let (apk_tot, apk_perd, apk_hoy) = crate::ring0::dev::usb::park_stats();
    r.txt(" apk="); r.dec(apk_tot as u64);
    r.txt(":"); r.dec(apk_perd as u64);
    r.txt(":"); r.dec(apk_hoy as u64);
    // ** `bus=turns:overlaps` -- THE PROOF THAT THE BUS DEPENDS ON NOBODY.
    //
    // `turns` is the kernel thread beating. **It must always rise**, including --
    // and especially -- while a Ring 3 program holds the input: if it stalls, the
    // thread died or never started, and the keyboard is back to depending on
    // somebody asking. `overlaps` are the meetings between the thread and a
    // syscall; not a failure, just the price of having two doors.
    let (bus_turns, bus_overlaps) = crate::ring0::dev::usb::bus_stats();
    r.txt(" bus="); r.dec(bus_turns);
    r.txt(":"); r.dec(bus_overlaps);
    // ** `puertas=esperando:PERDIDOS:barridos:reparados` -- QUE LAS PUERTAS SIGAN
    // ABIERTAS.
    //
    // El dueno lo pidio con esas palabras --*"mi Kernel tiene que tener siempre
    // abierto las puertas"*-- y hasta ahora no habia forma de saber si lo
    // estaban. `PERDIDOS` es la cola de avisos desbordada; `reparados` son los
    // barridos que encontraron una diferencia entre lo que el driver creia y lo
    // que dicen los puertos. Los dos deberian quedarse en **0**: si `reparados`
    // sube, el sistema se esta arreglando solo, y cada uno de esos es medio
    // segundo en que el teclado no respondia.
    let (av_esp, av_perd, barridos, reparados) = crate::ring0::dev::usb::puertas_stats();
    r.txt(" puertas="); r.dec(av_esp as u64);
    r.txt(":"); r.dec(av_perd as u64);
    r.txt(":"); r.dec(barridos);
    r.txt(":"); r.dec(reparados);
    let usb_color = if apk_perd > 0 { C_FAULT }
                    else if kbd && kev > 0 { C_OK }
                    else if kbd && kev == 0 { C_FAULT }
                    else { C_WARN };
    splash_dashboard_log_color(total - 3, r.as_str(), usb_color);

    // Ultima fila -- el ENDPOINT del teclado segun el xHC. `ep` debe ser 1
    // (Running); `bi`->`iv` muestra el bInterval del descriptor y el exponente
    // que programamos de verdad (el bug del Interval se ve aqui de un vistazo).
    let mut r = Buf::new();
    r.txt("kbd  ep=");
    r.txt(match kst { 0 => "Disabled", 1 => "Running", 2 => "Halted", 3 => "Stopped", 4 => "Error", _ => "?" });
    r.txt(" bi="); r.dec(kbi as u64);
    r.txt(" iv="); r.dec(kiv as u64);
    r.txt(" (2^iv x125us)");
    r.txt(" usbsts=0x"); r.hex(ksts as u64, 4);
    let ep_color = if !kbd { C_DIM }
                   else if kst == 1 && ksts & ((1 << 2) | (1 << 12)) == 0 { C_OK }
                   else { C_FAULT };
    splash_dashboard_log_color(total - 2, r.as_str(), ep_color);

    // -- El RATON, con los tres numeros que reparten la culpa --
    //
    // Con una foto de esta linea se sabe cual de los tres es, y son problemas
    // en sitios muy distintos:
    //
    //   mev = 0                -> el HID no entrega NADA. Es el USB: endpoint,
    //                            ring o timbre. Ni el kernel ni el compositor.
    //   mev sube, x/y quietos  -> llegan informes pero los deltas salen cero:
    //                            el formato del informe no es el que leemos
    //                            (protocolo boot vs report, o un report ID
    //                            delante que corre todos los campos uno).
    //   x/y se mueven          -> el kernel lo tiene y el cursor no se pinta:
    //                            entonces es del compositor, y ahi si es
    //                            dibujo.
    let mut r = Buf::new();
    r.txt("raton ev="); r.dec(mev as u64);
    r.txt(" x="); if mx < 0 { r.txt("-"); } r.dec(mx.unsigned_abs() as u64);
    r.txt(" y="); if my < 0 { r.txt("-"); } r.dec(my.unsigned_abs() as u64);
    r.txt(" bot=0b"); r.hex(btn as u64, 2);
    // El SLOT, que es lo que destapo el bug: si el raton sale en el MISMO
    // slot que el teclado, no es un raton -- es la interfaz de medios del
    // teclado haciendose pasar por uno.
    r.txt(" slot="); r.dec(ms as u64);
    if ms != 0 && ms == ks { r.txt("(=kbd!)"); }
    // -- Las dos cosas que el reparto por fin deja ver --
    //
    // `bmb` = tiene cada uno una transferencia ENCOLADA? Un periferico que
    // deja de bombear queda enumerado, con el endpoint en `Running`, y mudo
    // para siempre -- nadie le vuelve a pedir nada. `k-` o `r-` aqui es
    // exactamente eso, y antes no se podia ver de ninguna forma.
    //
    // `hu` = Transfer Events que no eran de NADIE. Unos pocos al arrancar son
    // normales (restos de la enumeracion); si sube **mientras se teclea**, el
    // informe llega con una direccion distinta de la que creemos y por eso
    // nadie rearma.
    let (bomba_k, bomba_r, huerfanos) = crate::ring0::dev::usb::reparto_stats();
    r.txt(" bmb="); r.txt(if bomba_k { "k+" } else { "k-" });
    r.txt(if bomba_r { "r+" } else { "r-" });
    r.txt(" hu="); r.dec(huerfanos as u64);
    r.txt("  (ev=0 -> USB - ev sube y x/y quietos -> formato del informe)");
    let raton_color = if !mouse { C_DIM } else if mev > 0 { C_OK } else { C_FAULT };
    splash_dashboard_log_color(total - 1, r.as_str(), raton_color);

    if saved_cr3 != kpml4 { crate::ring0::mm::vmm::switch_to(saved_cr3); }
}

// =====================================================================
//  CABINA A RING 3 -- mirar TODO sin poder tocar nada
// =====================================================================
//
// Hasta hoy CABINA se pintaba **solo desde el shell de Ring 0**, y desde que el
// escritorio es el arranque eso significa que casi nunca se ve. Lo que F11
// ensena es el KLOG, que es otra cosa: transcripcion en texto plano, 96 bytes
// por linea y **sin severidad**. La linea que dice si el SMP levanto los doce
// nucleos existe con su color y su capa, y a Ring 3 le llegaba en gris.
//
// Esto lo abre. Y **no es "ir a Ring 0"**: aqui no se ejecuta nada
// privilegiado, no se concede ningun objeto y no hay una sola operacion que
// escriba. El compositor sigue siendo un proceso con sus capabilities
// contadas, y lo unico que hace es PREGUNTAR -- igual que con `info`, el klog y
// la autopsia.
//
// En un sistema de capabilities **ver y poder son cosas separadas**, y que se
// pueda mirar TODO sin poder tocar nada es la mitad interesante de la
// transparencia que este proyecto declara. Un "terminal privilegiado" que de
// verdad ejecutara en Ring 0 tiraria el modelo a la basura para conseguir algo
// que se puede tener sin romper nada: mirar.

/// Campos de `TASK_OP_CABINA_INFO`. Son una TABLA, igual que `OP_INFO`:
/// anadir un dato es una fila, no una operacion nueva.
pub const CABINA_TOTAL: u64 = 0x00;
pub const CABINA_LOST: u64 = 0x01;
pub const CABINA_AVAILABLE: u64 = 0x02;
/// Los cinco de un evento concreto. `arg1` = cual (0 = el mas reciente).
pub const CABINA_SEVERITY: u64 = 0x03;
pub const CABINA_LAYER: u64 = 0x04;
pub const CABINA_VALUE: u64 = 0x05;
pub const CABINA_SEQ: u64 = 0x06;
pub const CABINA_TICK: u64 = 0x07;
/// De que INTENTO salio el evento. `0` = de ninguno. Ver `bmo-abi`: es lo que
/// permite que la ventana de Ring 3 filtre por ACCION y no solo por gravedad.
pub const CABINA_ATTEMPT: u64 = 0x08;
// ** EL BARRIDO. Ver `cabina/radar.rs`: cuenta lo que el anillo pierde.
pub const CABINA_BARRIDO_CUENTA: u64 = 0x10;
pub const CABINA_BARRIDO_ULTIMO: u64 = 0x11;
pub const CABINA_VENTANA: u64 = 0x12;
pub const CABINA_CLASES_FUERA: u64 = 0x13;

/// Que texto se pide en `TASK_OP_CABINA_TEXTO`.
pub const CABINA_TXT_MODULE: u64 = 0x00;
pub const CABINA_TXT_MESSAGE: u64 = 0x01;

/// Cuantos eventos se pueden leer AHORA. Nunca mas que el anillo.
pub fn disponibles() -> u64 {
    let total = unsafe { EV_TOTAL };
    if total > EVENT_RING as u64 { EVENT_RING as u64 } else { total }
}

/// Un dato numerico de CABINA. `n` = que evento (0 = el mas reciente).
///
/// Devuelve `None` para un campo que no existe, que el syscall traduce a "no
/// soportado" -- y no 0, que seria indistinguible de un evento con valor cero.
/// **El `seq` mas bajo que sigue dentro del anillo.**
///
/// Todo evento con un `seq` menor que este existio y **ya no se puede leer**.
/// Con menos de 48 eventos desde el arranque no se ha caido nada todavia, y
/// entonces la ventana empieza en el 1.
///
/// [!] `EV_SEQ` es el ULTIMO entregado, no el siguiente. Con 48 eventos justos,
/// el mas viejo es el `1` -- y un `+1` de mas aqui diria que el primero se cayo
/// cuando sigue ahi.
fn primer_seq_visible() -> u64 {
    let total = event_total();
    if total <= super::ring::EVENT_RING as u64 {
        1
    } else {
        total - super::ring::EVENT_RING as u64 + 1
    }
}

pub fn campo(campo: u64, n: u64) -> Option<u64> {
    match campo {
        CABINA_TOTAL => Some(event_total()),
        CABINA_LOST => Some(event_lost()),
        CABINA_AVAILABLE => Some(disponibles()),
        // ** LOS CUATRO DEL BARRIDO. Van ANTES del `_`, que resuelve un evento
        // del anillo: estos NO son de un evento, son de todos los que hubo --
        // incluidos los que el anillo ya no tiene.
        CABINA_BARRIDO_CUENTA => Some(super::radar::cuenta((n >> 8) as usize, (n & 0xFF) as usize)),
        CABINA_BARRIDO_ULTIMO => Some(super::radar::ultimo((n >> 8) as usize, (n & 0xFF) as usize)),
        CABINA_VENTANA => Some(primer_seq_visible()),
        CABINA_CLASES_FUERA => Some(super::radar::clases_fuera_de_ventana(primer_seq_visible())),
        _ => {
            let ev = event_back(n as usize)?;
            match campo {
                CABINA_SEVERITY => Some(ev.severity as u64),
                CABINA_LAYER => Some(ev.layer as u64),
                CABINA_VALUE => Some(ev.value),
                CABINA_SEQ => Some(ev.seq),
                CABINA_TICK => Some(ev.tick_ns),
                CABINA_ATTEMPT => Some(ev.intento as u64),
                _ => None,
            }
        }
    }
}

/// Ocho bytes del modulo o del mensaje del evento `n`, empaquetados
/// little-endian. `trozo` numera de 8 en 8; el cero corta.
///
/// Mismo formato que `TASK_OP_RUTA` y que el klog, y por la misma razon: **la
/// superficie congelada no acepta punteros**, asi que el texto viaja por valor.
pub fn texto(n: u64, cual: u64, trozo: u64) -> u64 {
    let ev = match event_back(n as usize) {
        Some(e) => e,
        None => return 0,
    };
    let bytes: &[u8] = match cual {
        CABINA_TXT_MODULE => &ev.module,
        CABINA_TXT_MESSAGE => &ev.msg,
        _ => return 0,
    };
    let base = (trozo as usize) * 8;
    let mut w = [0u8; 8];
    for i in 0..8 {
        let j = base + i;
        if j >= bytes.len() || bytes[j] == 0 {
            break;
        }
        w[i] = bytes[j];
    }
    u64::from_le_bytes(w)
}
