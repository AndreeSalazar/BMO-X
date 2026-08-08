//! **Driver USB HID de BMO** -- teclado y raton, cada uno en su sitio.
//!
//! ```text
//!   dir.rs      la DIRECCION (slot, dci): quien es quien en el bus
//!   enumera.rs  el BUS: puertos, descriptores, dejar un endpoint listo
//!   formato.rs  el REPORT DESCRIPTOR: donde esta cada campo, y de cuantos bits
//!   teclado.rs  el TECLADO: su informe, su tabla de scancodes, sus LEDs
//!   raton.rs    el RATON: su informe, sus botones, su rueda
//!   lib.rs      esto: cableado y reparto. Nada mas.
//! ```
//!
//! ## Por que se partio
//!
//! Era **un fichero de 602 lineas** con cuatro trabajos dentro: recorrer el
//! bus, decidir que interfaz es que, descifrar informes de teclado y descifrar
//! informes de raton. Los dos ultimos vivian pegados dentro de un `poll()` de
//! 120 lineas, compartiendo el contador de eventos y el buffer de salida.
//!
//! Y no era solo feo: **ya se cobro un bug**. El reparto se escribia asi --
//!
//! ```ignore
//! if let Some(k) = &mut self.kbd { if ev_slot == k.slot && ev_ep == k.dci { ... } }
//! if let Some(m) = &mut self.mouse { if ev_slot == m.slot && ev_ep == m.dci { ... } }
//! ```
//!
//! -- dos `if` INDEPENDIENTES. Mientras el teclado y el raton tuvieran
//! direcciones distintas no pasaba nada. Cuando el bug del teclado compuesto
//! los dejo a los dos en el mismo slot con el mismo DCI, **el mismo informe de
//! 8 bytes se leia como teclado Y como raton**: los tres primeros bytes de una
//! pulsacion se interpretaban como botones y desplazamiento. Nada avisaba.
//!
//! ## Las dos reglas que ahora son estructura, no cuidado
//!
//! 1. **Un informe tiene UN dueno.** El reparto es excluyente y lo que no es de
//!    nadie se CUENTA ([`UsbHidHal::huerfanos`]) en vez de desaparecer.
//! 2. **Dos perifericos no pueden compartir direccion.** Instalar un raton en
//!    la direccion del teclado se rechaza y se dice. Ver [`dir::Direccion::choca_con`].
//!
//! Anadir un tercer aparato es un modulo mas y un brazo mas en el reparto; no
//! se toca ni el bus ni los otros dos.

#![no_std]

pub mod dir;
pub mod enumera;
/// El Report Descriptor, leido. Es lo que convierte "8 o 16 bits?" de una
/// discusion sobre una foto en una pregunta que contesta el aparato.
pub mod formato;
/// La contabilidad de puertos: a cual se puede tocar y a cual no. Es la unica
/// parte del driver que se puede probar sin un xHC delante -- y era la que
/// estaba mal.
pub mod puertos;
pub mod raton;
pub mod teclado;

use bmo_input::event::InputEvent;
use bmo_input::hal::{InputHal, PointerMode};
use dir::Direccion;
use puertos::Puertos;
use raton::Raton;
use teclado::Teclado;

// Los scancodes propios son parte del contrato con el kernel (`ring0/dev/
// keyboard.rs` los compara), asi que se re-exportan desde la raiz: quien los
// usa no tiene por que saber en que fichero viven.
pub use teclado::{
    SC_ALTGR, SC_DELETE, SC_DOWN, SC_END, SC_HOME, SC_INSERT, SC_LEFT, SC_PGDN, SC_PGUP, SC_RIGHT,
    SC_UP,
};

pub struct UsbHidHal {
    teclado: Option<Teclado>,
    raton: Option<Raton>,
    inicializado: bool,
    /// Transfer Events que no eran de ningun periferico conocido.
    ///
    /// Suele haberlos y es normal --restos de control transfers de la
    /// enumeracion--, pero si esto sube **mientras se teclea**, el informe esta
    /// llegando con una direccion que no es la que creemos y por eso nadie
    /// rearma. Antes se descartaban sin contarlos.
    huerfanos: u32,
    /// Que puertos ya dieron un aparato y cuantas veces se ha intentado cada
    /// uno. Ver [`puertos`]: sin esto, la re-enumeracion reactiva se comia a si
    /// misma reseteando el puerto del teclado que ya estaba funcionando.
    puertos: Puertos,
}

