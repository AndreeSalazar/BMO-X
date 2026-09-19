//! **LAS PAGINAS**: por donde escribe el programa, y donde no puede.
//!
//! Aparte de `mod.rs` desde el 2026-09-19: la proteccion de solo lectura hizo
//! crecer el emulador por encima de las 1.000 lineas de L6a, y se pago sacando
//! la pregunta entera a su fichero, no subiendo el techo.

use super::Machine;

impl Machine {
    pub(super) fn write_u64(&mut self, addr: u64, value: u64) {
        for i in 0..8 {
            self.escribe(addr + i, ((value >> (i * 8)) & 0xFF) as u8);
        }
    }

    /// **El UNICO sitio por donde el PROGRAMA escribe memoria.** Si cae en una
    /// pagina de solo lectura, revienta como el Ryzen: con la direccion y el
    /// `rip`. (Lo que escribe el KERNEL emulado --`sistema.rs`, el reparto del
    /// DIRECTOR-- va por su camino, igual que en metal va por la fisica.)
    pub(super) fn escribe(&mut self, addr: u64, b: u8) {
        if self.solo_lectura.iter().any(|&(d, h)| addr >= d && addr < h) {
            panic!(
                "#PF: el programa escribe en una pagina de SOLO LECTURA en {addr:#x} \
                 (rip {:#x}) -- en el Ryzen esto es un fallo de pagina",
                self.rip
            );
        }
        self.mem.insert(addr, b);
    }
}
