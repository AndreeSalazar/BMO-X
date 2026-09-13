//! **LOS REGISTROS ALTOS COMO DESTINO: `r8`, `r9`, `r10`.**
//!
//! Hasta el 2026-09-12, `mov_de_marco` y `mov_a_marco` escribian el ModRM como
//! `0x85 | (reg << 3)` y el prefijo como `0x48` fijo. Con un registro de numero
//! 8 o mas, el ModRM se desbordaba a `mod=11`, la instruccion media tres bytes
//! en vez de siete, y los cuatro del desplazamiento SE EJECUTABAN.
//!
//! ** Y nadie lo vio en un mes porque hacen falta dos cosas a la vez: que el
//! destino sea un registro alto --el cuarto argumento de la puerta, el quinto o
//! sexto de una llamada-- Y que lo que se carga sea una VARIABLE. Con una
//! constante se usa otro codificador que si ponia el REX. `cpu.inti` y
//! `pulso.inti` corrieron en el Ryzen pasando `0, 0`. Lo destapo `bico.inti`,
//! el primer programa que paso una variable ahi.
//!
//! Estas pruebas son los cuatro programas minimos con los que se aislo. Cada
//! uno da un numero conocido y TERMINA: antes, tres de los cuatro morian con
//! `opcode 0xF8/0xE0/0xD8 no emitido por BMO`.

use bmo_lower::emu::{run, Machine};

fn emitido(texto: &str) -> bmo_inti_x86_64::Emitido {
    let arbol = bmo_inti_front::armar(texto);
    assert!(!arbol.hay_errores(), "no se lee: {}", arbol.pintar("prueba.inti"));
    let raices = bmo_mods::Roots::find();
    let modulos = bmo_inti_front::tablas::Modulos::cargar(&raices);
    let plano = bmo_inti_front::disposicion::comprobar(
        &arbol.valor,
        bmo_inti_front::disposicion::Medidas::cargar(&raices),
    );
    let metal = bmo_inti_front::ir::metal_que_declara(&arbol.valor, &raices, &modulos);
    let nec = bmo_inti_front::necesidades::Necesidades::por_defecto();
    let ir = bmo_inti_front::ir::bajar_con(&arbol.valor, &modulos, &plano.valor, &metal, &nec).valor;
    bmo_inti_x86_64::emitir(&ir)
}

fn corre(f: &str, archivo: Option<&[u8]>) -> Machine {
    let e = emitido(f);
    let mut m = Machine::new(e.codigo);
    if let Some(d) = archivo {
        m.poner_archivo("datos/foto.bmp", d);
    }
    run(m, 5_000_000)
}

/// ** VARIABLES en el cuarto y quinto argumento de la puerta (`r10`, `r8`).
#[test]
fn variables_en_el_cuarto_y_quinto_argumento_de_la_puerta() {
    let f = "perfil llano\nusa bmo\n\nfuncion principal devuelve entero32\n    cambiante a es natural64 = 5\n    cambiante b es natural64 = 7\n    invoca(mi_tarea, 0x06, 72, a, b - 1)\n    devuelve 0\n";
    let m = corre(f, None);
    assert!(m.exited, "el programa tiene que terminar");
}

/// ** SEIS parametros: el quinto y el sexto llegan en `r8` y `r9`, y el prologo
/// los guarda en el marco con `mov_a_marco`.
#[test]
fn una_funcion_de_seis_parametros_recibe_los_seis() {
    let f = "perfil llano\n\nfuncion seis(a es natural64, b es natural64, c es natural64, d es natural64, e es natural64, g es natural64) devuelve natural64\n    devuelve a + b * 2 + c * 3 + d * 4 + e * 5 + g * 6\n\nfuncion principal devuelve entero32\n    devuelve seis(1, 1, 1, 1, 1, 1)\n";
    let m = corre(f, None);
    assert!(m.exited);
    assert_eq!(m.syscalls.last().unwrap().arg0, 21);
}

/// Y el quinto y sexto NO se confunden: si fueran al registro equivocado, el
/// peso distinto de cada uno cambiaria la suma.
#[test]
fn el_quinto_y_el_sexto_no_se_cruzan() {
    let f = "perfil llano\n\nfuncion seis(a es natural64, b es natural64, c es natural64, d es natural64, e es natural64, g es natural64) devuelve natural64\n    devuelve e * 10 + g\n\nfuncion principal devuelve entero32\n    cambiante x es natural64 = 3\n    cambiante y es natural64 = 4\n    devuelve seis(0, 0, 0, 0, x, y)\n";
    let m = corre(f, None);
    assert!(m.exited);
    assert_eq!(m.syscalls.last().unwrap().arg0, 34);
}

/// ** El caso real: leer un fichero al bloque con `op_arch_leer_en`, donde el
/// desplazamiento (cuarto) y la medida (quinto) son variables.
#[test]
fn leer_un_fichero_al_bloque_con_variables() {
    let f = "perfil llano\nusa bmo\nusa memoria\n\nfuncion principal devuelve entero32\n    bloque es natural64 = invoca_valor(mi_tarea, op_pedir_memoria, 1048576, 0, 0)\n    invoca(mi_tarea, op_ruta, 8027155558672130404, 0, 0)\n    invoca(mi_tarea, op_ruta, 123615100956532, 0, 0)\n    f es natural64 = invoca_valor(mi_tarea, op_archivo_abrir, 0, 0, 0)\n    medida es natural64 = invoca_valor(f, op_arch_tamano, 0, 0, 0)\n    cambiante hechos es natural64 = 0\n    leidos es natural64 = invoca_valor(f, op_arch_leer_en, bloque, hechos, medida - hechos)\n    devuelve leidos\n";
    let m = corre(f, Some(b"XMabcdefghij"));
    assert!(m.exited);
    assert_eq!(m.syscalls.last().unwrap().arg0, 12, "los doce bytes del fichero");
}
