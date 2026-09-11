//! **UN FLOTANTE QUE NO ESTA EN UNA VARIABLE SUELTA.** `*p`, `t[i]`, `v.f`, `p->f`.
//!
//! # De donde salen estas filas
//!
//! El dueno pidio mirar el UB de C --*"mi sistema NO TIENE que adivinar"*-- y al
//! barrer las doce formas clasicas de comportamiento indefinido aparecio, en la
//! fila de control, **algo que no es UB**: C perfectamente ordinario dando un
//! numero equivocado.
//!
//! ```text
//!    float f; f = 3.5;  (int)f    ->  3            bien
//!    float *p = &f;     (int)*p   ->  1080033280   los BITS de 3.5
//! ```
//!
//! ** Y la fila de control existia para eso: para que si TODO sale mal se sepa
//! que el problema no es lo que se estaba mirando.
//!
//! # Eran DOS agujeros, y el segundo mataba al compilador
//!
//! ```text
//!    LEER     `expr_is_float` no tenia brazo para `*p` ni para `t[i]`, asi que
//!             se iban por el camino ENTERO: `mov eax,[rax]`, los bits crudos
//!
//!    ESCRIBIR ninguno de los cinco destinos --subindice, campo, flecha,
//!             indireccion, indice de puntero-- tenia ruta SSE. Guardaban CERO
//! ```
//!
//! *** Y `v.f` no daba un numero malo: **desbordaba la pila del compilador**.
//! `expr_is_float` decia que SI para `Field`, `Arrow` e `IndexPtr`, `emit_fexpr`
//! no tenia brazo para ellos, caia en el comodin `_ =>`, que llama a
//! `emit_fexpr_operand`, que pregunta `expr_is_float`, que dice que si, que
//! vuelve a llamar a `emit_fexpr`. **Un `struct` con un campo `float` no
//! compilaba: mataba al compilador.**
//!
//! ** Y EL AVISO ESTABA ESCRITO EN EL PROPIO FICHERO, veinte lineas mas arriba,
//! del dia de los intrinsecos:
//!
//! > *"Sin este brazo caia en el `_ =>` del final, que llama a
//! > `emit_fexpr_operand`, que pregunta `expr_is_float`, que ahora dice que SI,
//! > que vuelve a llamar aqui: la pila se desborda antes de emitir un byte. El
//! > comodin no se equivocaba de respuesta, se equivocaba de pregunta."*
//!
//!   > Una nota que explica un fallo y no lo cierra es una nota que describe el
//!   > fallo siguiente.
//!
//! # Por que estas nueve filas y no una
//!
//! Porque el fallo no era "los flotantes": era **cada FORMA de llegar a uno**.
//! Una sola fila con `*p` habria pasado a verde dejando `v.f` matando al
//! compilador. Aqui esta cada camino por separado, leyendo y escribiendo.
//!
//! [!] Y `local` y `double *p` son las filas de CONTROL. Si un dia se ponen
//! rojas, lo que se rompio no es esto: es la coma flotante entera.

use super::*;

fn cuadra(nombre: &str, espera: &str, fuente: &str) {
    let bef = compile_source_to_bef(fuente).expect("tiene que compilar");
    let sale = ejecutar_bef(&bef);
    assert_eq!(sale.trim_end(), espera, "{nombre}");
}

/// La fila de CONTROL: un flotante en una variable suelta. Llevaba bien desde
/// siempre, y por eso el fallo no se veia.
#[test]
fn un_flotante_en_una_variable_suelta() {
    cuadra("local", "3",
        "int main(){ float f; f = 3.5; printf(\"%d\", (int)f); return 0; }");
    cuadra("double local", "3",
        "int main(){ double d; d = 3.5; printf(\"%d\", (int)d); return 0; }");
}

/// *** POR PUNTERO. Daba `1080033280` -- el patron IEEE de 3.5 leido como int.
#[test]
fn un_flotante_leido_por_puntero() {
    cuadra("*p", "3",
        "int main(){ float f; float *p; f = 3.5; p = &f; \
         printf(\"%d\", (int)(*p)); return 0; }");
    cuadra("double *p", "3",
        "int main(){ double d; double *p; d = 3.5; p = &d; \
         printf(\"%d\", (int)(*p)); return 0; }");
    cuadra("p[0]", "3",
        "int main(){ float f; float *p; f = 3.5; p = &f; \
         printf(\"%d\", (int)p[0]); return 0; }");
}

/// *** EN UNA TABLA. La escritura guardaba CERO, asi que se leian todos a cero.
#[test]
fn un_flotante_en_una_tabla() {
    cuadra("t[i] global", "3",
        "float t[4]; int main(){ t[0] = 3.5; printf(\"%d\", (int)t[0]); return 0; }");
    cuadra("t[i] local", "3",
        "int main(){ float t[4]; t[0] = 3.5; printf(\"%d\", (int)t[0]); return 0; }");
    // Y que el de al lado NO se pise: el store tiene que ser de 4 bytes, no de 8.
    cuadra("t[0] y t[1] no se pisan", "3,7",
        "float t[4]; int main(){ t[0] = 3.5; t[1] = 7.5; \
         printf(\"%d,%d\", (int)t[0], (int)t[1]); return 0; }");
}

/// *** EN UN STRUCT. Esta era la que **desbordaba la pila del compilador**.
#[test]
fn un_flotante_dentro_de_un_struct() {
    cuadra("v.f", "3",
        "struct s { float f; }; int main(){ struct s v; v.f = 3.5; \
         printf(\"%d\", (int)v.f); return 0; }");
    cuadra("p->f", "3",
        "struct s { float f; }; int main(){ struct s v; struct s *p; v.f = 3.5; \
         p = &v; printf(\"%d\", (int)p->f); return 0; }");
    // Un campo entero al lado de uno flotante: ni el store ni el load se salen.
    cuadra("el campo de al lado", "3,9",
        "struct s { float f; int n; }; int main(){ struct s v; v.f = 3.5; v.n = 9; \
         printf(\"%d,%d\", (int)v.f, v.n); return 0; }");
}

/// Y la aritmetica, que es para lo que sirve todo lo anterior.
#[test]
fn cuentas_con_flotantes_que_no_son_variables() {
    cuadra("suma por puntero", "7",
        "int main(){ float a; float b; float *pa; float *pb; a = 3.5; b = 4.0; \
         pa = &a; pb = &b; printf(\"%d\", (int)(*pa + *pb)); return 0; }");
    cuadra("acumular en una tabla", "10",
        "float t[4]; int main(){ int i; t[0] = 1.5; t[1] = 2.5; t[2] = 3.0; t[3] = 3.0; \
         { float s; s = 0.0; for (i = 0; i < 4; i = i + 1) { s = s + t[i]; } \
         printf(\"%d\", (int)s); } return 0; }");
    cuadra("escribir por puntero", "8",
        "int main(){ float f; float *p; f = 0.0; p = &f; *p = 8.5; \
         printf(\"%d\", (int)f); return 0; }");
}
