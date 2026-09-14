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
//! ## *** Y desde E3, EL GATE RED (2026-09-13)
//!
//! [`abrir`] se paga UNA vez: el kernel pregunta en orden y cada no tiene
//! nombre. Si dice que si, el buzon queda mapeado en [`RED_BUZON_VA`] y a partir
//! de ahi [`enviar`] y [`recibir`] son `mov`, sin syscall. [`esperar`] duerme en
//! `WAIT` hasta que el latido meta algo. Si el radar revoca, el buzon pasa a una
//! LAPIDA: lo que se lee dice por que ([`motivo_del_cierre`]) y nada revienta.

use core::sync::atomic::{AtomicU64, Ordering};

use bmo_puerta_red::buzon::{self, Memoria};

use crate::{invoke, CURRENT_TASK, OP_RED};

/// La forma del buzon y los veredictos, los mismos que usa el kernel.
pub use bmo_puerta_red as puerta;

pub const RED_OP_ARMAR: u64 = 0x01;
pub const RED_OP_SONDEAR: u64 = 0x02;
pub const RED_OP_ABRIR: u64 = 0x03;
pub const RED_OP_CERRAR: u64 = 0x04;
pub const RED_OP_ESTADO: u64 = 0x05;
pub const RED_OP_VUELOS: u64 = 0x06;
/// Donde el kernel deja el buzon del pase.
pub const RED_BUZON_VA: u64 = 0x0000_0002_0000_0000;

/// El buzon en MI espacio, leido con `volatile`: el latido escribe a la vez.
struct Mapeado;

impl Memoria for Mapeado {
    fn tam(&self) -> usize {
        buzon::BYTES
    }
    fn leer8(&self, off: usize) -> u8 {
        if off >= buzon::BYTES {
            return 0;
        }
        unsafe { core::ptr::read_volatile((RED_BUZON_VA as *const u8).add(off)) }
    }
    fn escribir8(&mut self, off: usize, v: u8) {
        if off < buzon::BYTES {
            unsafe { core::ptr::write_volatile((RED_BUZON_VA as *mut u8).add(off), v) };
        }
    }
}

/// El handle del pase de ESTE proceso; `0` = nunca se abrio o se cerro.
///
/// [!] Mientras valga 0 no se toca `RED_BUZON_VA`: ahi no hay nada mapeado, y
/// leer seria un fallo de pagina. Despues de un pase, ahi queda la lapida.
static PASE: AtomicU64 = AtomicU64::new(0);

/// Por que no se abrio.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoAbre {
    Negado(puerta::pase::NoPase),
    Otro { code: u32, flags: u32 },
}

/// **EL GATE RED.** Pide el pase por `ms` milisegundos y `cupo` tramas.
pub fn abrir(ms: u64, cupo: u32) -> Result<u64, NoAbre> {
    let st = invoke(CURRENT_TASK, OP_RED, RED_OP_ABRIR, puerta::pase::empaquetar(ms, cupo), 0);
    if st.code != 0 {
        return Err(match puerta::pase::NoPase::desde_codigo(st.flags) {
            Some(n) => NoAbre::Negado(n),
            None => NoAbre::Otro { code: st.code, flags: st.flags },
        });
    }
    PASE.store(st.value, Ordering::Relaxed);
    Ok(st.value)
}

/// Tengo un pase y sigue vivo?
pub fn pase_abierto() -> bool {
    PASE.load(Ordering::Relaxed) != 0 && buzon::Cliente::vivo(&Mapeado)
}

/// `0` si esta abierto o nunca hubo; si no, el `puerta::radar::Motivo` del cierre.
pub fn motivo_del_cierre() -> u32 {
    if PASE.load(Ordering::Relaxed) == 0 {
        return 0;
    }
    buzon::Cliente::estado(&Mapeado)
}

/// **Deja una trama para el cable.** Sin syscall: sale en el siguiente latido.
pub fn enviar(trama: &[u8]) -> Result<(), buzon::NoEnvia> {
    if PASE.load(Ordering::Relaxed) == 0 {
        return Err(buzon::NoEnvia::Revocado);
    }
    buzon::Cliente::enviar(&mut Mapeado, trama)
}

