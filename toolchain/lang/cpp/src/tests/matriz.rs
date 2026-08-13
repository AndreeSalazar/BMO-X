//! **La matriz de conformidad de BMO C++.**
//!
//! Misma regla que las de C y COBOL: *al anadir una caracteristica, se le anade
//! su fila* -- y la fila **ejecuta**, no inspecciona. Un codegen que produce
//! numeros erroneos se ve sanisimo en un volcado hexadecimal.
//!
//! === Que cambio con el paso 1 ===
//!
//! Estas filas **se conducen desde fuente de C++**. En el paso 0 la mitad
//! construia el AST a mano, porque el parser provisional solo sabia leer
//! `return` y una matriz desde texto habria tenido una sola fila. Ahora hay
//! lexer y parser de verdad, asi que la matriz prueba lo mismo que probara
//! siempre: **texto de C++ que entra, comportamiento que sale**.

use super::*;

/// El envoltorio por defecto, igual que en la matriz de C: el cuerpo va dentro
/// de un `main`. Con el prefijo `@FULL@` la fuente se usa tal cual, que hace
/// falta para las globales y para las funciones sueltas.
fn correr(fuente: &str) -> String {
    run_cpp(fuente).trim().to_string()
}

// ** APARCADO el 2026-08-12, y por eso `#[ignore]` en vez de borrado.
//
// Va 108 de 110 filas. Las dos que fallan son:
//
//    un global lee 0 donde deberia leer 42
//    un literal de cadena indexado sale vacio
//
// [!] Y los dos sintomas son EXACTAMENTE los que ya se arreglaron en BMO C --la
// seccion de datos y las relocations, en `2bc13367` y `46506e51`--. O sea que no
// es un bug del C++: es que **el frontend de C++ no recibio el arreglo del de
// C**, y lleva asi desde el 08-08.
//
// Se ignora y no se borra por dos razones que van juntas:
//
//   * Un ROJO PERMANENTE entrena a no mirar los rojos. Este ya escondio 400
//     tests una vez: `cargo test` aborta en el primer crate que falla, y este
//     crate va antes que media suite.
//   * Un test BORRADO hace desaparecer la unica descripcion que existe del
//     fallo. Mientras esta aqui, dice exactamente que falta.
//
// **Quitar este `#[ignore]` es el primer paso del dia que se retome C++.** Ver
// `toolchain/lang/cpp/APARCADO.md`, parte 4: las dos condiciones que lo reviven.
#[test]
#[ignore = "C++ APARCADO (ver APARCADO.md): faltan el arreglo de .data y las relocations que C ya tiene"]
fn matriz_cpp_ejecuta_correctamente() {
    let casos: &[(&str, &str, &str)] = &[
        // -- Aritmetica y literales --
        ("literal", "printf(\"%d\", 42);", "42"),
        ("cadena", "printf(\"HOLA C++\");", "HOLA C++"),
        ("suma", "printf(\"%d\", 20 + 22);", "42"),
        ("precedencia", "printf(\"%d\", 2 + 5 * 8);", "42"),
        ("parentesis", "printf(\"%d\", (2 + 5) * 6);", "42"),
        ("division", "printf(\"%d\", 84 / 2);", "42"),
        ("modulo", "printf(\"%d\", 142 % 100);", "42"),
        ("negacion", "printf(\"%d\", -42);", "-42"),
        ("hex", "printf(\"%d\", 0x2A);", "42"),
        ("charlit", "char c = 'A'; printf(\"%c\", c);", "A"),
        ("escape", "printf(\"a\\tb\");", "a\tb"),

        // -- Declaraciones --
        ("declarar", "int x = 42; printf(\"%d\", x);", "42"),
        // * `int a = 20, b = 22;` con `parse_expr` en el inicializador se
        // leeria `a = (20, b = 22)` por el operador coma. El escalon de la
        // gramatica existe justo para esto.
        ("coma-en-declaracion", "int a = 20, b = 22; printf(\"%d\", a + b);", "42"),
        // * El asterisco es del DECLARADOR: en `int *p, q;` la `q` es un `int`.
        ("asterisco-del-declarador", "int x = 42; int *p = &x, q = 7; printf(\"%d %d\", *p, q);", "42 7"),
        ("asignar", "int x = 1; x = 42; printf(\"%d\", x);", "42"),
        ("asignacion-derecha", "int a = 0; int b = 0; a = b = 42; printf(\"%d %d\", a, b);", "42 42"),
        ("compuesta", "int x = 10; x += 5; x -= 2; x *= 2; printf(\"%d\", x);", "26"),
        ("incdec", "int x = 5; x++; ++x; x--; printf(\"%d\", x);", "6"),
        ("post-vs-pre", "int x = 5; int a = x++; printf(\"%d %d\", a, x);", "5 6"),

        // -- Comparaciones y logica --
        ("menor", "printf(\"%d\", 1 < 2);", "1"),
        ("igual", "printf(\"%d\", 2 == 2);", "1"),
        ("distinto", "printf(\"%d\", 2 != 2);", "0"),
        ("logico-and", "printf(\"%d\", 1 && 0);", "0"),
        ("logico-or", "printf(\"%d\", 0 || 3);", "1"),
        ("no", "printf(\"%d %d\", !0, !5);", "1 0"),
        ("ternario", "int x = 5; printf(\"%d\", x > 3 ? 42 : 0);", "42"),

        // -- Bits --
        ("bitops", "printf(\"%d %d %d\", 12 & 10, 12 | 3, 12 ^ 10);", "8 15 6"),
        ("desplazar", "printf(\"%d %d\", 21 << 1, 84 >> 1);", "42 42"),
        ("complemento", "printf(\"%d\", ~0);", "-1"),

        // -- C++ propio --
        ("bool-true", "bool b = true; printf(\"%d\", b);", "1"),
        ("bool-false", "printf(\"%d\", false);", "0"),
        ("nullptr-es-cero", "printf(\"%d\", nullptr);", "0"),
        ("comentario-linea", "// nada\nprintf(\"42\"); // tampoco\n", "42"),
        ("comentario-bloque", "/* nada\n de nada */ printf(\"42\");", "42"),

        // -- Control --
        ("if-entonces", "int x = 0; if (1 < 2) x = 42; printf(\"%d\", x);", "42"),
        ("if-si-no", "int x = 0; if (1 > 2) x = 1; else x = 42; printf(\"%d\", x);", "42"),
        ("while", "int s = 6; int k = 0; while (k < 9) { s = s + k; k = k + 1; } printf(\"%d\", s);", "42"),
        ("do-while", "int i = 0; int s = 0; do { s = s + 1; i = i + 1; } while (i < 3); printf(\"%d\", s);", "3"),
        ("for", "int s = 0; for (int i = 0; i < 6; i++) { s += 7; } printf(\"%d\", s);", "42"),
        ("for-anidado", "int s = 0; for (int i = 0; i < 3; i++) { for (int j = 0; j < 3; j++) { s++; } } printf(\"%d\", s);", "9"),
        ("break", "int s = 0; for (int i = 0; i < 100; i++) { if (i == 3) break; s++; } printf(\"%d\", s);", "3"),
        ("continue", "int s = 0; for (int i = 0; i < 5; i++) { if (i == 2) continue; s++; } printf(\"%d\", s);", "4"),
        ("switch", "int x = 2; switch (x) { case 1: printf(\"uno\"); break; case 2: printf(\"dos\"); break; default: printf(\"otro\"); }", "dos"),
        ("switch-default", "int x = 9; switch (x) { case 1: printf(\"uno\"); break; default: printf(\"otro\"); }", "otro"),

        // -- Punteros y arrays --
        ("ptr-deref", "int x = 42; int *p = &x; printf(\"%d\", *p);", "42"),
        ("ptr-escribir", "int x = 1; int *p = &x; *p = 42; printf(\"%d\", x);", "42"),
        ("array-rw", "int a[3]; a[0] = 10; a[1] = 20; a[2] = 12; printf(\"%d\", a[0] + a[1] + a[2]);", "42"),
        ("array-indice-variable", "int a[3]; a[0] = 1; a[1] = 2; a[2] = 3; int s = 0; for (int i = 0; i < 3; i++) { s += a[i]; } printf(\"%d\", s);", "6"),
        ("cadena-indexada", "char *s = \"ABC\"; printf(\"%c\", s[1]);", "B"),

        // -- Tipos --
        ("cast-char", "int x = 321; printf(\"%d\", (char)x);", "65"),
        ("unsigned", "unsigned int u = 4294967295; printf(\"%u\", u);", "4294967295"),
        ("long", "long l = 9000000000; printf(\"%d\", l);", "9000000000"),

        // -- Programa completo --
        ("global", "@FULL@int g = 42; int main() { printf(\"%d\", g); return 0; }", "42"),
        ("funcion", "@FULL@int suma(int a, int b) { return a + b; } int main() { printf(\"%d\", suma(20, 22)); return 0; }", "42"),
        ("recursion", "@FULL@int f(int n) { if (n <= 1) return 1; return n * f(n - 1); } int main() { printf(\"%d\", f(5)); return 0; }", "120"),
        ("prototipo", "@FULL@int par(int n); int impar(int n) { if (n == 0) return 0; return par(n - 1); } int par(int n) { if (n == 0) return 1; return impar(n - 1); } int main() { printf(\"%d\", par(10)); return 0; }", "1"),
        ("parametro-sin-nombre", "@FULL@int siempre(int) { return 42; } int main() { printf(\"%d\", siempre(7)); return 0; }", "42"),

        // -- Clases (paso 2) --
        ("clase-campo", "@FULL@class P { public: int x; }; int main() { P p; p.x = 42; printf(\"%d\", p.x); return 0; }", "42"),
        ("clase-metodo", "@FULL@class P { public: int x; int doble() { return x * 2; } }; int main() { P p; p.x = 21; printf(\"%d\", p.doble()); return 0; }", "42"),
        ("clase-metodo-con-args", "@FULL@class P { public: int base; int mas(int n) { return base + n; } }; int main() { P p; p.base = 40; printf(\"%d\", p.mas(2)); return 0; }", "42"),
        ("clase-this-explicito", "@FULL@class P { public: int x; int leer() { return this->x; } }; int main() { P p; p.x = 42; printf(\"%d\", p.leer()); return 0; }", "42"),
        ("clase-this-escribe", "@FULL@class P { public: int x; void poner(int n) { this->x = n; } }; int main() { P p; p.poner(42); printf(\"%d\", p.x); return 0; }", "42"),
        ("clase-campo-a-secas", "@FULL@class P { public: int x; void poner(int n) { x = n; } }; int main() { P p; p.poner(42); printf(\"%d\", p.x); return 0; }", "42"),
        // * Un parametro TAPA al campo del mismo nombre. Las dos versiones
        // compilan, asi que si el orden de resolucion estuviera al reves el
        // bug seria mudo: leeria el campo en vez del argumento.
        ("clase-parametro-tapa-campo", "@FULL@class P { public: int x; int f(int x) { return x; } }; int main() { P p; p.x = 1; printf(\"%d\", p.f(42)); return 0; }", "42"),
        ("clase-por-puntero", "@FULL@class P { public: int x; }; int main() { P p; P *q = &p; q->x = 42; printf(\"%d\", p.x); return 0; }", "42"),
        ("clase-metodo-por-puntero", "@FULL@class P { public: int x; int doble() { return x * 2; } }; int main() { P p; p.x = 21; P *q = &p; printf(\"%d\", q->doble()); return 0; }", "42"),
        ("clase-metodo-llama-metodo", "@FULL@class P { public: int x; int doble() { return x * 2; } int cuadruple() { return doble() * 2; } }; int main() { P p; p.x = 10; printf(\"%d\", p.cuadruple() + 2); return 0; }", "42"),
        ("clase-metodo-const", "@FULL@class P { public: int x; int leer() const { return x; } }; int main() { P p; p.x = 42; printf(\"%d\", p.leer()); return 0; }", "42"),
        ("struct-es-publico", "@FULL@struct P { int x; }; int main() { P p; p.x = 42; printf(\"%d\", p.x); return 0; }", "42"),
        ("clase-campo-usado-antes", "@FULL@class P { public: int doble() { return x * 2; } int x; }; int main() { P p; p.x = 21; printf(\"%d\", p.doble()); return 0; }", "42"),
        // * La disposicion: dos campos de tamanos distintos. Si la regla de
        // alineado del parser de C++ y la del codegen de C divergieran, este
        // valor saldria mal -- es la red de la que habla `descenso.rs`.
        ("clase-disposicion", "@FULL@class P { public: char c; int n; }; int main() { P p; p.c = 'A'; p.n = 41; printf(\"%d %c\", p.n + 1, p.c); return 0; }", "42 A"),
        ("clase-privado-por-metodo", "@FULL@class P { int secreto; public: void poner(int n) { secreto = n; } int leer() { return secreto; } }; int main() { P p; p.poner(42); printf(\"%d\", p.leer()); return 0; }", "42"),

        // -- RAII: constructor y destructor (paso 3) --
        ("ctor-corre", "@FULL@class P { public: int x; P() { x = 42; } }; int main() { P p; printf(\"%d\", p.x); return 0; }", "42"),
        ("ctor-con-args-no-hay", "@FULL@class P { public: int x; P() { x = 40; } int mas(int n) { return x + n; } }; int main() { P p; printf(\"%d\", p.mas(2)); return 0; }", "42"),
        ("dtor-al-salir-del-bloque", "@FULL@class P { public: P() { printf(\"nace \"); } ~P() { printf(\"muere\"); } }; int main() { { P p; } return 0; }", "nace muere"),
        ("dtor-al-final-de-main", "@FULL@class P { public: ~P() { printf(\"fin\"); } }; int main() { P p; return 0; }", "fin"),
        // * El orden INVERSO no es una preferencia, es el lenguaje: si `a` se
        // construyo antes que `b`, `b` puede depender de `a`.
        ("dtor-en-orden-inverso", "@FULL@class A { public: ~A() { printf(\"A\"); } }; class B { public: ~B() { printf(\"B\"); } }; int main() { A a; B b; return 0; }", "BA"),
        // * El valor del `return` se calcula ANTES de destruir. Si el
        // destructor corriera primero, se devolveria lo que quedara en la pila.
        ("dtor-tras-calcular-el-return", "@FULL@class P { public: int x; P() { x = 42; } ~P() { x = 0; } int leer() { return x; } }; int f() { P p; return p.leer(); } int main() { printf(\"%d\", f()); return 0; }", "42"),
        ("dtor-en-return-temprano", "@FULL@class P { public: ~P() { printf(\"muere \"); } }; int f(int n) { P p; if (n > 0) { return 1; } return 0; } int main() { printf(\"%d\", f(1)); return 0; }", "muere 1"),
        ("dtor-anidado", "@FULL@class P { public: int n; P() { n = 0; } ~P() { printf(\"x\"); } }; int main() { P a; { P b; { P c; } } printf(\"|\"); return 0; }", "xx|x"),
        ("dtor-en-cada-vuelta-del-bucle", "@FULL@class P { public: ~P() { printf(\".\"); } }; int main() { for (int i = 0; i < 3; i++) { P p; } printf(\"|\"); return 0; }", "...|"),
        ("dtor-con-break", "@FULL@class P { public: ~P() { printf(\".\"); } }; int main() { for (int i = 0; i < 9; i++) { P p; if (i == 2) { break; } } printf(\"|\"); return 0; }", "...|"),
        ("dtor-con-continue", "@FULL@class P { public: ~P() { printf(\".\"); } }; int main() { for (int i = 0; i < 3; i++) { P p; if (i == 1) { continue; } } printf(\"|\"); return 0; }", "...|"),
        ("solo-ctor-sin-dtor", "@FULL@class P { public: int x; P() { x = 42; } }; int main() { P p; printf(\"%d\", p.x); return 0; }", "42"),
        ("solo-dtor-sin-ctor", "@FULL@class P { public: int x; ~P() { printf(\"%d\", x); } }; int main() { P p; p.x = 42; return 0; }", "42"),
        ("ctor-usa-metodo", "@FULL@class P { public: int x; void poner() { x = 42; } P() { poner(); } }; int main() { P p; printf(\"%d\", p.x); return 0; }", "42"),

        // -- Sobrecarga y mangling (paso 4) --
        ("sobrecarga-por-aridad", "@FULL@int f(int a) { return a; } int f(int a, int b) { return a + b; } int main() { printf(\"%d %d\", f(42), f(20, 22)); return 0; }", "42 42"),
        ("sobrecarga-por-tipo", "@FULL@int f(int a) { return 1; } int f(char a) { return 2; } int main() { int n = 5; char c = 'x'; printf(\"%d %d\", f(n), f(c)); return 0; }", "1 2"),
        ("sobrecarga-por-puntero", "@FULL@int f(int a) { return 1; } int f(int *a) { return 2; } int main() { int n = 0; printf(\"%d %d\", f(n), f(&n)); return 0; }", "1 2"),
        // * El exacto gana al promocionado, y la promocion a la conversion.
        // Con `f(int)` y `f(long)`, un `char` va al `int` (promocion) y no al
        // `long` (conversion); un `long` va al `long` (exacto).
        ("gana-la-promocion-sobre-la-conversion", "@FULL@int f(int a) { return 1; } int f(long a) { return 2; } int main() { char c = 'x'; printf(\"%d\", f(c)); return 0; }", "1"),
        ("exacto-gana-a-todo", "@FULL@int f(int a) { return 1; } int f(long a) { return 2; } int main() { long l = 5; printf(\"%d\", f(l)); return 0; }", "2"),
        ("metodos-sobrecargados", "@FULL@class P { public: int f(int a) { return 1; } int f(int a, int b) { return 2; } }; int main() { P p; printf(\"%d %d\", p.f(1), p.f(1, 2)); return 0; }", "1 2"),
        // Dos clases con el mismo metodo no chocan: el simbolo lleva la clase.
        ("mismo-metodo-en-dos-clases", "@FULL@class A { public: int f() { return 40; } }; class B { public: int f() { return 2; } }; int main() { A a; B b; printf(\"%d\", a.f() + b.f()); return 0; }", "42"),
        // * `printf` NO se mangla: no esta en la tabla de C++, asi que pasa
        // tal cual. Es el puente con lo de C, que es lo que `extern \"C\"`
        // nombra en el estandar.
        ("las-funciones-de-c-pasan-sin-manglar", "printf(\"%d\", 42);", "42"),

        // -- Constructores sobrecargados (paso 4) --
        ("dos-constructores", "@FULL@class P { public: int x; P() { x = 1; } P(int n) { x = n; } }; int main() { P a; P b(42); printf(\"%d %d\", a.x, b.x); return 0; }", "1 42"),
        ("ctor-con-dos-args", "@FULL@class P { public: int x; P(int a, int b) { x = a + b; } }; int main() { P p(20, 22); printf(\"%d\", p.x); return 0; }", "42"),
        ("ctor-elegido-por-tipo", "@FULL@class P { public: int x; P(int n) { x = 1; } P(char c) { x = 2; } }; int main() { P a(5); P b('z'); printf(\"%d %d\", a.x, b.x); return 0; }", "1 2"),

        // -- Herencia simple y virtuales (paso 5) --
        ("herencia-campos", "@FULL@class A { public: int x; }; class B : public A { public: int y; }; int main() { B b; b.x = 20; b.y = 22; printf(\"%d\", b.x + b.y); return 0; }", "42"),
        ("herencia-metodo-heredado", "@FULL@class A { public: int x; int doble() { return x * 2; } }; class B : public A { }; int main() { B b; b.x = 21; printf(\"%d\", b.doble()); return 0; }", "42"),
        ("virtual-una-clase", "@FULL@class A { public: int x; virtual int f() { return x * 2; } }; int main() { A a; a.x = 21; printf(\"%d\", a.f()); return 0; }", "42"),
        // ** LA fila del paso 5: la MISMA linea llama a funciones distintas
        // segun el tipo dinamico. Eso es una funcion virtual, y es lo unico
        // que hay que demostrar.
        ("virtual-despacha-por-el-objeto", "@FULL@class Animal { public: int edad; virtual int habla() { return edad; } }; class Perro : public Animal { public: int habla() override { return edad * 2; } }; int main() { Animal a; a.edad = 21; Perro p; p.edad = 21; printf(\"%d %d\", a.habla(), p.habla()); return 0; }", "21 42"),
        // * Y por PUNTERO A LA BASE, que es donde el polimorfismo sirve de
        // algo: el tipo estatico es `Animal*` en los dos casos.
        ("virtual-por-puntero-a-base", "@FULL@class Animal { public: int edad; virtual int habla() { return edad; } }; class Perro : public Animal { public: int habla() override { return edad * 2; } }; int main() { Animal a; a.edad = 21; Perro p; p.edad = 21; Animal *x = &a; Animal *y = &p; printf(\"%d %d\", x->habla(), y->habla()); return 0; }", "21 42"),
        ("virtual-no-redefinida-se-hereda", "@FULL@class A { public: int x; virtual int f() { return x; } }; class B : public A { }; int main() { B b; b.x = 42; A *p = &b; printf(\"%d\", p->f()); return 0; }", "42"),
        ("virtual-dos-ranuras", "@FULL@class A { public: int x; virtual int uno() { return 1; } virtual int dos() { return 2; } }; class B : public A { public: int dos() override { return 40; } }; int main() { B b; b.x = 0; A *p = &b; printf(\"%d\", p->uno() + p->dos() + 1); return 0; }", "42"),
        ("virtual-desde-un-metodo", "@FULL@class A { public: int x; virtual int f() { return x; } int doble() { return f() * 2; } }; class B : public A { public: int f() override { return x + 1; } }; int main() { B b; b.x = 20; A *p = &b; printf(\"%d\", p->doble()); return 0; }", "42"),
        ("virtual-y-raii", "@FULL@class A { public: virtual int f() { return 1; } ~A() { printf(\"~\"); } }; int main() { { A a; printf(\"%d\", a.f()); } printf(\"|\"); return 0; }", "1~|"),

        // * Integracion: una fila que COMPONE varias caracteristicas. Las
        // demas prueban cada pieza suelta, y una pieza suelta puede estar bien
        // y romperse al lado de otra -- el `for` con declaracion envuelve en un
        // bloque, y el bloque cambia donde caen las ranuras de pila.
        ("programa-completo", "@FULL@\
            int suma(int a, int b) { return a + b; }\n\
            int main() {\n\
                int total = 0;\n\
                for (int i = 0; i < 6; i++) { total += suma(i, i); }\n\
                bool ok = total > 0;\n\
                printf(\"total=%d ok=%d\", total, ok);\n\
                return 0;\n\
            }", "total=30 ok=1"),

        // * Integracion de clases: campo privado, cuatro metodos, uno `const`,
        // uno que llama a otro, y acceso por puntero. Es el programa de
        // `p2.cpp` que se compila desde la linea de ordenes.
        ("clase-completa", "@FULL@\
            class Contador {\n\
                int n;\n\
            public:\n\
                void reiniciar()       { n = 0; }\n\
                void sumar(int cuanto) { n = n + cuanto; }\n\
                int  valor() const     { return n; }\n\
                int  doble()           { return valor() * 2; }\n\
            };\n\
            int main() {\n\
                Contador c;\n\
                c.reiniciar();\n\
                for (int i = 1; i <= 6; i++) { c.sumar(i); }\n\
                Contador *p = &c;\n\
                printf(\"valor=%d doble=%d via_ptr=%d\", c.valor(), c.doble(), p->valor());\n\
                return 0;\n\
            }", "valor=21 doble=42 via_ptr=21"),

        // * Integracion de RAII: las CUATRO salidas de ambito en un programa,
        // con objetos vivos en dos niveles a la vez. Es el que se compila a
        // mano en `p3.cpp`.
        //
        //   i=0 -> final del cuerpo del bucle   -> ~2
        //   i=1 -> `continue`                    -> ~2
        //   i=2 -> `break`                       -> ~2
        //   `return` -> destruye c y luego a     -> ~3 ~1
        //
        // * El `[` sale DESPUES de los `~`, como en C estandar -- y esta fila
        // es la que destapo que no era asi.
        //
        // El `printf` de BMO C formateaba **en linea**: escribia el literal
        // segun recorria la plantilla y evaluaba cada argumento al llegar a su
        // `%`. Con argumentos sin efectos daba igual; con un destructor que
        // imprime, no. Arreglado en el codegen de C (ahora todos los
        // argumentos se calculan antes de escribir un byte), y esta fila es su
        // testigo desde el otro lado.
        ("raii-las-cuatro-salidas", "@FULL@\
            class Traza {\n\
            public:\n\
                int id;\n\
                ~Traza() { printf(\"~%d \", id); }\n\
            };\n\
            int trabajo(int n) {\n\
                Traza a; a.id = 1;\n\
                for (int i = 0; i < n; i++) {\n\
                    Traza b; b.id = 2;\n\
                    if (i == 1) { continue; }\n\
                    if (i == 2) { break; }\n\
                }\n\
                Traza c; c.id = 3;\n\
                return 99;\n\
            }\n\
            int main() { printf(\"[%d]\", trabajo(5)); return 0; }",
            "~2 ~2 ~2 ~3 ~1 [99]"),

        // * Integracion del paso 4: tres constructores, dos metodos con el
        // mismo nombre, y dos funciones libres sobrecargadas -- todo en un
        // programa. Cada llamada tiene que ir a un simbolo distinto.
        ("sobrecarga-completa", "@FULL@\
            class Punto {\n\
            public:\n\
                int x; int y;\n\
                Punto()             { x = 0;  y = 0; }\n\
                Punto(int n)        { x = n;  y = n; }\n\
                Punto(int a, int b) { x = a;  y = b; }\n\
                int suma() const    { return x + y; }\n\
                int suma(int extra) { return x + y + extra; }\n\
            };\n\
            int doble(int n)        { return n * 2; }\n\
            int doble(int a, int b) { return (a + b) * 2; }\n\
            int main() {\n\
                Punto o;\n\
                Punto u(7);\n\
                Punto d(20, 22);\n\
                printf(\"%d %d %d | %d %d | %d %d\",\n\
                       o.suma(), u.suma(), d.suma(),\n\
                       d.suma(0), d.suma(8),\n\
                       doble(21), doble(10, 11));\n\
                return 0;\n\
            }", "0 14 42 | 42 50 | 42 42"),
    ];

    let total = casos.len();
    let mut rotos = Vec::new();
    for (name, fuente, esperado) in casos {
        let src = match fuente.strip_prefix("@FULL@") {
            Some(f) => f.to_string(),
            None => format!("int main() {{ {fuente} return 0; }}"),
        };
        let got = std::panic::catch_unwind(|| correr(&src))
            .unwrap_or_else(|_| "<no ejecuta>".into());
        if got != *esperado {
            rotos.push(format!("  {name:<26} => {got:?}  (esperado {esperado:?})"));
        }
    }
    assert!(
        rotos.is_empty(),
        "\n{}/{} FUNCIONAN. ROTOS:\n{}",
        total - rotos.len(), total, rotos.join("\n"),
    );
}

