//! **S1: el binario declara SUS REGLAS y donde corta cada una.**
//!
//! ## Lo que se prueba aqui y no en el ABI
//!
//! `bmo-abi::bef::katanas` prueba el FORMATO: que la tabla se escribe, se lee y
//! se caza cuando miente sobre sus propios limites. No sabe x86 y no tiene por
//! que.
//!
//! *** Lo que se prueba aqui es lo unico que le da sentido: **que en el offset
//! declarado haya de verdad un bloque de trampa, y con el codigo que dice**.
//! Una tabla que apunta correctamente a bytes que no son una trampa es una tabla
//! bien formada y mentirosa -- y es exactamente la mentira que esto viene a
//! cerrar.
//!
//! El patron, en bytes, y por que se puede escribir aqui:
//!
//! ```text
//!    48 B8 <imm64>    mov rax, codigo      IZQ = rax, el registro de retorno
//!    48 89 EC         mov rsp, rbp
//!    5D               pop rbp
//!    C3               ret
//! ```
//!
//! Este crate ES x86-64 -- lo dice su nombre y su cabecera. Escribir el patron
//! en el ABI habria metido conocimiento de una maquina en el sitio que
//! precisamente no puede tenerlo.

use std::path::PathBuf;
use std::process::Command;

use bmo_abi::bmo_abi::bef::katanas;
use bmo_abi::bmo_abi::bef::paquete;
use bmo_abi::bmo_abi::bef::sections::SectionKind;

fn caja(nombre: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("inti-katanas-{}", nombre));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).expect("no puedo crear la caja");
    d
}

fn compila(fuente: &PathBuf) -> Vec<u8> {
    let s = Command::new(env!("CARGO_BIN_EXE_inti"))
        .arg(fuente.to_str().unwrap())
        .output()
        .expect("no puedo ejecutar el compilador");
    assert!(
        s.status.success(),
        "no compilo:\n{}{}",
        String::from_utf8_lossy(&s.stdout),
        String::from_utf8_lossy(&s.stderr)
    );
    std::fs::read(fuente.with_extension("bex")).expect("no hay `.bex`")
}

/// Las tres reglas que llegan a bytes, cada una provocada a proposito.
const LAS_TRES: &str = "\
perfil llano

funcion desborda devuelve natural64
    cambiante x es entero64 = 4000000000
    devuelve x * x

funcion entre_cero devuelve natural64
    cambiante c es entero64 = 0
    devuelve 10 entre c

funcion convierte_de_mas devuelve natural64
    devuelve entero32(1e30)

funcion principal devuelve entero32
    devuelve 0
";

/// **En el offset declarado hay un bloque de trampa, y lleva el codigo que la
/// tabla dice.**
///
/// Es la prueba que convierte la tabla en un contrato. Sin ella la tabla es una
/// lista de numeros que nadie ha contrastado con nada.
#[test]
fn cada_katana_declarada_esta_donde_dice_y_lleva_su_codigo() {
    let d = caja("donde");
    let fuente = d.join("tres.inti");
    std::fs::write(&fuente, LAS_TRES).unwrap();
    let bex = compila(&fuente);

    let tabla = paquete::seccion(&bex, SectionKind::Katanas)
        .expect("el `.bex` no trae seccion Katanas");
    let codigo = paquete::seccion(&bex, SectionKind::Code).expect("sin seccion Code");

    let n = katanas::revisar(tabla, codigo.len()).expect("la tabla no se sostiene");
    assert!(n >= 3, "las tres reglas tenian que declararse, salieron {}", n);

    let mut vistos = Vec::new();
    for i in 0..n {
        let k = katanas::katana(tabla, i).unwrap();
        let b = &codigo[k.offset as usize..(k.offset + k.longitud) as usize];

        // `mov rax, imm64`
        assert_eq!(
            &b[..2],
            &[0x48, 0xB8],
            "la katana {} no empieza por `mov rax, imm64`: {:02x?}",
            i,
            &b[..2.min(b.len())]
        );
        let inmediato = u64::from_le_bytes(b[2..10].try_into().unwrap());
        assert_eq!(
            inmediato, k.codigo as u64,
            "la tabla dice codigo {} y el bloque devuelve {}",
            k.codigo, inmediato
        );
        // `mov rsp,rbp` + `pop rbp` + `ret`
        assert_eq!(
            &b[10..],
            &[0x48, 0x89, 0xEC, 0x5D, 0xC3],
            "la katana {} no acaba en un epilogo",
            i
        );
        vistos.push(k.codigo);
    }

    vistos.sort_unstable();
    vistos.dedup();
    for esperado in [1001u32, 1003, 1012] {
        assert!(
            vistos.contains(&esperado),
            "falta la regla E{} en la tabla: {:?}",
            esperado,
            vistos
        );
    }
}

/// **Un binario sin reglas trae la tabla VACIA, no ninguna tabla.**
///
/// ** Cero katanas es una respuesta --*"este binario no comprueba nada, y lo
/// dice"*--. No traer tabla es no decir nada, y las dos cosas no se pueden
/// confundir: la primera se puede contrastar, la segunda no.
#[test]
fn un_binario_sin_reglas_declara_cero_y_no_calla() {
    let d = caja("vacia");
    let fuente = d.join("simple.inti");
    std::fs::write(
        &fuente,
        "perfil llano\n\nfuncion principal devuelve entero32\n    devuelve 0\n",
    )
    .unwrap();
    let bex = compila(&fuente);

    let tabla = paquete::seccion(&bex, SectionKind::Katanas)
        .expect("hasta un binario sin reglas trae su tabla");
    assert_eq!(katanas::cuantas(tabla).unwrap(), 0);
}

/// **La sonda del Ryzen declara sus katanas.**
///
/// `cpu.bex` es el fichero que vuela, y su linea `reglas = 0` del 22-08 dice que
/// las tres atrapan en metal. A partir de hoy el binario ademas **dice donde**,
/// asi que esa afirmacion se puede contrastar sin arrancar la maquina.
#[test]
fn la_sonda_declara_las_reglas_que_lleva() {
    let cpu = PathBuf::from("../sondas/cpu.inti");
    let texto = std::fs::read_to_string(&cpu).expect("no encuentro la sonda");
    let d = caja("sonda");
    let fuente = d.join("cpu.inti");
    std::fs::write(&fuente, texto).unwrap();
    let bex = compila(&fuente);

    let tabla = paquete::seccion(&bex, SectionKind::Katanas).expect("sin Katanas");
    let codigo = paquete::seccion(&bex, SectionKind::Code).expect("sin Code");
    let n = katanas::revisar(tabla, codigo.len()).expect("la tabla no se sostiene");
    assert!(n > 0, "la sonda provoca las tres reglas y declaro {}", n);
}
