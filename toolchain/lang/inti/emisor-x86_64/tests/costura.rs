//! **LA COSTURA, vista desde fuera: de que fichero es la linea que se acusa.**
//!
//! ## Por que esto no lo cubre el banco de dentro
//!
//! El banco fabrica la costura a mano. Eso prueba que el analisis la LEE bien y
//! no prueba que `armar` la ESCRIBA -- que es la mitad que se rompe sola,
//! porque la escribe quien fusiona y la lee otro.
//!
//! ** Y hace falta un `$BMO_MODS` de mentira porque no hay otra forma honesta:
//! la unica pieza real del runtime hoy es el monton, y esta escrita en `llano`
//! como debe. Fabricar una pieza que se declara `pleno` es la unica manera de
//! preguntarle al compilador que hace con ella **antes** de que exista una de
//! verdad que lo haga por accidente.

use std::path::PathBuf;
use std::process::Command;

fn caja(nombre: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("inti-costura-{}", nombre));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).expect("no puedo crear la caja");
    d
}

/// Una raiz de tablas de mentira con UNA pieza dentro.
///
/// ** `$BMO_MODS` TAPA sin bifurcar: lo que esta aqui gana, y lo que no esta
/// sigue saliendo de las tablas de verdad. Por eso basta con el directorio de
/// la pieza y el compilador sigue encontrando `palabras.toml`.
fn raiz_con_pieza(d: &PathBuf, nombre: &str, fichero: &str, texto: &str) -> PathBuf {
    let dir = d.join("mods").join("lang").join("inti").join("runtime").join(nombre);
    std::fs::create_dir_all(&dir).expect("no puedo crear la raiz de mentira");
    std::fs::write(dir.join(fichero), texto).expect("no puedo escribir la pieza");
    d.join("mods")
}

fn compila_con(raiz: &PathBuf, args: &[&str]) -> (bool, String, String) {
    let s = Command::new(env!("CARGO_BIN_EXE_inti"))
        .env("BMO_MODS", raiz)
        .args(args)
        .output()
        .expect("no puedo ejecutar el compilador");
    (
        s.status.success(),
        String::from_utf8_lossy(&s.stdout).to_string(),
        String::from_utf8_lossy(&s.stderr).to_string(),
    )
}

/// **EL FALLO ESTA EN LA PIEZA Y EL AVISO LO DICE.**
///
/// Antes del 2026-08-22 este mismo fuente daba:
///
/// ```text
///    E0070 En el perfil `llano` no se puede usar `texto`.
///       en usuario.inti, linea 3:
/// ```
///
/// *** Y la linea 3 de `usuario.inti` esta EN BLANCO. El aviso era correcto en
/// el que-paso y mandaba a mirar a otro fichero en el donde.
#[test]
fn el_aviso_de_una_pieza_nombra_la_pieza_y_no_al_que_la_trae() {
    let d = caja("acusa");
    let raiz = raiz_con_pieza(
        &d,
        "cortesia",
        "saludos.inti",
        // [!] La pieza se declara `llano` y usa `texto`, que alli NO cabe: el
        // fallo es SUYO. El ejemplo cambio el 2026-08-23 con P2 -- antes decia
        // `perfil pleno` y bastaba, porque todo se juzgaba contra el perfil del
        // fichero principal. Desde P2 cada pieza se juzga contra el suyo, asi
        // que un `texto` en una pieza `pleno` es CORRECTO, y esta prueba
        // necesitaba un fallo que siguiera siendolo.
        "perfil llano

funcion saluda(a es texto) devuelve texto
    devuelve a
",
    );
    let fuente = d.join("usuario.inti");
    std::fs::write(
        &fuente,
        "perfil llano
usa cortesia

funcion principal devuelve entero32
    devuelve 0
",
    )
    .expect("no puedo escribir el fuente");

    let (bien, salida, err) = compila_con(&raiz, &[fuente.to_str().unwrap()]);
    let todo = format!("{}{}", salida, err);
    assert!(!bien, "tenia que denunciar el `texto` de la pieza:
{}", todo);
    assert!(
        todo.contains("cortesia/saludos.inti"),
        "el aviso no dice de que pieza es:
{}",
        todo
    );
    assert!(
        todo.contains("`usa cortesia`"),
        "el aviso no dice quien la trajo:
{}",
        todo
    );
    // ** La parte que de verdad se rompia: el DONDE no puede senalar al fichero
    // del usuario cuando el fallo no esta ahi.
    //
    // Se compara contra la RUTA COMPLETA porque es lo que el compilador pinta,
    // y comparar contra el nombre corto dejaria la prueba en verde por no
    // encontrar un texto que tampoco estaba antes.
    let acusa_al_usuario = format!("en {}, linea", fuente.display());
    assert!(
        !todo.contains(&acusa_al_usuario),
        "el aviso sigue acusando al fichero del usuario:
{}",
        todo
    );
}

/// **Y lo que SI es del usuario se sigue acusando al usuario.**
///
/// ** Una marca que se pone siempre no marca nada. Sin esta prueba, `con_pieza`
/// podria estamparse en todos los avisos y la de arriba seguiria en verde --
/// con el resultado de que un fallo propio mandaria a mirar al runtime.
#[test]
fn lo_que_escribio_el_usuario_se_le_sigue_acusando_a_el() {
    let d = caja("propio");
    let raiz = raiz_con_pieza(
        &d,
        "cortesia",
        "saludos.inti",
        "perfil llano

funcion dos devuelve entero32
    devuelve 2
",
    );
    let fuente = d.join("usuario.inti");
    std::fs::write(
        &fuente,
        "perfil llano
usa cortesia

funcion principal devuelve entero32
    saludo = \"hola\"
    devuelve 0
",
    )
    .expect("no puedo escribir el fuente");

    let (bien, salida, err) = compila_con(&raiz, &[fuente.to_str().unwrap()]);
    let todo = format!("{}{}", salida, err);
    assert!(!bien, "el `texto` del usuario tenia que denunciarse:
{}", todo);
    let acusa_al_usuario = format!("en {}, linea", fuente.display());
    assert!(
        todo.contains(&acusa_al_usuario),
        "un fallo propio tiene que acusar al fichero propio:
{}",
        todo
    );
    assert!(
        !todo.contains("la trajo"),
        "un fallo propio no viene de ninguna pieza:
{}",
        todo
    );
}
