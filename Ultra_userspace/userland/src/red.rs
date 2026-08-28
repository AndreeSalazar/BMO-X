//! **LA RED, DESDE DONDE VIVE EL DUENO.**
//!
//! ## Por que este fichero existe (2026-08-24)
//!
//! El `net` del escritorio esta escrito para **solo informar**, y cuando el
//! dueno pidio `net rx` en el Ryzen le contesto:
//!
//! ```text
//!    receptor    apagado   (net rx en Ring 0)
//! ```
//!
//! Mandandole a un shell **al que no se vuelve**.
//!
//! > Un camino que solo existe en Ring 0 es un camino que el dueno de su propia
//! > maquina no puede tomar.
//!
//! ** Y esto no es "el escritorio toca la NIC": es **Ring 3 pide y el kernel
//! decide**, que es para lo que existe la tabla de operaciones. Con la misma
//! forma que el disco, y con su misma regla -- el kernel lo apunta en CABINA
//! antes y despues.
//!
//! [!] Y NO SE PUEDE TRANSMITIR desde aqui, por construccion: no hay operacion
//! que encienda `CR.TE`. Un error de este lado no puede molestar a nadie mas de
//! la red, y eso es lo que hace que el paso 1 salga gratis.

use crate::{invoke, CURRENT_TASK, OP_RED};

pub const RED_OP_ARMAR: u64 = 0x01;
pub const RED_OP_SONDEAR: u64 = 0x02;

/// **El bit de enlace dentro de `INFO_NET_PHY_CRUDO`.**
///
/// === Por que este numero vive en Ring 3 y no solo en el driver ===
///
/// Porque el crudo se entrega **sin interpretar** a proposito, y quien recibe
/// un byte crudo necesita poder decir una cosa sobre el sin volver a preguntar:
/// *"esto de aqui es el enlace"*. Es el unico bit que se mira desde este lado.
///
/// ** Y sirve para una sola cosa, que es la que importa: **cazar la
/// contradiccion**. `INFO_NET_MEGABITS` sale de la foto del arranque --lo pinta
/// un panel que se repinta-- y el crudo sale del aparato AHORA. El dia que uno
/// diga ARRIBA y el otro diga ABAJO, el que tiene razon es el crudo, y decirlo
/// en pantalla es lo que separa *"la red esta bien"* de *"eso es de hace un
/// rato"*.
///
/// [!] Los bits de velocidad --10/100/1000-- NO se traen aqui. Interpretar la
/// velocidad es opinion del driver y ya la da `INFO_NET_MEGABITS`; duplicar esa
/// tabla en dos lados es como se consiguen dos verdades distintas.
pub const PHY_ENLACE_ARRIBA: u64 = 0x02;

/// Por que no se pudo armar. `Ok` es que si.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Armado {
    Ok,
    /// Sin cable. **Es un motivo propio y no un fallo del anillo**: sin enlace
    /// no van a llegar tramas por correcto que sea todo lo demas, y confundir
    /// las dos cosas cuesta una tarde buscando un bug en un driver que funciona.
    SinEnlace,
    /// El anillo no se pudo montar. CABINA dice por que.
    NoArma,
    /// No hay tarjeta que este kernel sepa leer.
    SinTarjeta,
    /// El kernel contesto algo que este lado no conoce.
    Raro(u64),
}

/// **Arma el receptor.** Idempotente: armar dos veces no arma dos anillos.
pub fn armar() -> Armado {
    match invoke(CURRENT_TASK, OP_RED, RED_OP_ARMAR, 0, 0).value {
        0 => Armado::Ok,
        1 => Armado::SinEnlace,
        2 => Armado::NoArma,
        3 => Armado::SinTarjeta,
        otro => Armado::Raro(otro),
    }
}

/// **Vacia lo que llego** y devuelve cuantas tramas se leyeron esta vez.
///
/// ** Sondear no es solo mirar: devuelve los descriptores que la tarjeta ya
/// uso, y eso es lo que hace que el anillo no se llene. Un `net rx` que solo
/// mirara acabaria con el receptor parado.
pub fn sondear() -> u64 {
    invoke(CURRENT_TASK, OP_RED, RED_OP_SONDEAR, 0, 0).value
}

// ===================================================================
//  LA PLACA -- contesta y no concede
// ===================================================================

pub const PLACA_OP_CUANTAS: u64 = 0x01;
pub const PLACA_OP_TABLA: u64 = 0x02;
pub const PLACA_OP_ECAM: u64 = 0x03;
pub const PLACA_OP_IOMMU: u64 = 0x04;

/// Cuantas tablas ofrece el firmware. Cero = no hay XSDT que leer.
pub fn placa_cuantas() -> u64 {
    invoke(CURRENT_TASK, crate::OP_PLACA, PLACA_OP_CUANTAS, 0, 0).value
}

/// La tabla `i`: la firma en los cuatro bytes bajos, bit 32 = paso su suma,
/// bit 33 = es AML.
///
/// [!] Va empaquetado porque **por la puerta cabe UN numero**. Es la misma
/// solucion que `INFO_NET_VENDOR_DEVICE`, y se dice aqui para que quien lea el
/// numero no tenga que adivinar el reparto de bits.
pub fn placa_tabla(i: u64) -> u64 {
    invoke(CURRENT_TASK, crate::OP_PLACA, PLACA_OP_TABLA, i, 0).value
}

/// La base de ECAM, o 0 si no hay MCFG -- y entonces la config de PCIe se queda
/// en 256 bytes por funcion, sin capabilities extendidas.
pub fn placa_ecam() -> u64 {
    invoke(CURRENT_TASK, crate::OP_PLACA, PLACA_OP_ECAM, 0, 0).value
}

/// Los registros del primer IOMMU, o 0 si no hay IVRS.
pub fn placa_iommu() -> u64 {
    invoke(CURRENT_TASK, crate::OP_PLACA, PLACA_OP_IOMMU, 0, 0).value
}
