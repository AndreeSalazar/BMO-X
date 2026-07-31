//! **Driver USB HID de BMO** — teclado y ratón, cada uno en su sitio.
//!
//! ```text
//!   dir.rs      la DIRECCION (slot, dci): quien es quien en el bus
//!   enumera.rs  el BUS: puertos, descriptores, dejar un endpoint listo
//!   teclado.rs  el TECLADO: su informe, su tabla de scancodes, sus LEDs
//!   raton.rs    el RATON: su informe, sus botones, su rueda
//!   lib.rs      esto: cableado y reparto. Nada mas.
//! ```
//!
//! ## Por qué se partió
//!
//! Era **un fichero de 602 líneas** con cuatro trabajos dentro: recorrer el
//! bus, decidir qué interfaz es qué, descifrar informes de teclado y descifrar
//! informes de ratón. Los dos últimos vivían pegados dentro de un `poll()` de
//! 120 líneas, compartiendo el contador de eventos y el buffer de salida.
//!
//! Y no era sólo feo: **ya se cobró un bug**. El reparto se escribía así —
//!
//! ```ignore
//! if let Some(k) = &mut self.kbd { if ev_slot == k.slot && ev_ep == k.dci { … } }
//! if let Some(m) = &mut self.mouse { if ev_slot == m.slot && ev_ep == m.dci { … } }
//! ```
//!
//! — dos `if` INDEPENDIENTES. Mientras el teclado y el ratón tuvieran
//! direcciones distintas no pasaba nada. Cuando el bug del teclado compuesto
//! los dejó a los dos en el mismo slot con el mismo DCI, **el mismo informe de
//! 8 bytes se leía como teclado Y como ratón**: los tres primeros bytes de una
//! pulsación se interpretaban como botones y desplazamiento. Nada avisaba.
//!
//! ## Las dos reglas que ahora son estructura, no cuidado
//!
//! 1. **Un informe tiene UN dueño.** El reparto es excluyente y lo que no es de
//!    nadie se CUENTA ([`UsbHidHal::huerfanos`]) en vez de desaparecer.
//! 2. **Dos periféricos no pueden compartir dirección.** Instalar un ratón en
//!    la dirección del teclado se rechaza y se dice. Ver [`dir::Direccion::choca_con`].
//!
//! Añadir un tercer aparato es un módulo más y un brazo más en el reparto; no
//! se toca ni el bus ni los otros dos.

#![no_std]

pub mod dir;
pub mod enumera;
pub mod raton;
pub mod teclado;

use bmo_input::event::InputEvent;
use bmo_input::hal::{InputHal, PointerMode};
use dir::Direccion;
use raton::Raton;
use teclado::Teclado;

// Los scancodes propios son parte del contrato con el kernel (`ring0/dev/
// keyboard.rs` los compara), así que se re-exportan desde la raíz: quien los
// usa no tiene por qué saber en qué fichero viven.
pub use teclado::{
    SC_ALTGR, SC_DELETE, SC_DOWN, SC_END, SC_HOME, SC_INSERT, SC_LEFT, SC_PGDN, SC_PGUP, SC_RIGHT,
    SC_UP,
};

pub struct UsbHidHal {
    teclado: Option<Teclado>,
    raton: Option<Raton>,
    inicializado: bool,
    /// Transfer Events que no eran de ningún periférico conocido.
    ///
    /// Suele haberlos y es normal —restos de control transfers de la
    /// enumeración—, pero si esto sube **mientras se teclea**, el informe está
    /// llegando con una dirección que no es la que creemos y por eso nadie
    /// rearma. Antes se descartaban sin contarlos.
    huerfanos: u32,
}

impl Default for UsbHidHal {
    fn default() -> Self {
        Self::new()
    }
}

impl UsbHidHal {
    pub const fn new() -> Self {
        Self { teclado: None, raton: None, inicializado: false, huerfanos: 0 }
    }

