//! # LA SONDA DEL LENGUAJE -- el censo de lo que BMO C sabe hacer
//!
//! ## Por que existe, y es una decision de METODO
//!
//! El 2026-08-13 cayeron **tres** defectos del codegen en un dia, y los tres se
//! descubrieron igual: flashear, ver morir a DOOM, cazar el bug, flashear otra
//! vez. Cuatro vueltas. Y los tres eran la misma forma --un contenedor, una
//! operacion, y un brazo del codegen que no cubria esa casilla--:
//!
//! | | |
//! |---|---|
//! | `&c->defaults[i]` | valia CERO |
//! | `p + 1` sobre `struct T *` | avanzaba UN BYTE |
//! | `p++` sobre cualquier puntero | avanzaba UN BYTE |
//!
//! Perseguirlos de uno en uno con arranques no escala. **Se pueden enumerar.**
//!
//! ## Y por que es una MATRIZ y no una lista de casos
//!
//! [!] Una sonda que se escribe a mano prueba **lo que se le ocurrio a quien la
//! escribio** -- es el patron 15 de esta casa, ya pagado con `c/sonda.bex`:
//! *"la sonda la escribio el mismo lado que escribio las defensas"*.
//!
//! Asi que esto no es una lista de ideas: es el producto de dos ejes que se
//! enumeran solos.
//!
//! ```text
//!   CONTENEDOR   variable . array . campo por `.` . campo por `->`
//!                subindice de puntero . array de structs . doble puntero
//!
//!   OPERACION    leer . escribir . tomar la direccion . ++ . -- . + n
//!                llamar (cuando el contenido es un puntero a funcion)
//!```
//!
//! Las casillas salen del cruce, no de la inspiracion. Si manana aparece un
//! contenedor nuevo, se anade la fila y salen sus casillas.
//!
//! ## El censo se COMPARA, y por eso no puede quedarse viejo
//!
//! El test no comprueba una a una: construye el informe entero y lo compara
//! contra la tabla escrita al final de este fichero. Eso da tres cosas:
//!
//! 1. **Una casilla rota no tapa a las demas.** El informe las lista todas, que
//!    es lo que convierte cuatro arranques en una ejecucion.
//! 2. **La suite se queda en verde** con defectos abiertos, porque el censo dice
//!    la verdad -- incluido lo que no funciona.
//! 3. ** Y en cuanto la realidad cambie --se arregle un `ROTO` o se rompa un
//!    `BIEN`-- **el test falla y obliga a actualizar el censo**. Es la cura del
//!    documento que miente, que hoy se ha pagado dos veces: `PLAN_DOOM.md`
//!    mandando a mirar una puerta ya cerrada, y el panel de ESTRATOS diciendo
//!    que no se podia escribir mientras el disco se escribia.
//!
//! O sea que esto es a la vez la prueba y **la respuesta a "que soporta BMO
//! C"**, que hasta hoy era una impresion.

use super::*;

/// Una casilla del censo: como se llama, que programa la ejerce, que tiene que
/// imprimir.
struct Casilla {
    nombre: &'static str,
    fuente: &'static str,
    espera: &'static str,
}

/// Lo que se pone delante de casi todas: un struct con un campo de cada clase
/// que hace falta, y una tabla de ellos.
const TIPOS: &str = "typedef struct { int n; char *txt; int (*fn)(int); } caja_t;\n\
                     typedef struct { caja_t *dentro; int k; } sobre_t;\n\
                     int doble(int x) { return x * 2; }\n\
                     int triple(int x) { return x * 3; }\n";

