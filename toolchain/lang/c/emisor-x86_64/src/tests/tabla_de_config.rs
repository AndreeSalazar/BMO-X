//! **Por que muere DOOM: `&c->defaults[i]` vale CERO**
//!
//! El 2026-08-13 DOOM murio en el Ryzen con esta linea y nada detras:
//!
//! ```text
//!    Unknown configuration variable: 'use_joystick'
//! ```
//!
//! Eso **no es un aviso**: `m_config.c:1954` lo emite con `I_Error`, o sea que
//! es la causa de muerte. El camino es exacto y son cuatro pasos:
//!
//!   `I_BindJoystickVariables` -> `M_BindVariable("use_joystick", ...)`
//!   -> `GetDefaultForName` -> `SearchCollection` -> `NULL` -> `I_Error`
//!
//! Y `SearchCollection` (`m_config.c:1567`) es un bucle de dos lineas:
//!
//! ```c
//! for (i = 0; i < collection->numdefaults; ++i)
//!     if (!strcmp(name, collection->defaults[i].name))
//!         return &collection->defaults[i];      // <-- ESTA
//! ```
//!
//! ## Lo que se descarto ANTES de acusar a nadie, y en este orden
//!
//! Cada una de estas es un test verde de abajo, o una medida sobre el fuente
//! expandido de verdad. Ninguna era el problema:
//!
//! | | |
//! |---|---|
//! | La cuenta `arrlen()` con longitud deducida | sale `3`, y `200` con 200 |
//! | Los punteros del array estatico | los 200 apuntan a su cadena |
//! | El operador `#` a traves de dos macros | `use_joystick` sale entero |
//! | La entrada existe en la tabla | `doom.i:37214`, dentro de `doom_defaults_list` |
//! | `strcmp` comparando por la tabla | encuentra el indice `1` |
//! | La lectura `c->defaults[i].name` | correcta: por eso `strcmp` acierta |
//!
//! O sea: la tabla es correcta, la cuenta es correcta, la comparacion acierta
//! **y aun asi la funcion devuelve `NULL`** -- porque lo que se rompe es el
//! `return`, no la busqueda.
//!
//! ## El defecto, en una linea -- ARREGLADO el 2026-08-13
//!
//! `codegen/mod.rs`, `Expr::AddrOf`, sabia emitir tres formas --`&var`,
//! `&array[i]` y `&*p`-- y cerraba con:
//!
//! ```rust
//! _ => self.emit_xor_eax(),
//! ```
//!
//! `&c->defaults[i]` es `AddrOf(IndexPtr(..))`, que **no estaba en la lista**.
//! El compilador emitia `xor eax,eax` y seguia: la direccion pedida salia CERO,
//! sin un aviso, y el programa se enteraba a 56.000 lineas de distancia.
//!
//! ** Era el patron del `char *mapa` del raycaster otra vez (`2bc13367`), y por
//! tercera vez: **un `_ =>` que rellena de ceros lo que no sabe traducir**.
//!
//! **Lo arreglado, y son dos cosas distintas:**
//!
//! 1. Tres brazos nuevos --`IndexPtr`, `Field` y `Arrow`--, que son la version
//!    SIN CARGA de los que ya existian. `&s.campo` y `&p->campo` tambien valian
//!    cero, y eso es C de todos los dias.
//! 2. **El `_ =>` ya no rellena: acumula un error con la expresion dentro.** Un
//!    compilador que no sabe tomar una direccion tiene que decirlo donde la
//!    frase esta entera.
//!
//! Y de rebote salio un tercero, mas general y peor: `pointer_scale` media con
//! `TypeSpec::stack_size()`, que contesta **0** para `StructRef` porque desde el
//! AST no hay tabla de tamanos. O sea que `p + 1` sobre un `struct T *`
//! avanzaba **UN BYTE**. Ahora mide con `type_stack_size`, que es la cuenta con
//! la tabla delante -- la misma que ya usaba el subindice. El subindice acertaba
//! y la suma no, siendo la misma direccion escrita de dos formas.
//!
//! [!] La forma de las tablas hay que conservarla al tocar estos tests: DOOM
//! las declara `default_t doom_defaults_list[] = { ... }` -- con `[]`, sin
//! numero, y con `default_t` siendo un **typedef**. `struct D l[] = {{..}}` con
//! la etiqueta a pelo NI SIQUIERA PARSEA hoy (`unexpected token: CloseBracket`,
//! defecto aparte y sin abrir). Reescribirlos con `struct` dejaria de probar lo
//! que DOOM hace.

