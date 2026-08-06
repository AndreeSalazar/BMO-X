//! **El teclado USB, y nada más.**
//!
//! Todo lo que sabe de un teclado vive aquí: cómo es su informe de 8 bytes en
//! protocolo boot, la tabla que lleva un HID Usage a un scancode Set 1, qué
//! bits son los modificadores, y cómo se le encienden las lucecitas.
//!
//! ## Qué se ganó al separarlo
//!
//! Esto estaba mezclado con el ratón dentro de un `poll()` de 120 líneas, en el
//! mismo fichero que la enumeración del bus. Tres trabajos distintos —recorrer
//! puertos, decidir qué interfaz es qué, y descifrar informes— compartiendo
//! variables. El precio ya se pagó: con las dos ramas de decodificación
//! seguidas y disparadas por `if` independientes, un informe podía entrar por
//! las dos. Ver [`crate::dir::Direccion`].
//!
//! Aquí dentro no se menciona al ratón ni una vez, y ésa es la prueba de que
//! están separados de verdad.

use crate::dir::Direccion;
use bmo_input::event::InputEvent;

/// El informe del protocolo BOOT: 8 bytes, fijos, iguales en todo teclado.
///
/// Se pide con `SET_PROTOCOL(boot)` justo por esto — el informe "de verdad"
/// (report protocol) lo describe el HID Report Descriptor y habría que
/// interpretarlo, que es un parser entero. Boot es un contrato de ocho bytes.
#[repr(C)]
#[derive(Clone, Copy)]
struct Informe {
    modificadores: u8,
    _reservado: u8,
    teclas: [u8; 6],
}

// ── USB HID usage → PS/2 Set 1 scancode ─────────────────────

static HID_TO_PS2: [u8; 104] = [
    0,0,0,0, 0x1E,0x30,0x2E,0x20,0x12,0x21,0x22,0x23,
    0x17,0x24,0x25,0x26,0x32,0x31,0x18,0x19,0x10,0x13,
    0x1F,0x14,0x16,0x2F,0x11,0x2D,0x15,0x2C,
    0x02,0x03,0x04,0x05,0x06,0x07,0x08,0x09,0x0A,0x0B,
    // El indice 50 (usage 0x32, "Non-US # and ~") es la tecla junto al Enter
    // de los teclados ISO: en español es la de } ] `. Mapea al mismo Set 1
    // 0x2B que la barra invertida; estaba en 0 = tecla muerta de verdad.
    0x1C,0x01,0x0E,0x0F,0x39,0x0C,0x0D,0x1A,0x1B,0x2B,0x2B,
    0x27,0x28,0x29,0x33,0x34,0x35,
    0x3A,0x3B,0x3C,0x3D,0x3E,0x3F,0x40,0x41,0x42,0x43,0x44,0x57,0x58,
    // Navegacion (Insert..flechas). Llevaban los MISMOS codigos Set 1 que
    // el teclado numerico (flecha arriba = 0x48 = KP8), asi que pulsar una
    // flecha escribia un numero. Set 1 real las distingue con el prefijo
    // 0xE0, que no cabe en un byte: se les da codigo propio 0x66..0x6F.
    0x37,0x46,0x45,SC_INSERT,SC_HOME,SC_PGUP,SC_DELETE,SC_END,SC_PGDN,
    SC_RIGHT,SC_LEFT,SC_DOWN,SC_UP,0x45,
    // El '/' del teclado NUMERICO (usage 0x54) llevaba 0x35, el mismo Set 1
    // que la tecla '/' de la fila principal. En US da igual porque ambas son
    // '/', pero en español esa tecla es '-': el numpad escribia guiones.
    // Set 1 real es 0xE0 0x35 (dos bytes); 0x62 esta libre y el consumidor lo
    // resuelve como '/' en cualquier distribucion.
    0x62,0x37,0x4A,0x4E,0x1C,0x4F,0x50,0x51,0x4B,0x4C,0x4D,0x47,0x48,0x49,0x52,0x53,
    // 0x64 = la tecla EXTRA de los teclados ISO (la de < > junto al Shift
    // izquierdo, que los US no tienen): Set 1 la llama 0x56. Estaba en 0 =
    // ignorada, así que en un teclado español faltaba una tecla entera.
    0x56,0,0,0,
];

fn hid_to_ps2(usage: u8) -> Option<u8> {
    let idx = usage as usize;
    if idx < HID_TO_PS2.len() {
        let v = HID_TO_PS2[idx];
        if v != 0 { Some(v) } else { None }
    } else {
        None
    }
}

/// Scancode propio para AltGr (Alt derecho). Set 1 lo expresa como la
/// secuencia `0xE0 0x38`, imposible de meter en un solo byte de InputEvent;
/// 0x63 está libre en Set 1 y el consumidor lo trata como AltGr. Sin esto
/// AltGr llegaba como 0x38 (Alt izquierdo) y el tercer nivel del teclado
/// español — @ # \ | { } [ ] — era inalcanzable.
pub const SC_ALTGR: u8 = 0x63;

