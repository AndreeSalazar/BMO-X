//! **LA COLA CRUDA DE TECLAS Y EL ESTADO DEL TECLADO**: scancodes,
//!
//! [carril]  AMARILLO  la cola cruda y el estado del teclado
//! [consumo] NADA      corre cuando alguien lo pide
//! modificadores, LEDs, repeticion y el puntero.
//!
//! ## Por que soy un fichero (L6b)
//!
//! Porque contesto *"que esta pasando en el teclado AHORA"*, y eso es distinto
//! de *"como se enciende el bus"* (`arranque.rs`) y de *"que hacer cuando algo
//! se enchufa"* (`enchufe.rs`).
//!
//! ## ** Y por que hay DOS colas y no una
//!
//! El kernel SIEMPRE tuvo esta informacion y la tiraba en la puerta. Lo que
//! llegaba a Ring 3 era un flujo de CARACTERES, y **un juego no pregunta que
//! letra se escribio: pregunta si la flecha abajo esta pulsada AHORA**. Sin el
//! soltar, quien anda no para nunca.
//!
//! [!] Y si la cola cruda se llena se tira lo NUEVO y se cuenta (2026-09-18).
//! Tiraba lo viejo --para no perder un `soltar`-- moviendo el indice del
//! consumidor desde el productor, y eso es lo unico que una cola entre dos
//! nucleos no puede hacer. Las dos politicas pierden lo mismo al llenarse;
//! la respuesta es que `perdidos` sea cero. Ver `bmo-cola`.
//!
//! ** El reparto es MOVER TEXTO (L6d): ni una linea cambia de contenido.

use super::*;

// -- LA COLA CRUDA DE TECLAS: scancode + pulsada/soltada -----------------
//
// ** El kernel SIEMPRE tuvo esta informacion y la tiraba en la puerta.
//
// `bmo_uhid::teclado` compara cada informe boot con el anterior y produce
// `InputEvent::key(scancode, pulsada)` -- las dos cosas, desde el primer dia.
// Lo que llegaba a Ring 3 era un flujo de CARACTERES: `INPUT_OP_TECLA` entrega
// un byte Latin-1 ya resuelto, que es lo correcto para escribir y **no sirve
// para jugar**. Un juego no pregunta "que letra se escribio", pregunta "esta
// la flecha abajo AHORA". Sin el soltar, quien anda no para nunca.
//
// Por eso esto no es una cola nueva de datos nuevos: es dejar de tirar lo que
// ya se tenia. La de caracteres se queda intacta y las dos se llenan del mismo
// sondeo -- no hay dos lectores del bus.
//
// 64 entries: un informe boot trae hasta 6 teclas y el sondeo va por
// fotograma. Si se llena, se tira lo nuevo y se cuenta (`perdidos`): si ese
// numero sube, el consumidor no drena lo bastante rapido, y es un numero, no
// una sospecha.
//
// ** ES UNA `bmo_cola::Cola` desde el 2026-09-18 (PLAN_EL_BUS_APARTE, A0):
// el productor es el hilo del bus y el consumidor el escritorio desde su
// syscall, y el dia que el bus viva en otro nucleo esto tiene que seguir
// siendo verdad sin cerrojo. Cada lado toca solo su indice.
const EVENTOS_CRUDOS: usize = 64;
static CRUDOS: bmo_cola::Cola<u16, EVENTOS_CRUDOS> = bmo_cola::Cola::nueva(0);
/// El productor no puede vaciar la cola (seria mover el indice del
/// consumidor): la PIDE vaciar, y el consumidor lo hace en su siguiente
/// lectura. Ver `vaciar_cola_cruda`.
static VACIAR_CRUDA: core::sync::atomic::AtomicBool = core::sync::atomic::AtomicBool::new(false);

pub(crate) fn empujar_evento(scancode: u8, pulsada: bool) {
    CRUDOS.empujar(if pulsada { 0x100 | scancode as u16 } else { scancode as u16 });
}

/// La siguiente tecla cruda: `Some((scancode Set 1, pulsada))`, o `None`.
///
/// **No bloquea** y **bombea el bus** si la cola esta vacia, por el mismo
/// motivo que `poll_ascii`: quien llama tiene un bucle de fotograma y el bus
/// solo avanza cuando alguien lo mira.
///
/// El envoltorio de CR3 es el de `poll_ascii` y por la misma razon -- tocar el
/// xHCI es escribir MMIO que solo esta mapeado en el PML4 del kernel, y esto se
/// recorre desde dentro de un syscall. Ver su cabecera.
pub fn evento_tecla() -> Option<(u8, bool)> {
    // ** El rescate se mira en LAS DOS salidas, y por eso no vale envolver solo
    // la de abajo: la de arriba es el camino rapido --la cola ya tenia algo-- y
    // es justo por donde pasa un juego que va sobrado de eventos. Ver
    // [`rescatar`].
    if let Some(v) = sacar_crudo() {
        return raw_key_from_owner(Some(v));
    }
    // The CR3 wrapper is no longer here: it lives inside [`pump_bus`], the only
    // thing that touches the bus. Having it in every caller was the way for a new
    // caller to forget it.
    pump_bus();
    raw_key_from_owner(sacar_crudo())
}

