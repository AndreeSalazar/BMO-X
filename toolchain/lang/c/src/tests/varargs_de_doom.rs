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
#[ignore = "defecto abierto: PostInc de un puntero no escala -- es la macro va_arg"]
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
#[ignore = "defecto abierto: `*p++` da 11 0 0 en vez de 11 22 33"]
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
