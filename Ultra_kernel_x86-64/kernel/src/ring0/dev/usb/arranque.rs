//! **ENCENDER EL BUS USB**: esperar a que un puerto declare algo, y enumerar.
//!
//! [carril]  ROJO      encender el bus y enumerar: sin esto no hay teclado
//!
//! ## Por que soy un fichero (L6b)
//!
//! Porque contesto una pregunta que ocurre **UNA VEZ** y las de al lado ocurren
//! un millon: aqui se arranca el controlador y se enumera lo que ya estaba
//! enchufado al encender. Lo que pasa despues --que se enchufe algo, que llegue
//! una tecla-- vive en `enchufe.rs` y en `teclas.rs`.
//!
//! *** Y el reparto no es cosmetico: `init` sola son 220 lineas. En un fichero
//! de mil doscientas, la funcion que decide si esta maquina tiene teclado
//! estaba enterrada entre las que lo leen.
//!
//! [!] Y aqui vive el numero que costo un teclado programado a 35 MINUTOS entre
//! sondeos: el barrido de espera es de **10 ms y no de 1**. Cada barrido son
//! `nports` lecturas de MMIO, y a 1 ms el bucle machacaria los registros del
//! controlador para nada -- el rebote es un tiempo FISICO del puerto, y leerlo
//! mas a menudo no hace que pase antes.
//!
//! ** El reparto es MOVER TEXTO (L6d): ni una linea cambia de contenido.

use super::*;

fn log(msg: &str) {
    serial_write(msg);
    if crate::info::has_fb() {
        crate::ring0::core::dashboard::dashboard_log(msg);
    }
}

/// Espera real en milisegundos por TSC. El spec USB pide tiempos HUMANOS
/// (100 ms de debounce de conexion, 20+ ms de estabilizacion de power) -- los
/// spin-counts heredados de QEMU duran microsegundos y en hardware real los
/// puertos aun no reportan CCS cuando el driver pregunta.
///
/// # ** Y ESA ESPERA ES DONDE CORRE LA INTRO
///
/// Estos milisegundos son tiempo muerto de verdad: el CPU no hace nada mientras
/// un puerto se estabiliza. Y son muchos -- dos controladoras, varios puertos,
/// y cada uno con su debounce y su reset.
///
/// El video del arranque del 2026-08-15 lo enseno por el otro lado: entre
/// `intro_paso(40)` y `intro_paso(70)` --que es este bloque-- la pantalla se
/// quedaba **mas de tres segundos con un solo fotograma**. La ciudad congelada
/// y el gato sin salir. No porque la animacion estuviera mal, sino porque nadie
/// pintaba: el kernel estaba aqui, girando en vacio.
///
/// Asi que aqui se gira pintando. La regla la pone `intro_latido`: pinta **solo
/// si el fotograma cabe en lo que queda de espera**, asi que el USB sigue
/// esperando exactamente lo que el spec pide. La animacion sale de tiempo que
/// ya se estaba tirando.
pub(crate) fn delay_ms(ms: u64) {
    let f = crate::ring0::task::scheduler::tsc_freq();
    if f == 0 {
        for _ in 0..ms * 2_000_000 {
            core::hint::spin_loop();
        }
        return;
    }
    let por_ms = (f / 1000).max(1);
    let end = crate::ring0::task::scheduler::rdtsc() + ms * por_ms;
    loop {
        let ahora = crate::ring0::task::scheduler::rdtsc();
        if ahora >= end {
            break;
        }
        // Lo que queda de espera, en milisegundos. Es lo unico que el latido
        // necesita saber para decidir si le da tiempo.
        let quedan = ((end - ahora) / por_ms) as u32;
        if !crate::ring0::core::splash::intro_latido(quedan) {
            core::hint::spin_loop();
        }
    }
}