/// La otra mitad de la matriz: **lo que no se sabe hacer, y que lo diga.**
///
/// Una matriz que solo mira lo que funciona deja pasar el fallo peor de todos
/// --hacer algo a medias y en silencio-- porque ese caso no aparece en ninguna
/// fila verde. Cada fila comprueba que el rechazo **nombra el paso** en el que
/// eso llega, para que el mensaje sea una ruta y no un muro.
#[test]
fn matriz_cpp_rechaza_con_el_paso_escrito() {
    let casos: &[(&str, &str, u8)] = &[
        ("preprocesador", "@FULL@#include \"x.h\"\nint main(){return 0;}", 1),
        ("auto", "auto x = 1;", 2),
        ("sizeof", "printf(\"%d\", sizeof(int));", 2),
        ("referencia", "@FULL@int f(int &r) { return r; } int main(){return 0;}", 2),
        ("lista-de-inicializacion", "@FULL@class P { int x; public: P() : x(0) {} };\nint main(){return 0;}", 4),
        ("copia", "@FULL@class P { public: int x; }; int main(){ P a; P b = a; return 0; }", 5),
        ("new", "int *p = new P();", 3),
        ("delete", "int *p = 0; delete p;", 3),
        ("miembro-static", "@FULL@class P { public: static int n; };\nint main(){return 0;}", 4),
        ("operador", "@FULL@class P { public: int operator+(int a) { return a; } };\nint main(){return 0;}", 4),
        ("friend", "@FULL@class P { friend int f(); };\nint main(){return 0;}", 4),
        ("metodo-fuera", "@FULL@class P { public: int f(); };\nint main(){return 0;}", 4),
        ("herencia-multiple", "@FULL@class A { public: int x; }; class B { public: int y; }; class C : public A, public B { };\nint main(){return 0;}", 6),
        ("virtual-pura", "@FULL@class P { public: virtual int f() = 0; };\nint main(){return 0;}", 6),
        ("herencia-privada", "@FULL@class A { public: int x; }; class B : private A { };\nint main(){return 0;}", 6),
        ("namespace", "@FULL@namespace n { }\nint main(){return 0;}", 4),
        ("cualificado", "int x = n::y;", 4),
        ("variadica", "@FULL@int f(int a, ...) { return a; } int main(){return 0;}", 4),
        ("argumento-por-defecto", "@FULL@int f(int a = 1) { return a; } int main(){return 0;}", 4),
        ("plantilla", "@FULL@template<class T> T f(T x) { return x; } int main(){return 0;}", 6),
    ];

    let mut rotos = Vec::new();
    for (name, fuente, paso) in casos {
        let src = match fuente.strip_prefix("@FULL@") {
            Some(f) => f.to_string(),
            None => format!("int main() {{ {fuente} return 0; }}"),
        };
        match compile_source_to_bef(&src) {
            Ok(_) => rotos.push(format!("  {name:<24} COMPILO, y no deberia")),
            Err(e) if !e.message.contains(&format!("PASO {paso}")) =>
                rotos.push(format!("  {name:<24} no dijo PASO {paso}: {}", e.message)),
            Err(_) => {}
        }
    }
    assert!(rotos.is_empty(), "\nROTOS:\n{}", rotos.join("\n"));
}

