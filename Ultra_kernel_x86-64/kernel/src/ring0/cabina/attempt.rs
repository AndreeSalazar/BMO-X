//! **THE ATTEMPT** -- what turns four unrelated lines into one story.
//!
//! [carril]  AMARILLO  cose cuatro lineas en una historia; si cose mal, cuenta otra
//!
//! === Why this is a file of its own ===
//!
//! Because it is the only thing in CABINA that has a LIFETIME. An event is a
//! moment; an attempt is a span, and while one is open everything emitted
//! carries its number.
//!
//! ** And the property worth preserving is what happens when nobody closes it:
//! the `Drop` marks it **"left OPEN"**. Nothing had to detect the failure --
//! the absence of an ending IS the report. A subsystem that has to notice its
//! own hang is a subsystem that will not.

use super::*;

// -- ** EL INTENTO: la unidad que convierte cuatro renglones en UNA historia --
//
// === El problema, con la foto del 2026-08-10 delante ===
//
// ```text
//    44 FAULT proc:   cabecera invalida =800
//    45 WARN  proc:   el .bex de disco no paso la admision =1
//    46 INFO  lanzar: bytes DIRECTOS del disco al marco =2C00
//    47 WARN  gui:    el .bex no paso la admision
// ```
//
// Cuatro lineas, un solo hecho. Para leerlo hay que saberse de memoria que la 45
// es eco de la 44 y la 47 eco de la 45. **Eso es trabajo del que mira**, y es
// justo el trabajo que se paga dos veces: una al escribirlo y otra cada vez que
// alguien vuelve a mirarlo seis meses despues.
//
// === Por que CABINA no era omnisciente ===
//
// Porque era **VOLUNTARIA**. Veia lo que alguien se acordo de contarle, y una
// funcion que se va por un `return` temprano sin decir nada, para CABINA **no
// paso**. Ese es el caso en el que mas falta hace saberlo.
//
// Asi que "verlo todo" no se arregla contando mas cosas: se arregla haciendo que
// **olvidarse sea imposible**. Y eso ya estaba inventado en esta casa -- el
// `Testigo` del disco, que suelta por todos los caminos incluido el sexto
// `return`.
//
// === Y sigue sin haber cerebro ===
//
// Nadie DEDUCE que los cuatro van juntos. Van juntos porque quien los emitio
// estaba dentro del mismo intento, y eso lo sabia en ese momento. La agrupacion
// deja de ser una interpretacion para ser **un dato que se apunta**.

/// Numero del intento en curso. `0` = no hay ninguno abierto.
pub(crate) static mut INTENTO_ACTUAL: u32 = 0;
/// El siguiente numero a repartir.
pub(crate) static mut INTENTO_SEQ: u32 = 0;

/// **Un intento abierto.** Mientras exista, todo lo que se emita lleva su numero.
///
/// [!] LO IMPORTANTE ES EL `Drop`. Si nadie lo cierra --un `return` temprano, un
/// `?` que se lleva la funcion, un camino de error que nadie penso-- se apunta
/// **"quedo ABIERTO"** al soltarse. Nadie tuvo que detectar nada: la ausencia de
/// un cierre ES el hecho, y se registra sola.
///
/// Eso es lo que hace que un cuelgue deje de ser invisible. Antes, una funcion
/// que se iba sin decir nada no dejaba rastro; ahora deja el rastro **por no
/// decir nada**.
pub struct Intento {
    num: u32,
    previo: u32,
    cerrado: bool,
}

/// **Abre un intento.** Todo lo que se emita hasta que se suelte lleva su numero.
///
/// `que` es lo que se esta intentando, en una palabra: `"lanzar"`, `"montar"`.
/// El detalle --la ruta, el pid-- va en el evento, que es donde cabe.
#[track_caller]
pub fn intento(que: &str) -> Intento {
    let (num, previo) = unsafe {
        INTENTO_SEQ = INTENTO_SEQ.wrapping_add(1).max(1);
        let previo = INTENTO_ACTUAL;
        INTENTO_ACTUAL = INTENTO_SEQ;
        (INTENTO_SEQ, previo)
    };
    record(Severity::Info, que, "empieza", num as u64);
    Intento { num, previo, cerrado: false }
}

impl Intento {
    /// **Cierra el intento diciendo como fue.** Consume el testigo: despues de
    /// esto ya no se puede volver a cerrar ni olvidar.
    pub fn cerrar(mut self, bien: bool) {
        self.cerrado = true;
        record(
            if bien { Severity::Info } else { Severity::Warning },
            "intento",
            if bien { "termino BIEN" } else { "termino MAL" },
            self.num as u64,
        );
    }

    /// El numero, para quien quiera nombrarlo en otro sitio.
    pub fn num(&self) -> u32 {
        self.num
    }
}

impl Drop for Intento {
    fn drop(&mut self) {
        if !self.cerrado {
            // ** NADIE LO CERRO. Y eso no es un descuido que haya que perdonar:
            // es el hecho mas interesante que puede registrar esta estructura.
            // Un camino que se fue sin decir como acabo es exactamente el que
            // nadie estaba mirando.
            record(Severity::Fault, "intento", "quedo ABIERTO: nadie dijo como acabo", self.num as u64);
        }
        // El de fuera vuelve a ser el actual. Anidar es legitimo --lanzar abre
        // uno y admitir podria abrir otro-- y restaurar el previo en vez de
        // poner cero es lo que hace que el de fuera no se quede huerfano.
        unsafe { INTENTO_ACTUAL = self.previo };
    }
}

/// Atajos por severidad -- el vocabulario del narrador.
#[track_caller]
pub fn info(module: &str, msg: &str, value: u64)  { record(Severity::Info, module, msg, value); }
#[track_caller]
pub fn warn(module: &str, msg: &str, value: u64)  { record(Severity::Warning, module, msg, value); }
#[track_caller]
pub fn fault(module: &str, msg: &str, value: u64) { record(Severity::Fault, module, msg, value); }
/// Lo irrecuperable: fault de kernel, doble falta. Ultima linea de la bitacora
/// antes de que la maquina se detenga.
#[track_caller]
pub fn panic_ev(module: &str, msg: &str, value: u64) { record(Severity::Panic, module, msg, value); }