use super::*;

// ===================================================================
// LO QUE FUNCIONA -- y hace falta que siga funcionando
// ===================================================================

/// La cuenta: `arrlen()` de DOOM sobre un array de longitud deducida.
///
/// Va primera porque es la que mataria mas barato: si `numdefaults` saliera
/// `0`, `SearchCollection` no entraria en el bucle ni una vez y contestaria
/// `NULL` sin comparar nada -- un fallo con el mismo sintoma y otra causa.
#[test]
fn arrlen_de_un_array_de_longitud_deducida() {
    let out = run_c(
        "typedef struct { char *name; int *loc; } def; \
         def lista[] = { {\"mouse_sensitivity\", 0}, \
                         {\"use_joystick\", 0}, \
                         {\"snd_channels\", 0} }; \
         int cuantos = sizeof(lista) / sizeof(*lista); \
         int main() { \
           printf(\"n=%d bytes=%d una=%d\\n\", \
                  cuantos, (int)sizeof(lista), (int)sizeof(def)); \
           return 0; }",
    );
    assert_eq!(out.trim(), "n=3 bytes=48 una=16");
}

/// La tabla con la forma de DOOM y los nombres de verdad: la cuenta y los dos
/// primeros punteros.
#[test]
fn la_tabla_de_config_de_doom_se_puede_leer() {
    let out = run_c(
        "typedef struct { char *name; int *loc; } def; \
         typedef struct { def *defaults; int numdefaults; void *sig; } coleccion; \
         def lista[] = { {\"mouse_sensitivity\", 0}, \
                         {\"use_joystick\", 0}, \
                         {\"snd_channels\", 0} }; \
         coleccion extra = { lista, sizeof(lista)/sizeof(*lista), 0 }; \
         int main() { \
           printf(\"n=%d\\n\", extra.numdefaults); \
           printf(\"uno=%s\\n\", extra.defaults[0].name); \
           printf(\"dos=%s\\n\", extra.defaults[1].name); \
           return 0; }",
    );
    assert_eq!(out.trim(), "n=3\nuno=mouse_sensitivity\ndos=use_joystick");
}

/// ** DE DONDE SALE CADA NOMBRE: el operador `#`.
///
/// Las tablas de DOOM no llevan cadenas escritas. Llevan esto
/// (`m_config.c:95`):
///
/// ```c
/// #define CONFIG_VARIABLE_GENERIC(name, type)  { #name, NULL, type, 0, 0, false }
/// #define CONFIG_VARIABLE_INT(name)            CONFIG_VARIABLE_GENERIC(name, DEFAULT_INT)
/// ```
///
/// O sea que **los ~190 nombres los fabrica el preprocesador**, con `#` a
/// traves de DOS niveles de macro. Un `#n` mal implementado no da un cuelgue:
/// da una cadena valida y equivocada --el literal `"n"`, o vacia-- para las 190
/// entradas a la vez, y el programa compila, enlaza y arranca igual.
///
/// Va con `run_c_con_pp` a proposito: `run_c` **no pasa por el preprocesador**,
/// asi que probar esto con `run_c` seria probar la nada.
#[test]
fn el_operador_de_cadena_atraviesa_dos_macros() {
    let out = run_c_con_pp(
        "#define GENERICA(n, t) { #n, t }\n\
         #define ENTERA(n) GENERICA(n, 1)\n\
         typedef struct { char *name; int tipo; } def;\n\
         def lista[] = { ENTERA(mouse_sensitivity), ENTERA(use_joystick) };\n\
         int main() {\n\
           printf(\"[%s] [%s]\\n\", lista[0].name, lista[1].name);\n\
           return 0;\n\
         }\n",
    );
    assert_eq!(out.trim(), "[mouse_sensitivity] [use_joystick]");
}

