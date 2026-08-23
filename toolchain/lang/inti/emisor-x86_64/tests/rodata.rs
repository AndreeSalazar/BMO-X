//! **LAS TABLAS CONGELADAS, EN `RoData` -- y no dentro del codigo.**
//!
//! ## Que se prueba aqui
//!
//! Que una `constante` cuyo valor es una lista de literales acaba en la seccion
//! `RoData = 0x02` del `.ibex`, con sus bytes, y que el codigo llega a ella por
//! una reubicacion `SeccionAbs64` -- **no por una direccion inventada al
//! emitir**, que no se puede saber hasta que el cargador la coloque.
//!
//! ## *** Y lo que NO se prueba aqui, dicho por delante
//!
//! Que el cargador la aplique. Eso pasa en Ring 0 y en el Ryzen; aqui se
//! comprueba que **lo que se escribe es lo que el cargador espera leer**: el
//! tipo, la seccion destino, el offset dentro de ella, y que en el hueco hay
//! ocho ceros y no basura.
//!
//! Es la misma frontera que la mesa de katanas: se puede demostrar que la
//! declaracion cuadra con los bytes; que el silicio la ejecute es otro sitio.

use std::path::PathBuf;
use std::process::Command;

use bmo_abi::bmo_abi::bef::paquete;
use bmo_abi::bmo_abi::bef::sections::SectionKind;

fn caja(nombre: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("inti-rodata-{}", nombre));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).expect("no puedo crear la caja");
    d
}

fn compila(fuente_texto: &str, nombre: &str) -> Vec<u8> {
    let d = caja(nombre);
    let f = d.join("prog.inti");
    std::fs::write(&f, fuente_texto).unwrap();
    let s = Command::new(env!("CARGO_BIN_EXE_inti"))
        .arg(f.to_str().unwrap())
        .output()
        .expect("no puedo ejecutar el compilador");
    assert!(
        s.status.success(),
        "no compilo:\n{}{}",
        String::from_utf8_lossy(&s.stdout),
        String::from_utf8_lossy(&s.stderr)
    );
    std::fs::read(f.with_extension("ibex")).expect("no hay `.ibex`")
}

const PRIMOS: &str = "\
perfil llano
usa memoria

PRIMOS = [2, 3, 5, 7, 11, 13, 17, 19]

funcion primo(i es natural64) devuelve natural64
    crudo
        devuelve lee_natural64(PRIMOS + i * 8)

funcion principal devuelve entero32
    devuelve 0
";

fn u64_en(b: &[u8], i: usize) -> u64 {
    u64::from_le_bytes(b[i..i + 8].try_into().unwrap())
}

/// ***UNA TABLA CONSTANTE LLEGA A BYTES, Y CON SUS VALORES.***
///
/// ** Hasta el 2026-08-22 esto no se podia ni escribir: `perfil` denunciaba la
/// lista con `E0070`, *"lo que crece pide memoria"* -- y una constante **no
/// crece**, esta CONGELADA. Es la seccion 10.2 del maestro aplicada a un caso
/// que la comprobacion no distinguia.
#[test]
fn una_tabla_constante_acaba_en_rodata() {
    let bex = compila(PRIMOS, "primos");
    let rodata = paquete::seccion(&bex, SectionKind::RoData)
        .expect("el `.ibex` no trae seccion RoData");

    assert_eq!(rodata.len(), 8 * 8, "ocho primos de ocho bytes");
    let leidos: Vec<u64> = (0..8).map(|i| u64_en(rodata, i * 8)).collect();
    assert_eq!(leidos, vec![2, 3, 5, 7, 11, 13, 17, 19]);
}

