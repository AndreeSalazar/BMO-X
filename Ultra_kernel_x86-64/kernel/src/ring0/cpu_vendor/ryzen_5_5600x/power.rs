//! **Los contadores de energia de este Zen 3, y solo de este.**
//!
//! Escalon 2 de la seccion 9 de `docs/maestro/AXION_MAESTRO.md`.
//!
//! # Por que esto vive en el PERFIL y no en `ring0/cpu/`
//!
//! Porque los tres MSR de abajo **son de AMD**. No estan en un Intel, no estan
//! en un ARM, y leer un MSR que no existe es un `#GP` -- o sea un fault de
//! kernel, desde un panel que se repinta.
//!
//! La regla de la casa, escrita en `profile.rs`: **se hardcodean los CONTRATOS y
//! se le preguntan los HECHOS al silicio**. Aqui el contrato es *"un Zen tiene
//! RAPL en estas tres direcciones"* --eso se sabe por el modelo y por eso vive
//! en el perfil-- y el hecho es *"cuanto vale una unidad de energia en ESTE
//! chip"*, que se le pregunta.
//!
//! Un perfil que dictara la unidad seria un kernel que informa de vatios falsos
//! el dia que AMD cambie el exponente. Se pregunta.
//!
//! # Lo que estos registros NO son
//!
//! No son un vatimetro. Son el **estimador del propio chip**: el mismo silicio
//! contando lo que cree haber gastado. Sirve para comparar dos instantes de esta
//! maquina --que es justo para lo que se quiere-- y no para publicar cifras.
//! Esta escrito tambien en la seccion 9.5 del plan, y conviene que este en los
//! dos sitios.

use core::arch::asm;

// El TIPO viene del contrato, no de aqui. Este fichero es un LECTOR: sabe DONDE
// estan los numeros en este silicio, y nada mas. Ver `profile::EnergiaCruda`.
use super::super::profile::EnergiaCruda;

/// `PWR_UNIT`: dice **cuanto vale una unidad** de los otros dos.
const MSR_PWR_UNIT: u32 = 0xC001_0299;
/// `CORE_ENERGY_STAT`: energia del nucleo en el que se lee.
const MSR_CORE_ENERGY: u32 = 0xC001_029A;
/// `PKG_ENERGY_STAT`: energia del paquete entero.
const MSR_PKG_ENERGY: u32 = 0xC001_029B;

unsafe fn rdmsr(msr: u32) -> u64 {
    let lo: u32;
    let hi: u32;
    asm!("rdmsr", in("ecx") msr, out("eax") lo, out("edx") hi, options(nomem, nostack));
    ((hi as u64) << 32) | lo as u64
}

/// **Lee los tres registros.** `None` si la unidad no es creible.
///
/// # Por que se comprueba la unidad y no la existencia
///
/// Porque la existencia ya la garantiza el perfil: si este fichero corre, el CPU
/// es el del perfil. Lo que si puede salir mal es leer un cero o un valor
/// absurdo --un firmware que no expone RAPL, una maquina virtual que devuelve
/// basura-- y **un exponente malo no da error: da vatios inventados**, que es
/// mucho peor que no dar ninguno.
///
/// Un exponente fuera de 8..24 no describe ninguna unidad razonable: con 8
/// serian 4 milijulios por incremento y con 24, 60 nanojulios.
pub fn leer() -> Option<EnergiaCruda> {
    let (unidad, nucleo, paquete) = unsafe {
        (rdmsr(MSR_PWR_UNIT), rdmsr(MSR_CORE_ENERGY), rdmsr(MSR_PKG_ENERGY))
    };
    // Bits 12:8 del PWR_UNIT.
    let exp = ((unidad >> 8) & 0x1F) as u8;
    if !(8..=24).contains(&exp) {
        return None;
    }
    Some(EnergiaCruda {
        paquete: paquete as u32,
        nucleo: nucleo as u32,
        exp,
    })
}
