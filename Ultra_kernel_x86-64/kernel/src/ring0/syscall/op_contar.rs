//! **CONTAR LO QUE EL KERNEL SABE**: CABINA, `info`, el registro de arranque y
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