fn censo() -> [Casilla; 26] {
    [
        // -- El contenedor mas simple: una variable ----------------------
        Casilla {
            nombre: "leer y escribir una local",
            fuente: "int main() { int a; a = 7; a = a + 1; printf(\"%d\\n\", a); return 0; }",
            espera: "8",
        },
        Casilla {
            nombre: "&local, y escribir por el",
            fuente: "void pon(int *d, int v) { *d = v; }\n\
                     int main() { int a; a = 0; pon(&a, 9); printf(\"%d\\n\", a); return 0; }",
            espera: "9",
        },
        Casilla {
            nombre: "un entero ++ avanza UNO",
            fuente: "int main() { int i; i = 5; i++; ++i; printf(\"%d\\n\", i); return 0; }",
            espera: "7",
        },
        // -- Array ------------------------------------------------------
        Casilla {
            nombre: "leer y escribir un array global",
            fuente: "int g[4] = {1,2,3,4};\n\
                     int main() { g[2] = 30; printf(\"%d %d\\n\", g[0], g[2]); return 0; }",
            espera: "1 30",
        },
        Casilla {
            nombre: "&global[i]",
            fuente: "int g[4] = {1,2,3,4};\n\
                     void pon(int *d, int v) { *d = v; }\n\
                     int main() { pon(&g[1], 20); printf(\"%d\\n\", g[1]); return 0; }",
            espera: "20",
        },
        // -- Campo por punto --------------------------------------------
        Casilla {
            nombre: "leer y escribir s.campo",
            fuente: "typedef struct { int a; int b; } par_t;\n\
                     int main() { par_t s; s.a = 1; s.b = 2; s.b = s.b + 10; \
                       printf(\"%d %d\\n\", s.a, s.b); return 0; }",
            espera: "1 12",
        },
        Casilla {
            nombre: "&s.campo",
            fuente: "typedef struct { int a; int b; } par_t;\n\
                     void pon(int *d, int v) { *d = v; }\n\
                     int main() { par_t s; s.a = 0; s.b = 0; pon(&s.b, 5); \
                       printf(\"%d %d\\n\", s.a, s.b); return 0; }",
            espera: "0 5",
        },
        // -- Campo por flecha -------------------------------------------
        Casilla {
            nombre: "leer y escribir p->campo",
            fuente: "typedef struct { int a; int b; } par_t;\n\
                     int main() { par_t s; par_t *p; s.a = 1; s.b = 2; p = &s; \
                       p->b = p->b + 10; printf(\"%d %d\\n\", p->a, p->b); return 0; }",
            espera: "1 12",
        },
        Casilla {
            nombre: "&p->campo",
            fuente: "typedef struct { int a; int b; } par_t;\n\
                     void pon(int *d, int v) { *d = v; }\n\
                     int main() { par_t s; par_t *p; s.b = 0; p = &s; pon(&p->b, 7); \
                       printf(\"%d\\n\", s.b); return 0; }",
            espera: "7",
        },
        // -- Subindice de puntero ---------------------------------------
        Casilla {
            nombre: "leer y escribir p[i]",
            fuente: "int g[4] = {1,2,3,4};\n\
                     int main() { int *p; p = g; p[2] = 30; printf(\"%d %d\\n\", p[0], p[2]); return 0; }",
            espera: "1 30",
        },
        Casilla {
            nombre: "&p[i]",
            fuente: "int g[4] = {1,2,3,4};\n\
                     void pon(int *d, int v) { *d = v; }\n\
                     int main() { int *p; p = g; pon(&p[3], 40); printf(\"%d\\n\", g[3]); return 0; }",
            espera: "40",
        },
        Casilla {
            nombre: "&c->campo[i]  (la de DOOM)",
            fuente: "typedef struct { char *name; int v; } def_t;\n\
                     typedef struct { def_t *tabla; int n; } col_t;\n\
                     def_t lista[3] = { {\"a\",10}, {\"b\",20}, {\"c\",30} };\n\
                     col_t col = { lista, 3 };\n\
                     def_t *dame(col_t *c, int i) { return &c->tabla[i]; }\n\
                     int main() { def_t *r; r = dame(&col, 1); \
                       if (r == 0) { printf(\"NULO\\n\"); } \
                       else { printf(\"%s %d\\n\", r->name, r->v); } return 0; }",
            espera: "b 20",
        },
        // -- Array de structs -------------------------------------------
        Casilla {
            nombre: "leer y escribir a[i].campo",
            fuente: "typedef struct { int a; int b; } par_t;\n\
                     par_t t[3];\n\
                     int main() { t[1].a = 4; t[1].b = 5; \
                       printf(\"%d %d\\n\", t[1].a, t[1].b); return 0; }",
            espera: "4 5",
        },
        Casilla {
            nombre: "&a[i].campo",
            fuente: "typedef struct { int a; int b; } par_t;\n\
                     par_t t[3];\n\
                     void pon(int *d, int v) { *d = v; }\n\
                     int main() { t[2].b = 0; pon(&t[2].b, 6); printf(\"%d\\n\", t[2].b); return 0; }",
            espera: "6",
        },
        // -- Aritmetica de punteros -------------------------------------
        Casilla {
            nombre: "p + n sobre int*",
            fuente: "int g[4] = {1,2,3,4};\n\
                     int main() { int *p; p = g; printf(\"%d\\n\", *(p + 2)); return 0; }",
            espera: "3",
        },
        Casilla {
            nombre: "p + n sobre struct*",
            fuente: "typedef struct { int a; int b; } par_t;\n\
                     par_t t[3];\n\
                     int main() { par_t *p; t[2].a = 9; p = t; p = p + 2; \
                       printf(\"%d %d\\n\", p->a, (int)(p == &t[2])); return 0; }",
            espera: "9 1",
        },
        Casilla {
            nombre: "p++ y p-- sobre int*",
            fuente: "int g[4] = {10,20,30,40};\n\
                     int main() { int *p; p = g; p++; p++; printf(\"%d \", *p); \
                       p--; printf(\"%d\\n\", *p); return 0; }",
            espera: "30 20",
        },
        Casilla {
            nombre: "*p++  (la macro va_arg)",
            fuente: "int g[3] = {11,22,33};\n\
                     int main() { int *p; p = g; \
                       printf(\"%d %d %d\\n\", *p++, *p++, *p++); return 0; }",
            espera: "11 22 33",
        },
        Casilla {
            nombre: "restar dos punteros",
            fuente: "int g[8];\n\
                     int main() { int *a; int *b; a = g; b = g + 5; \
                       printf(\"%d\\n\", (int)(b - a)); return 0; }",
            espera: "5",
        },
        Casilla {
            nombre: "recorrer hasta el centinela",
            fuente: "char *s = \"hola\";\n\
                     int main() { char *p; int n; p = s; n = 0; \
                       while (*p) { n = n + 1; p++; } printf(\"%d\\n\", n); return 0; }",
            espera: "4",
        },
        // -- Punteros a funcion, los cuatro contenedores -----------------
        Casilla {
            nombre: "llamar por variable",
            fuente: "int doble(int x) { return x * 2; }\n\
                     int main() { int (*f)(int); f = doble; printf(\"%d\\n\", f(21)); return 0; }",
            espera: "42",
        },
        Casilla {
            nombre: "llamar por tabla global",
            fuente: "int doble(int x) { return x * 2; }\n\
                     int triple(int x) { return x * 3; }\n\
                     int (*tabla[2])(int) = { doble, triple };\n\
                     int main() { printf(\"%d %d\\n\", tabla[0](5), tabla[1](5)); return 0; }",
            espera: "10 15",
        },
        Casilla {
            nombre: "llamar por s.campo",
            fuente: "typedef struct { int (*fn)(int); } caja_t;\n\
                     int doble(int x) { return x * 2; }\n\
                     int main() { caja_t c; c.fn = doble; printf(\"%d\\n\", c.fn(8)); return 0; }",
            espera: "16",
        },
        Casilla {
            // ** LA DE DOOM EN `W_AddFile`: `wad_file->file_class->Read(...)`.
            nombre: "llamar por p->campo",
            fuente: "typedef struct { int (*fn)(int); } caja_t;\n\
                     int doble(int x) { return x * 2; }\n\
                     int main() { caja_t c; caja_t *p; c.fn = doble; p = &c; \
                       printf(\"%d\\n\", p->fn(8)); return 0; }",
            espera: "16",
        },
        Casilla {
            // Y la de DOS saltos, que es literalmente la forma de DOOM.
            nombre: "llamar por p->otro->campo",
            fuente: "typedef struct { int (*fn)(int); } clase_t;\n\
                     typedef struct { clase_t *clase; int n; } obj_t;\n\
                     int doble(int x) { return x * 2; }\n\
                     int main() { clase_t k; obj_t o; obj_t *p; \
                       k.fn = doble; o.clase = &k; p = &o; \
                       printf(\"%d\\n\", p->clase->fn(8)); return 0; }",
            espera: "16",
        },
        // -- Doble puntero ----------------------------------------------
        Casilla {
            nombre: "doble puntero: **pp",
            fuente: "int main() { int a; int *p; int **pp; a = 3; p = &a; pp = &p; \
                       **pp = 4; printf(\"%d\\n\", a); return 0; }",
            espera: "4",
        },
    ]
}

