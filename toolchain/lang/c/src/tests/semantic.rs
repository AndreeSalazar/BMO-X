//! La tabla de intrinsecos y la libreria `semantic/`
//!
//! Parte del banco de pruebas de BMO C. Los ayudantes (`run_c`,
//! `run_c_sembrado`, `ejecutar_bef`) viven en `tests/mod.rs`.

use super::*;

// ═══════════════ La tabla de intrinsecos, ENTERA ═══════════════

/// ★ Compila una llamada a **cada fila** de `intrinsics.toml`.
///
/// Es la matriz de conformidad de la tabla, y hacía falta desde que dejó de
/// tener doce filas: el codegen valida el nombre de cada registro **al
/// emitir**, así que una fila con `"rex"` en vez de `"rax"` no falla hasta
/// que alguien la usa — y en una tabla de driver "alguien la usa" puede ser
/// dentro de seis meses, en metal, buscando otra cosa.
///
/// No comprueba que los bytes sean los correctos: eso lo dice el manual de
/// Intel y está en la fila. Comprueba que la fila es **emitible**.
#[test]
fn cada_intrinseco_de_la_tabla_compila() {
    let tabla = bmo_sem_asm::Intrinsics::load_x86_64().expect("la tabla tiene que cargar");
    let nombres = tabla.names();
    assert!(nombres.len() >= 40, "la tabla se ha quedado corta: {}", nombres.len());

    for nombre in nombres {
        let def = tabla.get(nombre).unwrap();
        let ceros: Vec<&str> = vec!["0"; def.args.len()];
        let fuente = format!(
            "int main() {{ __{nombre}({}); return 0; }}",
            ceros.join(", ")
        );
        match compile_source_to_bef(&fuente) {
            Ok(bef) => assert!(!bef.is_empty(), "__{nombre} no produjo nada"),
            Err(e) => panic!("__{nombre} no compila: {}", e.message),
        }
    }
}

/// Y la aridad se valida contra la tabla: pasarle un argumento de más a una
/// instrucción que no lo tiene es un error, no un argumento ignorado.
#[test]
fn un_intrinseco_con_argumentos_de_mas_se_rechaza() {
    let err = compile_source_to_bef("int main() { __hlt(1, 2); return 0; }")
        .expect_err("hlt no toma argumentos");
    assert!(err.message.contains("hlt"), "mensaje: {}", err.message);
}

/// Un nombre con `__` que no está en la tabla se dice, y se dice DÓNDE
/// mirar. El namespace `__` es de la implementación, así que aquí no puede
/// caer a "función desconocida".
#[test]
fn un_intrinseco_que_no_existe_dice_donde_estan() {
    let err = compile_source_to_bef("int main() { __inventado(); return 0; }")
        .expect_err("no existe");
    assert!(err.message.contains("intrinsics.toml"), "mensaje: {}", err.message);
}

// ═══════════════ La libreria SEMANTIC ═══════════════
//
// Cada funcion ES una instruccion. Ver `tables/semantic/semantic.h` para
// que hacen GCC, MSVC y Clang con esto mismo, y en que se diferencia BMO
// (aqui es una fila de TOML, alli son miles de lineas de C++).

#[test]
fn semantic_compila_entera() {
    let out = run_c_con_pp(
        "#include <semantic/semantic.h>
         int main() { respira(); barrera_total(); barrera_escrituras();              barrera_lecturas(); printf(\"ok\n\"); return 0; }",
    );
    assert_eq!(out.trim(), "ok");
}

/// ★ Un atomico devuelve **lo que HABIA**, no lo que se puso. Es lo que se
/// escribe al reves sin notarlo, y no se ve en un volcado de bytes.
#[test]
fn xchg_devuelve_lo_que_habia() {
    let out = run_c_con_pp(
        "#include <semantic/semantic.h>
         int main() { u64 c; c = 7;              printf(\"%d %d\n\", (int)atomico_xchg(&c, 42), (int)c); return 0; }",
    );
    assert_eq!(out.trim(), "7 42");
}

/// El compara-e-intercambia, en sus dos caminos: cuando cuadra cambia, y
/// cuando no cuadra **deja el valor y devuelve el de verdad**, que es lo que
/// permite reintentar sin releer.
#[test]
fn cas_cambia_solo_si_cuadra_y_siempre_dice_lo_que_habia() {
    let out = run_c_con_pp(
        "#include <semantic/semantic.h>
         int main() { u64 c; c = 5;              printf(\"%d %d \", (int)atomico_cas(&c, 5, 9), (int)c);              printf(\"%d %d\n\", (int)atomico_cas(&c, 5, 77), (int)c); return 0; }",
    );
    assert_eq!(out.trim(), "5 9 9 9");
}

/// `xadd` suma y entrega lo ANTERIOR: un contador que reparte numeros sin
/// dar el mismo dos veces.
#[test]
fn xadd_entrega_el_valor_anterior() {
    let out = run_c_con_pp(
        "#include <semantic/semantic.h>
         int main() { u64 c; c = 100;              printf(\"%d %d %d\n\", (int)atomico_sumar_y_devolver(&c, 1),              (int)atomico_sumar_y_devolver(&c, 1), (int)c); return 0; }",
    );
    assert_eq!(out.trim(), "100 101 102");
}

#[test]
fn los_atomicos_sin_retorno_modifican_la_memoria() {
    let out = run_c_con_pp(
        "#include <semantic/semantic.h>
         int main() { u64 c; c = 8; atomico_sumar(&c, 4);              atomico_encender(&c, 1);              printf(\"%d\n\", (int)c); return 0; }",
    );
    assert_eq!(out.trim(), "13");
}

/// Contar y buscar bits — de lo que vive un asignador de marcos.
#[test]
fn los_intrinsecos_de_bits_cuentan_bien() {
    let out = run_c_con_pp(
        "#include <semantic/semantic.h>
         int main() { printf(\"%d %d %d %x\n\", bits_contar(0xF0F0),              bits_primero(0x00100000), bits_ultimo(0x00100000),              bytes_al_reves(0x11223344)); return 0; }",
    );
    assert_eq!(out.trim(), "8 20 20 44332211");
}

/// ★ `bsf` con cero es INDEFINIDO y el emulador lo modela como el silicio:
/// deja el destino intacto. `tzcnt` sí está definido y da 32. La diferencia
/// es la que hace que un mapa de bits lleno reserve un marco ya dado.
#[test]
fn tzcnt_esta_definido_en_cero_y_bsf_no() {
    let out = run_c_con_pp(
        "#include <semantic/semantic.h>
         int main() { printf(\"%d\n\", bits_ceros_derecha(0)); return 0; }",
    );
    assert_eq!(out.trim(), "32");
}
