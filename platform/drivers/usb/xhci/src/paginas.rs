//! **LAS PAGINAS DE DMA DE CADA RANURA, pedidas UNA vez.**
//!
//! [carril]  AMARILLO  decide que pagina fisica recibe el xHC para escribir
//! [cuesta]  DATO      una pagina entregada a dos usos a la vez es un dato pisado
//! [riesgo]  ESPEJO    la tabla y el controlador tienen que creer lo mismo sobre
//!                     que pagina es de quien
//!
//! # La fuga que cierra (S3b.2a de `docs/plan/PLAN_CLOUD_LOCAL.md`, 2026-09-14)
//!
//! El HAL solo sabe PEDIR paginas; devolverlas no existe. Y se pedian nuevas:
//!
//! ```text
//!    cada transferencia de control con datos   1 pagina   (cada LED del teclado)
//!    cada Address Device (cada enchufe)        3 paginas  (anillo EP0 y dos contextos)
//!    cada Configure Endpoint                   2 paginas  (contexto y anillo)
//! ```
//!
//! Ahora cada una tiene SU sitio en una tabla por ranura (y por endpoint): la
//! primera vez se pide, las siguientes se reutiliza. El total deja de crecer con
//! el USO y pasa a estar acotado por el numero de ranuras que el controlador da.
//!
//! ** Reutilizar es seguro porque una ranura es EXCLUSIVA: la usa un solo
//! aparato a la vez, y el xHC no la toca despues de `Disable Slot`. Por eso la
//! cuenta es por NUMERO de ranura y no por aparato. Un anillo de endpoint VIVO no
//! pasa por aqui: ver `configure_endpoint`.
//!
//! [!] Lo que NO resuelve: devolver paginas al sistema. Un kernel que ha tenido
//! doce aparatos a la vez se queda con las paginas de doce ranuras. Es un techo,
//! no una fuga.

/// Las mismas que `enumerar::MAX_SLOTS`: una fila por numero de ranura.
pub(crate) const RANURAS: usize = 255;
/// DCI de 0 a 31: el 1 es EP0 (va por `Uso::AnilloEp0`), los demas por aqui.
pub(crate) const ENDPOINTS: usize = 32;

/// Para que se usa una pagina de ranura.
#[derive(Clone, Copy)]
pub(crate) enum Uso {
    AnilloEp0 = 0,
    Entrada = 1,
    Dispositivo = 2,
    Datos = 3,
}

/// **La tabla, pura.** Sabe cuando pedir y cuando reutilizar; `pedir` es el HAL.
pub(crate) struct Reserva {
    ranuras: [[u64; 4]; RANURAS],
    endpoints: [[u64; ENDPOINTS]; RANURAS],
    pub(crate) pedidas: u64,
    pub(crate) reutilizadas: u64,
}

impl Reserva {
    pub(crate) const fn nueva() -> Self {
        Self { ranuras: [[0; 4]; RANURAS], endpoints: [[0; ENDPOINTS]; RANURAS], pedidas: 0, reutilizadas: 0 }
    }

    fn tomar(hueco: &mut u64, pedidas: &mut u64, reutilizadas: &mut u64, pedir: &mut dyn FnMut() -> Option<u64>) -> Option<u64> {
        if *hueco != 0 {
            *reutilizadas += 1;
            return Some(*hueco);
        }
        // Si el sistema no da, el hueco se queda vacio: la proxima vez se vuelve
        // a intentar, en vez de recordar un fallo como si fuera una pagina.
        let p = pedir()?;
        *hueco = p;
        *pedidas += 1;
        Some(p)
    }

    pub(crate) fn de_ranura(&mut self, ranura: u8, uso: Uso, pedir: &mut dyn FnMut() -> Option<u64>) -> Option<u64> {
        let r = ranura as usize;
        if r == 0 || r >= RANURAS {
            return None;
        }
        Self::tomar(&mut self.ranuras[r][uso as usize], &mut self.pedidas, &mut self.reutilizadas, pedir)
    }

    pub(crate) fn de_endpoint(&mut self, ranura: u8, dci: u8, pedir: &mut dyn FnMut() -> Option<u64>) -> Option<u64> {
        let (r, d) = (ranura as usize, dci as usize);
        if r == 0 || r >= RANURAS || d < 2 || d >= ENDPOINTS {
            return None;
        }
        Self::tomar(&mut self.endpoints[r][d], &mut self.pedidas, &mut self.reutilizadas, pedir)
    }
}

static mut RESERVA: Reserva = Reserva::nueva();