/// El barrido entero, en una ejecucion.
///
/// [!] Cada casilla va dentro de un `catch_unwind` a proposito: una que **no
/// compile** --como `struct D l[] = {{..}}`, que hoy no parsea-- haria panic en
/// el `expect` del ayudante y se llevaria por delante el resto del censo. Aqui
/// se anota como `NO COMPILA` y el barrido sigue. Un censo que se para en el
/// primer hueco no es un censo.
#[test]
fn el_censo_del_lenguaje_no_ha_cambiado() {
    // El hook de panico se calla mientras dura el barrido: si no, la salida se
    // llena de trazas de las casillas rotas y el informe --que es lo que hay que
    // leer-- se pierde entre ellas.
    let anterior = std::panic::take_hook();
    std::panic::set_hook(Box::new(|_| {}));

    let mut informe = String::new();
    for c in censo().iter() {
        let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| run_c_con_pp(c.fuente)));
        let veredicto = match r {
            Ok(salida) if salida.trim() == c.espera => "BIEN".to_string(),
            Ok(salida) => format!("ROTO da {:?} y toca {:?}", salida.trim(), c.espera),
            Err(_) => "NO COMPILA o revienta".to_string(),
        };
        informe.push_str(&format!("{:<30} {}\n", c.nombre, veredicto));
    }

    std::panic::set_hook(anterior);

    assert_eq!(
        informe.trim_end(),
        CENSO.trim_end(),
        "\n\nEL CENSO DEL LENGUAJE CAMBIO.\n\
         Si se arreglo una casilla o se rompio otra, **actualiza la constante\n\
         `CENSO` de este fichero**: es el sitio donde esta escrito que soporta\n\
         BMO C, y un censo que no se actualiza es justo el documento que miente.\n"
    );
}

