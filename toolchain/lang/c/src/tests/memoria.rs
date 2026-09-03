//! **`malloc` sobre `KIND_MEMORIA`** -- la capability que un programa PIDE.
//!
//! Hasta que existio esto, un `malloc` en un test devolvia **0 sin decirlo**:
//! el emulador no modelaba `TASK_OP_MEMORIA_PEDIR`, caia en el `_ => {}` del
//! despacho y salia por el epilogo de exito con el valor a cero. O sea que
//! contestaba "toma tu bloque" y entregaba el puntero nulo -- y ningun test lo
//! notaba porque ninguno pedia memoria.
//!
//! Lo que se prueba aqui es lo que **el programa** puede notar: que hay
//! bloque, que las bases avanzan, que el tope de cuatro peticiones se cumple y
//! que la quinta devuelve 0. Lo que NO se puede probar aqui es la fisica --que
//! los marcos sean contiguos, que dos paginas no sean la misma-- porque la
//! memoria del emulador es un mapa disperso donde toda direccion funciona.
//! Eso lo prueba `examples/memoria_C.c` **en el Ryzen**, y en ningun otro
//! sitio.

use super::*;

/// El primer bloque cae en `MEMORIA_VA_BASE` y no en otro sitio.
///
/// Es un numero del contrato, no un detalle: `vmm::MEMORIA_VA_BASE` esta
/// elegido para que quepan las cuatro peticiones del tope sin acercarse al
/// framebuffer. Si alguien lo mueve, este test lo dice.
#[test]
fn el_primer_bloque_cae_en_la_base_declarada() {
    let out = run_c(
        "int main() { char *p = malloc(1024); printf(\"%x\\n\", p); return 0; }",
    );
    assert_eq!(out, "e0000000\n");
}

/// Se escribe y se relee. Un puntero no nulo solo prueba que el kernel
/// contesto; que la memoria EXISTA lo prueba leer lo que se escribio.
#[test]
fn el_bloque_se_escribe_y_se_relee() {
    let out = run_c(
        "int main() {\n\
         char *p = malloc(1024);\n\
         int i;\n\
         int malos = 0;\n\
         for (i = 0; i < 1024; i = i + 1) { p[i] = (i * 7) % 127; }\n\
         for (i = 0; i < 1024; i = i + 1) { if (p[i] != (i * 7) % 127) { malos = malos + 1; } }\n\
         printf(\"malos=%d\\n\", malos);\n\
         return 0; }",
    );
    assert_eq!(out, "malos=0\n");
}

/// Dos peticiones son dos rangos, y el segundo va POR ENCIMA del primero.
///
/// El kernel redondea a paginas hacia arriba, asi que pedir 1024 gasta 4096 y
/// el bloque siguiente empieza detras. Si el redondeo fuera hacia abajo los dos
/// bloques se solaparian, y ese es un fallo que no duele hasta que alguien
/// escribe en los dos.
#[test]
fn dos_peticiones_no_se_pisan() {
    let out = run_c(
        "int main() {\n\
         char *a = malloc(1024);\n\
         char *b = malloc(1024);\n\
         printf(\"%x %x %d\\n\", a, b, b - a);\n\
         return 0; }",
    );
    assert_eq!(out, "e0000000 e0001000 4096\n");
}

/// **El tope se cumple: la quinta peticion devuelve 0.**
///
/// No hay forma de devolver memoria, asi que el numero de peticiones ES el
/// numero de fugas posibles. Que la quinta falle no es una limitacion
/// incomoda: es lo que hace que un programa que pide en un bucle se rompa
/// pronto en vez de comerse la RAM en silencio.
#[test]
fn la_quinta_peticion_devuelve_cero() {
    let out = run_c(
        "int main() {\n\
         printf(\"%d\", malloc(4096) != 0);\n\
         printf(\"%d\", malloc(4096) != 0);\n\
         printf(\"%d\", malloc(4096) != 0);\n\
         printf(\"%d\", malloc(4096) != 0);\n\
         printf(\"%d\\n\", malloc(4096) != 0);\n\
         return 0; }",
    );
    assert_eq!(out, "11110\n");
}

/// Pedir mas del tope por peticion se rechaza, y **sin gastar peticion**.
///
/// El kernel comprueba el tamano ANTES de tocar el contador. Si lo hiciera al
/// reves, cuatro peticiones absurdas dejarian al programa sin poder pedir la
/// que si cabia.
#[test]
fn pasarse_del_tope_por_peticion_no_gasta_peticion() {
    let out = run_c(
        "int main() {\n\
         char *malo = malloc(100000000);\n\
         char *bueno = malloc(4096);\n\
         printf(\"%d %x\\n\", malo == 0, bueno);\n\
         return 0; }",
    );
    assert_eq!(out, "1 e0000000\n");
}

