//! **ABRIR ALGO Y RECIBIR UN HANDLE**: un directorio, un fichero, la consola,
//!
//! [carril]  AMARILLO  abrir algo y recibir un handle
//! el propio paquete.
//!
//! ## Por que estas seis van juntas (L6b)
//!
//! Porque todas contestan lo mismo y **es la frase de portada del sistema**:
//! *"la autoridad es funcional, nunca heredada"*. Cada una de estas seis
//! devuelve un HANDLE, y ese handle **es** el permiso -- no hay un `root` del
//! que heredar ni un chequeo que se pueda pasar dos veces.
//!
//! ** Tenerlas en una pagina es lo que deja comprobar que las seis conceden
//! igual. Repartidas entre cuarenta brazos, que una concediera de mas se veria
//! el dia que alguien la usara.
//!
//! [!] Y `MI_PAQUETE` esta aqui aunque suene distinto: devuelve la capability
//! del propio `.bex` para leer sus recursos. Es abrir algo, y lo que abre es
//! uno mismo.
//!
//! ## [!] Esto NO es un reparto puro de L6d, y se dice
//!
//! El cuerpo de cada brazo se movio tal cual; el brazo paso a ser una llamada.

use super::*;

pub(super) fn dir_abrir(arg0: u64, arg1: u64) -> BmoStatus {
        let _ = arg0;
        let pid = scheduler::current_pid();
        let ruta = ruta_tomar(pid);
        match crate::ring0::obj::directory::open(pid, ruta) {
            Ok(handle) => BmoStatus::ok_value(handle),
            Err(code) => BmoStatus::err(code),
        }
}

//// El eslabon que faltaba: el kernel sabia leer y escribir archivos y
//// Ring 3 no tenia con que pedirselo.
pub(super) fn archivo_abrir(arg0: u64, arg1: u64) -> BmoStatus {
        let _ = arg0;
        let pid = scheduler::current_pid();
        let ruta = ruta_tomar(pid);
        match crate::ring0::obj::file::open(pid, ruta) {
            Ok(handle) => BmoStatus::ok_value(handle),
            Err(code) => BmoStatus::err(code),
        }
}

pub(super) fn archivo_asinc(arg0: u64, arg1: u64) -> BmoStatus {
        let _ = arg0;
        let pid = scheduler::current_pid();
        let ruta = ruta_tomar(pid);
        match crate::ring0::obj::file::abrir_asinc(pid, ruta) {
            Ok(handle) => BmoStatus::ok_value(handle),
            Err(code) => BmoStatus::err(code),
        }
}

pub(super) fn archivo_crear(arg0: u64, arg1: u64) -> BmoStatus {
        let _ = arg0;
        let pid = scheduler::current_pid();
        let ruta = ruta_tomar(pid);
        match crate::ring0::obj::file::create(pid, ruta) {
            Ok(handle) => BmoStatus::ok_value(handle),
            Err(code) => BmoStatus::err(code),
        }
}

pub(super) fn mi_paquete(arg0: u64, arg1: u64) -> BmoStatus {
        let pid = scheduler::current_pid();
        // La ruta la sabe el KERNEL, no el programa. Si no la recuerda --los
        // binarios que el propio kernel embebe no vienen de ninguna-- se
        // dice que no, en vez de abrir cualquier cosa.
        let Some(ruta) = crate::ring0::task::package::ruta_de(pid) else {
            return BmoStatus::err(2);
        };
        match crate::ring0::obj::file::open(pid, ruta) {
            Ok(handle) => BmoStatus::ok_value(handle),
            Err(code) => BmoStatus::err(code),
        }
}

pub(super) fn consola_crear(arg0: u64, arg1: u64) -> BmoStatus {
        let _ = arg0;
        match crate::ring0::obj::console::create(scheduler::current_pid()) {
            Ok(handle) => BmoStatus::ok_value(handle),
            Err(code) => BmoStatus::err(code),
        }
}

