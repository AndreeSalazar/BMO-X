//! **`static` y los prototipos** — las dos que separaban "compila programas" de
//! "compila un programa de cincuenta ficheros".
//!
//! ★ Estos tests EJECUTAN. Que una construcción compile es media prueba y la
//! menos interesante: lo que define a `static` no es que el parser la acepte,
//! es que la variable **sobreviva a la llamada** y que su inicializador corra
//! **una sola vez**. Un compilador que la acepta y la trata como una local
//! normal pasa cualquier prueba de compilación y falla en la primera vuelta de
//! un contador — en silencio, dando siempre el mismo número.

use super::*;

// ── static: lo que la hace distinta de una local ──────────────────────

/// ★ La propiedad que define `static`: **sobrevive entre llamadas**. Es la
/// única razón de que exista, y la única que un test de compilación no ve.
#[test]
fn una_static_local_conserva_su_valor_entre_llamadas() {
    let fuente = "int cuenta() { static int n = 0; n = n + 1; return n; } \
                  int main() { printf(\"%d\", cuenta()); \
                               printf(\"%d\", cuenta()); \
                               printf(\"%d\", cuenta()); return 0; }";
    assert_eq!(run_c(fuente), "123");
}

/// ★ Y su inicializador corre **UNA vez**, no en cada llamada.
///
/// Es el mismo test del revés y hace falta: si el inicializador se emitiera
/// como una asignación dentro del cuerpo, el contador de arriba también daría
/// `111` — y "no cuenta" y "se reinicia" son dos bugs distintos con la misma
/// cara.
#[test]
fn el_inicializador_de_una_static_corre_una_sola_vez() {
    let fuente = "int suma() { static int total = 100; total = total + 1; return total; } \
                  int main() { suma(); suma(); printf(\"%d\", suma()); return 0; }";
    assert_eq!(run_c(fuente), "103");
}

/// Dos funciones pueden tener cada una **su** `static int n` sin pisarse. Es
/// lo que obliga a renombrar en vez de meterlas todas en el mismo saco de
/// globales.
#[test]
fn dos_funciones_pueden_tener_cada_una_su_static_con_el_mismo_nombre() {
    let fuente = "int a() { static int n = 10; n = n + 1; return n; } \
                  int b() { static int n = 20; n = n + 1; return n; } \
                  int main() { printf(\"%d,\", a()); printf(\"%d,\", b()); \
                               printf(\"%d,\", a()); printf(\"%d\", b()); return 0; }";
    assert_eq!(run_c(fuente), "11,21,12,22");
}

/// Una `static` local **no** se ve desde fuera de su función. Si se viera, el
/// renombrado estaría mal hecho y dos ámbitos serían uno.
#[test]
fn una_static_local_no_se_ve_desde_otra_funcion() {
    let fuente = "int pone() { static int oculta = 7; return oculta; } \
                  int main() { return oculta; }";
    assert!(compile_source_to_bef(fuente).is_err(),
            "`oculta` sólo existe dentro de pone()");
}

/// Una `static` de fichero es una global normal: aquí sólo hay una unidad de
/// traducción, así que no hay nadie de quien esconderla.
#[test]
fn una_static_de_fichero_es_una_global_normal() {
    let fuente = "static int g = 41; \
                  int main() { g = g + 1; printf(\"%d\", g); return 0; }";
    assert_eq!(run_c(fuente), "42");
}

/// `static` delante de una función se acepta (enlace interno) y la función
/// sigue funcionando igual.
#[test]
fn una_funcion_static_se_compila_y_se_llama() {
    let fuente = "static int doble(int x) { return x * 2; } \
                  int main() { printf(\"%d\", doble(21)); return 0; }";
    assert_eq!(run_c(fuente), "42");
}

// ── Prototipos: llamar antes de definir ───────────────────────────────

/// ★ Sin prototipos **la recursión mutua es imposible**, y un programa de
/// cincuenta ficheros está lleno de funciones que se llaman en círculo:
/// ninguna puede ir "antes" que todas las demás.
#[test]
fn se_puede_llamar_a_una_funcion_declarada_mas_abajo() {
    let fuente = "int tarde(int x); \
                  int main() { printf(\"%d\", tarde(20)); return 0; } \
                  int tarde(int x) { return x + 22; }";
    assert_eq!(run_c(fuente), "42");
}