/// **Waits for a port to declare a device, and LEAVES AS SOON AS ONE DOES.**
///
/// Returns how many ports have `PORTSC.CCS` set, and how many milliseconds it
/// actually waited.
///
/// # Why polling instead of a flat `delay_ms(200)`
///
/// The 200 ms were a **blind** wait: the USB spec asks for 100 ms of connection
/// debounce, the code doubled it for safety, and then it looked. So the
/// controller that HAS the keyboard -- the one that matters -- paid the full
/// wait even though its ports had settled long before.
///
/// Polling flips who pays. A populated controller answers in the first sweeps
/// and leaves; an empty one still pays the whole budget, **and that is
/// correct**: to declare a port empty you have to give it its time. Rushing
/// that is how a keyboard that was there gets missed.
///
/// [!] So this does NOT make an empty controller cheaper. In a machine with the
/// devices on the second xHC, the first one still costs its full budget. That
/// is the part a remembered hint could remove one day -- see the boot timeline
/// notes -- and it is deliberately not solved here, because reordering the init
/// of two controllers blind, on the path that every key travels, is not worth
/// what it buys.
///
/// # Why the sweep is 10 ms and not 1
///
/// Each sweep is `nports` MMIO reads. At 1 ms the loop would hammer the
/// controller's registers for nothing: the debounce is a physical time of the
/// port and reading it more often does not make it happen sooner.
fn wait_for_connection(nports: u8, budget_ms: u64) -> (u64, u64) {
    const SWEEP_MS: u64 = 10;
    let mut waited = 0u64;
    loop {
        let mut connected = 0u64;
        for p in 0..nports {
            if unsafe { bmo_xhci::port_peek(p) } & 1 != 0 {
                connected += 1;
            }
        }
        if connected > 0 {
            return (connected, waited);
        }
        if waited >= budget_ms {
            return (0, waited);
        }
        delay_ms(SWEEP_MS);
        waited += SWEEP_MS;
    }
}