// Teclas de navegacion con codigo propio (ver la nota en HID_TO_PS2).
pub const SC_UP: u8 = 0x66;
pub const SC_DOWN: u8 = 0x67;
pub const SC_LEFT: u8 = 0x68;
pub const SC_RIGHT: u8 = 0x69;
pub const SC_INSERT: u8 = 0x6A;
pub const SC_HOME: u8 = 0x6B;
pub const SC_PGUP: u8 = 0x6C;
pub const SC_DELETE: u8 = 0x6D;
pub const SC_END: u8 = 0x6E;
pub const SC_PGDN: u8 = 0x6F;

const MOD_LCTRL: u8 = 1 << 0;
const MOD_LSHIFT: u8 = 1 << 1;
const MOD_LALT: u8 = 1 << 2;
const MOD_LGUI: u8 = 1 << 3;
const MOD_RCTRL: u8 = 1 << 4;
const MOD_RSHIFT: u8 = 1 << 5;
const MOD_RALT: u8 = 1 << 6;
const MOD_RGUI: u8 = 1 << 7;

/// Cada bit de modificador con el scancode que le corresponde.
///
/// Era una escalera de ocho `if` copiados, y en uno de ellos —el Ctrl
/// derecho— se coló el scancode del izquierdo. Una tabla no se puede
/// equivocar de esa forma: la fila es el dato.
const MODIFICADORES: [(u8, u8); 8] = [
    (MOD_LCTRL, 0x1D),
    (MOD_LSHIFT, 0x2A),
    (MOD_LALT, 0x38),
    (MOD_LGUI, 0x5B),
    (MOD_RCTRL, 0x1D),
    (MOD_RSHIFT, 0x36),
    // AltGr, NO el Alt izquierdo: en las distribuciones latinas abre el tercer
    // nivel (@ # \ | { } [ ]).
    (MOD_RALT, SC_ALTGR),
    (MOD_RGUI, 0x5C),
];

/// Cuántos bytes ocupa el informe boot de un teclado.
pub const INFORME_BYTES: u16 = 8;

/// Un teclado USB enumerado y listo.
pub struct Teclado {
    dir: Direccion,
    /// Número de interface HID: lo pide `SET_REPORT` para encender los LEDs
    /// (Bloq Mayús / Num).
    iface: u8,
    buf_phys: u64,
    buf_virt: *mut u8,
    previo_mod: u8,
    previas: [u8; 6],
    /// ¿Hay una transferencia encolada esperando informe?
    ///
    /// **Si esto se queda en `false`, el teclado enmudece para siempre**: el
    /// endpoint sigue en `Running` y nadie vuelve a pedirle nada.
    bombeando: bool,
    /// Lo que el ENDPOINT dice que puede mandar de una vez.
    mps: u16,
    /// Transferencias que volvieron con error.
    ///
    /// El ratón llevaba esta cuenta desde el principio y el teclado no: su rama
    /// de error era un `if` sin `else`, así que **un teclado que fallaba lo
    /// hacía en absoluto silencio**. Justo el aparato del que se dijo "se
    /// desconecta sin sentido".
    errores: u32,
}

impl Teclado {
    pub fn nuevo(dir: Direccion, iface: u8, buf_phys: u64, buf_virt: *mut u8, mps: u16) -> Self {
        Self {
            dir,
            iface,
            buf_phys,
            buf_virt,
            previo_mod: 0,
            previas: [0; 6],
            errores: 0,
            bombeando: false,
            mps,
        }
    }

    /// Cuántos bytes se le piden al bus.
    ///
    /// Aquí siempre habían coincidido —el informe boot de un teclado mide 8 y
    /// su `mps` es 8—, y por eso este camino nunca falló mientras el del ratón
    /// sí. Se pone igual: una regla que sólo se cumple por casualidad en la
    /// mitad de los sitios es una trampa esperando al aparato siguiente.
    fn largo(&self) -> u16 {
        if self.mps == 0 { INFORME_BYTES } else { self.mps }
    }

    pub fn direccion(&self) -> Direccion { self.dir }
    pub fn slot(&self) -> u8 { self.dir.slot }
    pub fn dci(&self) -> u8 { self.dir.dci }
    pub fn bombeando(&self) -> bool { self.bombeando }

    /// Errores de transferencia vistos, igual que en el ratón.
    pub fn errores(&self) -> u32 { self.errores }

