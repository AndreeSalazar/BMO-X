//! **Un DIRECTOR de mentira** para el banco: quien contesta `MI_PADRE`, acepta
//! la oferta de una superficie y deja eventos en su buzon mientras la app
//! duerme. Con el se prueba el camino de la VENTANA sin que exista el
//! compositor -- que es Rust normal y no corre aqui.
//!
//! ** Lo que NO es: el DIRECTOR. No compone, no recorta, no decide VISTAS.
//! Lee la cabecera BSUP con la misma aritmetica que `scene/surface.rs` y
//! escribe en el buzon con la de `keys/app.rs`: si esas dos cambian, esto
//! tiene que cambiar con ellas, y el juez de los numeros es `superficie.rs`
//! del ABI.

use super::Machine;

impl Machine {
    /// Un `u32` de memoria, como lo leeria `campo()` del DIRECTOR.
    fn read_u32_mem(&self, addr: u64) -> u32 {
        let mut v = 0u32;
        for i in 0..4 {
            v |= (self.read_u8_mem(addr + i) as u32) << (i * 8);
        }
        v
    }

    fn write_u32(&mut self, addr: u64, value: u32) {
        for i in 0..4 {
            self.mem.insert(addr + i, ((value >> (i * 8)) & 0xFF) as u8);
        }
    }

    /// **El DIRECTOR de mentira reparte**: deja UN evento pendiente en el
    /// buzon de la primera superficie ofrecida, si hay sitio. Lee la cabecera
    /// BSUP como la lee `scene/surface.rs` (campo 6 = donde empieza el buzon,
    /// campo 7 = ranuras) y avanza la CABEZA; la cola es de la app.
    pub(super) fn repartir_buzon(&mut self) {
        use bmo_abi::syscalls::surface::{SUP_BUZON_CABECERA, SUP_BUZON_RANURA};
        let Some(&(base, desde, _, _)) = self.ofertas.first() else {
            return;
        };
        let Some(e) = self.buzon_pendiente.front().copied() else {
            return;
        };
        let s = base + desde;
        let buz = self.read_u32_mem(s + 24) as u64;
        let ranuras = self.read_u32_mem(s + 28) as u64;
        if buz == 0 || ranuras == 0 {
            return;
        }
        let cabeza = self.read_u32_mem(s + buz) as u64;
        let cola = self.read_u32_mem(s + buz + 4) as u64;
        let mascara = ranuras - 1;
        if ((cabeza + 1) & mascara) == (cola & mascara) {
            return; // lleno: se queda pendiente, como en el DIRECTOR
        }
        self.write_u64(s + buz + SUP_BUZON_CABECERA + (cabeza & mascara) * SUP_BUZON_RANURA, e);
        self.write_u32(s + buz, ((cabeza + 1) & mascara) as u32);
        self.buzon_pendiente.pop_front();
    }

}