/// ** LA ESCALA, que es la dimension que las de arriba no cubren.
///
/// Las tablas de DOOM no tienen tres entradas: `doom_defaults_list` tiene ~40 y
/// `extra_defaults_list` ~150. Son ~190 punteros a cadena en un solo array
/// estatico, o sea ~190 relocations para el mismo sitio.
///
/// Se prueba con 200 y se miran **la primera, una del medio y la ULTIMA**: si
/// algo se truncara --una tabla de fixups que se llena, un indice de 8 bits--
/// se notaria al final, y mirar solo la primera diria que todo va bien.
#[test]
fn doscientos_punteros_en_un_array_estatico() {
    let mut src = String::from("typedef struct { char *name; int tipo; } def;\ndef lista[] = {");
    for i in 0..200 {
        src.push_str(&format!(" {{\"var_{i}\", {i}}},"));
    }
    src.push_str("};\nint cuantos = sizeof(lista) / sizeof(*lista);\n");
    src.push_str(
        "int main() { \
           printf(\"n=%d [%s] [%s] [%s]\\n\", cuantos, \
                  lista[0].name, lista[100].name, lista[199].name); \
           return 0; }",
    );
    let out = run_c(&src);
    assert_eq!(out.trim(), "n=200 [var_0] [var_100] [var_199]");
}

/// La busqueda devolviendo el **indice**. Verde, y por eso vale: demuestra que
/// `strcmp` acierta y que `c->defaults[i].name` se LEE bien. Lo unico que
/// separa esta de la de abajo es que aquella devuelve la direccion.
#[test]
fn search_collection_encuentra_el_indice() {
    let out = run_c(
        "typedef struct { char *name; int *loc; } def; \
         typedef struct { def *defaults; int numdefaults; void *sig; } coleccion; \
         def lista[] = { {\"mouse_sensitivity\", 0}, \
                         {\"use_joystick\", 0}, \
                         {\"snd_channels\", 0} }; \
         coleccion extra = { lista, sizeof(lista)/sizeof(*lista), 0 }; \
         int buscar(coleccion *c, char *nombre) { \
           int i; \
           for (i = 0; i < c->numdefaults; i = i + 1) { \
             if (strcmp(nombre, c->defaults[i].name) == 0) { return i; } \
           } \
           return -1; \
         } \
         int main() { \
           printf(\"%d %d\\n\", buscar(&extra, \"use_joystick\"), \
                               buscar(&extra, \"no_existe\")); \
           return 0; }",
    );
    assert_eq!(out.trim(), "1 -1");
}

// ===================================================================
// LA GUARDA DEL DEFECTO -- rojas hasta el 2026-08-13, verdes desde el arreglo
// ===================================================================
//
// Estas cinco reprodujeron el fallo antes de arreglarlo, y el reparto de
// verde/rojo entre ellas FUE el diagnostico:
//
//   `&c->campo[i]`            ROJO   <- la de DOOM
//   `c->campo + i`            ROJO   <- otro defecto: la escala del struct
//   `p = c->campo; &p[i]`     VERDE  <- copiar a un local lo esquivaba
//   `&global[i]`              VERDE  <- sin campo en medio no pasaba
//
// Se conservan enteras y con el mismo nombre: son la guarda de que no vuelve, y
// la unica forma de volver a leer el diagnostico es tenerlas juntas.

const TABLA: &str = "typedef struct { char *name; int v; } def_t;\n\
     typedef struct { def_t *defaults; int n; } col_t;\n\
     def_t lista[] = { {\"a\", 10}, {\"b\", 20}, {\"c\", 30} };\n\
     col_t col = { lista, 3 };\n";

/// **LA DE DOOM, tal cual.** `SearchCollection` reducida a su ultima linea.
#[test]
fn amp_de_subindice_por_campo_puntero() {
    let s = format!(
        "{TABLA}def_t *dame(col_t *c, int i) {{ return &c->defaults[i]; }}\n\
         int main() {{ def_t *r = dame(&col, 1); \
           if (r == 0) {{ printf(\"NULO\\n\"); }} else {{ printf(\"%s %d\\n\", r->name, r->v); }} \
           return 0; }}"
    );
    assert_eq!(run_c_con_pp(&s).trim(), "b 20");
}

/// La misma direccion escrita como aritmetica. Falla tambien, asi que el
/// defecto **no es solo del `&`**: es de usar un campo-puntero como base.
#[test]
fn suma_de_puntero_por_campo() {
    let s = format!(
        "{TABLA}def_t *dame(col_t *c, int i) {{ return c->defaults + i; }}\n\
         int main() {{ def_t *r = dame(&col, 1); \
           if (r == 0) {{ printf(\"NULO\\n\"); }} else {{ printf(\"%s %d\\n\", r->name, r->v); }} \
           return 0; }}"
    );
    assert_eq!(run_c_con_pp(&s).trim(), "b 20");
}

