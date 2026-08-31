//! **CONTAR LO QUE EL KERNEL SABE**: CABINA, `info`, el registro de arranque y
//!
//! [carril]  VERDE     contar lo que el kernel ya sabe
//! la autopsia de la ultima tarea que murio.
//!
//! ## Por que estas ocho van juntas (L6b)
//!
//! Porque contestan la misma pregunta y **ninguna cambia nada**: el programa
//! pregunta y el kernel responde. Son la mitad del sistema que se puede llamar
//! sin miedo, y tenerlas juntas es lo que deja verlo.
//!
//! ** Y vienen por PAREJAS --`_INFO` y `_TEXTO`-- porque por la puerta cabe un
//! numero, y una frase no. Quien quiere el texto pide primero cuantos trozos
//! hay y luego los va sacando. Que las ocho esten en una pagina es lo que hace
//! evidente que son cuatro fuentes con el mismo patron, y no ocho operaciones.
//!
//! ## [!] Esto NO es un reparto puro de L6d, y se dice
//!
//! El CUERPO de cada brazo se movio tal cual. El brazo del `match` paso de
//! llevarlo dentro a ser una llamada: eso es UNA linea distinta por operacion, y
//! se cuenta como lo que es en vez de llamarlo "mover texto".

use super::*;

//// * EL SONIDO. Sin CR3 y sin mapeos: aqui no se entrega memoria, se
//// entrega el DERECHO -- que es justamente lo que hace que esta pieza se
//// pueda escribir hoy, con el driver de HDA todavia sin existir.
//// * CABINA. `arg0` = campo, `arg1` = que evento (0 = el mas reciente).
//// Un campo que no existe contesta "no soportado" y no 0: un cero seria
//// indistinguible de un evento cuyo valor ES cero.
pub(super) fn cabina_info(arg0: u64, arg1: u64) -> BmoStatus {
        match crate::ring0::cabina::campo(arg0, arg1) {
            Some(v) => BmoStatus::ok_value(v),
            None => unsupported(),
        }
}

//// `arg0` empaqueta `(evento << 32) | cual`, `arg1` es el trozo de 8 en
//// 8. Los dos indices en un argumento porque la puerta tiene tres y dos
//// ya estan ocupados -- la misma aritmetica que usa la autopsia.
pub(super) fn cabina_texto(arg0: u64, arg1: u64) -> BmoStatus {
        let evento = arg0 >> 32;
        let cual = arg0 & 0xFFFF_FFFF;
        BmoStatus::ok_value(crate::ring0::cabina::texto(evento, cual, arg1))
}

pub(super) fn info(arg0: u64, arg1: u64) -> BmoStatus {
        BmoStatus::ok_value(crate::ring0::core::report::campo(arg0))
}

pub(super) fn info_texto(arg0: u64, arg1: u64) -> BmoStatus {
        BmoStatus::ok_value(crate::ring0::core::report::texto(arg0, arg1))
}

pub(super) fn klog_info(arg0: u64, arg1: u64) -> BmoStatus {
        use crate::ring0::core::klog;
        BmoStatus::ok_value(match arg0 {
            0 => klog::disponibles(),
            1 => klog::total(),
            _ => 0,
        })
}

pub(super) fn klog_texto(arg0: u64, arg1: u64) -> BmoStatus {
        BmoStatus::ok_value(crate::ring0::core::klog::texto(arg0, arg1))
}

//// * LA AUTOPSIA. Contesta texto y nada mas, como el klog y como INFO:
//// no concede una capability, no deja escribir, no deja mirar el espacio
//// de nadie. Es la parte "meta" del metakernel puesta en una fila de
//// tabla -- el sistema informa sobre si mismo.
pub(super) fn autopsia_info(arg0: u64, arg1: u64) -> BmoStatus {
        use crate::ring0::core::autopsy;
        BmoStatus::ok_value(match arg0 {
            0 => autopsy::total(),
            1 => autopsy::disponibles(),
            2 => autopsy::renglones(arg1),
            _ => 0,
        })
}

pub(super) fn autopsia_texto(arg0: u64, arg1: u64) -> BmoStatus {
        // `arg0` trae los dos indices: informe arriba, fila abajo.
        let informe = arg0 >> 32;
        let fila = arg0 & 0xFFFF_FFFF;
        BmoStatus::ok_value(crate::ring0::core::autopsy::texto(informe, fila, arg1))
}

/// **QUE CUENTA LA PLACA DE SI MISMA.** Ver `TASK_OP_PLACA` en `ops.rs`.
///
/// ** Vive aqui y no en `op_maquina.rs` porque **no cambia nada**: contesta y
/// no concede. El corte de este despachador es por la pregunta, y esta pregunta
/// es *"que hay"*.
///
/// [!] Y por la puerta cabe UN numero, asi que las firmas de las tablas salen
/// EMPAQUETADAS: los cuatro bytes de la firma abajo y las banderas arriba. Es
/// la misma solucion que `INFO_NET_VENDOR_DEVICE`, y se dice aqui en vez de que
/// el que lea el numero tenga que adivinarlo.
///
/// ```text
///    bits  0..32   los cuatro caracteres de la firma, tal cual
///    bit   32      la tabla paso su suma de comprobacion
///    bit   33      es AML: un PROGRAMA, y aqui no se ejecuta
/// ```
pub(super) fn placa(arg0: u64, arg1: u64) -> BmoStatus {
    use crate::ring0::plat::placa as p;
    let rsdp = crate::ring0::plat::madt::rsdp_guardado();
    match arg0 {
        PLACA_OP_CUANTAS => match p::censar(rsdp) {
            Some(c) => BmoStatus::ok_value(c.cuantas() as u64),
            None => BmoStatus::ok_value(0),
        },
        PLACA_OP_TABLA => {
            let Some(c) = p::censar(rsdp) else {
                return BmoStatus::ok_value(0);
            };
            let Some(f) = c.filas().nth(arg1 as usize) else {
                return BmoStatus::ok_value(0);
            };
            let firma = u32::from_le_bytes(f.firma) as u64;
            let mut v = firma;
            if f.creible {
                v |= 1 << 32;
            }
            if f.programa {
                v |= 1 << 33;
            }
            BmoStatus::ok_value(v)
        }
        // La base de ECAM. Cero = no hay MCFG, y entonces la config de PCIe se
        // queda en 256 bytes por funcion: no es un fallo, es una respuesta.
        PLACA_OP_ECAM => {
            let mut r = [p::RangoEcam { base: 0, segmento: 0, bus_desde: 0, bus_hasta: 0 };
                p::MAX_ECAM];
            let n = p::ecam(rsdp, &mut r);
            BmoStatus::ok_value(if n > 0 { r[0].base } else { 0 })
        }
        // Los registros del primer IOMMU. Cero = no hay IVRS, y eso significa
        // que **nada limita adonde escribe un aparato con DMA**.
        PLACA_OP_IOMMU => {
            let mut v = [p::Ivhd {
                tipo: 0, banderas: 0, largo: 0, id_dispositivo: 0, base_mmio: 0, segmento: 0,
            }; p::MAX_IOMMU];
            let n = p::iommu(rsdp, &mut v);
            BmoStatus::ok_value(if n > 0 { v[0].base_mmio } else { 0 })
        }
        _ => unsupported(),
    }
}
