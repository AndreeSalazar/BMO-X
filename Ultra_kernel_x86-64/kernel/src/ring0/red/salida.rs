//! **LA SALIDA** -- el anillo de transmision de la tarjeta, con el plano, los
//! vuelos y el grifo de `bmo_net::tx`.
//!
//! [carril]  ROJO      enciende `CR.TE` y le dice a la tarjeta de donde leer
//! [consumo] APARATO   el transmisor queda encendido desde el primer pase; la
//!                     tarjeta solo lee cuando se toca la campana
//!
//! # E3 de `docs/plan/PLAN_RED_TX.md` (2026-09-13)
//!
//! Todo lo que aqui se puede equivocar sin fallo ya se probo en el anfitrion
//! (`cargo test -p bmo-net`): el `EOR` unico, el largo acotado, el orden de los
//! vuelos y el grifo. Este fichero solo pide la memoria, se la presta a la
//! tarjeta en el titular y escribe lo que el plano aprueba.
//!
//! [!] **Nadie de Ring 3 llega aqui con un puntero.** `enviar` recibe una COPIA
//! que ya esta en memoria del kernel (ver `puerta.rs`), y el bufer donde la
//! tarjeta lee es del corral, no del buzon.

use bmo_net::tx;

use crate::ring0::mm;

static mut PLANO: Option<tx::Plan> = None;
static mut VUELOS: tx::Vuelos = tx::Vuelos::nuevo();
static mut GRIFO: tx::Grifo = tx::Grifo::cerrado();

/// Por que una trama no llego a la campana.
pub enum Fallo {
    SinArmar,
    Grifo(tx::NoSale),
    /// Las 16 casillas las tiene la tarjeta. `hay_sitio` lo pregunta antes, asi
    /// que llegar aqui es que la tarjeta no devolvio entre las dos preguntas.
    Anillo,
}

/// Esta armado el transmisor?
pub fn armada() -> bool {
    unsafe { PLANO.is_some() }
}

unsafe fn desc(p: &tx::Plan, i: usize) -> *mut tx::TxDesc {
    (mm::phys_to_virt(p.descriptores()) as *mut tx::TxDesc).add(i)
}

/// **Arma el transmisor.** Idempotente. `false` y el motivo en CABINA si no.
///
/// El orden es el de la familia 8168: el anillo en memoria, `TNPDS`, `CR.TE`, y
/// `TCR` DESPUES de `TE` -- en esta familia `TxConfig` escrito con el
/// transmisor apagado no se queda (lo mismo hace `r8169` en `hw_start`).
pub fn armar(mmio: *mut u8) -> bool {
    if armada() {
        return true;
    }
    if mmio.is_null() {
        crate::ring0::cabina::warn("red", "no hay NIC legible: el transmisor no arma", 0);
        return false;
    }
    let bytes = tx::bytes_necesarios();
    let paginas = (bytes + mm::PAGE - 1) / mm::PAGE;
    // El corral lo pide `red/mod.rs`: el censo del NEUTRO cuenta un sitio por aparato.
    let Some(arena) = super::pedir_corral(paginas) else {
        crate::ring0::cabina::fault("red", "sin marcos contiguos para el corral de SALIDA", paginas);
        return false;
    };
    let plan = match tx::Plan::nuevo(arena, paginas * mm::PAGE) {
        Ok(p) => p,
        Err(_) => {
            crate::ring0::cabina::fault("red", "el corral de salida no pasa su propia revision", arena);
            return false;
        }
    };
    // ** PRESTADO ANTES DE QUE LA TARJETA SEPA DONDE MIRAR. Igual que la entrada.
    if crate::ring0::mm::titular::prestar_tramo(arena, paginas, crate::ring0::mm::titular::APARATO_NIC).is_err() {
        crate::ring0::cabina::fault("red", "el corral de SALIDA no se pudo prestar en el titular", paginas);
        return false;
    }
    unsafe {
        for i in 0..tx::ANILLO {
            match plan.quieto(i) {
                Some(d) => core::ptr::write_volatile(desc(&plan, i), d),
                None => {
                    crate::ring0::cabina::fault("red", "el plano de salida rechazo un descriptor", i as u64);
                    crate::ring0::mm::titular::devolver_tramo(arena, crate::ring0::mm::titular::APARATO_NIC);
                    return false;
                }
            }
        }
        core::sync::atomic::fence(core::sync::atomic::Ordering::SeqCst);

        use bmo_net::{cr, reg_rx};
        super::w8(mmio, reg_rx::CFG9346, 0xC0);
        let anillo = plan.descriptores();
        super::w32(mmio, tx::reg_tx::TNPDS_LO, (anillo & 0xFFFF_FFFF) as u32);
        super::w32(mmio, tx::reg_tx::TNPDS_HI, (anillo >> 32) as u32);
        let c = super::r8(mmio, reg_rx::CR);
        super::w8(mmio, reg_rx::CR, c | cr::TE);
        super::w32(mmio, tx::reg_tx::TCR, tx::TCR_VALOR);
        super::w8(mmio, reg_rx::CFG9346, 0x00);

        PLANO = Some(plan);
        *core::ptr::addr_of_mut!(VUELOS) = tx::Vuelos::nuevo();
    }
    crate::ring0::cabina::addr("red", "transmisor ARMADO (CR.TE), corral de salida en la fisica", plan.descriptores());
    true
}