    /// ¿Enumeró un teclado? (interface HID subclass 1 / protocol 1)
    pub fn has_kbd(&self) -> bool { self.teclado.is_some() }
    /// ¿Enumeró un ratón? (interface HID subclass 1 / protocol 2)
    pub fn has_mouse(&self) -> bool { self.raton.is_some() }
    /// Slot xHCI del teclado / ratón (0 si ausente) — para diagnóstico.
    pub fn kbd_slot(&self) -> u8 { self.teclado.as_ref().map_or(0, |k| k.slot()) }
    pub fn mouse_slot(&self) -> u8 { self.raton.as_ref().map_or(0, |m| m.slot()) }
    /// DCI del teclado — para comparar con el endpoint del Transfer Event.
    pub fn kbd_dci(&self) -> u8 { self.teclado.as_ref().map_or(0, |k| k.dci()) }
    pub fn mouse_dci(&self) -> u8 { self.raton.as_ref().map_or(0, |m| m.dci()) }

    /// Eventos que llegaron y no eran de nadie. Ver el campo.
    pub fn huerfanos(&self) -> u32 { self.huerfanos }

    /// ¿Están los dos con transferencia encolada?
    ///
    /// Un periférico que deja de bombear queda enumerado y mudo para siempre —
    /// el endpoint sigue en `Running` y nadie le vuelve a pedir nada. Es el
    /// estado que hay que poder ver desde fuera.
    pub fn bombeando(&self) -> (bool, bool) {
        (
            self.teclado.as_ref().is_some_and(|k| k.bombeando()),
            self.raton.as_ref().is_some_and(|m| m.bombeando()),
        )
    }

    /// Enciende/apaga los LEDs del teclado (bit0 Num, bit1 Mayús, bit2 Scroll).
    pub fn set_leds(&self, leds: u8) -> bool {
        self.teclado.as_ref().is_some_and(|k| k.leds(leds))
    }

    /// Instala un ratón, **rechazándolo si chocaría con el teclado**.
    ///
    /// Ésta es la regla 2 hecha código. Antes no existía y por eso el teclado y
    /// el ratón pudieron acabar siendo el mismo endpoint sin que nadie dijera
    /// nada; el síntoma era un ratón que "funcionaba" leyendo pulsaciones de
    /// tecla como desplazamientos.
    fn instalar_raton(&mut self, nuevo: Raton) -> bool {
        if let Some(k) = self.teclado.as_ref() {
            if k.direccion().choca_con(nuevo.direccion()) {
                let h = bmo_xhci::hal();
                h.log_u64(
                    "[uhid] raton RECHAZADO: misma direccion que el teclado, slot ",
                    nuevo.slot() as u64,
                );
                return false;
            }
        }
        self.raton = Some(nuevo);
        true
    }

    /// ¿Podemos quedarnos con esta interfaz de ratón?
    ///
    /// Sí si no hay ninguno, o si el que hay es el provisional de un teclado
    /// compuesto y éste viene de un aparato que NO es un teclado.
    fn raton_libre(&self, de_un_compuesto: bool) -> bool {
        match self.raton.as_ref() {
            None => true,
            Some(m) => m.es_provisional() && !de_un_compuesto,
        }
    }

    /// ¿El ratón que hay es de verdad, o la interfaz de medios de un teclado?
    fn raton_dedicado(&self) -> bool {
        self.raton.as_ref().is_some_and(|m| !m.es_provisional())
    }

    /// ¿Ya no hace falta mirar más? Teclado **y** ratón dedicado.
    ///
    /// Es la pregunta que decide si merece la pena tocar el bus. Re-enumerar
    /// cuando no falta nada cuesta control transfers sobre un controlador con
    /// dos aparatos ya bombeando, y eso es exactamente lo que no se quiere
    /// hacer sin motivo.
    pub fn completo(&self) -> bool {
        self.teclado.is_some() && self.raton_dedicado()
    }