fn sacar_crudo() -> Option<(u8, bool)> {
    // Lo primero: si el bus pidio vaciar (un aparato se fue o llego), lo que
    // hay es de ANTES y se tira aqui, que es donde esta el consumidor.
    if VACIAR_CRUDA.swap(false, core::sync::atomic::Ordering::AcqRel) {
        CRUDOS.vaciar();
    }
    let v = CRUDOS.sacar()?;
    Some(((v & 0xFF) as u8, v & 0x100 != 0))
}

/// Eventos crudos tirados por cola llena. Para el panel.
/// **VACIAR la cola cruda.** Se llama al enchufar y al desenchufar.
///
/// # Por que hace falta, y lo dijo el dueno
///
/// > *"cuando mi teclado estaba siendo usado hay datos e informacion basura que
/// > me gustaria que a la hora de conectar no tenga ese problema"*
///
/// ** Lo que queda en esta cola son scancodes de ANTES: teclas que se pulsaron
/// con el aparato viejo, o durante el desenchufe. Al reconectar salen como si
/// alguien las acabara de pulsar -- y no hay forma de distinguirlas, porque un
/// scancode no trae la hora ni de que teclado vino.
///
/// > Un evento de entrada que sobrevive a su aparato no es un evento tardio:
/// > es un evento de otro.
///
/// [!] Los perdidos NO se ponen a cero: son la bitacora de cuantos eventos se
/// tiraron por saturacion desde el arranque, y esa cuenta es un sintoma que hay
/// que poder seguir viendo crecer.
///
/// ** Y desde el 2026-09-18 esto NO vacia: lo PIDE. Quien llama es el hilo
/// del bus --el productor-- y vaciar es mover el indice del consumidor. Se
/// deja la peticion y `sacar_crudo` la cumple en la siguiente lectura, antes
/// de entregar nada. Lo que devuelve es cuanto habia en ese instante: lo que
/// se va a tirar, salvo lo que entre entre medias.
pub fn vaciar_cola_cruda() -> u32 {
    let habia = CRUDOS.cuantos() as u32;
    VACIAR_CRUDA.store(true, core::sync::atomic::Ordering::Release);
    habia
}

pub fn eventos_crudos_perdidos() -> u32 {
    CRUDOS.perdidos()
}

/// Esta activo el tercer nivel? AltGr, o el Ctrl+Alt al que acostumbra
/// Windows (y por tanto los dedos de medio mundo).
/// Mascara de modificadores VIVA, para Ring 3.
///
/// El byte que entrega `INPUT_OP_TECLA` viene ya resuelto --la `n` es `0xF1`--
/// y eso es lo correcto para escribir, pero deja fuera los atajos: un
/// compositor no puede distinguir `Ctrl+Alt` de nada porque `Ctrl+Alt` sin
/// otra tecla no produce caracter. Esto lo abre sin tocar el camino de
/// escritura.
pub const MOD_SHIFT: u8 = 1 << 0;
pub const MOD_CTRL: u8 = 1 << 1;
pub const MOD_ALT: u8 = 1 << 2;
pub const MOD_ALTGR: u8 = 1 << 3;
pub const MOD_CAPS: u8 = 1 << 4;
/// **La tecla WINDOWS.** Que significa lo decide Ring 3, no el kernel.
///
/// El bit existe para que el compositor pueda atarla a lo que quiera --abrir el
/// lanzador, cambiar de ventana-- sin que el kernel tenga una politica de
/// atajos dentro. Es la misma frontera que `WANTS_SCREEN`: el kernel arbitra,
/// Ring 3 manda.
///
/// [!] Va en el bit 5 y no en el 6 porque el 5 estaba libre: los numeros de
/// esta mascara son contrato con Ring 3 y no se reordenan.
pub const MOD_GUI: u8 = 1 << 5;

pub fn modificadores() -> u8 {
    unsafe {
        let mut m = 0;
        if SHIFT { m |= MOD_SHIFT; }
        if CTRL { m |= MOD_CTRL; }
        if LALT { m |= MOD_ALT; }
        if ALTGR { m |= MOD_ALTGR; }
        if CAPS { m |= MOD_CAPS; }
        if GUI { m |= MOD_GUI; }
        m
    }
}

