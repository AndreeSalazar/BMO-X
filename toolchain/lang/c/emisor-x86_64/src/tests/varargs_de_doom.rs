//! **`p++` SOBRE UN PUNTERO NO AVANZA UN ELEMENTO** -- y eso es `va_arg`
//!
//! DOOM, tras arreglar el `!`, muere justo despues de
//! `M_LoadDefaults: Load system defaults.` y antes de `saving config in %s`.
//! Entre esos dos prints solo hay una llamada:
//!
//! ```c
//! doom_defaults.filename = M_StringJoin(configdir, default_main_config, NULL);
//! ```
//!
//! Y `M_StringJoin` (`m_misc.c:426`) es **variadica con un bucle de `va_arg`
//! terminado por NULL**, recorrido DOS veces (una para medir, otra para copiar).
//!
//! [!] Y esa rama **nunca se habia ejecutado**: hasta el arreglo del `!`,
//! `M_CheckParmWithArgs("-config", 1)` casaba por error y DOOM se iba por el
//! `if`. Arreglar un bug **destapa el codigo que estaba tapado**.
//!
//! ## El defecto, clavado
//!
//! `<stdarg.h>` define la macro asi, y es correcta:
//!
//! ```c
//! typedef unsigned long long *va_list;
//! #define va_arg(ap, type)   ((type)(*(ap)++))
//! ```
//!
//! O sea que `va_arg` **ES** un `*p++` sobre un puntero. Y eso esta roto:
//!
//! ```text
//!    *p++ tres veces sobre {11, 22, 33}  ->  11 0 0     (deberia ser 11 22 33)
//! ```
//!
//! El post-incremento **no avanza un elemento**. La primera lectura acierta
//! --el puntero todavia esta donde se puso-- y de ahi en adelante lee donde no
//! debe. En `M_StringJoin` eso significa recorrer 19 punteros basura en vez de
//! 3 y hacerles `strlen`: memoria no mapeada, `#PF`, tarea eliminada.
//!
//! ** Es de la MISMA familia que el `pointer_scale` de esta manana --aritmetica
//! de punteros que no escala-- pero por otro camino: aquel era `Expr::Add`,
//! este es `Expr::PostInc`. Arreglar uno no arreglo el otro porque **son dos
//! brazos distintos que hacen la misma cuenta**, que es justo el patron 34.
//!
//! Y lo que significa fuera de DOOM: **recorrer un array con un puntero es el
//! idioma mas comun de C**. `while (*p) p++;` no funciona en BMO C.

use super::*;

/// El patron exacto: punteros hasta un NULL, contando por el camino.
#[test]
fn va_arg_de_punteros_hasta_null() {
    let out = run_c_con_pp(
        "#include <stdarg.h>\n\
         #include <string.h>\n\
         int juntar(const char *s, ...) {\n\
           va_list args;\n\
           const char *v;\n\
           int total;\n\
           total = strlen(s);\n\
           va_start(args, s);\n\
           for (;;) {\n\
             v = va_arg(args, const char *);\n\
             if (v == 0) { break; }\n\
             total = total + strlen(v);\n\
           }\n\
           va_end(args);\n\
           return total;\n\
         }\n\
         int main() {\n\
           printf(\"%d %d\\n\",\n\
             juntar(\".\", \"default.cfg\", 0),\n\
             juntar(\"aa\", \"bb\", \"cc\", 0));\n\
           return 0;\n\
         }\n",
    );
    assert_eq!(out.trim(), "12 6");
}

