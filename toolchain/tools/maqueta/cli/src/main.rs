//! # `maqueta` -- el binario
//!
//! ```text
//!   cargo run -p bmo-maqueta -- entrada.maqueta salida.rs
//! ```
//!
//! Las cinco generaciones en orden, y el veredicto ANTES de emitir: un emisor
//! que escribe una maquetacion que el bisnieto rechaza estaria escribiendo el
//! fallo en un fichero que despues nadie vuelve a mirar.
//!
//! El artefacto se COMMITEA, como `font16_data.rs` de `fontgen`. Es la
//! convencion de esta casa: el generador se puede volver a correr, y mientras
//! tanto el arbol no depende de que alguien lo corra.

use std::process::ExitCode;

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    let (Some(entrada), Some(salida)) = (args.next(), args.next()) else {
        eprintln!("uso: maqueta <entrada.maqueta> <salida.rs>");
        return ExitCode::from(2);
    };

    let src = match std::fs::read(&entrada) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("maqueta: no puedo leer {entrada}: {e}");
            return ExitCode::from(2);
        }
    };

    let doc = match bmo_maqueta_node::parse(&src) {
        Ok(d) => d,
        Err(e) => return fallo(&entrada, &src, &e),
    };
    let cascada = match bmo_maqueta_cascade::cascade(&doc) {
        Ok(c) => c,
        Err(e) => return fallo(&entrada, &src, &e),
    };
    let puesto = bmo_maqueta_layout::lay(&cascada);
    let reparos = bmo_maqueta_verdict::judge(&puesto, &cascada);
    if !reparos.is_empty() {
        return fallo(&entrada, &src, &reparos);
    }

    let codigo = bmo_maqueta_emit::rust::modulo(&entrada, &puesto);
    if let Err(e) = std::fs::write(&salida, codigo) {
        eprintln!("maqueta: no puedo escribir {salida}: {e}");
        return ExitCode::from(2);
    }
    println!(
        "maqueta: {entrada} -> {salida}   {}x{}, {} cajas, {} golpes, {} islas",
        puesto.canvas.0,
        puesto.canvas.1,
        puesto.all().len(),
        puesto.hits().len(),
        puesto.islands().len()
    );
    ExitCode::SUCCESS
}

fn fallo(entrada: &str, src: &[u8], errores: &[bmo_maqueta_diag::Error]) -> ExitCode {
    eprint!("{}", bmo_maqueta_diag::render(entrada, src, errores));
    eprintln!(
        "maqueta: {} reparo{}, no se ha escrito nada.",
        errores.len(),
        if errores.len() == 1 { "" } else { "s" }
    );
    ExitCode::FAILURE
}