/// * OJO al usar esto para atajos: en la distribucion espanola `Ctrl+Alt` ES
/// `AltGr` -- es lo que produce `@`, `#`, `[`, `]`, `\`, `|` y `EUR`. Un atajo
/// que dispare al PULSAR `Ctrl+Alt` rompe escribir todos esos caracteres. Ver
/// como lo resuelve el compositor: dispara al SOLTAR, y solo si no se escribio
/// nada mientras estaban pulsados.
pub(crate) fn altgr_active() -> bool {
    unsafe { ALTGR || (CTRL && LALT) }
}

/// Manda al teclado el estado de sus LEDs cuando cambia. Un SET_REPORT por
/// cambio, no por sondeo: es un control transfer y no hace falta mas.
pub(crate) fn sync_leds() {
    static mut LAST_LEDS: u8 = 0xFF;
    let want = crate::ring0::dev::keyboard::led_mask();
    unsafe {
        if LAST_LEDS == want { return; }
        LAST_LEDS = want;
        let hid = &*core::ptr::addr_of!(HID);
        hid.set_leds(want);
    }
}

/// Repite la tecla mantenida: tras `REPEAT_DELAY_MS` empieza a inyectarla
/// cada `REPEAT_RATE_MS`. El teclado USB solo avisa de bajada y subida --
/// repetir es trabajo del host, y sin esto mantener el retroceso no borra.
pub(crate) fn repeat_held() {
    unsafe {
        if HELD_CODE == 0 { return; }
        let hz = crate::ring0::task::scheduler::tsc_freq();
        if hz == 0 { return; }
        let now = crate::ring0::task::scheduler::rdtsc();
        let delay = hz / 1000 * REPEAT_DELAY_MS;
        let period = hz / 1000 * REPEAT_RATE_MS;
        if now.wrapping_sub(HELD_SINCE) < delay { return; }
        if now.wrapping_sub(HELD_LAST) < period { return; }
        HELD_LAST = now;
        keyboard::feed_full(HELD_CODE, HELD_SHIFT, HELD_ALTGR, CAPS, HELD_CTRL);
    }
}

/// Saca un caracter de la cola del teclado y lleva la cuenta. Aqui se graba
/// la PRIMERA tecla que cruza de verdad -- en el instante exacto, no deducida
/// despues comparando contadores.
pub(crate) fn drain() -> Option<u8> {
    let b = crate::ring0::dev::keyboard::pop_out()?;
    KEY_EVENTS.fetch_add(1, core::sync::atomic::Ordering::Relaxed);
    unsafe {
        if !FIRST_KEY {
            FIRST_KEY = true;
            crate::ring0::cabina::info("usb", "primera tecla recibida: el teclado WRITES", b as u64);
        }
    }
    Some(b)
}

/// Estado DETALLADO del HID para el panel de diagnostico (fila fija, sobrevive
/// al auto-clear). Devuelve: (teclado_listo, mouse_listo, slot_kbd, slot_mouse,
/// eventos_mouse, x_mouse, y_mouse, botones, eventos_tecla).
/// El puntero: `(x, y, botones, eventos)`.
///
/// Lo que `KIND_INPUT` entrega a Ring 3. Son los deltas del HID ya acumulados;
/// el recorte al panel lo hace `input.rs`, que es quien sabe de pantallas.
pub fn puntero() -> (i32, i32, u8, u32) {
    use core::sync::atomic::Ordering::Relaxed;
    (MOUSE_X.load(Relaxed), MOUSE_Y.load(Relaxed), MOUSE_BTN.load(Relaxed), MOUSE_EVENTS.load(Relaxed))
}

/// Las vueltas de rueda desde la ultima vez, y las pone a cero.
///
/// Consumir al leer y no dar un acumulado: quien pregunta quiere saber cuanto
/// se ha girado DESDE QUE MIRO, no desde el arranque. Un acumulado obligaria a
/// cada llamante a guardar el anterior y restar, y el primero que lo olvidara
/// tendria un scroll que se va solo.
pub fn rueda() -> i32 {
    // `swap` y no leer-y-poner-a-cero: entre las dos el bus puede sumar una
    // vuelta, y esa vuelta se perderia.
    MOUSE_WHEEL.swap(0, core::sync::atomic::Ordering::AcqRel)
}

/// Vuelve a leer del driver quien hay y en que slot.
///
/// Se llama tras enumerar Y tras cada adopcion en caliente. Antes esto estaba
/// copiado en linea dentro de `init` y por eso no existia la posibilidad de
/// actualizarlo: un raton adoptado mas tarde habria seguido saliendo como
/// ausente en el panel aunque estuviera bombeando, y la fila del diagnostico
/// habria mentido justo cuando por fin decia la verdad.
///
/// # Safety
/// Toca los estaticos del modulo; solo desde el camino de USB.
pub(crate) unsafe fn refrescar_presencia() {
    let hid = &*core::ptr::addr_of!(HID);
    KBD_RDY = hid.has_kbd();
    MOUSE_RDY = hid.has_mouse();
    KBD_SLOT = hid.kbd_slot();
    MOUSE_SLOT = hid.mouse_slot();
}
