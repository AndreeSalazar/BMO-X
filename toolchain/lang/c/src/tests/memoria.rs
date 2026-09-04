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
