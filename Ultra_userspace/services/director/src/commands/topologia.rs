//! **CUANTO FIARSE de los nucleos que ensena esta terminal.**
//!
//! # Por que existe este fichero, y por que es un fichero
//!
//! El 2026-08-25 el panel del escritorio pinto, en un Ryzen 5 5600X que es 6/12:
//!
//! ```text
//!    nucleos    27 fisicos
//!    hilos      54 logicos
//!    en pie      1 de 54
//! ```
//!
//! Con la misma tipografia y la misma confianza que un numero bueno. **El
//! kernel SI lo sabia** --la MADT declaraba otra cosa-- pero ese careo vivia
//! dentro de `smp::despertar()`, o sea que solo corria si alguien tecleaba
//! `smp`, y gritaba a un log de Ring 0 al que desde aqui no se vuelve.
//!
//! > Un diagnostico al que no se llega desde donde se ve el sintoma
//! > no es un diagnostico.
//!
//! ## Y por que NO vive dentro de `reports.rs`
//!
//! Porque al meterlo alli el censo dijo que no, y tenia razon. `reports.rs`
//! cruzo las 1.000 lineas de codigo con estas mismas lineas dentro -- **es el
//! heredero de `gui/main.rs`, el fichero que crecio 1.244 lineas teniendo un
//! plan escrito para partirlo**, que es la razon por la que ese guardian existe.
//!
//! *** Y el corte no es una huida: es L6b. `reports.rs` contesta *"que hay"* y
//! este fichero contesta *"cuanto te lo puedes creer"*, que son dos preguntas.

use bmo_userland as bmo;

use crate::scene::output::{Output, INK_ECHO, INK_PLAIN};

/// El bit que dice **que el perfil desmiente al silicio**: no es este CPU.
const DUDA_PERFIL: u64 = 1 << 2;
/// Las dos hojas de CPUID no dicen lo mismo.
const DUDA_CPUID: u64 = 1 << 0;
/// La MADT declara otros hilos que CPUID.
const DUDA_MADT: u64 = 1 << 3;
/// Los hilos por nucleo no se pudieron medir.
const DUDA_SIN_MEDIR: u64 = 1 << 1;

/// **Que duda hay, en una frase.** `b""` cuando los tres testigos coinciden.
///
/// [!] Se ensena el motivo **MAS GRAVE y solo uno**: una fila con cuatro quejas
/// no se lee. El orden es el de quien manda -- si el perfil desmiente al
/// silicio, da igual lo que opine la MADT.
///
/// Y devolver `b""` cuando todo cuadra no es un descuido: **un aviso que sale
/// siempre deja de ser un aviso.** Es la misma leccion que el verificador de
/// XSAVE, que cantaba DIFIERE en cada arranque hasta que se le dieron los
/// numeros de lo que de verdad comparaba.
pub(crate) fn duda_nota() -> &'static [u8] {
    let d = bmo::info(bmo::INFO_CPU_TOPOLOGIA_DUDA);
    if d & DUDA_PERFIL != 0 {
        b"este NO es el CPU que el perfil dice: no te fies del numero"
    } else if d & DUDA_CPUID != 0 {
        b"las dos hojas de CPUID se contradicen"
    } else if d & DUDA_MADT != 0 {
        b"la MADT declara otros hilos (SMT apagado en la BIOS?)"
    } else if d & DUDA_SIN_MEDIR != 0 {
        b"hilos por nucleo SIN MEDIR: `fisicos` no es una division"
    } else {
        b""
    }
}

/// **De donde sale el `fisicos`**, y la fila de duda si la hay.
///
/// Lo primero es lo que mas cuesta ver y menos ocupa: hasta el 25-08 el kernel
/// calculaba los nucleos fisicos dividiendo los hilos **entre un 2 escrito a
/// mano**. Con eso, la comprobacion que existia --`hilos == nucleos * 2`-- no
/// podia fallar nunca, porque comparaba un numero contra su propia definicion.
///
/// Si esta linea no aparece, es que no se pudo medir; y entonces `fisicos` es
/// una **copia** de `hilos`, no una division.
pub(crate) fn detalle(s: &mut Output, label: fn(&mut Output, &[u8])) {
    let hpn = bmo::info(bmo::INFO_CPU_HILOS_POR_NUCLEO);
    if hpn > 0 {
        s.text(b"   (");
        s.dec(hpn);
        s.text(b" por nucleo, MEDIDO)");
    }
    s.byte(b'\n');

    let nota = duda_nota();
    if !nota.is_empty() {
        label(s, b"[!] duda");
        s.with_ink(INK_ECHO);
        s.text(nota);
        s.with_ink(INK_PLAIN);
        s.byte(b'\n');
    }
}