/// Y el mismo `va_list` recorrido DOS veces con dos `va_start`, que es lo que
/// hace `M_StringJoin`: una pasada para medir y otra para copiar. Un `va_start`
/// que no reinicie el cursor da la primera bien y la segunda vacia.
#[test]
fn dos_pasadas_sobre_los_mismos_varargs() {
    let out = run_c_con_pp(
        "#include <stdarg.h>\n\
         #include <string.h>\n\
         int dos_veces(const char *s, ...) {\n\
           va_list args;\n\
           const char *v;\n\
           int a;\n\
           int b;\n\
           a = 0; b = 0;\n\
           va_start(args, s);\n\
           for (;;) { v = va_arg(args, const char *); if (v == 0) { break; } a = a + 1; }\n\
           va_end(args);\n\
           va_start(args, s);\n\
           for (;;) { v = va_arg(args, const char *); if (v == 0) { break; } b = b + 1; }\n\
           va_end(args);\n\
           return a * 10 + b;\n\
         }\n\
         int main() { printf(\"%d\\n\", dos_veces(\"x\", \"a\", \"b\", \"c\", 0)); return 0; }\n",
    );
    assert_eq!(out.trim(), "33");
}

/// El sospechoso de la macro: `*(ap)++` sobre un `unsigned long long *`.
/// Tiene que avanzar OCHO bytes, no uno.
#[test]
fn el_postincremento_de_un_puntero_avanza_un_elemento() {
    let out = run_c_con_pp(
        "int main() {\n\
           unsigned long long v[4];\n\
           unsigned long long *p;\n\
           v[0] = 11; v[1] = 22; v[2] = 33; v[3] = 0;\n\
           p = v;\n\
           printf(\"%d %d %d\n\", (int)(*p++), (int)(*p++), (int)(*p++));\n\
           return 0;\n\
         }\n",
    );
    assert_eq!(out.trim(), "11 22 33");
}

/// Los CUATRO brazos, no solo el que fallaba. `emit_inc_var` y `emit_dec_var`
/// los comparten pre y post, asi que arreglar uno toca los cuatro -- y un
/// arreglo que se prueba en uno solo es un arreglo a medias.
#[test]
fn los_cuatro_brazos_avanzan_un_elemento() {
    let out = run_c_con_pp(
        "int main() {\n\
           int v[5];\n\
           int *p;\n\
           v[0] = 10; v[1] = 20; v[2] = 30; v[3] = 40; v[4] = 50;\n\
           p = v;\n\
           p++;      /* post  */\n\
           ++p;      /* pre   */\n\
           printf(\"%d \", *p);\n\
           p--;      /* post  */\n\
           --p;      /* pre   */\n\
           printf(\"%d\n\", *p);\n\
           return 0;\n\
         }\n",
    );
    assert_eq!(out.trim(), "30 10");
}

/// ** Y LA NO-REGRESION, que es la mitad que se olvida.
///
/// Un `int` tiene que seguir avanzando de UNO. Si el paso saliera del tipo sin
/// distinguir puntero de entero, `i++` avanzaria cuatro y todos los bucles del
/// sistema contarian de cuatro en cuatro -- un fallo mucho peor que el que se
/// arreglaba.
#[test]
fn un_entero_sigue_avanzando_de_uno() {
    let out = run_c_con_pp(
        "int main() {\n\
           int i;\n\
           int n;\n\
           char c;\n\
           n = 0;\n\
           for (i = 0; i < 5; i++) { n = n + 1; }\n\
           c = 'a';\n\
           c++;\n\
           printf(\"%d %d %c\n\", i, n, c);\n\
           return 0;\n\
         }\n",
    );
    assert_eq!(out.trim(), "5 5 b");
}

/// Un puntero a struct: el paso son los 16 bytes del elemento, no 8 ni 1.
/// Es el mismo tamano que ya usa el subindice, y el que fallaba esta manana por
/// el otro camino (`Expr::Add`).
#[test]
fn un_puntero_a_struct_avanza_su_tamano() {
    let out = run_c_con_pp(
        "typedef struct { char *nombre; int v; } item_t;\n\
         item_t lista[3] = { {\"a\", 10}, {\"b\", 20}, {\"c\", 30} };\n\
         int main() {\n\
           item_t *p;\n\
           p = lista;\n\
           p++;\n\
           printf(\"%s %d %d\n\", p->nombre, p->v, (int)(p == &lista[1]));\n\
           return 0;\n\
         }\n",
    );
    assert_eq!(out.trim(), "b 20 1");
}