/// **Recoge una trama recibida.** Sin syscall.
pub fn recibir(dst: &mut [u8]) -> Option<usize> {
    if PASE.load(Ordering::Relaxed) == 0 {
        return None;
    }
    buzon::Cliente::recibir(&mut Mapeado, dst)
}

/// **Duerme** hasta que llegue algo, se revoque el pase o pase `timeout_ns`.
pub fn esperar(timeout_ns: u64) -> u64 {
    let h = PASE.load(Ordering::Relaxed);
    if h == 0 {
        return 0;
    }
    let visto = buzon::Cliente::secuencia(&Mapeado);
    crate::sys::wait(h, visto, timeout_ns).value
}

/// Cierra el pase propio. Idempotente.
pub fn cerrar() {
    let h = PASE.swap(0, Ordering::Relaxed);
    if h != 0 {
        invoke(CURRENT_TASK, OP_RED, RED_OP_CERRAR, h, 0);
    }
}

/// `(despegues, aterrizajes)`: dadas a la tarjeta, y devueltas ENVIADAS por ella.
pub fn vuelos() -> (u64, u64) {
    let v = invoke(CURRENT_TASK, OP_RED, RED_OP_VUELOS, 0, 0).value;
    (v >> 32, v & 0xFFFF_FFFF)
}

/// El estado empaquetado: ver `RED_OP_ESTADO` en el ABI.
pub fn estado() -> u64 {
    invoke(CURRENT_TASK, OP_RED, RED_OP_ESTADO, 0, 0).value
}

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

/// **Los cinco motivos por los que no hay censo** (L6j, 2026-09-12).
///
/// Espejo de `ring0::plat::placa`. Hasta hoy los cinco llegaban como el mismo
/// cero con codigo de exito, y el DIRECTOR pintaba el PRIMERO como si fuera el
/// unico: *"el firmware no dio un RSDP"*. Los otros cuatro mandan a mirar a otro
/// sitio -- y `PLACA_CABECERA_MALA` ni siquiera acusa al firmware.
pub const PLACA_SIN_RSDP: u32 = 1;
pub const PLACA_SIN_XSDT: u32 = 2;
pub const PLACA_CABECERA_MALA: u32 = 3;
pub const PLACA_LARGO_IMPOSIBLE: u32 = 4;
pub const PLACA_SIN_ESA_FILA: u32 = 5;

/// El motivo en palabras. Una sola version del texto en todo Ring 3.
pub fn placa_por_que(motivo: u32) -> &'static str {
    match motivo {
        PLACA_SIN_RSDP => "el firmware no dio un RSDP de ACPI 2.0+",
        PLACA_SIN_XSDT => "hay RSDP y no lleva a un XSDT (ACPI 1.0?)",
        PLACA_CABECERA_MALA => "el XSDT esta donde dice y su cabecera no se lee",
        PLACA_LARGO_IMPOSIBLE => "el XSDT declara un largo menor que su cabecera",
        PLACA_SIN_ESA_FILA => "esa fila no existe: el censo trae menos tablas",
        _ => "motivo que este userland no conoce",
    }
}

/// Cuantas tablas ofrece el firmware. Cero = **cero tablas**, que desde el
/// 12-09 ya no significa tambien "no se pudo censar": para eso esta
/// [`placa_cuantas_motivo`].
pub fn placa_cuantas() -> u64 {
    invoke(CURRENT_TASK, crate::OP_PLACA, PLACA_OP_CUANTAS, 0, 0).value
}

/// **Cuantas tablas, y POR QUE no si no las hay.** `(cuantas, motivo)`, con
/// `motivo == 0` cuando el censo salio.
///
/// No sustituye a [`placa_cuantas`]: quien solo quiera el numero sigue igual.
/// Esto es para quien tenga que DECIRLO, que es el que pinta la pantalla.
pub fn placa_cuantas_motivo() -> (u64, u32) {
    let st = invoke(CURRENT_TASK, crate::OP_PLACA, PLACA_OP_CUANTAS, 0, 0);
    (st.value, if st.code == 0 { 0 } else { st.flags })
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