/// El parámetro de un prototipo puede ir **sin nombre**: es C legal y es como
/// se escriben las cabeceras de cualquier programa de verdad.
#[test]
fn un_prototipo_acepta_parametros_sin_nombre() {
    let fuente = "int tarde(int); \
                  int main() { printf(\"%d\", tarde(21)); return 0; } \
                  int tarde(int x) { return x * 2; }";
    assert_eq!(run_c(fuente), "42");
}

/// ★ La recursión MUTUA, que es el caso que justifica todo lo anterior.
#[test]
fn dos_funciones_pueden_llamarse_en_circulo() {
    let fuente = "int impar(int n); \
                  int par(int n) { if (n == 0) return 1; return impar(n - 1); } \
                  int impar(int n) { if (n == 0) return 0; return par(n - 1); } \
                  int main() { printf(\"%d%d\", par(4), par(7)); return 0; }";
    assert_eq!(run_c(fuente), "10");
}

/// Un prototipo **no emite nada**: declarar y no definir no puede inventarse
/// un cuerpo. Se comprueba llamando a algo que se declaró y nunca se escribió.
#[test]
fn un_prototipo_sin_definicion_no_se_inventa_la_funcion() {
    let fuente = "int fantasma(int x); \
                  int main() { return fantasma(1); }";
    assert!(compile_source_to_bef(fuente).is_err(),
            "no hay cuerpo que llamar");
}

// ── auto y register: se aceptan y no cambian nada ─────────────────────

/// `auto` y `register` se aceptan y se **tiran**. No es pereza: `register` es
/// una sugerencia que todos los compiladores ignoran desde hace treinta años y
/// `auto` es redundante desde 1978. Lo que importa del test es que el programa
/// dé **lo mismo** con ellas y sin ellas.
#[test]
fn auto_y_register_se_aceptan_y_no_cambian_el_resultado() {
    let con = "int main() { register int a = 20; auto int b = 22; \
               printf(\"%d\", a + b); return 0; }";
    let sin = "int main() { int a = 20; int b = 22; \
               printf(\"%d\", a + b); return 0; }";
    assert_eq!(run_c(con), "42");
    assert_eq!(run_c(con), run_c(sin));
}

// ── varargs: los argumentos que no tienen nombre ──────────────────────

/// Declarar `...` y usar los parametros CON nombre. Es la mitad barata, y sin
/// ella ni siquiera compila una cabecera que declare `printf`.
#[test]
fn una_funcion_variadica_compila_y_usa_sus_parametros_con_nombre() {
    let fuente = "int cuantos(int n, ...) { return n; } \
                  int main() { printf(\"%d\", cuantos(3, 10, 20, 30)); return 0; }";
    assert_eq!(run_c(fuente), "3");
}

/// ★ Y LEERLOS, que es la mitad que importa.
///
/// `__va_arg(i)` da el variadico numero `i`. Funciona porque BMO C pasa los
/// argumentos **por la pila** de derecha a izquierda: los que no tienen nombre
/// estan seguidos justo detras de los que si. En la convencion de registros de
/// SysV esto pediria volcar seis registros en el prologo y llevar dos
/// cursores; aqui es una suma.
#[test]
fn una_funcion_variadica_lee_sus_argumentos_sin_nombre() {
    let fuente = "int suma(int n, ...) { int t; int i; t = 0; \
                  for (i = 0; i < n; i = i + 1) { t = t + __va_arg(i); } return t; } \
                  int main() { printf(\"%d\", suma(3, 10, 20, 30)); return 0; }";
    assert_eq!(run_c(fuente), "60");
}

/// El indice es de EJECUCION, no una constante: sin eso no se puede recorrer
/// los argumentos en un bucle — que es exactamente lo que hace un `vsprintf`,
/// y `vsprintf` es lo que pide `I_Error(fmt, ...)`.
#[test]
fn el_indice_de_va_arg_puede_ser_una_variable() {
    let fuente = "int elige(int cual, ...) { return __va_arg(cual); } \
                  int main() { printf(\"%d,%d\", elige(0, 7, 9), elige(1, 7, 9)); return 0; }";
    assert_eq!(run_c(fuente), "7,9");
}
