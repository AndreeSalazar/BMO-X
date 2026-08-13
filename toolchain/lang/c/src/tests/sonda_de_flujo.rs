//! # LA SONDA DEL FLUJO -- switch, goto y recursion
//!
//! ## El eje, y por que este y no otro
//!
//! Contado sobre el arbol de DOOM: **85 `switch` y 20 `goto`** solo en
//! `p_*.c` y `r_*.c`, y `R_RenderBSPNode` es **recursiva con dos llamadas por
//! nodo**. O sea que el playsim y el renderizador no son aritmetica con un
//! bucle: son un grafo de saltos.
//!
//! Y es un eje distinto de los anteriores porque aqui no hay valores mal: hay
//! **caminos**. Un `continue` que salte al sitio equivocado da un programa que
//! calcula bien y hace otra cosa.
//!
//! ## Las filas que son casos de esquina de verdad
//!
//! Tres de las dieciseis no son "comprobar que funciona", son trampas conocidas
//! de C que un emisor escrito de memoria falla:
//!
//! - **`continue` dentro de un `switch`** pertenece al BUCLE, no al switch.
//! - **`continue` en un `do..while`** salta a la CONDICION, no al principio del
//!   cuerpo. Es el que casi nadie emite bien.
//! - **`while (0)` da cero vueltas y `do..while(0)` da una.** Emitir los dos
//!   con la misma plantilla es el fallo natural, y no se nota hasta que la
//!   condicion es falsa de entrada.
//!
//! ## ** Resultado: limpio, las 16
//!
//! Incluida la recursion de dos ramas con locales propias, que es la forma
//! exacta de `R_RenderBSPNode`. Cuando el BSP falle en metal, **el sospechoso
//! no es el flujo**.

use super::censo::{barrer, Casilla};

