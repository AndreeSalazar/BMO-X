//! **`static` y los prototipos** -- las dos que separaban "compila programas" de
//! "compila un programa de cincuenta ficheros".
//!
//! * Estos tests EJECUTAN. Que una construccion compile es media prueba y la
//! menos interesante: lo que define a `static` no es que el parser la acepte,
//! es que la variable **sobreviva a la llamada** y que su inicializador corra
//! **una sola vez**. Un compilador que la acepta y la trata como una local
//! normal pasa cualquier prueba de compilacion y falla en la primera vuelta de
//! un contador -- en silencio, dando siempre el mismo numero.

use super::*;

// -- static: lo que la hace distinta de una local ----------------------

/// * La propiedad que define `static`: **sobrevive entre llamadas**. Es la
/// unica razon de que exista, y la unica que un test de compilacion no ve.
#[test]
fn una_static_local_conserva_su_valor_entre_llamadas() {
    let fuente = "int cuenta() { static int n = 0; n = n + 1; return n; } \
                  int main() { printf(\"%d\", cuenta()); \
                               printf(\"%d\", cuenta()); \
                               printf(\"%d\", cuenta()); return 0; }";
    assert_eq!(run_c(fuente), "123");
}

/// * Y su inicializador corre **UNA vez**, no en cada llamada.
///
/// Es el mismo test del reves y hace falta: si el inicializador se emitiera
/// como una asignacion dentro del cuerpo, el contador de arriba tambien daria
/// `111` -- y "no cuenta" y "se reinicia" son dos bugs distintos con la misma
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

/// Una `static` local **no** se ve desde fuera de su funcion. Si se viera, el
/// renombrado estaria mal hecho y dos ambitos serian uno.
#[test]
fn una_static_local_no_se_ve_desde_otra_funcion() {
    let fuente = "int pone() { static int oculta = 7; return oculta; } \
                  int main() { return oculta; }";
    assert!(compile_source_to_bef(fuente).is_err(),
            "`oculta` sólo existe dentro de pone()");
}

/// Una `static` de fichero es una global normal: aqui solo hay una unidad de
/// traduccion, asi que no hay nadie de quien esconderla.
#[test]
fn una_static_de_fichero_es_una_global_normal() {
    let fuente = "static int g = 41; \
                  int main() { g = g + 1; printf(\"%d\", g); return 0; }";
    assert_eq!(run_c(fuente), "42");
}

/// `static` delante de una funcion se acepta (enlace interno) y la funcion
/// sigue funcionando igual.
#[test]
fn una_funcion_static_se_compila_y_se_llama() {
    let fuente = "static int doble(int x) { return x * 2; } \
                  int main() { printf(\"%d\", doble(21)); return 0; }";
    assert_eq!(run_c(fuente), "42");
}

// -- Prototipos: llamar antes de definir -------------------------------

/// * Sin prototipos **la recursion mutua es imposible**, y un programa de
/// cincuenta ficheros esta lleno de funciones que se llaman en circulo:
/// ninguna puede ir "antes" que todas las demas.
#[test]
fn se_puede_llamar_a_una_funcion_declarada_mas_abajo() {
    let fuente = "int tarde(int x); \
                  int main() { printf(\"%d\", tarde(20)); return 0; } \
                  int tarde(int x) { return x + 22; }";
    assert_eq!(run_c(fuente), "42");
}

/// El parametro de un prototipo puede ir **sin nombre**: es C legal y es como
/// se escriben las cabeceras de cualquier programa de verdad.
#[test]
fn un_prototipo_acepta_parametros_sin_nombre() {
    let fuente = "int tarde(int); \
                  int main() { printf(\"%d\", tarde(21)); return 0; } \
                  int tarde(int x) { return x * 2; }";
    assert_eq!(run_c(fuente), "42");
}

/// * La recursion MUTUA, que es el caso que justifica todo lo anterior.
#[test]
fn dos_funciones_pueden_llamarse_en_circulo() {
    let fuente = "int impar(int n); \
                  int par(int n) { if (n == 0) return 1; return impar(n - 1); } \
                  int impar(int n) { if (n == 0) return 0; return par(n - 1); } \
                  int main() { printf(\"%d%d\", par(4), par(7)); return 0; }";
    assert_eq!(run_c(fuente), "10");
}

/// Un prototipo **no emite nada**: declarar y no definir no puede inventarse
/// un cuerpo. Se comprueba llamando a algo que se declaro y nunca se escribio.
#[test]
fn un_prototipo_sin_definicion_no_se_inventa_la_funcion() {
    let fuente = "int fantasma(int x); \
                  int main() { return fantasma(1); }";
    assert!(compile_source_to_bef(fuente).is_err(),
            "no hay cuerpo que llamar");
}

