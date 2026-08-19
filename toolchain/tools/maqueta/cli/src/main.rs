//! # `maqueta` -- el binario
//!
//! generacion: ninguna -- el binario que las junta
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

    let codigo = bmo_maqueta_emit::rust::modulo(&procedencia(&entrada), &puesto);
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

/// **De donde salio esta cara, dicho igual lo escriba quien lo escriba.**
///
/// === El defecto que esto cierra, y lo cazo un guardian el 2026-08-18 ===
///
/// El emisor recibia la ruta **tal como se tecleo**, y la escribe en la primera
/// linea del modulo generado. O sea que generar la MISMA cara desde el mismo
/// fichero daba DOS artefactos distintos:
///
/// ```text
///   maqueta pruebas/calc.maqueta ...        -> //! ... DESDE `pruebas/calc.maqueta`
///   maqueta C:/Users/.../calc.maqueta ...   -> //! ... DESDE `C:/Users/...`
/// ```
///
/// Con el fichero commiteado eso no es cosmetico: **un artefacto que depende de
/// quien lo genera no se puede comparar**, y comparar es lo unico que impide que
/// la cara pintada y su `.maqueta` se separen. El guardian de `build.ps1` lo
/// invocaba con ruta absoluta y veia una deriva que no existia.
///
/// La procedencia se deduce **del fichero**, no de la invocacion: se sube hasta
/// el `.git` y se cuenta desde ahi, con barras hacia adelante. Fuera de un
/// repositorio se contesta lo que se tecleo, normalizado -- decir algo cierto
/// vale mas que no decir nada.
fn procedencia(entrada: &str) -> String {
    let barras = |s: String| s.replace(std::path::MAIN_SEPARATOR, "/");
    let Ok(abs) = std::fs::canonicalize(entrada) else {
        return barras(entrada.to_string());
    };
    let mut raiz = abs.as_path();
    while let Some(padre) = raiz.parent() {
        if padre.join(".git").exists() {
            if let Ok(rel) = abs.strip_prefix(padre) {
                return barras(rel.to_string_lossy().into_owned());
            }
            break;
        }
        raiz = padre;
    }
    barras(entrada.to_string())
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