impl Default for UsbHidHal {
    fn default() -> Self {
        Self::new()
    }
}

impl UsbHidHal {
    pub const fn new() -> Self {
        Self {
            teclado: None,
            raton: None,
            inicializado: false,
            huerfanos: 0,
            puertos: Puertos::nuevo(),
        }
    }

    /// Se desenchufo algo del puerto: queda libre y con los intentos devueltos.
    /// Lo llama el kernel al recibir el aviso de desconexion.
    pub fn soltar_puerto(&mut self, port: u8) {
        self.puertos.release(port);
    }

    /// Para el panel: `(puertos tomados, intentos gastados en el ultimo)`.
    pub fn puertos(&self) -> &Puertos {
        &self.puertos
    }

    /// Enumero un teclado? (interface HID subclass 1 / protocol 1)
    pub fn has_kbd(&self) -> bool { self.teclado.is_some() }
    /// Enumero un raton? (interface HID subclass 1 / protocol 2)
    pub fn has_mouse(&self) -> bool { self.raton.is_some() }
    /// Slot xHCI del teclado / raton (0 si ausente) -- para diagnostico.
    pub fn kbd_slot(&self) -> u8 { self.teclado.as_ref().map_or(0, |k| k.slot()) }
    pub fn mouse_slot(&self) -> u8 { self.raton.as_ref().map_or(0, |m| m.slot()) }
    /// DCI del teclado -- para comparar con el endpoint del Transfer Event.
    pub fn kbd_dci(&self) -> u8 { self.teclado.as_ref().map_or(0, |k| k.dci()) }
    pub fn mouse_dci(&self) -> u8 { self.raton.as_ref().map_or(0, |m| m.dci()) }

    /// Este `(slot, endpoint)` es de alguno de los dos aparatos?
    ///
    /// La misma pregunta que hace el reparto, pero sin pedir prestado el
    /// `&mut`: hace falta antes de repartir, para decidir si toca resucitar el
    /// endpoint. Un evento huerfano no se recupera -- no se sabe de quien es el
    /// anillo ni quien volveria a encolar.
    pub fn es_de_alguien(&self, slot: u8, ep: u8) -> bool {
        self.teclado.as_ref().is_some_and(|k| k.direccion().es_mio(slot, ep))
            || self.raton.as_ref().is_some_and(|m| m.direccion().es_mio(slot, ep))
    }

    /// Eventos que llegaron y no eran de nadie. Ver el campo.
    pub fn huerfanos(&self) -> u32 { self.huerfanos }

    /// Estan los dos con transferencia encolada?
    ///
    /// Un periferico que deja de bombear queda enumerado y mudo para siempre --
    /// el endpoint sigue en `Running` y nadie le vuelve a pedir nada. Es el
    /// estado que hay que poder ver desde fuera.
    pub fn bombeando(&self) -> (bool, bool) {
        (
            self.teclado.as_ref().is_some_and(|k| k.bombeando()),
            self.raton.as_ref().is_some_and(|m| m.bombeando()),
        )
    }

    /// Enciende/apaga los LEDs del teclado (bit0 Num, bit1 Mayus, bit2 Scroll).
    pub fn set_leds(&self, leds: u8) -> bool {
        self.teclado.as_ref().is_some_and(|k| k.leds(leds))
    }

    /// Instala un raton, **rechazandolo si chocaria con el teclado**.
    ///
    /// Esta es la regla 2 hecha codigo. Antes no existia y por eso el teclado y
    /// el raton pudieron acabar siendo el mismo endpoint sin que nadie dijera
    /// nada; el sintoma era un raton que "funcionaba" leyendo pulsaciones de
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

    /// Podemos quedarnos con esta interfaz de raton?
    ///
    /// Si si no hay ninguno, o si el que hay es el provisional del teclado y
    /// este sale de **otro aparato**.
    ///
    /// * El parametro es un HECHO, no una adivinanza. Antes se le pasaba
    /// `es_compuesto` --"este aparato trae interfaz de teclado y de raton"-- y
    /// eso describe igual de bien a un teclado con teclas de medios que a un
    /// **raton de juego con teclas de macro**. El de esta maquina las tiene:
    /// se le miraba, se le clasificaba de "teclado compuesto" y se le
    /// descartaba, dejando puesto el raton provisional del teclado de verdad.
    /// En la foto: `raton ev=0 slot=2(=kbd!)` y "nada que adoptar" tres veces.
    fn raton_libre(&self, sale_del_teclado: bool) -> bool {
        match self.raton.as_ref() {
            None => true,
            Some(m) => m.es_provisional() && !sale_del_teclado,
        }
    }