// -- auto y register: se aceptan y no cambian nada ---------------------

/// `auto` y `register` se aceptan y se **tiran**. No es pereza: `register` es
/// una sugerencia que todos los compiladores ignoran desde hace treinta anos y
/// `auto` es redundante desde 1978. Lo que importa del test es que el programa
/// de **lo mismo** con ellas y sin ellas.
#[test]
fn auto_y_register_se_aceptan_y_no_cambian_el_resultado() {
    let con = "int main() { register int a = 20; auto int b = 22; \
               printf(\"%d\", a + b); return 0; }";
    let sin = "int main() { int a = 20; int b = 22; \
               printf(\"%d\", a + b); return 0; }";
    assert_eq!(run_c(con), "42");
    assert_eq!(run_c(con), run_c(sin));
}

// -- varargs: los argumentos que no tienen nombre ----------------------

/// Declarar `...` y usar los parametros CON nombre. Es la mitad barata, y sin
/// ella ni siquiera compila una cabecera que declare `printf`.
#[test]
fn una_funcion_variadica_compila_y_usa_sus_parametros_con_nombre() {
    let fuente = "int cuantos(int n, ...) { return n; } \
                  int main() { printf(\"%d\", cuantos(3, 10, 20, 30)); return 0; }";
    assert_eq!(run_c(fuente), "3");
}

/// * Y LEERLOS, que es la mitad que importa.
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
/// los argumentos en un bucle -- que es exactamente lo que hace un `vsprintf`,
/// y `vsprintf` es lo que pide `I_Error(fmt, ...)`.
#[test]
fn el_indice_de_va_arg_puede_ser_una_variable() {
    let fuente = "int elige(int cual, ...) { return __va_arg(cual); } \
                  int main() { printf(\"%d,%d\", elige(0, 7, 9), elige(1, 7, 9)); return 0; }";
    assert_eq!(run_c(fuente), "7,9");
}

// -- La biblioteca que se emite EN LINEA -------------------------------
//
// No hay libreria que enlazar, y eso NO es una carencia: es el modelo. Un
// `.bex` es una imagen entera y BEF no resuelve relocaciones contra un `.so`.
// El bucle cuesta treinta bytes y ahorra un enlazador, un formato de libreria
// y un cargador dinamico.

/// * `memcpy` -- por aqui pasa el blit de cada fotograma de DOOM.
#[test]
fn memcpy_mueve_los_bytes_y_devuelve_el_destino() {
    let fuente = "int main() { char a[8]; char b[8]; \
                  b[0]=7; b[1]=8; b[2]=9; \
                  memcpy(a, b, 3); \
                  printf(\"%d%d%d\", a[0], a[1], a[2]); return 0; }";
    assert_eq!(run_c(fuente), "789");
}

/// Copiar CERO bytes es valido y frecuente (un bucle que acaba de vaciarse).
/// Si el guardia no estuviera, el contador daria la vuelta y copiaria 2^64.
#[test]
fn memcpy_de_cero_bytes_no_toca_nada() {
    let fuente = "int main() { char a[4]; a[0]=5; \
                  memcpy(a, a, 0); printf(\"%d\", a[0]); return 0; }";
    assert_eq!(run_c(fuente), "5");
}

#[test]
fn memset_rellena_el_bloque() {
    let fuente = "int main() { char b[8]; b[0]=1; b[1]=1; b[2]=1; \
                  memset(b, 65, 3); \
                  printf(\"%d,%d,%d\", b[0], b[1], b[2]); return 0; }";
    assert_eq!(run_c(fuente), "65,65,65");
}

/// El terminador NO se cuenta -- que es lo que dice `strlen` y lo que mas se
/// equivoca al reimplementarlo.
#[test]
fn strlen_no_cuenta_el_terminador() {
    let fuente = "int main() { printf(\"%d\", strlen(\"hola\")); return 0; }";
    assert_eq!(run_c(fuente), "4");
}

#[test]
fn strlen_de_la_cadena_vacia_es_cero() {
    let fuente = "int main() { printf(\"%d\", strlen(\"\")); return 0; }";
    assert_eq!(run_c(fuente), "0");
}