    /// Enumera UN puerto e instala lo que falte. El camino compartido por el
    /// arranque y por el enchufe en caliente: uno solo, así no pueden divergir.
    ///
    /// # Safety
    /// Toca MMIO del xHC: hay que llamarlo con el CR3 del kernel puesto.
    unsafe fn cosechar_puerto(&mut self, port: u8) -> Cosecha {
        let h = bmo_xhci::hal();
        let mut cosecha = Cosecha { teclado: false, raton: false };

        let slot = match enumera::direccionar_puerto(port) {
            Some(s) => s,
            None => return cosecha,
        };
        let mut cfg = [0u8; enumera::MAX_CFG];
        let (cfg_val, largo) = match enumera::leer_descriptores(slot, &mut cfg) {
            Some(v) => v,
            None => return cosecha,
        };
        let cfg = &cfg[..largo];

        let mut ifaces = [(0u8, 0u8, 0u8, 0u8); enumera::MAX_IFACES];
        let n_ifs = enumera::interfaces(cfg, &mut ifaces);
        let compuesto = enumera::es_compuesto(&ifaces[..n_ifs]);

        for (iface, clase, subclase, proto) in &ifaces[..n_ifs] {
            if *clase != enumera::CLASE_HID || *subclase != enumera::SUBCLASE_BOOT {
                continue;
            }
            let es_teclado = *proto == enumera::PROTO_TECLADO && self.teclado.is_none();
            let es_raton = *proto == enumera::PROTO_RATON && self.raton_libre(compuesto);
            if !es_teclado && !es_raton {
                continue;
            }

            let (_addr, mps, interval, dci) = match enumera::intr_in(cfg, *iface) {
                Some(e) => e,
                None => continue,
            };
            let (buf_phys, buf_virt) =
                match enumera::preparar_endpoint(slot, dci, mps, interval, *iface, cfg_val) {
                    Some(b) => b,
                    None => continue,
                };

            let direccion = Direccion::nueva(slot, dci);
            if es_teclado {
                self.teclado = Some(Teclado::nuevo(direccion, *iface, buf_phys, buf_virt));
                cosecha.teclado = true;
                h.log("[uhid] teclado listo\n");
            } else {
                if compuesto {
                    h.log("[uhid] iface de raton en un TECLADO: provisional\n");
                }
                if self.instalar_raton(Raton::nuevo(direccion, buf_phys, buf_virt, compuesto)) {
                    cosecha.raton = true;
                    h.log("[uhid] raton listo\n");
                }
            }
        }
        cosecha
    }

    /// Enciende las bombas de lo que esté instalado y todavía parado.
    ///
    /// Se llama al final de la enumeración y tras cada adopción. El guardia
    /// `bombeando()` no es cosmético: encolar dos veces en el mismo endpoint
    /// deja dos TRB vivos y el segundo informe llega a un buffer que ya nadie
    /// espera.
    fn arrancar_bombas(&mut self) {
        let h = bmo_xhci::hal();
        if let Some(k) = self.teclado.as_mut() {
            if !k.bombeando() && !k.arrancar() {
                // Sin anillo no hay transferencia, y sin transferencia el
                // teclado enmudece para siempre. Callarlo fue lo que costó las
                // fotos.
                h.log("[uhid] teclado SIN anillo: no se pudo encolar\n");
            }
        }
        if let Some(m) = self.raton.as_mut() {
            if !m.bombeando() && !m.arrancar() {
                h.log("[uhid] raton SIN anillo: no se pudo encolar\n");
            }
        }
    }

    /// **Algo se enchufó en `port`: adóptalo.**
    ///
    /// ★ Ésta es la mitad que faltaba. El aviso de cambio de puerto ya llegaba
    /// —la foto lo enseñó, `usb: puerto: algo se ENCHUFO (sin re-enumerar aun)`—
    /// y **nadie hacía nada con él**. El comentario decía "primero ver, luego
    /// hacer"; ya se vio.
    ///
    /// Sin esto, la enumeración era una carrera de un solo intento contra el
    /// arranque: un aparato que tarda en engancharse —un ratón con firmware RGB
    /// tarda— se perdía **hasta el siguiente reinicio**. De ahí el síntoma que
    /// no encajaba con nada: unas veces arrancaba el teclado y otras el ratón,
    /// nunca los dos, sin tocar una línea de código entre arranque y arranque.
    /// No era intermitencia del hardware: era quién llegaba a tiempo.
    ///
    /// Devuelve `true` si se instaló algo nuevo.
    ///
    /// # Safety
    /// Toca MMIO del xHC: hay que llamarlo con el CR3 del kernel puesto.
    pub unsafe fn adoptar_puerto(&mut self, port: u8) -> bool {
        // Si no falta nada, no se toca el bus. Re-enumerar por gusto mete
        // control transfers en un controlador con dos aparatos bombeando.
        if self.completo() {
            return false;
        }
        let cosecha = self.cosechar_puerto(port);
        if cosecha.teclado || cosecha.raton {
            self.arrancar_bombas();
            return true;
        }
        false
    }
}

