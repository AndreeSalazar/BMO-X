//! El compilador, usado como lo usa una persona.
//!
//! ## ** Por que esto no lo cubren las pruebas de dentro
//!
//! El banco del emisor compila llamando a las funciones. Eso prueba el
//! compilador y **no prueba el programa**: que lea un fichero, que pinte los
//! avisos, que no escriba nada cuando algo esta mal, y que devuelva el codigo de
//! salida correcto son cuatro cosas que solo se ven desde fuera.
//!
//! Y son justo las que se rompen sin que nadie se entere, porque ninguna hace
//! fallar una compilacion.

use std::path::PathBuf;
use std::process::Command;

/// Un directorio propio para cada prueba, para que dos que corren a la vez no
/// se pisen el `.bex`.
fn caja(nombre: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("inti-prueba-{}", nombre));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).expect("no puedo crear la caja");
    d
}

fn escribe(d: &PathBuf, nombre: &str, texto: &str) -> PathBuf {
    let p = d.join(nombre);
    std::fs::write(&p, texto).expect("no puedo escribir el fuente");
    p
}

fn compila(args: &[&str]) -> (bool, String, String) {
    let s = Command::new(env!("CARGO_BIN_EXE_inti"))
        .args(args)
        .output()
        .expect("no puedo ejecutar el compilador");
    (
        s.status.success(),
        String::from_utf8_lossy(&s.stdout).to_string(),
        String::from_utf8_lossy(&s.stderr).to_string(),
    )
}

const BUENO: &str = "\
perfil llano

funcion escala(x es flotante64, factor es flotante64) devuelve flotante64
    devuelve x * factor + 0.5

funcion principal devuelve entero32
    y es flotante64 = escala(2.0, 3.0)
    devuelve entero32(y)
";

/// **UN `.inti` DE UN DISCO PRODUCE UN `.bex` DE UN DISCO.**
///
/// ** Es la prueba que no se podia escribir hasta hoy, y su ausencia era lo que
/// hacia imposible la foto del Ryzen: sin esto no hay fichero que llevar a la
/// maquina.
#[test]
fn un_fuente_de_disco_produce_un_bex_de_disco() {
    let d = caja("basico");
    let fuente = escribe(&d, "float.inti", BUENO);
    let (bien, salida, err) = compila(&[fuente.to_str().unwrap()]);
    assert!(bien, "no compilo: {}{}", salida, err);

    let bex = d.join("float.bex");
    assert!(bex.exists(), "no escribio el `.bex`");
    let bytes = std::fs::read(&bex).expect("no puedo leer el `.bex`");
    assert!(bytes.len() > 64, "el `.bex` mide {} bytes", bytes.len());

    // ** Y es un BEF de verdad, no un fichero con la extension puesta. El gate
    // ya lo dijo dentro del compilador; esto lo comprueba desde fuera, que es
    // donde importa.
    assert_eq!(&bytes[..4], b"BEF1", "no lleva la marca del formato");
}

/// `-o` manda sobre el nombre por defecto.
#[test]
fn la_salida_se_puede_elegir() {
    let d = caja("salida");
    let fuente = escribe(&d, "x.inti", BUENO);
    let otro = d.join("otro_nombre.bex");
    let (bien, _, err) = compila(&[
        fuente.to_str().unwrap(),
        "-o",
        otro.to_str().unwrap(),
    ]);
    assert!(bien, "{}", err);
    assert!(otro.exists());
    assert!(!d.join("x.bex").exists(), "escribio ademas el de por defecto");
}

/// ** LO QUE ESTA MAL NO DEJA UN FICHERO EN EL DISCO.
///
/// Es la regla del gate mirada desde fuera: un compilador que escribe y luego se
/// queja deja un `.bex` malo con un mensaje al lado, y el que lo encuentre
/// manana vera el fichero y no el mensaje.
#[test]
fn un_fuente_con_errores_no_escribe_nada() {
    let d = caja("malo");
    let fuente = escribe(
        &d,
        "malo.inti",
        "perfil llano\n\nfuncion f(x es flotante64, n es entero64) devuelve flotante64\n    devuelve x + n\n",
    );
    let (bien, _, err) = compila(&[fuente.to_str().unwrap()]);
    assert!(!bien, "un fuente mal tiene que fallar");
    assert!(err.contains("E0022"), "no dijo cual era el problema: {}", err);
    assert!(!d.join("malo.bex").exists(), "escribio el `.bex` de todos modos");
}

