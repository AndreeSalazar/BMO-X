//! **El raton USB, y nada mas.**
//!
//! Hermano de [`crate::teclado`]: mismo esqueleto (direccion, buffer, arrancar,
//! atender, rearmar) y **cero conocimiento del otro**. Lo unico que comparten
//! es la forma, no el estado.
//!
//! ## El informe de un raton
//!
//! Botones, dx, dy y rueda. `dx`/`dy` son **relativos y con signo** -- un raton
//! no dice donde esta, dice cuanto se ha movido; quien lleva la posicion
//! absoluta y la recorta a la pantalla es el kernel, que es el que sabe de que
//! tamano es el panel.
//!
//! * **Donde cae cada uno lo dice el aparato**, no este archivo. El formato
//! sale de su Report Descriptor ([`crate::formato`]) y llega ya resuelto: que
//! bit, cuantos bits y si hay un Report ID delante. Antes se suponia el informe
//! BOOT --cuatro bytes de 8 bits-- y este raton no lo cumple: ignoro el
//! `SET_PROTOCOL` y manda ejes que pueden ser de 16 bits, con lo que el byte
//! que se leia como `dy` era la mitad alta de `dx`.
//!
//! * La rueda es opcional en el informe boot estricto. Se lee igual porque
//! todos los ratones reales la mandan, y porque `queue_interrupt_in` pide el
//! paquete entero: si el raton manda menos, el Transfer Event llega con `cc=13`
//! (Short Packet) y lo que falte se queda a cero, que es "no giro". Por eso
//! `cc=13` se trata como bueno y no como error.

use crate::dir::Direccion;
use crate::formato::Formato;
use bmo_input::event::InputEvent;

/// Cuantos bytes ocupa el informe BOOT de un raton: botones, dx, dy, rueda.
///
/// * **No es lo que se le pide al bus.** Ver [`Raton::largo`]: se le pide lo que
/// el endpoint declara poder mandar, que puede ser mas. Pedir menos es un
/// *babble* por construccion.
pub const INFORME_BYTES: u16 = 4;

/// Un raton USB enumerado y listo.
pub struct Raton {
    dir: Direccion,
    buf_phys: u64,
    buf_virt: *mut u8,
    botones_previos: u8,
    /// Vino de la interfaz de raton de un TECLADO en vez de un raton de
    /// verdad?
    ///
    /// Muchos teclados exponen una segunda interfaz HID con protocolo de raton
    /// (protocol=2) para sus teclas de medios. Sirve como suplente, pero un
    /// raton dedicado que aparezca despues lo sustituye -- y hay que recordar
    /// cual es cual para saber si se puede sustituir.
    provisional: bool,
    bombeando: bool,
    /// Lo que el ENDPOINT dice que puede mandar de una vez.
    mps: u16,
    /// Cuantas veces el xHC contesto con un error de transferencia.
    errores: u32,
    /// **Donde esta cada campo del informe**, dicho por el aparato.
    ///
    /// Antes esto era un solo `usize`: cuantos bytes saltarse. Servia para el
    /// Report ID y para nada mas -- no sabia de anchos, asi que un raton con
    /// ejes de 16 bits seguia leyendose mal aunque el salto fuera correcto.
    formato: Formato,
    /// Informes vistos. Los primeros se ensenan crudos: un formato que no se
    /// entiende no se arregla razonando, se arregla mirandolo.
    vistos: u32,
}