/// Queda una casilla libre en el anillo? Se pregunta ANTES de sacar la trama del
/// buzon: sacarla sin sitio seria perderla.
pub fn hay_sitio() -> bool {
    armada() && unsafe { (*core::ptr::addr_of!(VUELOS)).proxima().is_ok() }
}

/// **Juzga, copia al corral y toca la campana.** Devuelve el largo que salio.
pub fn enviar(mmio: *mut u8, trama: &[u8], mi_mac: bmo_net::Mac, ahora: u64) -> Result<usize, Fallo> {
    let Some(plan) = (unsafe { PLANO }) else {
        return Err(Fallo::SinArmar);
    };
    if mmio.is_null() {
        return Err(Fallo::SinArmar);
    }
    let vuelos = unsafe { &mut *core::ptr::addr_of_mut!(VUELOS) };
    let grifo = unsafe { &mut *core::ptr::addr_of_mut!(GRIFO) };
    let i = vuelos.proxima().map_err(|_| Fallo::Anillo)?;
    let Some(buf) = plan.bufer(i) else {
        return Err(Fallo::SinArmar);
    };
    // El destino es el bufer del CORRAL, en el espejo del kernel.
    let salida = unsafe { core::slice::from_raw_parts_mut(mm::phys_to_virt(buf) as *mut u8, tx::BUFER as usize) };
    let largo = grifo.juzgar(trama, mi_mac, ahora, salida).map_err(Fallo::Grifo)?;
    let Some(d) = plan.para_enviar(i, largo) else {
        return Err(Fallo::Grifo(tx::NoSale::SinSitio));
    };
    vuelos.despegar(i, ahora).map_err(|_| Fallo::Anillo)?;
    unsafe {
        core::ptr::write_volatile(desc(&plan, i), d);
        core::sync::atomic::fence(core::sync::atomic::Ordering::SeqCst);
        super::w8(mmio, tx::reg_tx::TPPOLL, tx::tppoll::NPQ);
    }
    Ok(largo)
}

/// **Recoge lo que la tarjeta ya envio** y deja sus casillas quietas otra vez.
pub fn recoger(ahora: u64) -> u32 {
    let Some(plan) = (unsafe { PLANO }) else {
        return 0;
    };
    let vuelos = unsafe { &mut *core::ptr::addr_of_mut!(VUELOS) };
    let mut n = 0;
    for _ in 0..tx::ANILLO {
        let Some(i) = vuelos.mas_vieja() else { break };
        let d = unsafe { core::ptr::read_volatile(desc(&plan, i)) };
        match vuelos.recoger(d.lo_tiene_la_tarjeta(), ahora) {
            tx::Recogida::Nada => break,
            tx::Recogida::Aterrizo { casilla, .. } => {
                if let Some(q) = plan.quieto(casilla) {
                    unsafe { core::ptr::write_volatile(desc(&plan, casilla), q) };
                }
                n += 1;
            }
        }
    }
    n
}

pub fn abrir_grifo(ahora: u64, ms: u64, cupo: u32) -> (u64, u32) {
    unsafe { (*core::ptr::addr_of_mut!(GRIFO)).abrir(ahora, ms, cupo) }
}

pub fn cerrar_grifo() {
    unsafe { (*core::ptr::addr_of_mut!(GRIFO)).cerrar() }
}

pub fn cupo() -> u32 {
    unsafe { (*core::ptr::addr_of!(GRIFO)).cupo() }
}

/// `(despegues, aterrizajes)`: tramas dadas a la tarjeta y tramas que la tarjeta
/// DEVOLVIO enviadas. **La diferencia es la pregunta de E3**: si una despega y no
/// aterriza nunca, el grifo la dejo salir y la tarjeta no la puso en el cable.
pub fn vuelos() -> (u64, u64) {
    let v = unsafe { &*core::ptr::addr_of!(VUELOS) };
    (v.despegues, v.aterrizajes)
}

/// `(salieron, negadas, codigo del ultimo no)`.
pub fn contadores() -> (u64, u64, u32) {
    let g = unsafe { &*core::ptr::addr_of!(GRIFO) };
    (g.salieron, g.negadas, g.ultimo_no.map_or(0, |n| n.codigo()))
}
