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

/// *** COPIAR UN STRUCT DE UN PUNTERO A OTRO: `*a = *b`.
///
/// Es `cliprange_t` de `r_bsp.c:122` -- `*next = *(next-1)` --, el desplazamiento
/// de la lista de recorte del BSP de DOOM. Dos `int`, ocho bytes.
#[test]
fn copiar_un_struct_de_un_puntero_a_otro() {
    let salida = run_c(
        "struct par { int a; int b; };
         int main() {
             struct par v[2]; struct par *p;
             v[0].a = 11; v[0].b = 22;
             v[1].a = 0;  v[1].b = 0;
             p = v;
             *(p + 1) = *p;
             printf(\"%d %d\", v[1].a, v[1].b);
             return 0;
         }",
    );
    assert_eq!(salida, "11 22", "la copia tiene que llevar los DOS campos");
}

/// ** Y uno de DOCE bytes, que es el que estaba roto DESDE ANTES.
///
/// El brazo viejo de `Deref` cargaba ocho bytes a pelo, asi que un
/// `cliprange_t` (dos `int`) acertaba por casualidad. Uno de tres campos se
/// habria copiado a medias y nadie lo habria visto: los dos primeros bien y el
/// tercero con lo que hubiera.
#[test]
fn copiar_un_struct_de_mas_de_ocho_bytes_por_puntero() {
    let salida = run_c(
        "struct tres { int a; int b; int c; };
         int main() {
             struct tres v[2]; struct tres *p;
             v[0].a = 11; v[0].b = 22; v[0].c = 33;
             v[1].a = 0;  v[1].b = 0;  v[1].c = 0;
             p = v;
             *(p + 1) = *p;
             printf(\"%d %d %d\", v[1].a, v[1].b, v[1].c);
             return 0;
         }",
    );
    assert_eq!(salida, "11 22 33", "los TRES campos, no los dos primeros");
}

// == TANDA DE SONDA 2026-09-03: las formas que usa `r_bsp.c` ================
//
// Se escriben JUNTAS y a proposito. Cada una es una forma de C que el port de
// DOOM usa y que el banco no ejercia; lo que se busca no es que pasen, es
// SABER CUALES NO.

#[test] fn s1_struct_a_funcion_por_valor_desde_puntero() {
    assert_eq!(run_c("struct par { int a; int b; };
int suma(struct par p) { return p.a + p.b; }
int main(){ struct par v; struct par *q; v.a=11; v.b=22; q=&v;
 printf(\"%d\", suma(*q)); return 0; }"), "33");
}

#[test] fn s2_campo_de_struct_dentro_de_struct_por_puntero() {
    assert_eq!(run_c("struct in { int x; int y; };
struct out { int pad; struct in d; };
int main(){ struct out o; struct out *p; p=&o;
 p->d.x = 7; p->d.y = 9; o.pad = 5;
 printf(\"%d %d %d\", o.pad, p->d.x, p->d.y); return 0; }"), "5 7 9");
}

#[test] fn s3_puntero_a_struct_con_postincremento() {
    assert_eq!(run_c("struct par { int a; int b; };
int main(){ struct par v[3]; struct par *p; int i;
 for(i=0;i<3;i=i+1){ v[i].a=i; v[i].b=i*10; }
 p=v; p++;
 printf(\"%d %d\", p->a, p->b); return 0; }"), "1 10");
}

#[test] fn s4_comparar_punteros_a_struct() {
    assert_eq!(run_c("struct par { int a; int b; };
struct par v[3];
int main(){ struct par *p; struct par *q; p=v; q=v+2;
 printf(\"%d %d %d\", (int)(p<q), (int)(q<p), (int)(q-p)); return 0; }"), "1 0 2");
}

#[test] fn s5_while_con_flecha_sobre_puntero_avanzando() {
    assert_eq!(run_c("struct par { int a; int b; };
int main(){ struct par v[4]; struct par *p; int n;
 v[0].a=1; v[1].a=1; v[2].a=1; v[3].a=0;
 p=v; n=0;
 while (p->a) { n=n+1; p++; }
 printf(\"%d\", n); return 0; }"), "3");
}

#[test] fn s6_asignar_campo_a_traves_de_puntero_calculado() {
    assert_eq!(run_c("struct par { int a; int b; };
int main(){ struct par v[3]; struct par *tope;
 v[2].a=0; v[2].b=0; tope=v+3;
 (tope-1)->a = 41; (tope-1)->b = 42;
 printf(\"%d %d\", v[2].a, v[2].b); return 0; }"), "41 42");
}

/// [!] LIMITE DECLARADO, no fallo silencioso: un PROTOTIPO que devuelve
/// `struct X *` no parsea -- *"expected type, got Ident"*. Con `typedef` si.
///
/// ** Se fija como casilla porque lo importante es que **rechaza diciendolo**,
/// que es la diferencia entre un hueco y un bug. Y no bloquea a DOOM: sus
/// cabeceras declaran `side_t *getSide(...)`, o sea por alias.
#[test] fn s7_prototipo_que_devuelve_struct_crudo_se_rechaza_diciendolo() {
    let e = crate::compile_source_to_bef(
        "struct par { int a; int b; };
         struct par *dame(int i);
         int main() { return 0; }",
    ).expect_err("hoy no se parsea");
    assert!(e.message.contains("expected type"), "tiene que decir QUE no entiende: {}", e.message);
}

/// Y la misma forma CON alias, que es la que usa DOOM: compila.
#[test] fn s7b_con_typedef_el_prototipo_vale() {
    assert_eq!(run_c("struct par { int a; int b; };
typedef struct par par_t;
par_t v[2];
par_t *dame(int i);
int main(){ v[1].a=8; v[1].b=9; printf(\"%d %d\", dame(1)->a, dame(1)->b); return 0; }
par_t *dame(int i){ return &v[i]; }"), "8 9");
}

/// [!] LIMITE DECLARADO: un `double` GLOBAL no esta soportado, y el compilador
/// lo dice con esas palabras --*"usa locales"*--. Tampoco bloquea a DOOM: su
/// render es de enteros de punto fijo, sin un solo flotante.
#[test] fn s8_un_double_global_se_rechaza_diciendolo() {
    // [!] DECLARARLO compila; lo que se rechaza es USARLO. La primera version
    // de esta casilla solo lo declaraba y pasaba en verde sin comprobar nada --
    // una casilla que no ejerce lo que dice su nombre.
    let e = crate::compile_source_to_bef(
        "double d = 0;
         int main() { d = 2.5; printf(\"%d\", (int)(d * 2)); return 0; }",
    ).expect_err("hoy no se soporta");
    assert!(e.message.contains("aun no soportada"), "tiene que decirlo: {}", e.message);
}

// == SONDA `R_ClearClipSegs`: la lista de recorte del BSP, literal ==========

#[test] fn r1_constantes_negativas_en_campos_de_un_array_de_structs() {
    assert_eq!(run_c("struct cr { int first; int last; };
struct cr seg[4];
int w = 320;
int main(){
  seg[0].first = -0x7fffffff; seg[0].last = -1;
  seg[1].first = w;           seg[1].last = 0x7fffffff;
  printf(\"%d %d %d %d\", seg[0].first, seg[0].last, seg[1].first, seg[1].last);
  return 0; }"), "-2147483647 -1 320 2147483647");
}

#[test] fn r2_leer_por_puntero_mas_uno() {
    assert_eq!(run_c("struct cr { int first; int last; };
struct cr seg[4];
int main(){
  struct cr *p;
  seg[0].first = 10; seg[0].last = 11;
  seg[1].first = 20; seg[1].last = 21;
  p = seg;
  printf(\"%d %d %d %d\", p->first, p->last, (p+1)->first, (p+1)->last);
  return 0; }"), "10 11 20 21");
}

/// `newend = solidsegs+2; newend++;` -- el puntero global que lleva la cabeza
/// de la lista de recorte.
///
/// [!] Va con `typedef` PORQUE ASI LO ESCRIBE DOOM (`cliprange_t* newend;`).
/// La forma cruda --`struct cr *fin;` a nivel de fichero-- no parsea hoy, y eso
/// tiene su propia casilla debajo: es un hueco declarado, no un fallo mudo.
#[test] fn r3_puntero_global_a_struct_que_avanza() {
    assert_eq!(run_c("struct cr { int first; int last; };
typedef struct cr cr_t;
cr_t seg[8];
cr_t *fin;
int main(){
  fin = seg + 2;
  fin->first = 7; fin->last = 8;
  fin++;
  fin->first = 9;
  printf(\"%d %d %d %d\", seg[2].first, seg[2].last, seg[3].first, (int)(fin - seg));
  return 0; }"), "7 8 9 3");
}

/// [!] LIMITE DECLARADO, gemelo del de `s7`: un puntero GLOBAL a `struct X`
/// sin alias no parsea. Se fija para que siga siendo un "no" y no se convierta
/// un dia en un cero callado.
#[test] fn r3b_puntero_global_a_struct_crudo_se_rechaza_diciendolo() {
    let e = crate::compile_source_to_bef(
        "struct cr { int first; int last; };
         struct cr *fin;
         int main() { return 0; }",
    ).expect_err("hoy no se parsea");
    assert!(e.message.contains("expected type"), "tiene que decir QUE no entiende: {}", e.message);
}

#[test] fn r4_el_bucle_de_busqueda_del_recorte() {
    assert_eq!(run_c("struct cr { int first; int last; };
struct cr seg[4];
int main(){
  struct cr *start; int first;
  seg[0].first = -2147483647; seg[0].last = -1;
  seg[1].first = 320;         seg[1].last = 2147483647;
  first = 7;
  start = seg;
  while (start->last < first-1) start++;
  printf(\"%d %d\", (int)(start - seg), start->first);
  return 0; }"), "1 320");
}

/// *** `R_ClipSolidWallSegment` DE DOOM, PORTADO TAL CUAL.
///
/// # Por que existe
///
/// El metal contesto `Bad R_RenderWallRange: 7 to -1` -- el `RANGECHECK` del
/// propio DOOM viendo `start > stop`. Antes decia `28 to 27`, o sea que el
/// arreglo de la copia de struct **cambio el sintoma sin cerrarlo**.
///
/// ** Esta casilla parte la duda en dos, y esa es toda su gracia:
///
/// ```text
///    si produce un rango invertido AQUI   -> el fallo es del compilador
///    si no lo produce                     -> el fallo esta en lo que le ENTRA
/// ```
///
/// [!] No compara contra un compilador de referencia --no hay ninguno a mano--
/// sino contra una PROPIEDAD: un rango guardado nunca puede tener el principio
/// despues del final. Eso no depende de quien compile.
#[test]
fn el_recorte_del_bsp_de_doom_no_produce_rangos_invertidos() {
    let salida = run_c(
        "/* R_ClipSolidWallSegment de `r_bsp.c`, portado tal cual.
 *
 * El unico cambio es que `R_StoreWallRange` no dibuja: CUENTA los rangos
 * invertidos, que es lo que el RANGECHECK de DOOM detecta en el metal.
 */

struct cliprange { int first; int last; };
typedef struct cliprange cliprange_t;

cliprange_t solidsegs[40];
cliprange_t *newend;
int viewwidth = 320;

int malos = 0;
int guardados = 0;
int ult_a = 0;
int ult_b = 0;

void R_StoreWallRange(int start, int stop)
{
    guardados = guardados + 1;
    if (start > stop) {
        malos = malos + 1;
        ult_a = start;
        ult_b = stop;
    }
}

void R_ClearClipSegs(void)
{
    solidsegs[0].first = -0x7fffffff;
    solidsegs[0].last = -1;
    solidsegs[1].first = viewwidth;
    solidsegs[1].last = 0x7fffffff;
    newend = solidsegs + 2;
}

void R_ClipSolidWallSegment(int first, int last)
{
    cliprange_t *next;
    cliprange_t *start;

    start = solidsegs;
    while (start->last < first - 1)
        start++;

    if (first < start->first) {
        if (last < start->first - 1) {
            R_StoreWallRange(first, last);
            next = newend;
            newend++;
            while (next != start) {
                *next = *(next - 1);
                next--;
            }
            next->first = first;
            next->last = last;
            return;
        }
        R_StoreWallRange(first, start->first - 1);
        start->first = first;
    }

    if (last <= start->last)
        return;

    next = start;
    while (last >= (next + 1)->first - 1) {
        R_StoreWallRange(next->last + 1, (next + 1)->first - 1);
        next++;
        if (last <= next->last) {
            start->last = next->last;
            goto crunch;
        }
    }

    R_StoreWallRange(next->last + 1, last);
    start->last = last;

crunch:
    if (next == start)
        return;
    while (next++ != newend)
        start++[1] = next[0];
    newend = start + 1;
}

int main(void)
{
    int i;
    R_ClearClipSegs();
    /* Unas cuantas paredes, en el orden en que las da un BSP. */
    R_ClipSolidWallSegment(100, 150);
    R_ClipSolidWallSegment(10, 40);
    R_ClipSolidWallSegment(200, 260);
    R_ClipSolidWallSegment(45, 90);
    R_ClipSolidWallSegment(160, 190);
    R_ClipSolidWallSegment(0, 300);

    printf(\"guardados %d  invertidos %d  ultimo %d a %d\\n\",
           guardados, malos, ult_a, ult_b);
    printf(\"lista (%d):\", (int)(newend - solidsegs));
    for (i = 0; i < (int)(newend - solidsegs); i++)
        printf(\" [%d,%d]\", solidsegs[i].first, solidsegs[i].last);
    printf(\"\\n\");
    return 0;
}
",
    );
    assert!(
        salida.contains("invertidos 0"),
        "el recorte produjo un rango invertido: {}",
        salida
    );
}

// == SONDA `R_AddLine`: la aritmetica de angulos, que es TODA sin signo =====
//
// `typedef unsigned angle_t`, y el recorte de la vista se apoya en que la resta
// ENVUELVE y en que las comparaciones son SIN SIGNO con el bit 31 puesto.

#[test] fn a1_comparar_sin_signo_con_el_bit_31_puesto() {
    assert_eq!(run_c("typedef unsigned angle_t;
int main(){ angle_t span; angle_t ANG180;
 ANG180 = 0x80000000; span = 0x80000000;
 printf(\"%d %d %d\", (int)(span >= ANG180), (int)(span > ANG180),
        (int)(0x7FFFFFFF >= ANG180));
 return 0; }"), "1 0 0");
}

#[test] fn a2_la_resta_de_angulos_ENVUELVE() {
    assert_eq!(run_c("typedef unsigned angle_t;
int main(){ angle_t a; angle_t b; angle_t span;
 a = 10; b = 20; span = a - b;
 printf(\"%u %d\", span, (int)(span >= 0x80000000));
 return 0; }"), "4294967286 1");
}

#[test] fn a3_negar_un_unsigned() {
    assert_eq!(run_c("typedef unsigned angle_t;
int main(){ angle_t clip; angle_t neg;
 clip = 0x20000000; neg = -clip;
 printf(\"%u\", neg); return 0; }"), "3758096384");
}

#[test] fn a4_dos_por_un_angulo_que_desborda() {
    assert_eq!(run_c("typedef unsigned angle_t;
int main(){ angle_t clip; angle_t dos;
 clip = 0x60000000; dos = 2*clip;
 printf(\"%u %d\", dos, (int)(dos > clip));
 return 0; }"), "3221225472 1");
}

/// *** EL RECORTE DE LA VISTA ENTERO, y la casilla que me cazo a MI.
///
/// La primera version esperaba "1 1 0" y BMO C dio "1 0 1". Antes de tocar el
/// compilador se hizo la cuenta a mano, y **la equivocada era la expectativa**:
///
/// ```text
///    caso 2   tspan = 0x90000000 + 0x20000000 = 0xB0000000 > 0x40000000
///             tspan -= 0x40000000  ->  0x70000000 >= span (0x01000000)
///             -> return 0                    <- BMO C acierta
///    caso 3   span = 0 - 0xF0000000 = 0x10000000 (envuelve)
///             ningun tspan pasa del tope
///             -> return 1                    <- BMO C acierta
/// ```
///
/// [!] Se deja escrito porque es la tercera vez en dos dias que una teoria
/// razonable resulta falsa al medirla. **Una casilla roja no dice quien se
/// equivoco; solo dice que dos cuentas no coinciden.**
#[test] fn a5_el_recorte_de_la_vista_entero() {
    assert_eq!(run_c("typedef unsigned angle_t;
angle_t clipangle;
int recorta(angle_t a1, angle_t a2);
int main(){ clipangle = 0x20000000;
 printf(\"%d %d %d\", recorta(0x10000000, 0x0F000000),
        recorta(0x90000000, 0x8F000000), recorta(0x00000000, 0xF0000000));
 return 0; }
int recorta(angle_t angle1, angle_t angle2){
 angle_t span; angle_t tspan;
 span = angle1 - angle2;
 if (span >= 0x80000000) return 0;
 tspan = angle1 + clipangle;
 if (tspan > 2*clipangle) {
   tspan = tspan - 2*clipangle;
   if (tspan >= span) return 0;
   angle1 = clipangle;
 }
 tspan = clipangle - angle2;
 if (tspan > 2*clipangle) {
   tspan = tspan - 2*clipangle;
   if (tspan >= span) return 0;
   angle2 = -clipangle;
 }
 return 1; }"), "1 0 1");
}

// == SONDA COMA FIJA: `FixedMul` y `FixedDiv`, el suelo de TODO el render ====
//
// `R_InitTextureMapping` construye `viewangletox` con estas dos, y
// `finetangent` es NEGATIVO en media circunferencia. Si el signo o el ancho de
// 64 bits se pierden, la tabla sale mal y `R_AddLine` devuelve columnas
// invertidas -- que es el `Bad R_RenderWallRange` del metal.

#[test] fn f1_fixedmul_con_intermedio_de_64_bits() {
    // 2.5 * 3.0 en 16.16 = 7.5   -> 0x00078000
    assert_eq!(run_c("typedef int fixed_t;
long long ancho(fixed_t a, fixed_t b);
int main(){ printf(\"%d\", (int)((ancho(0x28000, 0x30000)) >> 16)); return 0; }
long long ancho(fixed_t a, fixed_t b){ return ((long long)a) * ((long long)b); }"),
        "491520");
}

#[test] fn f2_fixedmul_con_UN_operando_negativo() {
    // -2.5 * 3.0 = -7.5  -> el desplazamiento tiene que ser ARITMETICO
    assert_eq!(run_c("typedef int fixed_t;
int fmul(fixed_t a, fixed_t b);
int main(){ printf(\"%d %d\", fmul(-0x28000, 0x30000), fmul(0x28000, -0x30000)); return 0; }
int fmul(fixed_t a, fixed_t b){ return (int)((((long long)a) * ((long long)b)) >> 16); }"),
        "-491520 -491520");
}

#[test] fn f3_desplazar_a_la_derecha_un_NEGATIVO() {
    assert_eq!(run_c("int main(){ int t; long long g;
 t = -100; g = -100;
 printf(\"%d %d\", t >> 4, (int)(g >> 4)); return 0; }"), "-7 -7");
}

#[test] fn f4_fixeddiv_desplazando_a_la_izquierda_en_64_bits() {
    // (2.0 << 16) / 4.0 en 16.16 = 0.5 -> 0x8000
    assert_eq!(run_c("typedef int fixed_t;
int fdiv(fixed_t a, fixed_t b);
int main(){ printf(\"%d %d\", fdiv(0x20000, 0x40000), fdiv(-0x20000, 0x40000)); return 0; }
int fdiv(fixed_t a, fixed_t b){ long long r; r = (((long long)a) << 16) / b; return (int)r; }"),
        "32768 -32768");
}

#[test] fn f5_la_linea_que_construye_viewangletox() {
    // t = (centerxfrac - t + FRACUNIT-1) >> FRACBITS, con t NEGATIVO
    assert_eq!(run_c("int main(){ int centerxfrac; int t;
 centerxfrac = 160 << 16; t = -(3 << 16);
 t = (centerxfrac - t + 65535) >> 16;
 printf(\"%d\", t); return 0; }"), "163");
}

#[test] fn f6_abs_y_el_signo_del_cociente() {
    assert_eq!(run_c("int mabs(int v);
int main(){ int a; int b;
 a = -5; b = 3;
 printf(\"%d %d %d\", mabs(a), mabs(b), (int)((a^b) < 0));
 return 0; }
int mabs(int v){ if (v < 0) return -v; return v; }"), "5 3 1");
}

/// *** UNA TABLA GLOBAL GRANDE CON NEGATIVOS, como `finetangent[4096]`.
///
/// # Por que es la ultima pieza sin sondear del camino de DOOM
///
/// `R_InitTextureMapping` construye `viewangletox` a partir de `finetangent`, y
/// esa tabla son **4.096 constantes de las que 770 lineas llevan negativos**,
/// empezando por `-170910304`. Ya estan exonerados el recorte del BSP, la
/// aritmetica de angulos y la coma fija; si la TABLA sale mal, todo lo de
/// encima esta bien y da igual.
///
/// [!] La tabla de esta casilla se genera aqui y no se lee de `tables.c`: una
/// prueba que depende de un fichero que vive fuera del repo --y bajo otra
/// licencia-- es una prueba que un dia no se puede correr.
#[test]
fn una_tabla_global_grande_con_negativos_se_lee_entera() {
    let salida = run_c(
        "const int t[1024] = {-170910304,170909973,-170909642,170909311,-170908980,170908649,-170908318,170907987,-170907656,170907325,-170906994,170906663,-170906332,170906001,-170905670,170905339,-170905008,170904677,-170904346,170904015,-170903684,170903353,-170903022,170902691,-170902360,170902029,-170901698,170901367,-170901036,170900705,-170900374,170900043,-170899712,170899381,-170899050,170898719,-170898388,170898057,-170897726,170897395,-170897064,170896733,-170896402,170896071,-170895740,170895409,-170895078,170894747,-170894416,170894085,-170893754,170893423,-170893092,170892761,-170892430,170892099,-170891768,170891437,-170891106,170890775,-170890444,170890113,-170889782,170889451,-170889120,170888789,-170888458,170888127,-170887796,170887465,-170887134,170886803,-170886472,170886141,-170885810,170885479,-170885148,170884817,-170884486,170884155,-170883824,170883493,-170883162,170882831,-170882500,170882169,-170881838,170881507,-170881176,170880845,-170880514,170880183,-170879852,170879521,-170879190,170878859,-170878528,170878197,-170877866,170877535,-170877204,170876873,-170876542,170876211,-170875880,170875549,-170875218,170874887,-170874556,170874225,-170873894,170873563,-170873232,170872901,-170872570,170872239,-170871908,170871577,-170871246,170870915,-170870584,170870253,-170869922,170869591,-170869260,170868929,-170868598,170868267,-170867936,170867605,-170867274,170866943,-170866612,170866281,-170865950,170865619,-170865288,170864957,-170864626,170864295,-170863964,170863633,-170863302,170862971,-170862640,170862309,-170861978,170861647,-170861316,170860985,-170860654,170860323,-170859992,170859661,-170859330,170858999,-170858668,170858337,-170858006,170857675,-170857344,170857013,-170856682,170856351,-170856020,170855689,-170855358,170855027,-170854696,170854365,-170854034,170853703,-170853372,170853041,-170852710,170852379,-170852048,170851717,-170851386,170851055,-170850724,170850393,-170850062,170849731,-170849400,170849069,-170848738,170848407,-170848076,170847745,-170847414,170847083,-170846752,170846421,-170846090,170845759,-170845428,170845097,-170844766,170844435,-170844104,170843773,-170843442,170843111,-170842780,170842449,-170842118,170841787,-170841456,170841125,-170840794,170840463,-170840132,170839801,-170839470,170839139,-170838808,170838477,-170838146,170837815,-170837484,170837153,-170836822,170836491,-170836160,170835829,-170835498,170835167,-170834836,170834505,-170834174,170833843,-170833512,170833181,-170832850,170832519,-170832188,170831857,-170831526,170831195,-170830864,170830533,-170830202,170829871,-170829540,170829209,-170828878,170828547,-170828216,170827885,-170827554,170827223,-170826892,170826561,-170826230,170825899,-170825568,170825237,-170824906,170824575,-170824244,170823913,-170823582,170823251,-170822920,170822589,-170822258,170821927,-170821596,170821265,-170820934,170820603,-170820272,170819941,-170819610,170819279,-170818948,170818617,-170818286,170817955,-170817624,170817293,-170816962,170816631,-170816300,170815969,-170815638,170815307,-170814976,170814645,-170814314,170813983,-170813652,170813321,-170812990,170812659,-170812328,170811997,-170811666,170811335,-170811004,170810673,-170810342,170810011,-170809680,170809349,-170809018,170808687,-170808356,170808025,-170807694,170807363,-170807032,170806701,-170806370,170806039,-170805708,170805377,-170805046,170804715,-170804384,170804053,-170803722,170803391,-170803060,170802729,-170802398,170802067,-170801736,170801405,-170801074,170800743,-170800412,170800081,-170799750,170799419,-170799088,170798757,-170798426,170798095,-170797764,170797433,-170797102,170796771,-170796440,170796109,-170795778,170795447,-170795116,170794785,-170794454,170794123,-170793792,170793461,-170793130,170792799,-170792468,170792137,-170791806,170791475,-170791144,170790813,-170790482,170790151,-170789820,170789489,-170789158,170788827,-170788496,170788165,-170787834,170787503,-170787172,170786841,-170786510,170786179,-170785848,170785517,-170785186,170784855,-170784524,170784193,-170783862,170783531,-170783200,170782869,-170782538,170782207,-170781876,170781545,-170781214,170780883,-170780552,170780221,-170779890,170779559,-170779228,170778897,-170778566,170778235,-170777904,170777573,-170777242,170776911,-170776580,170776249,-170775918,170775587,-170775256,170774925,-170774594,170774263,-170773932,170773601,-170773270,170772939,-170772608,170772277,-170771946,170771615,-170771284,170770953,-170770622,170770291,-170769960,170769629,-170769298,170768967,-170768636,170768305,-170767974,170767643,-170767312,170766981,-170766650,170766319,-170765988,170765657,-170765326,170764995,-170764664,170764333,-170764002,170763671,-170763340,170763009,-170762678,170762347,-170762016,170761685,-170761354,170761023,-170760692,170760361,-170760030,170759699,-170759368,170759037,-170758706,170758375,-170758044,170757713,-170757382,170757051,-170756720,170756389,-170756058,170755727,-170755396,170755065,-170754734,170754403,-170754072,170753741,-170753410,170753079,-170752748,170752417,-170752086,170751755,-170751424,170751093,-170750762,170750431,-170750100,170749769,-170749438,170749107,-170748776,170748445,-170748114,170747783,-170747452,170747121,-170746790,170746459,-170746128,170745797,-170745466,170745135,-170744804,170744473,-170744142,170743811,-170743480,170743149,-170742818,170742487,-170742156,170741825,-170741494,170741163,-170740832,170740501,-170740170,170739839,-170739508,170739177,-170738846,170738515,-170738184,170737853,-170737522,170737191,-170736860,170736529,-170736198,170735867,-170735536,170735205,-170734874,170734543,-170734212,170733881,-170733550,170733219,-170732888,170732557,-170732226,170731895,-170731564,170731233,-170730902,170730571,-170730240,170729909,-170729578,170729247,-170728916,170728585,-170728254,170727923,-170727592,170727261,-170726930,170726599,-170726268,170725937,-170725606,170725275,-170724944,170724613,-170724282,170723951,-170723620,170723289,-170722958,170722627,-170722296,170721965,-170721634,170721303,-170720972,170720641,-170720310,170719979,-170719648,170719317,-170718986,170718655,-170718324,170717993,-170717662,170717331,-170717000,170716669,-170716338,170716007,-170715676,170715345,-170715014,170714683,-170714352,170714021,-170713690,170713359,-170713028,170712697,-170712366,170712035,-170711704,170711373,-170711042,170710711,-170710380,170710049,-170709718,170709387,-170709056,170708725,-170708394,170708063,-170707732,170707401,-170707070,170706739,-170706408,170706077,-170705746,170705415,-170705084,170704753,-170704422,170704091,-170703760,170703429,-170703098,170702767,-170702436,170702105,-170701774,170701443,-170701112,170700781,-170700450,170700119,-170699788,170699457,-170699126,170698795,-170698464,170698133,-170697802,170697471,-170697140,170696809,-170696478,170696147,-170695816,170695485,-170695154,170694823,-170694492,170694161,-170693830,170693499,-170693168,170692837,-170692506,170692175,-170691844,170691513,-170691182,170690851,-170690520,170690189,-170689858,170689527,-170689196,170688865,-170688534,170688203,-170687872,170687541,-170687210,170686879,-170686548,170686217,-170685886,170685555,-170685224,170684893,-170684562,170684231,-170683900,170683569,-170683238,170682907,-170682576,170682245,-170681914,170681583,-170681252,170680921,-170680590,170680259,-170679928,170679597,-170679266,170678935,-170678604,170678273,-170677942,170677611,-170677280,170676949,-170676618,170676287,-170675956,170675625,-170675294,170674963,-170674632,170674301,-170673970,170673639,-170673308,170672977,-170672646,170672315,-170671984,170671653,-170671322,170670991,-170670660,170670329,-170669998,170669667,-170669336,170669005,-170668674,170668343,-170668012,170667681,-170667350,170667019,-170666688,170666357,-170666026,170665695,-170665364,170665033,-170664702,170664371,-170664040,170663709,-170663378,170663047,-170662716,170662385,-170662054,170661723,-170661392,170661061,-170660730,170660399,-170660068,170659737,-170659406,170659075,-170658744,170658413,-170658082,170657751,-170657420,170657089,-170656758,170656427,-170656096,170655765,-170655434,170655103,-170654772,170654441,-170654110,170653779,-170653448,170653117,-170652786,170652455,-170652124,170651793,-170651462,170651131,-170650800,170650469,-170650138,170649807,-170649476,170649145,-170648814,170648483,-170648152,170647821,-170647490,170647159,-170646828,170646497,-170646166,170645835,-170645504,170645173,-170644842,170644511,-170644180,170643849,-170643518,170643187,-170642856,170642525,-170642194,170641863,-170641532,170641201,-170640870,170640539,-170640208,170639877,-170639546,170639215,-170638884,170638553,-170638222,170637891,-170637560,170637229,-170636898,170636567,-170636236,170635905,-170635574,170635243,-170634912,170634581,-170634250,170633919,-170633588,170633257,-170632926,170632595,-170632264,170631933,-170631602,170631271,-170630940,170630609,-170630278,170629947,-170629616,170629285,-170628954,170628623,-170628292,170627961,-170627630,170627299,-170626968,170626637,-170626306,170625975,-170625644,170625313,-170624982,170624651,-170624320,170623989,-170623658,170623327,-170622996,170622665,-170622334,170622003,-170621672,170621341,-170621010,170620679,-170620348,170620017,-170619686,170619355,-170619024,170618693,-170618362,170618031,-170617700,170617369,-170617038,170616707,-170616376,170616045,-170615714,170615383,-170615052,170614721,-170614390,170614059,-170613728,170613397,-170613066,170612735,-170612404,170612073,-170611742,170611411,-170611080,170610749,-170610418,170610087,-170609756,170609425,-170609094,170608763,-170608432,170608101,-170607770,170607439,-170607108,170606777,-170606446,170606115,-170605784,170605453,-170605122,170604791,-170604460,170604129,-170603798,170603467,-170603136,170602805,-170602474,170602143,-170601812,170601481,-170601150,170600819,-170600488,170600157,-170599826,170599495,-170599164,170598833,-170598502,170598171,-170597840,170597509,-170597178,170596847,-170596516,170596185,-170595854,170595523,-170595192,170594861,-170594530,170594199,-170593868,170593537,-170593206,170592875,-170592544,170592213,-170591882,170591551,-170591220,170590889,-170590558,170590227,-170589896,170589565,-170589234,170588903,-170588572,170588241,-170587910,170587579,-170587248,170586917,-170586586,170586255,-170585924,170585593,-170585262,170584931,-170584600,170584269,-170583938,170583607,-170583276,170582945,-170582614,170582283,-170581952,170581621,-170581290,170580959,-170580628,170580297,-170579966,170579635,-170579304,170578973,-170578642,170578311,-170577980,170577649,-170577318,170576987,-170576656,170576325,-170575994,170575663,-170575332,170575001,-170574670,170574339,-170574008,170573677,-170573346,170573015,-170572684,170572353,-170572022,170571691};
         int main() {
             printf(\"%d %d %d %d\", t[0], t[1], t[512], t[1023]);
             return 0;
         }",
    );
    assert_eq!(
        salida,
        "-170910304 170909973 -170740832 170571691",
        "la tabla tiene que llegar entera y con los signos"
    );
}