/// `free` no devuelve nada al kernel -- y eso se DICE, no se finge.
///
/// Lo que si tiene que hacer es evaluar su argumento, por si lleva efectos
/// secundarios, y no cruzar la puerta: una llamada al kernel que no hace nada
/// es peor que ninguna.
#[test]
fn free_no_cruza_la_puerta() {
    let m = run_c_maquina(
        "int main() { char *p = malloc(1024); free(p); return 0; }",
    );
    use bmo_abi::syscalls::surface::TASK_OP_MEMORIA_PEDIR;
    let peticiones = m
        .syscalls
        .iter()
        .filter(|s| s.operation == TASK_OP_MEMORIA_PEDIR)
        .count();
    assert_eq!(peticiones, 1, "un malloc y un free son UNA peticion");
}

/// El ejemplo del repositorio, ejecutado entero.
///
/// Es el `.bex` que se va a lanzar en el Ryzen (`c/memc.bex`), asi que esta
/// salida es **la que hay que ver en la pantalla**. Si cambia aqui y no alli,
/// lo desplegado no corresponde a esta fuente.
#[test]
fn el_ejemplo_de_memoria_pasa_sus_cuatro_pruebas() {
    let m = run_c_maquina(include_str!("../../examples/memoria_C.c"));
    let esperado = [
        "KIND_MEMORIA - la primera vez que un programa PIDE",
        "malloc(1024) = 0xe0000000",
        "1024 bytes verificados, 0 malos",
        "malloc(65536) = 0xe0001000",
        "16 paginas verificadas, 0 malas",
        "ultimo byte del bloque = 42",
        "peticion 3 = 0xe0011000   peticion 4 = 0xe0012000",
        "la 5a peticion devolvio 0: el tope se cumple",
        "MEMORIA: las cuatro pruebas pasan",
    ]
    .map(|l| format!("{l}\n"))
    .concat();
    assert_eq!(m.console, esperado);

    // Y lo que el programa NO puede contarse a si mismo: lo que el kernel dice
    // que entrego. 4096 + 65536 + 4096 + 4096 -- el primer bloque son 1024
    // bytes pedidos y una pagina entregada.
    assert_eq!(m.memoria_entregada(), 4096 + 65536 + 4096 + 4096);
}

/// **Compilar C para Ring 0 se RECHAZA, y con su motivo.**
///
/// Emitia `syscall; ret` en linea, y ese `ret` retornaba de la funcion entera
/// en cuanto volvia el syscall -- el stub es un *llamable* y ponerlo en linea se
/// come el `call` y deja el `ret`. No lo cazo nadie porque nada construye este
/// perfil; un camino muerto que emite bytes incorrectos es peor que uno que no
/// existe.
#[test]
fn compilar_para_ring0_se_rechaza_diciendo_por_que() {
    use crate::codegen::{compile_with_target, TargetProfile};
    let p = parse("int main() { char *q = malloc(64); return 0; }").unwrap();
    let r = compile_with_target(&p, TargetProfile::Ring0Kernel);
    let e = r.expect_err("Ring 0 no se compila");
    let texto = alloc_fmt(&e);
    assert!(texto.contains("Ring 0"), "el motivo tiene que nombrar Ring 0: {texto}");
    assert!(
        texto.contains("llama") || texto.contains("LLAMADA"),
        "y decir que la salida es la llamada directa: {texto}"
    );
}

fn alloc_fmt(e: &CError) -> String {
    format!("{e:?}")
}

// == COMA FLOTANTE, EJECUTADA ==========================================
//
// De los 9 tests de `float`/`double` que ya existian, **ninguno ejecutaba**:
// los nueve comparaban ventanas de bytes (`bef.windows(3).any(...)`), que es
// exactamente el metodo que la cabecera de `bmo_lower::emu` declara
// insuficiente -- "si el autor entendio mal una codificacion, el test la repite
// y pasa igual de mal".
//
// Estos corren. Es la primera vez que la ruta SSE de BMO C se ejecuta en algun
// sitio.

/// Suma y resta de doubles, impresas como entero para no depender de `%f`
/// (que todavia no se compila).
#[test]
fn los_doubles_suman_y_restan() {
    let out = run_c(
        "int main() { double a; double b; a = 2.5; b = 1.25; \
         printf(\"%d %d\n\", (int)(a + b), (int)(a - b)); return 0; }",
    );
    assert_eq!(out, "3 1\n");
}

/// **El orden importa en las NO conmutativas.**
///
/// Es el mismo fallo que el banco ya cazo una vez en los enteros: se emitian
/// sobre `b - a`. Con `+` y `*` no se nota; con `-` y `/`, si.
#[test]
fn las_no_conmutativas_respetan_el_orden() {
    let out = run_c(
        "int main() { double a; double b; a = 10.0; b = 4.0; \
         printf(\"%d %d\n\", (int)(a - b), (int)(a / b)); return 0; }",
    );
    assert_eq!(out, "6 2\n");
}