    /// Encola la primera transferencia y toca el timbre. **Hasta que esto se
    /// llama, el teclado está enumerado pero mudo.**
    ///
    /// Se hace al FINAL de la enumeración, no al reconocerlo: un endpoint que
    /// empieza a postear informes mientras todavía se enumera el puerto
    /// siguiente mete sus eventos en medio de los control transfers del otro
    /// aparato.
    pub fn arrancar(&mut self) -> bool {
        let largo = self.largo();
        self.bombeando = unsafe {
            bmo_xhci::queue_interrupt_in(self.dir.slot, self.dir.dci, self.buf_phys, largo)
        };
        if self.bombeando {
            unsafe { bmo_xhci::ring_doorbell(self.dir.slot, self.dir.dci) };
        }
        self.bombeando
    }

    /// Atiende un Transfer Event que YA se ha comprobado que es suyo.
    ///
    /// Devuelve cuántos eventos de entrada escribió en `salida`. Rearma la
    /// transferencia siempre, incluso si el informe vino con un código de
    /// error: dejar de rearmar por un informe malo apaga el teclado por un
    /// tropiezo.
    pub fn atender(&mut self, cc: u8, salida: &mut [InputEvent]) -> usize {
        let mut n = 0usize;
        self.bombeando = false;

        // 1 = Success, 13 = Short Packet. Un informe más corto de lo pedido es
        // normal y trae datos buenos.
        if cc == 1 || cc == 13 {
            let informe = unsafe { core::ptr::read_volatile(self.buf_virt as *const Informe) };
            n = self.descifrar(&informe, salida);
            self.previo_mod = informe.modificadores;
            self.previas = informe.teclas;
        } else {
            // Un `cc` malo NO se descifra —el buffer trae lo que trajera— pero
            // sí se dice. Antes esta rama no existía: el informe se tiraba, se
            // rearmaba, y si el endpoint había quedado parado el teclado moría
            // sin dejar una línea. Quien resucita el endpoint es el reparto
            // (`Hid::poll`), que ya sabe de quién es el evento; aquí sólo se
            // cuenta y se cuenta EN VOZ ALTA.
            self.errores = self.errores.saturating_add(1);
            let h = bmo_xhci::hal();
            h.log_u64("[uhid] teclado: transferencia con error cc=", cc as u64);
            h.log_u64("  (errores=", self.errores as u64);
            h.log(")\n");
        }

        self.rearmar();
        n
    }

    /// Vuelve a encolar. Es lo que mantiene viva la bomba.
    fn rearmar(&mut self) {
        let largo = self.largo();
        unsafe {
            if bmo_xhci::queue_interrupt_in(self.dir.slot, self.dir.dci, self.buf_phys, largo) {
                bmo_xhci::ring_doorbell(self.dir.slot, self.dir.dci);
                self.bombeando = true;
            }
        }
    }

    /// El informe → eventos de pulsar y soltar.
    ///
    /// Un informe boot dice **qué teclas están abajo AHORA**, no qué cambió.
    /// Las pulsaciones y las sueltas salen de comparar con el informe anterior;
    /// por eso `previas` no es una optimización, es de dónde sale la mitad de
    /// los eventos.
    fn descifrar(&self, informe: &Informe, salida: &mut [InputEvent]) -> usize {
        let mut n = 0usize;

        let cambio = informe.modificadores ^ self.previo_mod;
        for (bit, scancode) in MODIFICADORES {
            if cambio & bit != 0 && n < salida.len() {
                salida[n] = InputEvent::key(scancode, informe.modificadores & bit != 0);
                n += 1;
            }
        }

        // Soltadas: estaban antes y ya no están.
        for &antes in &self.previas {
            if antes == 0 { continue; }
            if !informe.teclas.contains(&antes) {
                if let Some(sc) = hid_to_ps2(antes) {
                    if n < salida.len() {
                        salida[n] = InputEvent::key(sc, false);
                        n += 1;
                    }
                }
            }
        }
        // Pulsadas: están ahora y no estaban.
        for &ahora in &informe.teclas {
            if ahora == 0 { continue; }
            if !self.previas.contains(&ahora) {
                if let Some(sc) = hid_to_ps2(ahora) {
                    if n < salida.len() {
                        salida[n] = InputEvent::key(sc, true);
                        n += 1;
                    }
                }
            }
        }

        n
    }

    /// Enciende/apaga los LEDs (bit0 Num, bit1 Mayús, bit2 Scroll).
    ///
    /// Las lucecitas NO las maneja el teclado por su cuenta: es el HOST quien
    /// le dice cómo dejarlas, con un `SET_REPORT` de tipo Output. Por eso Bloq
    /// Mayús funcionaba por dentro mientras la luz seguía apagada — nadie se lo
    /// estaba contando al teclado.
    pub fn leds(&self, leds: u8) -> bool {
        let mut datos = [leds];
        // bmRequestType 0x21 = Host->Device, Class, Interface.
        // bRequest 0x09 = SET_REPORT; wValue 0x0200 = Output report id 0.
        let n = unsafe {
            bmo_xhci::control_transfer(
                self.dir.slot, 0x21, 0x09, 0x0200, self.iface as u16, &mut datos, false,
            )
        };
        n > 0
    }
}