/// La pagina de `uso` de la ranura: pedida la primera vez, reutilizada despues.
pub(crate) unsafe fn de_ranura(ranura: u8, uso: Uso) -> Option<u64> {
    let h = crate::hal();
    (*core::ptr::addr_of_mut!(RESERVA)).de_ranura(ranura, uso, &mut || h.alloc_dma_pages(1))
}

/// El anillo de un endpoint que NO esta vivo: pedido la primera vez, reutilizado despues.
pub(crate) unsafe fn de_endpoint(ranura: u8, dci: u8) -> Option<u64> {
    let h = crate::hal();
    (*core::ptr::addr_of_mut!(RESERVA)).de_endpoint(ranura, dci, &mut || h.alloc_dma_pages(1))
}

/// **`(paginas pedidas al sistema, veces que se reutilizo una)`.** Si la primera
/// sube con cada Bloq Mayus, la fuga ha vuelto.
pub fn cuentas_dma() -> (u64, u64) {
    unsafe {
        let r = &*core::ptr::addr_of!(RESERVA);
        (r.pedidas, r.reutilizadas)
    }
}

#[cfg(test)]
mod pruebas {
    extern crate std;
    use super::*;
    use std::boxed::Box;

    fn contador() -> (impl FnMut() -> Option<u64>, std::rc::Rc<std::cell::Cell<u64>>) {
        let n = std::rc::Rc::new(std::cell::Cell::new(0u64));
        let m = n.clone();
        (move || {
            m.set(m.get() + 1);
            Some(0x1000 * m.get())
        }, n)
    }

    /// *** LA FUGA: cien LEDs del teclado piden UNA pagina, no cien.
    #[test]
    fn cien_transferencias_de_control_una_sola_pagina() {
        let mut r = Box::new(Reserva::nueva());
        let (mut pedir, veces) = contador();
        let primera = r.de_ranura(3, Uso::Datos, &mut pedir).unwrap();
        for _ in 0..99 {
            assert_eq!(r.de_ranura(3, Uso::Datos, &mut pedir), Some(primera));
        }
        assert_eq!((veces.get(), r.pedidas, r.reutilizadas), (1, 1, 99));
    }

    /// Diez enchufes en la misma ranura: las tres paginas de direccionar, una vez.
    #[test]
    fn diez_enchufes_tres_paginas() {
        let mut r = Box::new(Reserva::nueva());
        let (mut pedir, veces) = contador();
        for _ in 0..10 {
            for uso in [Uso::AnilloEp0, Uso::Entrada, Uso::Dispositivo] {
                r.de_ranura(5, uso, &mut pedir).unwrap();
            }
        }
        assert_eq!(veces.get(), 3);
    }

    #[test]
    fn cada_uso_y_cada_ranura_tiene_la_suya() {
        let mut r = Box::new(Reserva::nueva());
        let (mut pedir, _) = contador();
        let a = r.de_ranura(1, Uso::Datos, &mut pedir).unwrap();
        let b = r.de_ranura(1, Uso::Entrada, &mut pedir).unwrap();
        let c = r.de_ranura(2, Uso::Datos, &mut pedir).unwrap();
        let d = r.de_endpoint(1, 3, &mut pedir).unwrap();
        let e = r.de_endpoint(1, 4, &mut pedir).unwrap();
        let todas = [a, b, c, d, e];
        for i in 0..todas.len() {
            for j in i + 1..todas.len() {
                assert_ne!(todas[i], todas[j], "dos usos compartiendo pagina");
            }
        }
    }

    #[test]
    fn lo_que_no_existe_no_pide() {
        let mut r = Box::new(Reserva::nueva());
        let (mut pedir, veces) = contador();
        assert_eq!(r.de_ranura(0, Uso::Datos, &mut pedir), None, "la ranura 0 no existe");
        assert_eq!(r.de_ranura(255, Uso::Datos, &mut pedir), None);
        assert_eq!(r.de_endpoint(1, 1, &mut pedir), None, "EP0 no va por aqui");
        assert_eq!(r.de_endpoint(1, 32, &mut pedir), None);
        assert_eq!(veces.get(), 0);
    }

    #[test]
    fn si_el_sistema_no_da_se_vuelve_a_intentar() {
        let mut r = Box::new(Reserva::nueva());
        assert_eq!(r.de_ranura(4, Uso::Datos, &mut || None), None);
        assert_eq!(r.de_ranura(4, Uso::Datos, &mut || Some(0x9000)), Some(0x9000), "el fallo no se recordo");
    }
}