/// Descubre e inicializa xHCI + HID. Reporta al panel.
///
/// Estrategia hardware-real:
/// 1. Scan PCI propio (dev::pci, detras de bridges, MEM+BME habilitados).
/// 2. Los Ryzen traen VARIOS xHC (CPU + chipset): se prueban en orden.
/// 3. Por controlador: init -> power a TODOS los puertos -> 200 ms de settle
///    (spec: 100 ms debounce) -> censo PORTSC. Si algun puerto tiene CCS=1
///    (dispositivo FISICAMENTE presente), ese controlador gana y el HID
///    enumera ahi. El censo se pinta: dice donde esta el teclado
///    electricamente aunque la enumeracion posterior fallara.
pub fn init(_ctx: &BootContext) {
    bmo_xhci::init_hal(&HAL);

    let mut chosen = false;
    // El censo: cual vio mas aparatos, y cuantos hay en total repartidos entre
    // todos los controladores. La resta de los dos es lo que se queda fuera.
    let mut mejor: Option<pci::XhciLoc> = None;
    let mut mejor_vistos: u32 = 0;
    let mut vistos_total: u32 = 0;
    for skip in 0..4usize {
        let loc = match pci::find_xhci(skip) {
            Some(l) => l,
            None => break,
        };
        // MMIO virtual: SIEMPRE por el physmap. La identidad de s2 vive en
        // PML4[0] y un espacio de Ring 3 solo hereda su primer GiB, asi que
        // tocar un BAR de ~4 GiB bajo el CR3 de un proceso es un #PF en Ring 0.
        // Aqui no se notaba porque el sondeo del xHC corre en una tarea de
        // Ring 0; el mismo fallo SI mataba al disco. Ver la nota larga en
        // `dev/disk.rs`.
        let mmio_va = mm::phys_to_virt(loc.mmio);
        dlog_push("[usb] xHC pci ");
        dlog_u64(loc.bus as u64);
        dlog_push(":");
        dlog_u64(loc.dev as u64);
        dlog_push(".");
        dlog_u64(loc.func as u64);
        dlog_push(" mmio=");
        dlog_u64(loc.mmio);
        dlog_push("\n");

        crate::ring0::cabina::info("usb", "controlador xHCI hallado en PCI", loc.mmio);

        bmo_xhci::reset_ctrl();
        bmo_xhci::set_mmio(mmio_va);
        if !unsafe { bmo_xhci::init(mmio_va) } {
            dlog_push("[usb] init fallo en este xHC, probando siguiente\n");
            crate::ring0::cabina::warn("usb", "el xHC no inicializo, probando el siguiente", loc.mmio);
            continue;
        }
        let nports = match bmo_xhci::controller() {
            Some(c) => c.max_ports,
            None => continue,
        };
        // Power a todos los puertos y settle REAL (el uhid hace su propio
        // power+reset despues; para entonces CCS ya estara latcheado).
        // * Encender los ocho y esperar UNA vez, no ocho.
        //
        // La estabilizacion de VBUS es un tiempo fisico del puerto y los
        // puertos se estabilizan en paralelo. Antes cada `port_power_on`
        // esperaba sus 20 ms por su cuenta: ocho puertos por dos controladores
        // eran 320 ms de arranque comprando exactamente nada.
        for p in 0..nports {
            unsafe { bmo_xhci::port_power_solo(p) };
        }
        // ** THE WAIT NOW ENDS WHEN THE PORTS ANSWER, not when a constant does.
        //
        // It was a flat `delay_ms(200)`: power the ports, wait blind, then
        // look. The controller that HAS the keyboard paid the full 200 ms even
        // though its ports had settled long before -- and that controller is
        // the one on the critical path of the boot.
        //
        // `wait_for_connection` sweeps every 10 ms and leaves on the first
        // device. An empty controller still pays the whole budget, which is the
        // right answer: declaring a port empty in a hurry is how a keyboard
        // that WAS there gets missed.
        let (connected, waited_ms) = wait_for_connection(nports, 200);
        crate::ring0::cabina::info("usb", "ms esperados a que el puerto conteste", waited_ms);
        // Censo, ya sin esperar: solo para DECIR cuales son.
        for p in 0..nports {
            let sc = unsafe { bmo_xhci::port_peek(p) };
            if sc & 1 != 0 {
                dlog_push(" p");
                dlog_u64(p as u64);
                dlog_push("=");
                dlog_u64(sc as u64);
            }
        }
        if connected > 0 {
            dlog_push("\n");
        }
        dlog_push("[usb] puertos con dispositivo: ");
        dlog_u64(connected);
        dlog_push("\n");
        if connected > 0 {
            crate::ring0::cabina::info("usb", "puertos con dispositivo fisico (CCS=1)", connected);
        }
        // ** SE CENSAN TODOS ANTES DE ELEGIR, Y AQUI ESTA EL PORQUE.
        //
        // === El sintoma del dueno, 2026-08-17 ===
        //
        // *"reinicie desde Windows para bootear BMO-X: el raton se movio pero el
        // teclado NO aparece dentro, y al desconectarlo y conectarlo **no
        // prende su RGB**"*.
        //
        // El RGB es un dato, no un adorno: casi ningun teclado lo enciende hasta
        // que **completa `SET_CONFIGURATION`**. Un RGB apagado dice que el
        // aparato no llego a `Configured`, o sea que **nadie le hablo**.
        //
        // === Y este bucle era el que no le hablaba ===
        //
        // Decia `if connected > 0 { chosen = true; break; }`: **el primer
        // controlador con CUALQUIER cosa enchufada ganaba**, y `CTRL` es UNO.
        // Todo lo que estuviera en el otro xHC quedaba invisible para siempre --
        // y ni siquiera con corriente, porque el `port_power_solo` de arriba
        // solo se ejecuta en los controladores que se llegan a probar.
        //
        // ** Y ESTA PLACA TIENE DOS. Lo dice el comentario de tres lineas mas
        // arriba --*"los Ryzen traen VARIOS xHC (CPU + chipset)"*-- sin sacar la
        // consecuencia: si el raton cae en uno y el teclado en el otro, **el
        // raton funciona y el teclado no existe**. Que es exactamente el
        // sintoma, incluido el RGB apagado y el "una sola vez y ya".
        //
        // Tambien explica la INTERMITENCIA entre arranques: cual gana depende de
        // cual tenga algo enchufado primero, y eso cambia entre frio y caliente
        // porque el firmware deja los puertos en estados distintos.
        //
        // === Lo que se hace hoy, y lo que NO ===
        //
        // Manejar los dos controladores a la vez es otra cosa: `CTRL` es un solo
        // `static`, y repartirlo es una reforma del driver. Lo que se arregla
        // aqui es que **el kernel deje de callarselo**:
        //
        //   1. se censan TODOS los controladores antes de elegir;
        //   2. gana el que MAS dispositivos vea, no el primero;
        //   3. si quedan aparatos en otro, se GRITA con su numero.
        //
        // Un fallo que se ve es medio arreglo; este llevaba meses siendo mudo.
        if connected > (mejor_vistos as u64) {
            mejor_vistos = connected as u32;
            mejor = Some(loc);
        }
        vistos_total += connected as u32;
        chosen = true;
    }

    if !chosen || mejor.is_none() {
        log("[usb] ningun xHC ve el teclado (probar otro puerto fisico)\n");
        crate::ring0::cabina::fault("usb", "ningun xHC ve dispositivos (probar otro puerto)", 0);
        return;
    }

    // ** LOS QUE SE QUEDAN FUERA, DICHOS EN VOZ ALTA.
    //
    // `vistos_total - mejor_vistos` son aparatos que existen, que estan
    // enchufados y encendidos, y que este kernel **no va a mirar jamas**. Si el
    // teclado es uno de ellos, esta linea es la unica explicacion que va a haber
    // -- y sin ella el dueno solo ve "el teclado no aparece".
    let huerfanos = vistos_total.saturating_sub(mejor_vistos);
    if huerfanos > 0 {
        crate::ring0::cabina::fault(
            "usb",
            "aparatos en OTRO xHC que este kernel no maneja (cambialos de puerto)",
            huerfanos as u64,
        );
    }

    // El elegido se vuelve a inicializar: el censo dejo puesto el ULTIMO que se
    // probo, no el que gano.
    let loc = mejor.unwrap();
    let mmio_va = mm::phys_to_virt(loc.mmio);
    bmo_xhci::reset_ctrl();
    bmo_xhci::set_mmio(mmio_va);
    if !unsafe { bmo_xhci::init(mmio_va) } {
        crate::ring0::cabina::fault("usb", "el xHC elegido no reinicializo", loc.mmio);
        return;
    }
    if let Some(c) = bmo_xhci::controller() {
        let n = c.max_ports;
        for p in 0..n {
            unsafe { bmo_xhci::port_power_solo(p) };
        }
        wait_for_connection(n, 200);
    }
    crate::ring0::cabina::info("usb", "xHC elegido (el que mas aparatos ve)", loc.mmio);

    let ok = unsafe {
        let hid = &mut *core::ptr::addr_of_mut!(HID);
        let r = hid.init();
        refrescar_presencia();
        r
    };
    unsafe {
        PRESENT = true;
        READY = ok;
    }
    // Resumen detallado en serial + panel (ademas del status fijo en pantalla).
    unsafe {
        if KBD_RDY {
            log("[usb] teclado USB listo (slot ");
            dlog_u64(KBD_SLOT as u64);
            log(")\n");
            crate::ring0::cabina::info("usb", "teclado enumerado y configurado", KBD_SLOT as u64);
            // Lo que de verdad decide si el teclado hablara: el estado del
            // endpoint segun el xHC y el intervalo que quedo programado.
            let (st, bi, iv, _sp, sts) = kbd_ep_debug();
            crate::ring0::cabina::info("xhci", "kbd bInterval->Interval programado", ((bi as u64) << 8) | iv as u64);
            if st != 1 {
                crate::ring0::cabina::fault("xhci", "endpoint del teclado NO quedo Running", st as u64);
            }
            // HSE (bit 2) o HCE (bit 12): el controlador se cayo, todo lo demas
            // que veamos despues es ruido.
            if sts & ((1 << 2) | (1 << 12)) != 0 {
                crate::ring0::cabina::fault("xhci", "controlador en error (USBSTS HSE/HCE)", sts as u64);
            }
        } else {
            log("[usb] SIN teclado (no enumero interface kbd)\n");
            crate::ring0::cabina::warn("usb", "ninguna interface de teclado enumero", 0);
        }
        if MOUSE_RDY {
            log("[usb] mouse USB listo (slot ");
            dlog_u64(MOUSE_SLOT as u64);
            log(")\n");
            crate::ring0::cabina::info("usb", "mouse enumerado y configurado", MOUSE_SLOT as u64);
        } else {
            log("[usb] SIN mouse (no enumero interface mouse)\n");
            crate::ring0::cabina::warn("usb", "ninguna interface de mouse enumero", 0);
        }
    }
}
