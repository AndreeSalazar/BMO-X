//! **El ratón USB, y nada más.**
//!
//! Hermano de [`crate::teclado`]: mismo esqueleto (dirección, buffer, arrancar,
//! atender, rearmar) y **cero conocimiento del otro**. Lo único que comparten
//! es la forma, no el estado.
//!
//! ## El informe de un ratón boot
//!
//! Cuatro bytes: botones, dx, dy, rueda. `dx`/`dy` son **relativos y con
//! signo** — un ratón no dice dónde está, dice cuánto se ha movido; quién lleva
//! la posición absoluta y la recorta a la pantalla es el kernel, que es el que
//! sabe de qué tamaño es el panel.
//!
//! ★ El cuarto byte, la rueda, es opcional en el informe boot estricto. Se lee
//! igual porque todos los ratones reales lo mandan, y porque `queue_interrupt_in`
//! pide 4 bytes: si el ratón sólo manda 3, el Transfer Event llega con `cc=13`
//! (Short Packet) y el cuarto byte se queda a cero, que es "no giró". Por eso
//! `cc=13` se trata como bueno y no como error.

use crate::dir::Direccion;
use bmo_input::event::InputEvent;

/// El informe del protocolo BOOT de un ratón.
#[repr(C)]
#[derive(Clone, Copy)]
struct Informe {
    botones: u8,
    dx: i8,
    dy: i8,
    rueda: i8,
}

/// Cuántos bytes ocupa el informe BOOT de un ratón: botones, dx, dy, rueda.
///
/// ★ **No es lo que se le pide al bus.** Ver [`Raton::largo`]: se le pide lo que
/// el endpoint declara poder mandar, que puede ser más. Pedir menos es un
/// *babble* por construcción.
pub const INFORME_BYTES: u16 = 4;

/// Un ratón USB enumerado y listo.
pub struct Raton {
    dir: Direccion,
    buf_phys: u64,
    buf_virt: *mut u8,
    botones_previos: u8,
    /// ¿Vino de la interfaz de ratón de un TECLADO en vez de un ratón de
    /// verdad?
    ///
    /// Muchos teclados exponen una segunda interfaz HID con protocolo de ratón
    /// (protocol=2) para sus teclas de medios. Sirve como suplente, pero un
    /// ratón dedicado que aparezca después lo sustituye — y hay que recordar
    /// cuál es cuál para saber si se puede sustituir.
    provisional: bool,
    bombeando: bool,
    /// Lo que el ENDPOINT dice que puede mandar de una vez.
    mps: u16,
    /// Cuántas veces el xHC contestó con un error de transferencia.
    errores: u32,
}

impl Raton {
    pub fn nuevo(
        dir: Direccion,
        buf_phys: u64,
        buf_virt: *mut u8,
        provisional: bool,
        mps: u16,
    ) -> Self {
        Self {
            dir,
            buf_phys,
            buf_virt,
            botones_previos: 0,
            provisional,
            bombeando: false,
            mps,
            errores: 0,
        }
    }

    /// **Cuántos bytes se le piden al bus.**
    ///
    /// ★ El informe BOOT son 4 bytes y aquí se pedían 4 — y ése era el bug. El
    /// endpoint de este ratón declara `mps=8`: el aparato manda 8, no caben en
    /// los 4 que se le reservaron, y el xHC lo marca **Babble** (`cc=3`) y
    /// **para el endpoint**. En la foto salió como `lev=3:3:3` y un ratón
    /// enumerado que no se movía nunca.
    ///
    /// El teclado nunca lo sufrió porque su informe mide exactamente su `mps`.
    /// La regla es del bus, no del informe: **a un endpoint de interrupción se
    /// le ofrece siempre su paquete entero**, y lo que sobre se ignora al
    /// descifrar. El buffer es una página de 4 KiB: cabe cualquier `mps`.
    fn largo(&self) -> u16 {
        if self.mps == 0 { INFORME_BYTES } else { self.mps }
    }

    /// Errores de transferencia vistos. Si sube, el aparato y el driver no se
    /// están entendiendo.
    pub fn errores(&self) -> u32 { self.errores }

    pub fn direccion(&self) -> Direccion { self.dir }
    pub fn slot(&self) -> u8 { self.dir.slot }
    pub fn dci(&self) -> u8 { self.dir.dci }
    pub fn es_provisional(&self) -> bool { self.provisional }
    pub fn bombeando(&self) -> bool { self.bombeando }

    /// Encola la primera transferencia y toca el timbre. Igual que en el
    /// teclado: hasta que se llama, está enumerado y mudo.
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
    pub fn atender(&mut self, cc: u8, salida: &mut [InputEvent]) -> usize {
        let mut n = 0usize;
        self.bombeando = false;

        if cc == 1 || cc == 13 {
            let informe = unsafe { core::ptr::read_volatile(self.buf_virt as *const Informe) };
            n = self.descifrar(&informe, salida);
        } else {
            // Un `cc` que no es éxito ni "corto" es un error del bus, y los tres
            // que importan —3 Babble, 4 Transaction, 6 Stall— dejan el endpoint
            // **parado**: rearmarlo no lo despierta, hace falta un Reset
            // Endpoint. Decirlo con su número en vez de reintentar en silencio,
            // que es como un ratón acaba "enumerado y quieto para siempre".
            self.errores = self.errores.saturating_add(1);
            let h = bmo_xhci::hal();
            h.log_u64("[uhid] raton: transferencia con error cc=", cc as u64);
            h.log_u64("  (mps=", self.mps as u64);
            h.log(")\n");
        }

        self.rearmar();
        n
    }

    fn rearmar(&mut self) {
        let largo = self.largo();
        unsafe {
            if bmo_xhci::queue_interrupt_in(self.dir.slot, self.dir.dci, self.buf_phys, largo) {
                bmo_xhci::ring_doorbell(self.dir.slot, self.dir.dci);
                self.bombeando = true;
            }
        }
    }

    /// El informe → eventos de mover, pulsar y girar.
    ///
    /// Los tres van por separado a propósito: un movimiento sin cambio de
    /// botón no debe generar un evento de botón, o el consumidor vería un
    /// clic sostenido en cada informe.
    fn descifrar(&mut self, informe: &Informe, salida: &mut [InputEvent]) -> usize {
        let mut n = 0usize;

        if (informe.dx != 0 || informe.dy != 0) && n < salida.len() {
            salida[n] = InputEvent::mouse_move(informe.dx as i16, informe.dy as i16);
            n += 1;
        }
        // Sólo el CAMBIO. Un ratón manda su informe muchas veces por segundo
        // con los botones tal como estén.
        if informe.botones != self.botones_previos && n < salida.len() {
            salida[n] = InputEvent::mouse_button(informe.botones);
            self.botones_previos = informe.botones;
            n += 1;
        }
        if informe.rueda != 0 && n < salida.len() {
            salida[n] = InputEvent::mouse_wheel(informe.rueda);
            n += 1;
        }

        n
    }
}