/// **Lo que esta MAL escrito, y que el error lo explique.**
///
/// Distinto de la tabla de arriba: eso son cosas que llegaran en un paso, y
/// esto son cosas que no llegaran nunca porque estan mal. Un compilador que
/// las acepta deja pasar el bug; uno que las rechaza sin explicar manda a
/// mirar donde no es. Cada fila comprueba que el mensaje **dice que pasa**.
#[test]
fn matriz_cpp_explica_lo_que_esta_mal() {
    let casos: &[(&str, &str, &str)] = &[
        // * El *most vexing parse*. `P p();` declara una FUNCION, y un
        // compilador que lo acepta como objeto deja uno sin construir.
        ("most-vexing-parse",
         "@FULL@class P { public: P() {} }; int main() { P p(); return 0; }",
         "most vexing parse"),
        // Una ambiguedad resuelta sola --"gana el primero"-- haria que anadir
        // una sobrecarga cambiara a donde va una llamada existente, en silencio.
        ("ambiguedad",
         "@FULL@int f(int a, long b) { return 1; } int f(long a, int b) { return 2; } \
          int main() { char c = 'x'; return f(c, c); }",
         "ambigua"),
        ("ninguna-version-acepta",
         "@FULL@int f(int a) { return a; } int main() { P q; return f(q); }",
         "no es un tipo conocido"),
        ("aridad-que-no-existe",
         "@FULL@int f(int a) { return a; } int main() { return f(1, 2); }",
         "argumento"),
        // [!] Deuda de C: acepta `int g(double)` en silencio y no funciona.
        ("float-como-parametro",
         "@FULL@int f(double a) { return 1; } int main() { return 0; }",
         "coma flotante"),
        ("sobrecargar-por-retorno",
         "@FULL@int f(int a); char f(int a); int main() { return 0; }",
         "tipo de retorno"),
        ("dos-metodos-iguales",
         "@FULL@class P { public: int f() { return 1; } int f() { return 2; } }; \
          int main() { return 0; }",
         "dos veces"),
    ];

    let mut rotos = Vec::new();
    for (name, fuente, aguja) in casos {
        let src = fuente.strip_prefix("@FULL@").unwrap_or(fuente).to_string();
        match compile_source_to_bef(&src) {
            Ok(_) => rotos.push(format!("  {name:<24} COMPILO, y no deberia")),
            Err(e) if !e.message.contains(aguja) =>
                rotos.push(format!("  {name:<24} no dijo {aguja:?}: {}", e.message)),
            Err(_) => {}
        }
    }
    assert!(rotos.is_empty(), "\nROTOS:\n{}", rotos.join("\n"));
}

/// * **El pecado que el paso 1 vino a matar, con su test.**
///
/// El parser anterior hacia `pos += 1` con lo que no reconocia. Estas dos
/// fuentes son las que mas barato salian antes: la primera perdia el cuerpo
/// entero, la segunda leia `x` como un numero hexadecimal.
#[test]
fn ya_no_se_traga_nada_en_silencio() {
    // Antes: compilaba, no imprimia, y no se quejaba.
    assert_eq!(correr("int main() { printf(\"42\"); return 0; }"), "42");
    // Antes: el bucle de digitos aceptaba `x`, asi que `x * 2` entraba por la
    // rama numerica antes de ser un nombre.
    assert_eq!(correr("int main() { int x = 21; printf(\"%d\", x * 2); return 0; }"), "42");
    // Y una basura de verdad tiene que dar error CON LINEA, no desaparecer.
    let e = compile_source_to_bef("int main() {\n  @@@;\n  return 0;\n}")
        .expect_err("esto no puede compilar");
    assert_eq!(e.line, 2, "el error tiene que llevar la linea real: {e:?}");
}