    /// El raton que hay es de verdad, o la interfaz de medios de un teclado?
    fn raton_dedicado(&self) -> bool {
        self.raton.as_ref().is_some_and(|m| !m.es_provisional())
    }

    /// Ya no hace falta mirar mas? Teclado **y** raton dedicado.
    ///
    /// Es la pregunta que decide si merece la pena tocar el bus. Re-enumerar
    /// cuando no falta nada cuesta control transfers sobre un controlador con
    /// dos aparatos ya bombeando, y eso es exactamente lo que no se quiere
    /// hacer sin motivo.
    pub fn completo(&self) -> bool {
        self.teclado.is_some() && self.raton_dedicado()
    }

    /// Enumera UN puerto e instala lo que falte. El camino compartido por el
    /// arranque y por el enchufe en caliente: uno solo, asi no pueden divergir.
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

        // * Sale este aparato del MISMO sitio que mi teclado? Es la pregunta
        // exacta que `es_compuesto` intentaba adivinar contando interfaces, y
        // que fallaba con un raton de macros. Aqui se compara el slot, que es
        // la identidad del aparato en el bus: dos interfaces del mismo aparato
        // comparten slot y las de aparatos distintos no.
        //
        // El segundo termino cubre el orden inverso: si la interfaz de raton
        // del teclado se mira ANTES que la de teclado, todavia no hay slot con
        // el que comparar y hay que fiarse de la forma del aparato.
        let sale_del_teclado = self.teclado.as_ref().is_some_and(|k| k.slot() == slot)
            || (self.teclado.is_none() && compuesto);

        for (iface, clase, subclase, proto) in &ifaces[..n_ifs] {
            // Toda interfaz se DICE antes de juzgarla. Sin esto, un aparato
            // descartado y un aparato ausente se ven exactamente igual --que es
            // lo que costo esta ronda de fotos.
            h.log_u64("[uhid] iface=", *iface as u64);
            h.log_u64(" clase=", *clase as u64);
            h.log_u64(" sub=", *subclase as u64);
            h.log_u64(" proto=", *proto as u64);
            if *clase != enumera::CLASE_HID {
                h.log(" (no es HID)\n");
                continue;
            }
            if *subclase != enumera::SUBCLASE_BOOT {
                // Un HID sin protocolo de arranque manda sus informes en el
                // formato que describa su Report Descriptor.
                //
                // * Y ese descriptor **ya se sabe leer** ([`formato`]), asi que
                // el motivo original de este rechazo ha caducado: hoy la puerta
                // se podria abrir cambiando esta condicion por "que su
                // descriptor declare X e Y". No se hace todavia a proposito --
                // eso cambia QUE aparatos se adoptan en el arranque, y ningun
                // CPU lo ha ejecutado. Primero se confirma en el Ryzen que el
                // descriptor del raton actual se lee bien; despues se ensancha.
                h.log(" (HID sin subclase BOOT: no lo adopto todavia)\n");
                continue;
            }
            let es_teclado = *proto == enumera::PROTO_TECLADO && self.teclado.is_none();
            let es_raton = *proto == enumera::PROTO_RATON && self.raton_libre(sale_del_teclado);
            if !es_teclado && !es_raton {
                h.log(" (ya cubierto)\n");
                continue;
            }
            h.log(" -> lo tomo\n");

            let (_addr, mps, interval, dci) = match enumera::intr_in(cfg, *iface) {
                Some(e) => e,
                None => continue,
            };
            let (buf_phys, buf_virt, protocolo) =
                match enumera::preparar_endpoint(slot, dci, mps, interval, *iface, cfg_val) {
                    Some(b) => b,
                    None => continue,
                };

            let direccion = Direccion::nueva(slot, dci);
            if es_teclado {
                self.teclado = Some(Teclado::nuevo(direccion, *iface, buf_phys, buf_virt, mps));
                cosecha.teclado = true;
                h.log("[uhid] teclado listo\n");
            } else {
                if sale_del_teclado {
                    h.log("[uhid] iface de raton en MI TECLADO: provisional\n");
                }
                // * Se le pasa el `mps` del ENDPOINT, no el tamano del informe:
                // pedirle menos de lo que puede mandar es un babble, y asi fue
                // como este raton se paro nada mas adoptarlo. Ver `Raton::largo`.
                //
                // Y su FORMATO, sacado del Report Descriptor: que bit es cada
                // campo y de cuantos bits. Antes se le pasaba el protocolo a
                // secas y el raton deducia una sola cosa --si habia un Report ID
                // delante--, que arreglaba el corrimiento y dejaba abierto el
                // ancho de los ejes.
                //
                // Si el descriptor no se puede leer o no se entiende, se cae al
                // formato BOOT **conservando el salto del Report ID**, que es lo
                // que ya funciona en el Ryzen. Un reserva que pierde lo
                // aprendido seria una regresion disfrazada de prudencia.
                let formato = enumera::leer_formato_raton(slot, *iface, cfg)
                    .unwrap_or_else(|| formato::Formato::boot_con_id(protocolo == 1));
                if self.instalar_raton(Raton::nuevo(
                    direccion,
                    buf_phys,
                    buf_virt,
                    sale_del_teclado,
                    mps,
                    formato,
                )) {
                    cosecha.raton = true;
                    h.log("[uhid] raton listo\n");
                }
            }
        }