impl Raton {
    pub fn nuevo(
        dir: Direccion,
        buf_phys: u64,
        buf_virt: *mut u8,
        provisional: bool,
        mps: u16,
        formato: Formato,
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
            formato,
            vistos: 0,
        }
    }

    /// El formato con el que se esta descifrando. Para el panel: si el puntero
    /// se mueve mal, lo primero que hay que ver es con que reglas se lee.
    pub fn formato(&self) -> Formato {
        self.formato
    }

    /// Un raton sin bus detras, para probar el DESCIFRADO.
    ///
    /// `descifrar` no toca el buffer de DMA --recibe los bytes ya copiados--, asi
    /// que se puede ejercer entero sin un xHC delante. Sin esto, la conversion
    /// de informe a eventos solo se podia probar en el Ryzen, que es donde
    /// menos falta hace tener dudas.
    #[cfg(test)]
    fn para_pruebas(formato: Formato) -> Self {
        Self {
            dir: Direccion::nueva(0, 0),
            buf_phys: 0,
            buf_virt: core::ptr::null_mut(),
            botones_previos: 0,
            provisional: false,
            bombeando: false,
            mps: 8,
            errores: 0,
            formato,
            vistos: 0,
        }
    }

    /// **Cuantos bytes se le piden al bus.**
    ///
    /// * El informe BOOT son 4 bytes y aqui se pedian 4 -- y ese era el bug. El
    /// endpoint de este raton declara `mps=8`: el aparato manda 8, no caben en
    /// los 4 que se le reservaron, y el xHC lo marca **Babble** (`cc=3`) y
    /// **para el endpoint**. En la foto salio como `lev=3:3:3` y un raton
    /// enumerado que no se movia nunca.
    ///
    /// El teclado nunca lo sufrio porque su informe mide exactamente su `mps`.
    /// La regla es del bus, no del informe: **a un endpoint de interrupcion se
    /// le ofrece siempre su paquete entero**, y lo que sobre se ignora al
    /// descifrar. El buffer es una pagina de 4 KiB: cabe cualquier `mps`.
    fn largo(&self) -> u16 {
        if self.mps == 0 { INFORME_BYTES } else { self.mps }
    }

    /// Errores de transferencia vistos. Si sube, el aparato y el driver no se
    /// estan entendiendo.
    pub fn errores(&self) -> u32 { self.errores }

    pub fn direccion(&self) -> Direccion { self.dir }
    pub fn slot(&self) -> u8 { self.dir.slot }
    pub fn dci(&self) -> u8 { self.dir.dci }
    pub fn es_provisional(&self) -> bool { self.provisional }
    pub fn bombeando(&self) -> bool { self.bombeando }

    /// Encola la primera transferencia y toca el timbre. Igual que en el
    /// teclado: hasta que se llama, esta enumerado y mudo.
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
            // Los tres primeros informes, CRUDOS.
            //
            // Se quedan aunque ya no haya nada que adivinar: son la unica forma
            // de comprobar que el formato leido del descriptor CORRESPONDE con
            // lo que el aparato manda de verdad. Un parser correcto sobre un
            // descriptor que miente da bytes mal con toda la razon del mundo.
            if self.vistos < 3 {
                let h = bmo_xhci::hal();
                h.log("[uhid] raton informe:");
                for i in 0..8usize {
                    let b = unsafe { core::ptr::read_volatile(self.buf_virt.add(i)) };
                    h.log_u64(" ", b as u64);
                }
                h.log_u64("   (id=", self.formato.report_id as u64);
                h.log_u64(" ejes de ", self.formato.x.map_or(0, |c| c.bits) as u64);
                h.log(" bits)\n");
                self.vistos += 1;
            }
            // El informe se copia a la pila ENTERO y desde ahi se descifra. No
            // se puede seguir leyendolo como un `struct` de cuatro bytes: los
            // campos ya no estan en posiciones fijas ni tienen tamano fijo, que
            // es justo lo que este cambio arregla.
            let mut informe = [0u8; MAX_INFORME];
            let salto = self.formato.desplazamiento();
            for (i, b) in informe.iter_mut().enumerate() {
                *b = unsafe { core::ptr::read_volatile(self.buf_virt.add(salto + i)) };
            }
            n = self.descifrar(&informe, salida);
        } else {
            // Un `cc` que no es exito ni "corto" es un error del bus, y los tres
            // que importan --3 Babble, 4 Transaction, 6 Stall-- dejan el endpoint
            // **parado**: rearmarlo no lo despierta, hace falta un Reset
            // Endpoint. Decirlo con su numero en vez de reintentar en silencio,
            // que es como un raton acaba "enumerado y quieto para siempre".
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

    /// El informe -> eventos de mover, pulsar y girar, **segun su formato**.
    ///
    /// Los tres van por separado a proposito: un movimiento sin cambio de
    /// boton no debe generar un evento de boton, o el consumidor veria un
    /// clic sostenido en cada informe.
    fn descifrar(&mut self, informe: &[u8], salida: &mut [InputEvent]) -> usize {
        let mut n = 0usize;

        let dx = self.formato.x.map_or(0, |c| c.leer_con_signo(informe));
        let dy = self.formato.y.map_or(0, |c| c.leer_con_signo(informe));
        if (dx != 0 || dy != 0) && n < salida.len() {
            // Se recorta a `i16` porque es lo que lleva el evento. Un
            // desplazamiento que no cabe en 16 bits no es un movimiento de
            // mano: es un informe mal leido, y saturar deja el puntero pegado
            // al borde en vez de teletransportarlo al otro lado.
            salida[n] = InputEvent::mouse_move(recortar(dx), recortar(dy));
            n += 1;
        }
        // Solo el CAMBIO. Un raton manda su informe muchas veces por segundo
        // con los botones tal como esten.
        let botones = self.formato.botones.map_or(0, |c| c.leer_crudo(informe) as u8);
        if botones != self.botones_previos && n < salida.len() {
            salida[n] = InputEvent::mouse_button(botones);
            self.botones_previos = botones;
            n += 1;
        }
        let rueda = self.formato.rueda.map_or(0, |c| c.leer_con_signo(informe));
        if rueda != 0 && n < salida.len() {
            // El evento lleva la rueda en `i8`, asi que aqui se SATURA en vez
            // de truncar: un `as i8` sobre 256 muescas daria 0, o sea "no
            // giro" -- el silencio con aire de dato.
            salida[n] = InputEvent::mouse_wheel(rueda.clamp(-128, 127) as i8);
            n += 1;
        }

        n
    }
}

