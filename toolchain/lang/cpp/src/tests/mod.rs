//! **El banco de pruebas de BMO C++.**
//!
//! Mismo criterio que C y COBOL: **se ejecutan los bytes emitidos**, no se
//! comparan contra bytes escritos a mano. Un codegen que produce números
//! erróneos se ve sanísimo en un volcado hexadecimal.
//!
//! Aquí viven los ayudantes; cada tema tendrá su fichero según crezca.

use super::*;
use bmo_abi::bef::sections::{SectionEntry, SectionKind};
use bmo_lower::emu::{run, Machine};

mod matriz;

/// Compila un programa de C++ y devuelve lo que el kernel habría pintado.
fn run_cpp(fuente: &str) -> String {
    let bef = compile_source_to_bef(fuente)
        .unwrap_or_else(|e| panic!("debe compilar: {}", e.message));
    ejecutar_bef(&bef)
}

/// Compila y ejecuta, y devuelve la máquina entera.
///
/// ⚠ **El código de retorno de `main` NO es observable**, y no es un descuido
/// de C++: `TASK_OP_EXIT` no acepta código de salida hoy —el kernel hace
/// revoke + reap— así que el codegen de C descarta `rax` a propósito y lo deja
/// escrito. Un test que comprobara el 42 estaría comprobando algo que el
/// sistema no expone.
fn ejecutar(fuente: &str) -> Machine {
    let bef = compile_source_to_bef(fuente)
        .unwrap_or_else(|e| panic!("debe compilar: {}", e.message));
    maquina_de_bef(&bef)
}

/// La imagen se rearma en el MISMO orden en que el codegen la dispuso:
/// código, luego rodata, luego data. Los `lea [rip+disp]` con los que se
/// alcanzan las cadenas se calcularon asumiendo que van pegadas detrás del
/// código; cargar sólo la sección CODE deja esos punteros en el vacío.
///
/// Es copia deliberada del ayudante de C: el banco de pruebas es de cada
/// lenguaje (ver `HERENCIA.md`, regla 4 — los tests no se mezclan).
fn ejecutar_bef(bef: &[u8]) -> String {
    maquina_de_bef(bef).console
}

fn maquina_de_bef(bef: &[u8]) -> Machine {
    let hdr = unsafe { &*(bef.as_ptr() as *const bmo_abi::bef::header::BefHeader) };
    let entry = hdr.entry_offset as usize;
    let sec_off = hdr.section_table_offset as usize;

    let mut code = Vec::new();
    for kind in [SectionKind::Code, SectionKind::RoData, SectionKind::Data] {
        for i in 0..hdr.section_count as usize {
            let e = sec_off + i * SectionEntry::SIZE;
            if bef[e] == kind as u8 {
                let off = u64::from_le_bytes(bef[e + 8..e + 16].try_into().unwrap()) as usize;
                let size = u64::from_le_bytes(bef[e + 16..e + 24].try_into().unwrap()) as usize;
                code.extend_from_slice(&bef[off..off + size]);
            }
        }
    }
    assert!(!code.is_empty(), "el BEF no tiene seccion CODE");

    let mut machine = Machine::new(code);
    machine.rip = entry;
    let machine = run(machine, 500_000);
    assert!(machine.exited, "el programa debe terminar por INVOKE(EXIT)");
    machine
}

// ── Paso 0: que emita un byte ───────────────────────────────────────

/// ★ **La prueba de vida del paso 0.**
///
/// Antes de esto, el frontend no producía bytes para NINGUNA entrada — ni
/// para un fichero vacío. Si este test se pone en rojo, C++ ha vuelto a no
/// existir, dé lo que dé todo lo demás.
#[test]
fn emite_un_bef_de_verdad() {
    let bef = compile_source_to_bef("int main() { return 42; }")
        .expect("el paso 0 es que ESTO compile");
    assert!(bef.len() > 48, "un BEF con cabecera y nada dentro no es un BEF");
    assert_eq!(
        u32::from_le_bytes(bef[..4].try_into().unwrap()),
        bmo_abi::bef::BEF_MAGIC,
        "los primeros cuatro bytes tienen que ser el magic del BEF",
    );
}