        if cosecha.teclado || cosecha.raton {
            // De aqui salio algo que funciona: este puerto no se vuelve a
            // tocar. Resetearlo solo podria matarlo.
            self.puertos.take(port);
        } else {
            // * Y si no salio nada, **el slot se devuelve**. El aparato quedo
            // direccionado y nadie lo va a leer; dejar el slot pedido es lo que
            // agoto los 64 del controlador en el arranque del 2026-07-31, con
            // el registro contandolo a la vista: 0x30, 0x31, ... 0x40, y despues
            // `cc=0x9` para siempre.
            //
            // Con un seguro: **jamas el slot de un aparato instalado**. Un
            // teclado compuesto puede volver a ofrecer su interfaz de raton,
            // que se rechaza por chocar de direccion -- y entonces "no se adopto
            // nada" seria cierto y devolver el slot mataria al teclado que esta
            // escribiendo en el. Devolver un recurso que otro esta usando es
            // peor que no devolverlo.
            if slot == self.kbd_slot() || slot == self.mouse_slot() {
                h.log_u64("[uhid] no devuelvo el slot: lo usa un aparato vivo, ", slot as u64);
            } else {
                h.log_u64("[uhid] nada que adoptar, devuelvo el slot ", slot as u64);
                bmo_xhci::disable_slot(slot);
            }
        }
        cosecha
    }

    /// Enciende las bombas de lo que este instalado y todavia parado.
    ///
    /// Se llama al final de la enumeracion y tras cada adopcion. El guardia
    /// `bombeando()` no es cosmetico: encolar dos veces en el mismo endpoint
    /// deja dos TRB vivos y el segundo informe llega a un buffer que ya nadie
    /// espera.
    fn arrancar_bombas(&mut self) {
        let h = bmo_xhci::hal();
        if let Some(k) = self.teclado.as_mut() {
            if !k.bombeando() && !k.arrancar() {
                // Sin anillo no hay transferencia, y sin transferencia el
                // teclado enmudece para siempre. Callarlo fue lo que costo las
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

    /// **Algo se enchufo en `port`: adoptalo.**
    ///
    /// * Esta es la mitad que faltaba. El aviso de cambio de puerto ya llegaba
    /// --la foto lo enseno, `usb: puerto: algo se ENCHUFO (sin re-enumerar aun)`--
    /// y **nadie hacia nada con el**. El comentario decia "primero ver, luego
    /// hacer"; ya se vio.
    ///
    /// Sin esto, la enumeracion era una carrera de un solo intento contra el
    /// arranque: un aparato que tarda en engancharse --un raton con firmware RGB
    /// tarda-- se perdia **hasta el siguiente reinicio**. De ahi el sintoma que
    /// no encajaba con nada: unas veces arrancaba el teclado y otras el raton,
    /// nunca los dos, sin tocar una linea de codigo entre arranque y arranque.
    /// No era intermitencia del hardware: era quien llegaba a tiempo.
    ///
    /// Devuelve `true` si se instalo algo nuevo.
    ///
    /// # Safety
    /// Toca MMIO del xHC: hay que llamarlo con el CR3 del kernel puesto.
    pub unsafe fn adoptar_puerto(&mut self, port: u8) -> bool {
        // Si no falta nada, no se toca el bus. Re-enumerar por gusto mete
        // control transfers en un controlador con dos aparatos bombeando.
        if self.completo() {
            return false;
        }
        // * Y aunque falte algo: **a este puerto en concreto, se le puede
        // tocar?** Esto es lo que faltaba, y sin ello la adopcion reactiva se
        // comia a si misma -- resetear un puerto ES un cambio de puerto, asi que
        // el aviso que la dispara lo genera ella misma, para siempre. Peor: el
        // puerto que giraba era el del teclado ya enumerado, que moria con el
        // primer reset. Ver [`puertos`].
        if !self.puertos.se_puede_intentar(port) {
            return false;
        }
        // Contar ANTES de tocar el bus: si la enumeracion se va por otro
        // camino, el intento ya esta gastado.
        self.puertos.anotar_intento(port);
        let cosecha = self.cosechar_puerto(port);
        if cosecha.teclado || cosecha.raton {
            self.arrancar_bombas();
            return true;
        }
        false
    }
}

/// Que se instalo al mirar un puerto.
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

        // -- Recorrer los puertos ------------------------------------------
        for port in 0..ctrl.max_ports {
            let cosecha = unsafe { self.cosechar_puerto(port) };

            // Solo se corta con un raton DEDICADO. Un teclado compuesto marca
            // las dos banderas --su interfaz de medios cuenta como raton-- y
            // cortar ahi dejaba el puerto del raton de verdad sin visitar jamas.
            if cosecha.teclado && cosecha.raton && self.raton_dedicado() {
                break;
            }
        }

        // -- Arrancar las bombas, AL FINAL ---------------------------------
        //
        // Un endpoint de interrupcion, en cuanto se le encola una transferencia
        // y se toca su timbre, **empieza a postear eventos solo**. Si eso pasa
        // mientras todavia se enumera el puerto siguiente, sus informes caen en
        // el anillo compartido en medio de los control transfers del otro
        // aparato. Ver el aparcadero de `bmo_xhci`.
        //
        // Y un segundo motivo: el raton PROVISIONAL de un teclado compuesto se
        // arrancaba y luego se sustituia por el dedicado, dejando una
        // transferencia viva en un endpoint que ya no lee nadie.
        self.arrancar_bombas();
        let _ = h;

        self.inicializado = true;
        self.teclado.is_some()
    }

    fn name(&self) -> &'static str { "USB-HID" }

    /// El REPARTO. Un evento, un dueno.
    ///
    /// `else if` y no dos `if` sueltos: ver la nota de la cabecera del modulo.
    fn poll(&mut self, buf: &mut [InputEvent]) -> usize {
        if !self.inicializado {
            return 0;
        }
        let mut n = 0usize;
        unsafe {
            while let Some((slot, ep, cc)) = bmo_xhci::poll_transfer_event() {
                // * Aunque no quede sitio para mas eventos, el informe hay que
                // ATENDERLO: atender es lo que rearma la transferencia, y sin
                // rearmar el periferico se para para siempre. Se le pasa la
                // rodaja que quede, aunque este vacia -- se pierde el evento de
                // entrada, que es recuperable; no la bomba, que no lo es.
                let desde = n.min(buf.len());
                let resto = &mut buf[desde..];

                // * RESUCITAR ANTES DE ATENDER, y aqui y no dentro de cada
                // aparato: `atender` termina rearmando, y rearmar un endpoint
                // parado no hace nada --el xHC ignora el timbre de un endpoint
                // Halted--. Ese es el aparato que "se desconecta" sin que nadie
                // lo toque: sigue enumerado, sigue teniendo anillo, y no vuelve.
                //
                // Va en el reparto porque el reparto es el unico sitio que ya
                // sabe de quien es el evento. Metido en `teclado` y en `raton`
                // serian dos copias de la misma decision, que es exactamente
                // como se colo el bug del Ctrl derecho.
                if bmo_xhci::cc_halta_endpoint(cc) && self.es_de_alguien(slot, ep) {
                    bmo_xhci::recuperar_endpoint(slot, ep);
                }

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