/// Cuantos bytes del informe se descifran. Ocho cubren el peor caso real --
/// cinco botones, dos ejes de 16 bits y rueda son seis-- y son los mismos ocho
/// que se registran crudos.
const MAX_INFORME: usize = 8;

fn recortar(v: i32) -> i16 {
    if v > i16::MAX as i32 {
        i16::MAX
    } else if v < i16::MIN as i32 {
        i16::MIN
    } else {
        v as i16
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::formato::Campo;
    use bmo_input::event::InputEventKind;

    fn vacia() -> [InputEvent; 4] {
        [InputEvent::mouse_move(0, 0); 4]
    }

    /// El formato del raton que motivo todo: Report ID delante y ejes de 16
    /// bits. Los bits van sin el Report ID, que ya se salta el llamante.
    fn formato_16() -> Formato {
        Formato {
            report_id: 1,
            botones: Some(Campo { bit: 0, bits: 5 }),
            x: Some(Campo { bit: 8, bits: 16 }),
            y: Some(Campo { bit: 24, bits: 16 }),
            rueda: Some(Campo { bit: 40, bits: 8 }),
            bits: 48,
        }
    }

    /// **El bug entero, en un test.**
    ///
    /// Con el formato BOOT, este informe --`dx = 300`-- se leia como `dx = 44`
    /// (el byte bajo) y `dy = 1` (el alto). O sea: mover en horizontal movia
    /// tambien en vertical. Con el formato del aparato sale `dx=300, dy=0`.
    #[test]
    fn un_eje_de_16_bits_no_se_derrama_en_el_otro() {
        let mut r = Raton::para_pruebas(formato_16());
        let mut ev = vacia();
        // botones=0, dx=300 (0x012C), dy=0, rueda=0
        let n = r.descifrar(&[0x00, 0x2C, 0x01, 0x00, 0x00, 0x00, 0, 0], &mut ev);
        assert_eq!(n, 1);
        assert_eq!(ev[0].kind, InputEventKind::MouseMove);
        assert_eq!(ev[0].mouse_dx(), 300);
        assert_eq!(ev[0].mouse_dy(), 0, "mover en X no puede mover en Y");

        // Y lo que el formato BOOT habria leido del MISMO informe, para que se
        // vea que no es una diferencia teorica.
        let mut b = Raton::para_pruebas(Formato::boot());
        let mut ev2 = vacia();
        b.descifrar(&[0x00, 0x2C, 0x01, 0x00, 0x00, 0x00, 0, 0], &mut ev2);
        assert_eq!(ev2[0].mouse_dx(), 44);
        assert_eq!(ev2[0].mouse_dy(), 1);
    }

    /// Los desplazamientos son con signo, tambien a 16 bits.
    #[test]
    fn el_movimiento_negativo_es_negativo() {
        let mut r = Raton::para_pruebas(formato_16());
        let mut ev = vacia();
        // dx = -1 (0xFFFF), dy = -300 (0xFED4)
        r.descifrar(&[0x00, 0xFF, 0xFF, 0xD4, 0xFE, 0x00, 0, 0], &mut ev);
        assert_eq!(ev[0].mouse_dx(), -1);
        assert_eq!(ev[0].mouse_dy(), -300);
    }

    /// Solo el CAMBIO de botones genera evento. Un raton manda su informe
    /// cientos de veces por segundo con los botones tal como esten: si cada uno
    /// generara evento, un boton pulsado seria un clic sostenido.
    #[test]
    fn los_botones_solo_hablan_cuando_cambian() {
        let mut r = Raton::para_pruebas(formato_16());
        let mut ev = vacia();
        let pulsado = [0x01, 0, 0, 0, 0, 0, 0, 0];

        assert_eq!(r.descifrar(&pulsado, &mut ev), 1, "el primero es un cambio");
        assert_eq!(ev[0].kind, InputEventKind::MouseButton);
        assert_eq!(ev[0].mouse_buttons(), 1);

        assert_eq!(r.descifrar(&pulsado, &mut ev), 0, "sostenido no es un clic");
        assert_eq!(r.descifrar(&[0; 8], &mut ev), 1, "soltarlo si lo es");
        assert_eq!(ev[0].mouse_buttons(), 0);
    }

    /// Un informe quieto no genera nada. Con `SET_IDLE(0)` no deberia llegar,
    /// pero un aparato que lo ignore no puede inundar la cola de eventos.
    #[test]
    fn un_informe_quieto_no_genera_eventos() {
        let mut r = Raton::para_pruebas(formato_16());
        let mut ev = vacia();
        assert_eq!(r.descifrar(&[0; 8], &mut ev), 0);
    }

    /// Un raton sin rueda declarada no inventa muescas. `None` es "este aparato
    /// no lo manda", y leer ceros de un campo que no existe daria lo mismo por
    /// casualidad -- hasta el dia que ahi caiga otro dato.
    #[test]
    fn sin_rueda_declarada_no_hay_rueda() {
        let mut f = formato_16();
        f.rueda = None;
        let mut r = Raton::para_pruebas(f);
        let mut ev = vacia();
        // Byte 5 a 0x7F: donde estaria la rueda si la hubiera.
        let n = r.descifrar(&[0x00, 0, 0, 0, 0, 0x7F, 0, 0], &mut ev);
        assert_eq!(n, 0);
    }

    /// Mover y pulsar a la vez son DOS eventos, y en ese orden.
    #[test]
    fn mover_y_pulsar_a_la_vez_son_dos_eventos() {
        let mut r = Raton::para_pruebas(formato_16());
        let mut ev = vacia();
        let n = r.descifrar(&[0x02, 0x05, 0x00, 0x03, 0x00, 0x00, 0, 0], &mut ev);
        assert_eq!(n, 2);
        assert_eq!(ev[0].kind, InputEventKind::MouseMove);
        assert_eq!(ev[0].mouse_dx(), 5);
        assert_eq!(ev[1].kind, InputEventKind::MouseButton);
        assert_eq!(ev[1].mouse_buttons(), 2);
    }
}