/// `cvtsi2sd` es **con signo**: -7 tiene que dar -7.0, no 1.8e19.
#[test]
fn el_entero_negativo_a_double_conserva_el_signo() {
    let out = run_c(
        "int main() { int n; double d; n = 0 - 7; d = n; \
         printf(\"%d\n\", (int)(d * 2.0)); return 0; }",
    );
    assert_eq!(out, "-14\n");
}

/// `cvttsd2si` **trunca hacia cero**, no redondea. `(int)2.9` son 2.
#[test]
fn el_cast_a_entero_trunca_no_redondea() {
    let out = run_c(
        "int main() { printf(\"%d %d\n\", (int)2.9, (int)(0.0 - 2.9)); return 0; }",
    );
    assert_eq!(out, "2 -2\n");
}

/// `comisd` deja el resultado en ZF/CF, y los saltos que le siguen son los
/// SIN signo. Si el emulador lo modelara con SF, esto saltaria al reves.
#[test]
fn las_comparaciones_de_double_deciden_bien() {
    let out = run_c(
        "int main() { double a; double b; a = 1.5; b = 2.5;\n\
         if (a < b) { printf(\"menor\n\"); } else { printf(\"MAL\n\"); }\n\
         if (b > a) { printf(\"mayor\n\"); } else { printf(\"MAL\n\"); }\n\
         if (a == a) { printf(\"igual\n\"); } else { printf(\"MAL\n\"); }\n\
         return 0; }",
    );
    assert_eq!(out, "menor\nmayor\nigual\n");
}

/// Un `float` guarda MENOS precision que un `double`, y tiene que perderla.
/// Si `cvtsd2ss` no recortara, el test veria mas digitos que el silicio.
#[test]
fn un_float_pierde_precision_y_eso_se_ve() {
    let out = run_c(
        "int main() { float f; double d; d = 1.0 / 3.0; f = d;\n\
         printf(\"%d\n\", (int)((d - f) != 0.0)); return 0; }",
    );
    assert_eq!(out, "1\n", "guardar en float y volver NO puede dar el mismo numero");
}

/// El cero de la coma flotante se hace con `xorpd`, y tiene que ser cero.
#[test]
fn un_double_sin_inicializar_vale_cero() {
    let out = run_c(
        "int main() { double d; printf(\"%d\n\", (int)(d + 5.0)); return 0; }",
    );
    assert_eq!(out, "5\n");
}

/// *** `*p = x` SOBRE UN `char *` ESCRIBE UN BYTE, NO OCHO.
///
/// # De donde sale: los 4 bytes que DOOM pisaba tras su pantalla
///
/// El 2026-09-03 el Ryzen enseno esto, con DOOM ya jugandose:
///
/// ```text
///    I_VideoBuffer  en +1825056,  64000 bytes,  ACABA en +1889056
///    BLOQUE 1336    en +1889056   <- los 4 bytes valen 0
///    EL CANARIO CAZO EN: R_RenderPlayerView
/// ```
///
/// La suma es exacta --1825056 + 64000 = 1889056-- asi que lo pisado es lo que
/// hay JUSTO detras del buffer de pantalla. Y `r_draw.c` escribe sus pixeles
/// con `*dest = dc_colormap[...]`, o sea `Expr::AssignDeref` sobre un `byte *`.
///
/// ** El codegen emitia `mov [rax], rdx` --OCHO bytes-- sin preguntar a que
/// apunta. Escribir el ULTIMO pixel se lleva siete bytes por delante del final.
///
/// [!] Y explica por que la pantalla se VEIA bien: DOOM dibuja las columnas de
/// izquierda a derecha, asi que los siete bytes que cada escritura se lleva los
/// vuelve a escribir la columna siguiente. **Solo sobrevive el desperdicio de la
/// ultima**, que es justo la que cae fuera. Un fallo que se repara solo en el
/// 99,7% de los casos es de los que duran meses.
#[test]
fn guardar_por_un_puntero_a_char_escribe_un_solo_byte() {
    let salida = run_c(
        "int main() {
             char buf[8]; char *p;
             buf[0] = 9; buf[1] = 7; buf[2] = 5; buf[3] = 3;
             p = buf;
             *p = 1;
             printf(\"%d %d %d %d\", buf[0], buf[1], buf[2], buf[3]);
             return 0;
         }",
    );
    assert_eq!(salida, "1 7 5 3", "`*p = 1` sobre un char* no puede tocar a los vecinos");
}

/// Gemela: un `short *` escribe DOS, y un `int *` CUATRO.
#[test]
fn guardar_por_un_puntero_respeta_el_ancho_de_lo_apuntado() {
    let salida = run_c(
        "int main() {
             int cuatro[2]; int *pi;
             short dos[4]; short *ps;
             cuatro[0] = 0; cuatro[1] = 123;
             pi = cuatro; *pi = 7;
             dos[0] = 0; dos[1] = 456;
             ps = dos; *ps = 8;
             printf(\"%d %d %d %d\", cuatro[0], cuatro[1], dos[0], dos[1]);
             return 0;
         }",
    );
    assert_eq!(salida, "7 123 8 456", "el vecino de al lado no se toca");
}