/// ** LOS AVISOS SALEN TODOS, no solo el primero.
///
/// Un compilador que para en el primer error obliga a compilar diez veces para
/// ver diez errores -- y es la clase de detalle que decide si el lenguaje se
/// siente facil, que es de lo que va la seccion 9 del maestro.
#[test]
fn se_pintan_todos_los_avisos_y_no_solo_el_primero() {
    let d = caja("muchos");
    let fuente = escribe(
        &d,
        "dos.inti",
        "perfil llano\n\nfuncion f(x es flotante64, n es entero64) devuelve flotante64\n    si n\n        devuelve x + n\n    devuelve x\n",
    );
    let (_, _, err) = compila(&[fuente.to_str().unwrap()]);
    assert!(err.contains("E0040"), "falta el de la condicion: {}", err);
    assert!(err.contains("E0022"), "falta el de la mezcla: {}", err);
}

/// El mensaje lleva sus cuatro partes tambien aqui.
///
/// ** El contrato del mensaje no es del formateador: es del compilador. Si al
/// salir por la consola se perdiera una parte, el contrato seria una promesa que
/// solo se cumple en los tests.
#[test]
fn el_mensaje_conserva_sus_cuatro_partes() {
    let d = caja("mensaje");
    let fuente = escribe(
        &d,
        "m.inti",
        "perfil llano\n\nfuncion f(x es flotante64, n es entero64) devuelve flotante64\n    devuelve x + n\n",
    );
    let (_, _, err) = compila(&[fuente.to_str().unwrap()]);
    assert!(err.contains("E0022"), "sin codigo");
    assert!(err.contains("linea 4"), "sin donde: {}", err);
    assert!(err.contains("INTI no convierte"), "sin que habia");
    assert!(err.contains("prueba:"), "sin que hacer");
}

/// `-c` compila y no ensucia el disco.
#[test]
fn comprobar_no_escribe() {
    let d = caja("comprueba");
    let fuente = escribe(&d, "c.inti", BUENO);
    let (bien, salida, err) = compila(&[fuente.to_str().unwrap(), "-c"]);
    assert!(bien, "{}", err);
    assert!(salida.contains("compila"));
    assert!(!d.join("c.bex").exists(), "escribio con `-c`");
}

/// ** EL INFORME: los numeros que el compilador SABE.
///
/// Se calculaban desde F2a y no los veia nadie, porque no habia por donde
/// pedirlos. Un numero que nadie puede leer no se puede seguir en el tiempo, y
/// seguirlo en el tiempo era el punto entero de CABINA.
#[test]
fn el_informe_saca_los_numeros_de_cabina() {
    let d = caja("informe");
    let fuente = escribe(
        &d,
        "i.inti",
        "perfil llano\nusa x86_64\n\nfuncion f(a es entero64, b es entero64) devuelve entero64\n    crudo\n        devuelve lee_reloj()\n",
    );
    let (bien, salida, err) = compila(&[fuente.to_str().unwrap(), "-i"]);
    assert!(bien, "{}", err);
    for que in [
        "perfil",
        "bloques crudo",
        "instrucciones de maquina",
        "reglas pedidas",
        "reglas emitidas",
        "temporales en registro",
    ] {
        assert!(salida.contains(que), "el informe no dice `{}`:\n{}", que, salida);
    }
    // ** Y dice a QUE MAQUINA SE ATA, que es el numero con mas valor de todos:
    // un fichero que declara `usa x86_64` no se porta, y eso tiene que verse sin
    // leer el fuente.
    assert!(salida.contains("x86_64"), "no dice a que se ata:\n{}", salida);
}

/// Un fuente que NO nombra ninguna maquina lo dice, que es la otra mitad.
#[test]
fn un_fuente_portable_lo_dice_en_el_informe() {
    let d = caja("portable");
    let fuente = escribe(&d, "p.inti", BUENO);
    let (bien, salida, _) = compila(&[fuente.to_str().unwrap(), "-i"]);
    assert!(bien);
    assert!(
        salida.contains("se porta"),
        "no dice que se porta:\n{}",
        salida
    );
}
/// Sin argumentos, la ayuda -- y un codigo de salida que no es cero, para que un
/// guion que lo llame mal no siga adelante creyendo que hizo algo.
#[test]
fn sin_fichero_sale_la_ayuda_y_no_dice_que_todo_fue_bien() {
    let (bien, salida, _) = compila(&[]);
    assert!(!bien, "sin fichero no puede salir bien");
    assert!(salida.contains("INTI"));
}


