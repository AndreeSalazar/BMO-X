//! **UN PNG ESCRITO EN INTI, ejecutado y validado.**
//!
//! ## Por que esta prueba existe
//!
//! Pregunta de Eddi (2026-08-22): *"INTI podria llegar a ser versatil parecido a
//! Python? Digamos construir .png, .svg... o pixeles?"*
//!
//! ** Se podia contestar de palabra. Se contesta ejecutandolo: `ejemplos/png.inti`
//! construye una imagen de 8x8 en memoria, la envuelve en un PNG y la escribe en
//! el disco; aqui se corre en el emulador, se saca el fichero y **se valida el
//! formato byte a byte** -- firma, trozos, longitudes y los CRC-32 de verdad.
//!
//! *** Y la validacion se hace AQUI y no dentro del programa a proposito. Si el
//! propio `png.inti` comprobara su CRC, estaria comprobando su propia
//! aritmetica contra si misma. El CRC se recalcula en Rust, con otro codigo, y
//! por eso el acuerdo significa algo.
//!
//! ## Lo que este programa demuestra, y lo que no
//!
//! **Demuestra** que `perfil llano` --sin monton que crezca, sin texto, sin
//! listas-- alcanza para escribir un formato de fichero real: bytes, aritmetica
//! entera exacta, bucles, y la puerta del sistema.
//!
//! **No demuestra** que INTI sea versatil como Python. El PNG sale sin comprimir
//! --bloques `stored` de zlib, que el formato admite-- porque DEFLATE de verdad
//! pide tablas de Huffman, y eso pide `lista de T`.

use std::path::PathBuf;

use bmo_lower::emu::{run, Machine};

/// El programa, tal y como se entrega. No una copia dentro del test: una copia
/// probaria que el test compila su propia copia.
fn fuente() -> String {
    std::fs::read_to_string(PathBuf::from("../ejemplos/png.inti"))
        .expect("no encuentro `ejemplos/png.inti`")
}

fn emitido(texto: &str) -> bmo_inti_x86_64::Emitido {
    let arbol = bmo_inti_front::armar(texto);
    assert!(
        !arbol.hay_errores(),
        "el programa no se lee: {}",
        arbol.pintar("png.inti")
    );
    let raices = bmo_mods::Roots::find();
    let modulos = bmo_inti_front::tablas::Modulos::cargar(&raices);
    let plano = bmo_inti_front::disposicion::comprobar(
        &arbol.valor,
        bmo_inti_front::disposicion::Medidas::cargar(&raices),
    );
    let metal = bmo_inti_front::ir::metal_que_declara(&arbol.valor, &raices, &modulos);
    let ir = bmo_inti_front::ir::bajar_con(&arbol.valor, &modulos, &plano.valor, &metal).valor;
    bmo_inti_x86_64::emitir(&ir)
}

/// El CRC-32 del PNG, recalculado en Rust. **Otro codigo, a proposito.**
fn crc32(datos: &[u8]) -> u32 {
    let mut c: u32 = 0xFFFF_FFFF;
    for b in datos {
        c ^= *b as u32;
        for _ in 0..8 {
            c = if c & 1 == 1 { (c >> 1) ^ 0xEDB8_8320 } else { c >> 1 };
        }
    }
    c ^ 0xFFFF_FFFF
}

/// ***`llano` ESCRIBE UN PNG VALIDO.***
#[test]
fn el_png_que_escribe_inti_es_un_png() {
    let e = emitido(&fuente());
    assert!(e.arranca, "el programa tiene `principal`");
    // ** El presupuesto es grande porque el CRC son ocho vueltas por byte sobre
    // doscientos bytes, y ademas el adler. Un limite corto no daria un fallo:
    // daria un fichero a medias, que es peor.
    let m = run(Machine::new(e.codigo), 20_000_000);

    let png = m
        .archivo("/inti/hola.png")
        .expect("el programa no dejo el fichero en el disco");

    // -- La firma, que es lo unico que un PNG no puede no tener.
    assert_eq!(
        &png[..8],
        &[137, 80, 78, 71, 13, 10, 26, 10],
        "la firma no es la de un PNG. mide {} y empieza: {:02X?}",
        png.len(),
        &png[..32.min(png.len())]
    );

    // -- Los trozos, recorridos como los recorreria un lector de verdad.
    let mut i = 8usize;
    let mut vistos: Vec<String> = Vec::new();
    while i + 12 <= png.len() {
        let largo = u32::from_be_bytes(png[i..i + 4].try_into().unwrap()) as usize;
        let nombre = String::from_utf8_lossy(&png[i + 4..i + 8]).to_string();
        let fin = i + 8 + largo;
        assert!(fin + 4 <= png.len(), "el trozo `{}` se sale del fichero", nombre);

        // *** El CRC cubre el NOMBRE y los datos, no la longitud. Es el error
        // clasico de quien escribe un PNG por primera vez, y por eso se
        // comprueba: si INTI lo hubiera hecho mal, aqui saldria.
        let esperado = crc32(&png[i + 4..fin]);
        let dice = u32::from_be_bytes(png[fin..fin + 4].try_into().unwrap());
        assert_eq!(
            dice, esperado,
            "el CRC del trozo `{}` no cuadra: dice {:#010x} y es {:#010x}",
            nombre, dice, esperado
        );

        if nombre == "IHDR" {
            let ancho = u32::from_be_bytes(png[i + 8..i + 12].try_into().unwrap());
            let alto = u32::from_be_bytes(png[i + 12..i + 16].try_into().unwrap());
            assert_eq!((ancho, alto), (8, 8), "la imagen no mide lo que dice");
            assert_eq!(png[i + 16], 8, "ocho bits por canal");
            assert_eq!(png[i + 17], 2, "color 2 = RGB");
        }
        vistos.push(nombre.clone());
        i = fin + 4;
        if nombre == "IEND" {
            break;
        }
    }

    assert_eq!(
        vistos,
        vec!["IHDR".to_string(), "IDAT".to_string(), "IEND".to_string()],
        "los trozos no son los que un PNG minimo necesita"
    );
    assert_eq!(i, png.len(), "sobran bytes despues del IEND");
}

/// **Y el programa se PORTA**: ni una instruccion de maquina.
///
/// ** No es un detalle: un escritor de formatos que se ata a un procesador no
/// sirve para lo que sirve un escritor de formatos. Lo unico que este programa
/// pide del sistema es la puerta, y la puerta es la misma en toda maquina.
#[test]
fn el_escritor_de_png_no_se_ata_a_ninguna_maquina() {
    let texto = fuente();
    let (parte, _) = bmo_inti_front::informar(&texto, "png.inti");
    assert!(
        parte.arquitecturas.is_empty(),
        "el escritor de PNG se ato a {:?}",
        parte.arquitecturas
    );
    assert_eq!(parte.perfil, "llano");
}
