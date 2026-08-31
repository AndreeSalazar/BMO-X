//! **LA COLA CRUDA DE TECLAS Y EL ESTADO DEL TECLADO**: scancodes,
//!
//! [carril]  AMARILLO  la cola cruda y el estado del teclado
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
//! [!] Y si la cola cruda se llena se tira **lo mas VIEJO**, no lo nuevo: tirar
//! lo nuevo dejaria entregado un `pulsar` cuyo `soltar` no llega, y el juego se
//! quedaria andando solo.
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
// fotograma. Si se llena, se tira **lo mas VIEJO** y se cuenta. Tirar lo nuevo
// seria peor de una forma concreta: se perderia el `soltar` de una tecla cuyo
// `pulsar` ya se entrego, y el juego se quedaria andando solo.
const EVENTOS_CRUDOS: usize = 64;
static mut CRUDOS: [u16; EVENTOS_CRUDOS] = [0; EVENTOS_CRUDOS];
static mut CRUDOS_LEE: usize = 0;
static mut CRUDOS_ESCRIBE: usize = 0;
/// Cuantos se han tirado por cola llena. Si esto sube, el consumidor no esta
/// drenando lo bastante rapido -- y es un numero, no una sospecha.
static mut CRUDOS_PERDIDOS: u32 = 0;

pub(crate) fn empujar_evento(scancode: u8, pulsada: bool) {
    unsafe {
        let siguiente = (CRUDOS_ESCRIBE + 1) % EVENTOS_CRUDOS;
        if siguiente == CRUDOS_LEE {
            // Llena: se tira la mas vieja para hacer sitio.
            CRUDOS_LEE = (CRUDOS_LEE + 1) % EVENTOS_CRUDOS;
            CRUDOS_PERDIDOS = CRUDOS_PERDIDOS.saturating_add(1);
        }
        CRUDOS[CRUDOS_ESCRIBE] = if pulsada {
            0x100 | scancode as u16
        } else {
            scancode as u16
        };
        CRUDOS_ESCRIBE = siguiente;
    }
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
    unsafe {
        if CRUDOS_LEE == CRUDOS_ESCRIBE {
            return None;
        }
        let v = CRUDOS[CRUDOS_LEE];
        CRUDOS_LEE = (CRUDOS_LEE + 1) % EVENTOS_CRUDOS;
        Some(((v & 0xFF) as u8, v & 0x100 != 0))
    }
}

/// Eventos crudos tirados por cola llena. Para el panel.
pub fn eventos_crudos_perdidos() -> u32 {
    unsafe { CRUDOS_PERDIDOS }
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

pub fn modificadores() -> u8 {
    unsafe {
        let mut m = 0;
        if SHIFT { m |= MOD_SHIFT; }
        if CTRL { m |= MOD_CTRL; }
        if LALT { m |= MOD_ALT; }
        if ALTGR { m |= MOD_ALTGR; }
        if CAPS { m |= MOD_CAPS; }
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
    unsafe {
        KEY_EVENTS = KEY_EVENTS.wrapping_add(1);
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
    unsafe { (MOUSE_X, MOUSE_Y, MOUSE_BTN, MOUSE_EVENTS) }
}

/// Las vueltas de rueda desde la ultima vez, y las pone a cero.
///
/// Consumir al leer y no dar un acumulado: quien pregunta quiere saber cuanto
/// se ha girado DESDE QUE MIRO, no desde el arranque. Un acumulado obligaria a
/// cada llamante a guardar el anterior y restar, y el primero que lo olvidara
/// tendria un scroll que se va solo.
pub fn rueda() -> i32 {
    unsafe {
        let v = MOUSE_WHEEL;
        MOUSE_WHEEL = 0;
        v
    }
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