// ===================================================================
//  ** LO QUE EL COMPILADOR SABE TIENE QUE SALIR POR LA PUERTA
// ===================================================================
//
//  El compilador corre CINCO analisis. La linea de ordenes pintaba TRES.
//
//  Los otros dos --`perfil` y `nombres`-- se calculaban y se tiraban, asi que
//  `crudo` dentro de `perfil pleno` no se denunciaba y un nombre desconocido
//  tampoco. La sonda `p04_crudo_en_pleno` del censo daba su `E0071` en el banco
//  y salia LIMPIA por la linea de ordenes -- que es por donde lo usa una
//  persona.
//
//  *** Y la causa no fue olvidar dos lineas: fue escribir a mano una lista que
//  ya existia en otro sitio. Es el mismo fallo que el censo tenia con sus diez
//  sondas, y el mismo que `SOLO_EN_METAL` evita al ser una lista de exenciones.
//
//  ** Por eso esta prueba NO enumera los cinco analisis. Compara lo que
//  `comprobar` produce contra lo que la consola imprime, sea lo que sea. Un
//  analisis nuevo entra en la comparacion solo. Una lista de cinco aqui tendria
//  exactamente el fallo que esta vigilando.

/// Fuentes que rompen cada analisis por separado.
///
/// ** Uno por analisis y no un programa que los rompa todos: si se mezclaran,
/// un aviso podria tapar a otro --`nombres` calla si `sintaxis` no leyo el
/// arbol-- y la prueba pasaria por el motivo equivocado.
const ROMPE_UNO: &[(&str, &str)] = &[
    (
        "perfil",
        "perfil pleno\n\nfuncion principal\n    crudo\n        cambiante x = 1\n",
    ),
    (
        "nombres",
        "perfil llano\n\nfuncion f(a es entero64, b es entero64) devuelve entero64\n    devuelve no_existe_esto(a)\n",
    ),
    (
        "disposicion",
        "perfil llano\n\nregistro P\n    x es texto\n",
    ),
    (
        "tipos",
        "perfil llano\n\nfuncion f(x es flotante64, n es entero64) devuelve flotante64\n    devuelve x + n\n",
    ),
];

/// **EL INVARIANTE**: ningun aviso se queda dentro.
///
/// Lo que `comprobar` sabe y la consola no dice es un fallo que el usuario no
/// puede ver de ninguna forma -- y un compilador que se calla la mitad de lo que
/// sabe es peor que uno que no lo sabe, porque el silencio parece aprobacion.
#[test]
fn la_consola_no_se_calla_ningun_aviso_que_el_compilador_conozca() {
    for (analisis, fuente) in ROMPE_UNO {
        let d = caja(&format!("puerta-{}", analisis));
        let ruta = escribe(&d, "x.inti", fuente);

        // Lo que el compilador sabe, preguntado en proceso.
        let sabe: Vec<&str> = bmo_inti_front::comprobar(fuente).codigos();
        assert!(
            !sabe.is_empty(),
            "el fuente de `{}` ya no rompe nada: la prueba dejo de probar algo",
            analisis
        );

        // Y lo que dice por la consola.
        let (_, _, err) = compila(&[ruta.to_str().unwrap(), "-c"]);
        for codigo in &sabe {
            assert!(
                err.contains(codigo),
                "el analisis de `{}` produce `{}` y la consola NO lo dice.\n\
                 sabe: {:?}\nconsola:\n{}",
                analisis,
                codigo,
                sabe,
                err
            );
        }
    }
}

/// Y no escribe un `.bex` cuando cualquiera de ellos falla.
///
/// ** Es la otra mitad y no es la misma prueba: un compilador podria pintar el
/// aviso y escribir el fichero igual. Entonces el que lo encuentre manana veria
/// el `.bex` y no el mensaje -- que es la regla del gate mirada desde fuera.
#[test]
fn ningun_analisis_deja_escribir_un_bex_cuando_denuncia() {
    for (analisis, fuente) in ROMPE_UNO {
        let d = caja(&format!("gate-{}", analisis));
        let ruta = escribe(&d, "y.inti", fuente);
        let (bien, _, _) = compila(&[ruta.to_str().unwrap()]);
        assert!(!bien, "`{}` denuncia y el compilador salio bien", analisis);
        assert!(
            !d.join("y.bex").exists(),
            "`{}` denuncia y aun asi escribio el `.bex`",
            analisis
        );
    }
}

/// ** LA SONDA DEL CENSO, POR LA PUERTA DE VERDAD.
///
/// `p04_crudo_en_pleno` declara `E0071` desde F0 y el banco lo comprobaba
/// llamando a `comprobar` directamente. Por la linea de ordenes salia limpia.
///
/// Que una sonda del corpus se compruebe SOLO por dentro es la misma clase de
/// hueco que tenia el propio censo: se prueba el camino que no usa nadie.
#[test]
fn una_sonda_del_censo_da_su_codigo_por_la_linea_de_ordenes() {
    let sonda = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("censo")
        .join("p04_crudo_en_pleno.inti");
    let (bien, _, err) = compila(&[sonda.to_str().unwrap(), "-c"]);
    assert!(!bien, "una sonda que declara un error no puede compilar");
    assert!(
        err.contains("E0071"),
        "la sonda declara E0071 y la consola dice:\n{}",
        err
    );
}