/// Qué se instaló al mirar un puerto.
struct Cosecha {
    teclado: bool,
    raton: bool,
}

impl InputHal for UsbHidHal {
    fn init(&mut self) -> bool {
        if self.inicializado {
            return true;
        }

        if !bmo_xhci::is_controller_initialized() {
            let mmio = match bmo_xhci::get_mmio() { Some(m) => m, None => return false };
            if !unsafe { bmo_xhci::init(mmio) } { return false; }
        }
        let ctrl = match bmo_xhci::controller() { Some(c) => c, None => return false };
        let h = bmo_xhci::hal();

        // ── Recorrer los puertos ──────────────────────────────────────────
        for port in 0..ctrl.max_ports {
            let cosecha = unsafe { self.cosechar_puerto(port) };

            // Sólo se corta con un ratón DEDICADO. Un teclado compuesto marca
            // las dos banderas —su interfaz de medios cuenta como ratón— y
            // cortar ahí dejaba el puerto del ratón de verdad sin visitar jamás.
            if cosecha.teclado && cosecha.raton && self.raton_dedicado() {
                break;
            }
        }

        // ── Arrancar las bombas, AL FINAL ─────────────────────────────────
        //
        // Un endpoint de interrupción, en cuanto se le encola una transferencia
        // y se toca su timbre, **empieza a postear eventos solo**. Si eso pasa
        // mientras todavía se enumera el puerto siguiente, sus informes caen en
        // el anillo compartido en medio de los control transfers del otro
        // aparato. Ver el aparcadero de `bmo_xhci`.
        //
        // Y un segundo motivo: el ratón PROVISIONAL de un teclado compuesto se
        // arrancaba y luego se sustituía por el dedicado, dejando una
        // transferencia viva en un endpoint que ya no lee nadie.
        self.arrancar_bombas();
        let _ = h;

        self.inicializado = true;
        self.teclado.is_some()
    }

    fn name(&self) -> &'static str { "USB-HID" }

    /// El REPARTO. Un evento, un dueño.
    ///
    /// `else if` y no dos `if` sueltos: ver la nota de la cabecera del módulo.
    fn poll(&mut self, buf: &mut [InputEvent]) -> usize {
        if !self.inicializado {
            return 0;
        }
        let mut n = 0usize;
        unsafe {
            while let Some((slot, ep, cc)) = bmo_xhci::poll_transfer_event() {
                // ★ Aunque no quede sitio para más eventos, el informe hay que
                // ATENDERLO: atender es lo que rearma la transferencia, y sin
                // rearmar el periférico se para para siempre. Se le pasa la
                // rodaja que quede, aunque esté vacía — se pierde el evento de
                // entrada, que es recuperable; no la bomba, que no lo es.
                let desde = n.min(buf.len());
                let resto = &mut buf[desde..];
                if let Some(k) = self.teclado.as_mut().filter(|k| k.direccion().es_mio(slot, ep)) {
                    n += k.atender(cc, resto);
                } else if let Some(m) =
                    self.raton.as_mut().filter(|m| m.direccion().es_mio(slot, ep))
                {
                    n += m.atender(cc, resto);
                } else {
                    self.huerfanos = self.huerfanos.wrapping_add(1);
                }
            }
        }
        n
    }

    fn pointer_mode(&self) -> PointerMode { PointerMode::Relative }
    fn is_ready(&self) -> bool { self.inicializado }
}
