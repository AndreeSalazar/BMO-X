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

/// Cuántos bytes se le piden a un ratón por transferencia.
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
}

impl Raton {
    pub fn nuevo(dir: Direccion, buf_phys: u64, buf_virt: *mut u8, provisional: bool) -> Self {
        Self {
            dir,
            buf_phys,
            buf_virt,
            botones_previos: 0,
            provisional,
            bombeando: false,
        }
    }

    pub fn direccion(&self) -> Direccion { self.dir }
    pub fn slot(&self) -> u8 { self.dir.slot }
    pub fn dci(&self) -> u8 { self.dir.dci }
    pub fn es_provisional(&self) -> bool { self.provisional }
    pub fn bombeando(&self) -> bool { self.bombeando }

    /// Encola la primera transferencia y toca el timbre. Igual que en el
    /// teclado: hasta que se llama, está enumerado y mudo.
    pub fn arrancar(&mut self) -> bool {
        self.bombeando = unsafe {
            bmo_xhci::queue_interrupt_in(self.dir.slot, self.dir.dci, self.buf_phys, INFORME_BYTES)
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
        }

        self.rearmar();
        n
    }

    fn rearmar(&mut self) {
        unsafe {
            if bmo_xhci::queue_interrupt_in(
                self.dir.slot, self.dir.dci, self.buf_phys, INFORME_BYTES,
            ) {
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