/// Sin `return` de por medio: la direccion a un local, en el mismo `main`.
/// Falla igual, asi que **no es el camino de retorno**: es la expresion.
#[test]
fn amp_de_subindice_a_un_local() {
    let s = format!(
        "{TABLA}int main() {{ col_t *c = &col; def_t *r = &c->defaults[1]; \
           if (r == 0) {{ printf(\"NULO\\n\"); }} else {{ printf(\"%s %d\\n\", r->name, r->v); }} \
           return 0; }}"
    );
    assert_eq!(run_c_con_pp(&s).trim(), "b 20");
}

/// `SearchCollection` entera, con el bucle y el `strcmp` de verdad. Es la que
/// hay que ver verde para decir que DOOM pasa de aqui.
#[test]
fn search_collection_devolviendo_la_direccion() {
    let s = format!(
        "{TABLA}def_t *buscar(col_t *c, char *n) {{ int i; \
           for (i = 0; i < c->n; i = i + 1) {{ \
             if (!strcmp(n, c->defaults[i].name)) {{ return &c->defaults[i]; }} }} \
           return 0; }}\n\
         int main() {{ def_t *r = buscar(&col, \"b\"); \
           if (r == 0) {{ printf(\"NULO\\n\"); }} else {{ printf(\"%s %d\\n\", r->name, r->v); }} \
           return 0; }}"
    );
    assert_eq!(run_c_con_pp(&s).trim(), "b 20");
}

/// ** `&s.campo` y `&p->campo` -- los otros dos que valian CERO.
///
/// Salieron al arreglar el de DOOM y son peores por lo comunes: pasar un campo
/// por referencia es C de todos los dias (`scanf("%d", &cfg.puerto)`), y valia
/// cero. Nadie lo habia notado porque ningun ejemplo de BMO lo hacia.
#[test]
fn la_direccion_de_un_campo_por_punto_y_por_flecha() {
    let s = "typedef struct { int a; int b; int c; } tres_t;\n\
         tres_t g = { 10, 20, 30 };\n\
         void mete(int *donde, int v) { *donde = v; }\n\
         int main() { tres_t *p = &g; \
           mete(&g.b, 99); \
           mete(&p->c, 77); \
           printf(\"%d %d %d\\n\", g.a, g.b, g.c); return 0; }";
    assert_eq!(run_c_con_pp(s).trim(), "10 99 77");
}

/// ** `p + 1` SOBRE UN PUNTERO A STRUCT, que avanzaba UN BYTE.
///
/// El tercero, y el mas general de los tres: `pointer_scale` media con
/// `TypeSpec::stack_size()`, que contesta `0` para un `StructRef` --desde el AST
/// no hay tabla de tamanos-- y con `0` la funcion decidia que "esto no es un
/// puntero" y no escalaba nada.
///
/// La prueba compara las DOS formas de escribir la misma direccion: si el
/// subindice y la suma no dan lo mismo, una de las dos miente. Antes del
/// arreglo, `lista + 1` caia dentro de la PRIMERA entrada.
#[test]
fn la_suma_a_un_puntero_a_struct_avanza_un_elemento_entero() {
    let s = "typedef struct { char *nombre; int v; } item_t;\n\
         item_t lista[3] = { {\"a\", 10}, {\"b\", 20}, {\"c\", 30} };\n\
         int main() { item_t *p = lista; \
           item_t *q = p + 2; \
           printf(\"%s %d | %s %d | %d\\n\", \
                  q->nombre, q->v, lista[2].nombre, lista[2].v, \
                  (int)(q == &lista[2])); \
           return 0; }";
    assert_eq!(run_c_con_pp(s).trim(), "c 30 | c 30 | 1");
}

/// Y las DOS que quedan verdes al lado, que son las que dicen donde NO estaba el
/// problema. Sin ellas, las de arriba acusarian a media familia.
#[test]
fn el_campo_copiado_a_un_local_si_funciona() {
    let s = format!(
        "{TABLA}def_t *dame(col_t *c, int i) {{ def_t *p = c->defaults; return &p[i]; }}\n\
         def_t *global(int i) {{ return &lista[i]; }}\n\
         int main() {{ def_t *a = dame(&col, 1); def_t *b = global(2); \
           printf(\"%s %s\\n\", a->name, b->name); return 0; }}"
    );
    assert_eq!(run_c_con_pp(&s).trim(), "b c");
}