/// **EL CENSO, al 2026-08-13.**
///
/// Esto no es la lista de deseos: es lo que la maquina contesto la ultima vez
/// que alguien corrio el barrido. Cada `ROTO` es un defecto abierto **con su
/// sintoma exacto al lado**, que es mas util que una fila en un `TODO`.
const CENSO: &str = "\
leer y escribir una local      BIEN
&local, y escribir por el      BIEN
un entero ++ avanza UNO        BIEN
leer y escribir un array global BIEN
&global[i]                     BIEN
leer y escribir s.campo        BIEN
&s.campo                       BIEN
leer y escribir p->campo       BIEN
&p->campo                      BIEN
leer y escribir p[i]           BIEN
&p[i]                          BIEN
&c->campo[i]  (la de DOOM)     BIEN
leer y escribir a[i].campo     BIEN
&a[i].campo                    BIEN
p + n sobre int*               BIEN
p + n sobre struct*            BIEN
p++ y p-- sobre int*           BIEN
*p++  (la macro va_arg)        BIEN
restar dos punteros            BIEN
recorrer hasta el centinela    BIEN
llamar por variable            BIEN
llamar por tabla global        BIEN
llamar por s.campo             BIEN
llamar por p->campo            BIEN
llamar por p->otro->campo      BIEN
doble puntero: **pp            BIEN
";