/// ***Y EL CODIGO LLEGA A ELLA POR UNA REUBICACION, no por un numero inventado.***
///
/// La direccion de `RoData` **no se puede saber al emitir**: la elige el
/// cargador. Asi que el emisor deja un `mov rax, imm64` con ocho ceros y apunta
/// donde esta el hueco.
///
/// *** Esta prueba mira las cuatro cosas que el cargador va a leer, porque
/// cualquiera de las cuatro mal deja un binario que carga y salta a otro sitio:
/// el TIPO de reubicacion, la SECCION destino, el OFFSET dentro de ella, y que
/// en el hueco haya ceros.
#[test]
fn el_codigo_apunta_a_la_tabla_con_una_reubicacion() {
    let bex = compila(PRIMOS, "reloc");
    let relocs = paquete::seccion(&bex, SectionKind::Relocs)
        .expect("el `.ibex` no trae seccion Relocs");
    let codigo = paquete::seccion(&bex, SectionKind::Code).expect("sin Code");

    assert_eq!(relocs.len(), 24, "una sola reubicacion, de 24 bytes");
    let donde = u64_en(relocs, 0) as usize;
    let seccion_destino = u32::from_le_bytes(relocs[8..12].try_into().unwrap());
    let tipo = relocs[12];
    let seccion_del_hueco = relocs[13];
    let dentro = i64::from_le_bytes(relocs[16..24].try_into().unwrap());

    // `SeccionAbs64 = 0x04`: una posicion dentro de otra seccion, sin simbolo.
    assert_eq!(tipo, 0x04, "tiene que ser SeccionAbs64");
    // ** Y ojo con la trampa que el propio formato deja escrita: los codigos de
    // seccion de una reubicacion NO son los de `SectionKind`. Aqui 2 = rodata,
    // 0 = code.
    assert_eq!(seccion_destino, 2, "el destino es rodata");
    assert_eq!(seccion_del_hueco, 0, "el hueco vive en el codigo");
    assert_eq!(dentro, 0, "la primera tabla empieza en el byte 0 de rodata");

    // Los dos bytes de antes son `mov rax, imm64`, y el hueco son OCHO CEROS.
    assert_eq!(
        &codigo[donde - 2..donde],
        &[0x48, 0xB8],
        "el hueco no viene detras de un `mov rax, imm64`"
    );
    assert_eq!(
        &codigo[donde..donde + 8],
        &[0u8; 8],
        "el hueco tiene que estar a cero: si trae basura, el cargador escribe encima de algo"
    );
}

/// **Dos tablas van a sitios distintos y alineadas a ocho.**
///
/// ** La alineacion no es cosmetica: una tabla de `entero64` leida a medias es
/// lenta en el mejor caso y una excepcion en el peor.
#[test]
fn dos_tablas_no_se_pisan_y_van_alineadas() {
    let f = "\
perfil llano
usa memoria

UNOS = [1, 1, 1]
DOSES = [2, 2]

funcion suma devuelve natural64
    crudo
        devuelve lee_natural64(UNOS) + lee_natural64(DOSES)

funcion principal devuelve entero32
    devuelve 0
";
    let bex = compila(f, "dos");
    let rodata = paquete::seccion(&bex, SectionKind::RoData).expect("sin RoData");
    let relocs = paquete::seccion(&bex, SectionKind::Relocs).expect("sin Relocs");

    assert_eq!(rodata.len(), 5 * 8, "tres unos y dos doses");
    assert_eq!(u64_en(rodata, 0), 1);
    assert_eq!(u64_en(rodata, 24), 2, "la segunda tabla empieza en el byte 24");

    let mut destinos: Vec<i64> = (0..relocs.len() / 24)
        .map(|i| i64::from_le_bytes(relocs[i * 24 + 16..i * 24 + 24].try_into().unwrap()))
        .collect();
    destinos.sort_unstable();
    destinos.dedup();
    assert_eq!(destinos, vec![0, 24], "las dos tablas tienen que ir a sitios distintos");
    assert!(destinos.iter().all(|d| d % 8 == 0), "alineadas a ocho");
}

/// **Y el gate lo acepta**: un `.ibex` con RoData y reubicaciones sigue pasando.
///
/// ** No es obvio: `validate_reloc_section` comprueba que cada reubicacion caiga
/// dentro de su seccion, y una mal escrita rechazaria el fichero. Que pase
/// quiere decir que lo que se escribio tiene la forma que el validador espera.
#[test]
fn el_gate_acepta_un_binario_con_tabla() {
    let bex = compila(PRIMOS, "gate");
    let (veredicto, avisos) = bmo_verify::verify_verbose(&bex);
    assert!(veredicto.is_ok(), "el gate lo rechaza: {:?}", veredicto);
    assert!(
        !avisos.iter().any(|a| a.contains("reloc")),
        "el validador se queja de las reubicaciones: {:?}",
        avisos
    );
}

/// ***Y LOS DATOS NO ESTAN DENTRO DEL CODIGO.***
///
/// *** Es la prueba que protege la exclusividad de INTI. Meter la tabla en
/// `Code` habria sido mas corto --no harian falta reubicaciones-- y **habria
/// roto el barrido lineal**: un recorrido en linea recta empezaria a decodificar
/// primos como si fueran instrucciones.
///
/// Un binario de C mete datos entre las instrucciones y por eso no se puede
/// recorrer. Que INTI no lo haga es la restriccion que paga esa propiedad, y
/// esta prueba es la que impide perderla sin querer.
#[test]
fn la_tabla_no_esta_en_la_seccion_de_codigo() {
    let bex = compila(PRIMOS, "puro");
    let codigo = paquete::seccion(&bex, SectionKind::Code).expect("sin Code");

    // Los primos, en little-endian, no pueden aparecer dentro del codigo.
    for p in [11u64, 13, 17, 19] {
        let patron = p.to_le_bytes();
        assert!(
            !codigo.windows(8).any(|w| w == patron),
            "el primo {} aparece DENTRO del codigo: la tabla se colo en `Code`",
            p
        );
    }
}