/// ★ Y que además CORRA, **desde la fuente**.
///
/// Emitir bytes con la forma correcta y que no ejecuten es el fallo que un
/// test de cabecera no ve. Esta es la prueba de vida completa del paso 0:
/// texto de C++ → parseo → descenso → codegen de C → bytes → **ejecución**.
///
/// `maquina_de_bef` ya exige que el programa termine por la puerta; si se
/// quedara dando vueltas o se saliera del código emitido, esto se pone rojo.
#[test]
fn el_programa_minimo_corre_desde_la_fuente() {
    let m = ejecutar("int main() { return 42; }");
    assert!(m.exited, "tiene que salir por INVOKE(EXIT)");
}

/// ★★ **El contrato de `HERENCIA.md`, ejecutable.**
///
/// Si C++ produce el `Program` de C, entonces para una fuente que es válida en
/// los dos lenguajes tiene que salir **exactamente el mismo BEF, byte a byte**.
///
/// Es la prueba más fuerte que el paso 0 puede dar y no necesita observar
/// nada: cualquier cosa que el descenso se invente —un tipo distinto, una
/// ranura de pila de más, un nodo perdido— cambia los bytes y esto lo caza.
/// El día que difieran, o C++ dejó de heredar o alguien los combinó.
#[test]
fn los_bytes_son_identicos_a_los_de_bmo_c() {
    for fuente in ["int main() { return 42; }", "int main() { return 0; }"] {
        let de_cpp = compile_source_to_bef(fuente).expect("C++ debe compilarlo");
        let de_c = bmo_c_front::compile_source_to_bef(fuente).expect("C debe compilarlo");
        assert_eq!(
            de_cpp, de_c,
            "el BEF de C++ y el de C tienen que ser identicos para {fuente:?}",
        );
    }
}

/// Un fichero vacío tiene que dar un error, no desbordar la pila.
///
/// Es literalmente la entrada con la que el frontend anterior moría: 12,12 MB
/// de `IrModule` construidos en la pila antes de mirar el AST.
#[test]
fn el_fichero_vacio_no_desborda_la_pila() {
    let e = compile_source_to_bef("").expect_err("sin `main` no hay programa");
    assert!(e.message.contains("main"), "el error tiene que decir que falta `main`: {}", e.message);
}

/// Un error de sintaxis tiene que llevar **la línea de verdad**.
///
/// Es lo que el frontend anterior no podía dar: sin lexer no hay token con
/// línea, y el parser contaba saltos a mano mientras adivinaba.
#[test]
fn los_errores_llevan_la_linea_real() {
    let e = compile_source_to_bef("int main() {\n  int x = 1;\n  int y = ;\n  return 0;\n}")
        .expect_err("`int y = ;` no es una expresion");
    assert_eq!(e.line, 3, "la linea tiene que ser la 3: {e:?}");
}

// ── La regla del descenso: rechazar DICIENDO en qué paso llega ──────

/// Lo que todavía no baja se rechaza **con el paso escrito**. Nunca en
/// silencio — ése era el pecado del `parse_body` anterior, que hacía
/// `pos += 1` con lo que no reconocía y dejaba desaparecer cuerpos enteros.
#[test]
fn lo_que_falta_se_rechaza_diciendo_el_paso() {
    let casos: &[(&str, &str)] = &[
        ("#include \"algo.h\"\nint main(){ return 0; }", "PASO 1"),
        ("namespace n { }\nint main(){ return 0; }", "PASO 4"),
        ("template<class T> T f(T x) { return x; }\nint main(){ return 0; }", "PASO 6"),
    ];
    for (fuente, paso) in casos {
        let e = compile_source_to_bef(fuente)
            .expect_err(&format!("esto no puede compilar todavia: {fuente:?}"));
        assert!(
            e.message.contains(paso),
            "el rechazo tiene que decir en que paso llega.\n  fuente: {fuente:?}\n  dijo:   {}",
            e.message,
        );
    }
}