fn censo() -> Vec<Casilla> {
    vec![
        Casilla {
            nombre: "switch, tres casos y default",
            fuente: "int f(int x) { switch (x) { case 1: return 10; case 2: return 20; \
                       default: return 99; } return -1; }\n\
                     int main() { printf(\"%d %d %d\\n\", f(1), f(2), f(7)); return 0; }",
            espera: "10 20 99",
        },
        Casilla {
            // ** El fallthrough: sin `break` se cae al caso de abajo. DOOM lo
            // usa a proposito en `P_SpecialThing` y en el menu.
            nombre: "switch sin break se cae",
            fuente: "int main() { int n; int x; n = 0; x = 1; \
                       switch (x) { case 1: n = n + 1; case 2: n = n + 10; break; \
                         case 3: n = n + 100; } \
                       printf(\"%d\\n\", n); return 0; }",
            espera: "11",
        },
        Casilla {
            nombre: "switch con default en medio",
            fuente: "int f(int x) { int r; r = 0; \
                       switch (x) { case 1: r = 1; break; default: r = 9; break; \
                         case 2: r = 2; break; } return r; }\n\
                     int main() { printf(\"%d %d %d\\n\", f(1), f(2), f(5)); return 0; }",
            espera: "1 2 9",
        },
        Casilla {
            nombre: "switch con valores dispersos",
            fuente: "int f(int x) { switch (x) { case 0: return 1; case 100: return 2; \
                       case -5: return 3; case 1000000: return 4; } return 0; }\n\
                     int main() { printf(\"%d %d %d %d %d\\n\", f(0), f(100), f(-5), \
                       f(1000000), f(7)); return 0; }",
            espera: "1 2 3 4 0",
        },
        Casilla {
            // ** `continue` DENTRO de un switch: el `continue` es del BUCLE, no
            // del switch. Confundirlos es un clasico.
            nombre: "continue dentro de un switch",
            fuente: "int main() { int i; int n; n = 0; \
                       for (i = 0; i < 5; i++) { \
                         switch (i) { case 2: continue; default: break; } \
                         n = n + 1; } \
                       printf(\"%d\\n\", n); return 0; }",
            espera: "4",
        },
        Casilla {
            // ** `continue` en un `do..while` va a la CONDICION, no al principio
            // del cuerpo. Es el caso de esquina que casi nadie emite bien.
            nombre: "continue en do..while",
            fuente: "int main() { int i; int n; i = 0; n = 0; \
                       do { i = i + 1; if (i == 2) { continue; } n = n + 1; } \
                       while (i < 4); \
                       printf(\"%d %d\\n\", i, n); return 0; }",
            espera: "4 3",
        },
        Casilla {
            nombre: "break sale de UN bucle, no dos",
            fuente: "int main() { int i; int j; int n; n = 0; \
                       for (i = 0; i < 3; i++) { for (j = 0; j < 3; j++) { \
                         if (j == 1) { break; } n = n + 1; } } \
                       printf(\"%d\\n\", n); return 0; }",
            espera: "3",
        },
        Casilla {
            // El uso real de `goto` en C: salir de dos bucles de golpe.
            nombre: "goto sale de dos bucles",
            fuente: "int main() { int i; int j; int n; n = 0; \
                       for (i = 0; i < 4; i++) { for (j = 0; j < 4; j++) { \
                         n = n + 1; if (n == 5) { goto fuera; } } } \
                       fuera: printf(\"%d\\n\", n); return 0; }",
            espera: "5",
        },
        Casilla {
            nombre: "goto hacia atras es un bucle",
            fuente: "int main() { int n; n = 0; \
                       otra: n = n + 1; if (n < 6) { goto otra; } \
                       printf(\"%d\\n\", n); return 0; }",
            espera: "6",
        },
        Casilla {
            // ** `R_RenderBSPNode` es recursiva y se llama dos veces por nodo.
            nombre: "recursion: factorial",
            fuente: "int fact(int n) { if (n <= 1) { return 1; } return n * fact(n - 1); }\n\
                     int main() { printf(\"%d\\n\", fact(10)); return 0; }",
            espera: "3628800",
        },
        Casilla {
            // La forma exacta del BSP: dos llamadas recursivas por nivel, y el
            // resultado depende de las dos.
            nombre: "recursion de DOS ramas (el BSP)",
            fuente: "int arbol(int n) { if (n <= 0) { return 1; } \
                       return arbol(n - 1) + arbol(n - 1); }\n\
                     int main() { printf(\"%d\\n\", arbol(10)); return 0; }",
            espera: "1024",
        },
        Casilla {
            // Profundidad de verdad: el BSP de E1M1 baja unos 20 niveles, pero
            // lo que se prueba es que el marco de pila no se pise a si mismo.
            nombre: "recursion profunda (200)",
            fuente: "int baja(int n) { if (n == 0) { return 0; } return 1 + baja(n - 1); }\n\
                     int main() { printf(\"%d\\n\", baja(200)); return 0; }",
            espera: "200",
        },
        Casilla {
            // Con LOCALES dentro, que es lo que de verdad ejercita el marco:
            // cada nivel tiene que tener las suyas.
            nombre: "recursion con locales propias",
            fuente: "int suma(int n) { int mio; if (n == 0) { return 0; } \
                       mio = n * 2; return mio + suma(n - 1); }\n\
                     int main() { printf(\"%d\\n\", suma(100)); return 0; }",
            espera: "10100",
        },
        Casilla {
            nombre: "recursion mutua",
            fuente: "int impar(int n);\n\
                     int par(int n) { if (n == 0) { return 1; } return impar(n - 1); }\n\
                     int impar(int n) { if (n == 0) { return 0; } return par(n - 1); }\n\
                     int main() { printf(\"%d %d\\n\", par(10), par(7)); return 0; }",
            espera: "1 0",
        },
        Casilla {
            nombre: "return desde dentro de un switch",
            fuente: "int f(int x) { int i; \
                       for (i = 0; i < 10; i++) { switch (i) { case 3: \
                         if (x) { return i * 100; } break; } } return -1; }\n\
                     int main() { printf(\"%d %d\\n\", f(1), f(0)); return 0; }",
            espera: "300 -1",
        },
        Casilla {
            // `while` con la condicion falsa de entrada: CERO vueltas. Y su
            // gemelo `do..while`, que da UNA. Emitir los dos igual es el fallo.
            nombre: "while cero vueltas, do..while una",
            fuente: "int main() { int a; int b; a = 0; b = 0; \
                       while (0) { a = a + 1; } \
                       do { b = b + 1; } while (0); \
                       printf(\"%d %d\\n\", a, b); return 0; }",
            espera: "0 1",
        },
    ]
}

#[test]
fn el_censo_del_flujo_no_ha_cambiado() {
    barrer(
        &censo(),
        CENSO,
        "EL CENSO DEL FLUJO CAMBIO.\n\
         Este eje estaba limpio entero, asi que un ROTO aqui es una REGRESION.\n\
         Si cae una de recursion, mirar el marco de pila (`codegen/marco.rs`);\n\
         si cae una de `switch` o `goto`, el enlazado (`codegen/enlazado.rs`).",
    );
}

/// **EL CENSO DEL FLUJO, al 2026-08-13.** Verde entero desde el primer barrido.
const CENSO: &str = "\
switch, tres casos y default   BIEN
switch sin break se cae        BIEN
switch con default en medio    BIEN
switch con valores dispersos   BIEN
continue dentro de un switch   BIEN
continue en do..while          BIEN
break sale de UN bucle, no dos BIEN
goto sale de dos bucles        BIEN
goto hacia atras es un bucle   BIEN
recursion: factorial           BIEN
recursion de DOS ramas (el BSP) BIEN
recursion profunda (200)       BIEN
recursion con locales propias  BIEN
recursion mutua                BIEN
return desde dentro de un switch BIEN
while cero vueltas, do..while una BIEN
";