/// * `strcmp` devuelve la DIFERENCIA con signo, no un si/no. Un `comparar`
/// que solo dijera "iguales o distintas" pareceria suficiente hasta el dia
/// que alguien ordene una lista con el.
#[test]
fn strcmp_devuelve_cero_igual_y_el_signo_correcto() {
    let fuente = "int main() { \
                  printf(\"%d,\", strcmp(\"abc\", \"abc\")); \
                  if (strcmp(\"abc\", \"abd\") < 0) printf(\"menor,\"); \
                  if (strcmp(\"abd\", \"abc\") > 0) printf(\"mayor\"); \
                  return 0; }";
    assert_eq!(run_c(fuente), "0,menor,mayor");
}

/// Una cadena que es prefijo de otra es MENOR: el terminador vale cero y
/// cualquier byte real es mayor.
#[test]
fn strcmp_un_prefijo_es_menor_que_la_cadena_larga() {
    let fuente = "int main() { if (strcmp(\"ab\", \"abc\") < 0) printf(\"si\"); return 0; }";
    assert_eq!(run_c(fuente), "si");
}

/// `strcpy` copia **con** el terminador: si no, la cadena destino no acabaria
/// nunca y el siguiente `strlen` leeria memoria ajena.
#[test]
fn strcpy_copia_con_el_terminador() {
    let fuente = "int main() { char d[8]; d[5]=88; \
                  strcpy(d, \"hola\"); \
                  printf(\"%d,%d\", strlen(d), d[4]); return 0; }";
    assert_eq!(run_c(fuente), "4,0");
}

#[test]
fn abs_da_el_valor_absoluto() {
    let fuente = "int main() { printf(\"%d,%d,%d\", abs(-3), abs(3), abs(0)); return 0; }";
    assert_eq!(run_c(fuente), "3,3,0");
}

/// * Un literal DENTRO de una condicion apunta a la cadena correcta.
///
/// Este test no es sobre cadenas: es sobre un fallo que llevaba ahi desde
/// siempre. `collect_strings` recorria las ramas de un `if` y **tiraba la
/// condicion**, asi que un literal escrito ahi nunca entraba en la tabla y el
/// `unwrap_or(0)` del emisor lo hacia apuntar a **la primera cadena del
/// programa**. No fallaba: apuntaba a otro sitio.
///
/// No se vio nunca porque hacia falta poder escribir algo como
/// `if (strcmp(s, "salir") == 0)` -- y `strcmp` no existia hasta hoy. Lo cazo
/// el primer test que lo piso, comparando "abc" contra el formato de un
/// `printf` anterior.
///
/// Las condiciones de `while`, `do-while`, `for` y `switch` estaban igual.
#[test]
fn un_literal_en_una_condicion_apunta_a_su_cadena_y_no_a_la_primera() {
    let fuente = "int main(){ printf(\"hola,\"); \
                  if (strcmp(\"abc\", \"abd\") < 0) printf(\"if,\"); \
                  while (strcmp(\"x\", \"x\") != 0) { printf(\"nunca\"); } \
                  printf(\"fin\"); return 0; }";
    assert_eq!(run_c(fuente), "hola,if,fin");
}

// -- Declaradores multiples: `int a, b;` -------------------------------

#[test]
fn se_pueden_declarar_varias_variables_en_una_linea() {
    let fuente = "int main(){ int a, b, c; a=1; b=2; c=3; \
                  printf(\"%d%d%d\", a, b, c); return 0; }";
    assert_eq!(run_c(fuente), "123");
}

/// Con inicializador cada uno por su cuenta.
#[test]
fn cada_declarador_lleva_su_propio_inicializador() {
    let fuente = "int main(){ int a = 20, b = 22; printf(\"%d\", a + b); return 0; }";
    assert_eq!(run_c(fuente), "42");
}

/// * El detalle de C que mas se salta al implementarlo: en `int *a, b;` la
/// `b` es un **int**, NO un puntero. El asterisco es del DECLARADOR, no del
/// tipo. Quien lo trate al reves compila el programa y le cambia el
/// significado -- que es peor que no compilarlo.
#[test]
fn el_asterisco_es_del_declarador_y_no_del_tipo() {
    let fuente = "int main(){ int n; int *p, b; n = 7; p = &n; b = 35; \
                  printf(\"%d\", *p + b); return 0; }";
    assert_eq!(run_c(fuente), "42");
}

/// Y el `[n]` tambien es de cada uno.
#[test]
fn cada_declarador_lleva_su_propio_array() {
    let fuente = "int main(){ char a[4], b; a[0]=40; b=2; \
                  printf(\"%d\", a[0] + b); return 0; }";
    assert_eq!(run_c(fuente), "42");
}
