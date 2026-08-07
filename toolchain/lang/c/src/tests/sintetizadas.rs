//! **Las funciones SINTETIZADAS** — emitidas una vez, alcanzadas con `call`.
//!
//! Lo que se prueba aquí no es que `memcpy` funcione: eso ya lo fijaba
//! `almacenamiento::memcpy_mueve_los_bytes_y_devuelve_el_destino` cuando se
//! emitía en línea, y **seguiría pasando si el cuerpo se duplicara doscientas
//! veces**. Un test de comportamiento no ve la diferencia entre una copia y
//! doscientas, que es justo la propiedad que este mecanismo añade.
//!
//! Así que aquí se mira **cuántas veces sale el cuerpo en el binario**. Es la
//! única forma de que "se emite una sola vez" sea una prueba y no una
//! intención — y si alguien devuelve `memcpy` a la vía en línea, esto se pone
//! rojo en vez de seguir en verde contando otra cosa.

use super::*;

/// Cuántas veces aparece una secuencia de bytes. Hermana de `contains_bytes`,
/// que sólo dice si está: aquí el número ES la prueba.
fn cuantas_veces(pajar: &[u8], aguja: &[u8]) -> usize {
    if aguja.is_empty() || pajar.len() < aguja.len() {
        return 0;
    }
    pajar.windows(aguja.len()).filter(|v| *v == aguja).count()
}

/// El corazón de `bmo_lower::memoria::copiar`, que es el cuerpo de `memcpy`:
/// `mov al,[rsi]` · `mov [rdi],al` · `inc rsi` · `inc rdi` · `dec rcx`.
///
/// Se eligen estas cinco instrucciones y no el prólogo porque son las que
/// **sólo** puede haber puesto el bucle de copia: trece bytes seguidos que no
/// salen por casualidad en un programa de cien.
const BUCLE_COPIAR: &[u8] = &[
    0x8A, 0x06, 0x88, 0x07, 0x48, 0xFF, 0xC6, 0x48, 0xFF, 0xC7, 0x48, 0xFF, 0xC9,
];

/// ★ LA PRUEBA DE LA PIEZA: veinte llamadas, UN cuerpo.
#[test]
fn veinte_memcpy_emiten_un_solo_cuerpo() {
    let llamadas = "memcpy(a, b, 3); ".repeat(20);
    let fuente = format!(
        "int main() {{ char a[64]; char b[64]; b[0]=7; b[1]=8; b[2]=9; {llamadas} \
         printf(\"%d%d%d\", a[0], a[1], a[2]); return 0; }}"
    );
    let bef = compile_source_to_bef(&fuente).expect("el programa debe compilar");
    assert_eq!(
        cuantas_veces(&bef, BUCLE_COPIAR),
        1,
        "el cuerpo de memcpy tiene que estar UNA vez, no una por llamada"
    );
}

/// Y sigue haciendo lo que dice: mover los bytes. Que se emita una vez no
/// vale nada si la llamada llega mal — el prólogo traduce la ABI de PILA de
/// BMO C (`[rbp+16]`, `[rbp+24]`, `[rbp+32]`) a los registros que `copiar`
/// espera, y confundir eso con SysV daría un binario que compila y copia
/// desde donde nadie escribió.
#[test]
fn memcpy_sintetizado_mueve_los_bytes_en_llamadas_seguidas() {
    let fuente = "int main() { char a[8]; char b[8]; char c[8]; \
                  b[0]=1; b[1]=2; c[0]=3; c[1]=4; \
                  memcpy(a, b, 2); printf(\"%d%d\", a[0], a[1]); \
                  memcpy(a, c, 2); printf(\"%d%d\", a[0], a[1]); return 0; }";
    assert_eq!(run_c(fuente), "1234");
}

/// `memcpy` devuelve el DESTINO, y por la vía enlazada eso lo pone un
/// `mov rax,[rbp+16]` del epílogo — no la pila, como hacía `soltar_tres`.
/// Es el punto donde una conversión descuidada devolvería `rdi` ya avanzado,
/// o sea el final del bloque en vez del principio.
#[test]
fn memcpy_sintetizado_devuelve_el_destino_no_el_final() {
    let fuente = "int main() { char a[8]; char b[8]; char *p; \
                  b[0]=9; \
                  p = memcpy(a, b, 1); \
                  printf(\"%d\", p[0]); return 0; }";
    assert_eq!(run_c(fuente), "9");
}

/// Copiar cero bytes por la vía enlazada: el guardia `test rcx,rcx` sigue
/// dentro del cuerpo, así que sigue valiendo. Sin él el contador daría la
/// vuelta y copiaría 2^64.
#[test]
fn memcpy_sintetizado_de_cero_bytes_no_toca_nada() {
    let fuente = "int main() { char a[4]; a[0]=5; \
                  memcpy(a, a, 0); printf(\"%d\", a[0]); return 0; }";
    assert_eq!(run_c(fuente), "5");
}

/// El caso que la tabla tenía que reproducir para merecer existir: el stub de
/// syscall estaba cableado a mano y llevaba semanas corriendo en el Ryzen.
/// Ahora sale de la tabla, y **una sola vez** — `syscall; ret`.
///
/// Se usa `malloc` y no `printf`, y eso se descubrió midiendo: `printf` emite
/// su `syscall` EN LÍNEA (por `bmo_lower::console`), así que un programa que
/// sólo imprime no referencia el stub y este test habría contado cero. Quien
/// pasa por la puerta es `malloc` y el `Expr::Syscall` crudo.
#[test]
fn el_stub_de_syscall_se_emite_una_sola_vez() {
    let fuente = "int main() { char *p; char *q; \
                  p = malloc(16); q = malloc(16); \
                  printf(\"%d\", p == q); return 0; }";
    let bef = compile_source_to_bef(fuente).expect("el programa debe compilar");
    assert_eq!(
        cuantas_veces(&bef, &[0x0F, 0x05, 0xC3]),
        1,
        "dos malloc, UN stub"
    );
}

/// ★ Y el efecto secundario del cambio, que conviene fijar porque es una
/// mejora silenciosa y por tanto fácil de perder: el stub ya **no** se emite
/// en un programa que no lo llama.
///
/// Antes salía siempre que el perfil fuera Ring 3 —tres bytes de `syscall;
/// ret` que nadie alcanzaba—, porque la decisión la tomaba el PERFIL. Ahora la
/// toma la DEMANDA: existe si hay una reloc que lo pida.
#[test]
fn un_programa_que_no_llama_al_stub_no_lo_lleva() {
    let fuente = "int main() { printf(\"hola\"); return 0; }";
    let bef = compile_source_to_bef(fuente).expect("el programa debe compilar");
    assert_eq!(
        cuantas_veces(&bef, &[0x0F, 0x05, 0xC3]),
        0,
        "printf emite su syscall en linea: aqui no hay a quien llamar, \
         asi que el stub no debe estar"
    );
}

// ── `printf`: la conversión que de verdad se repite ───────────────────
//
// `memcpy` no lo llama ni un ejemplo del repo. `printf` lo llaman los seis, y
// hasta ahora cada `%d` se llevaba el conversor de entero a decimal completo.

/// El camino del signo de `bmo_lower::fmt::formatear_i64`: `mov r10d, 1` +
/// `neg rax`. Nueve bytes que sólo puede haber puesto ese formateador.
const SIGNO_I64: &[u8] = &[0x41, 0xBA, 0x01, 0x00, 0x00, 0x00, 0x48, 0xF7, 0xD8];

/// ★ Cinco `%d` en una llamada, UN formateador.
///
/// En línea eran cinco copias. El número que este test fija no es "1 está
/// bien": es que **no crece con las conversiones**.
#[test]
fn cinco_conversiones_emiten_un_solo_formateador() {
    let fuente = "int main() { printf(\"%d %d %d %d %d\", 1, 2, 3, 4, 5); return 0; }";
    let bef = compile_source_to_bef(fuente).expect("el programa debe compilar");
    assert_eq!(
        cuantas_veces(&bef, SIGNO_I64),
        1,
        "el conversor de entero tiene que estar UNA vez, no una por %d"
    );
}

/// Y sigue imprimiendo. Las cinco conversiones de una pasada, porque el riesgo
/// de convertirlas a `call` es idéntico en las cinco y probar sólo `%d` dejaría
/// cuatro sin mirar.
#[test]
fn printf_sigue_imprimiendo_las_cinco_conversiones() {
    let fuente = "int main() { printf(\"%d|%u|%x|%c|%s\", 0 - 5, 7, 255, 65, \"hi\"); return 0; }";
    assert_eq!(run_c(fuente), "-5|7|ff|A|hi");
}

/// El orden importa y es lo que rompería un `call` mal colocado: el argumento
/// se carga de la pila con un desplazamiento relativo a `rsp` ANTES del
/// `call`, y el `ret` devuelve los ocho bytes de la dirección de retorno. Si
/// eso no cuadrara, la segunda conversión leería el argumento de la primera.
#[test]
fn los_argumentos_no_se_desordenan_entre_conversiones() {
    let fuente = "int main() { printf(\"%d,%d,%d\", 11, 22, 33); return 0; }";
    assert_eq!(run_c(fuente), "11,22,33");
}

/// ★ EL LÍMITE DEL MECANISMO, escrito para que no se espere lo que no da.
///
/// Un `printf` de puro literal no comparte nada: `console::write_const` mete el
/// texto **dentro de las instrucciones** como inmediatos, así que su cuerpo es
/// distinto en cada llamada y no hay función que sintetizar.
///
/// Esto explica una medida que si no parece un fallo: `raycaster_C.c` tiene
/// tres `printf` y su código **no se redujo ni un byte** al convertir las
/// conversiones. Los tres son literales sin `%`.
#[test]
fn un_printf_de_solo_literal_no_llama_a_ningun_formateador() {
    let fuente = "int main() { printf(\"hola que tal\\n\"); return 0; }";
    let bef = compile_source_to_bef(fuente).expect("el programa debe compilar");
    assert_eq!(
        cuantas_veces(&bef, SIGNO_I64),
        0,
        "sin conversiones no hay formateador que emitir"
    );
}

/// ★ Y la puerta que NO se abrió: la tabla no convierte este codegen en un
/// enlazador. Un nombre que no está definido y no está en la tabla sigue
/// fallando en COMPILACIÓN y diciendo cuál es.
///
/// Hace falta escrito porque el riesgo del cambio va en esa dirección: un
/// mecanismo que inyecta cuerpos por nombre invita a que un día una llamada
/// desconocida se resuelva sola con un cuerpo vacío. Eso daría un `.bex` que
/// enlaza y no hace nada, que es peor que no compilar.
#[test]
fn una_funcion_desconocida_sigue_fallando_con_su_nombre() {
    let fuente = "int main() { ni_idea_de_esto(1, 2); return 0; }";
    let err = compile_source_to_bef(fuente)
        .expect_err("llamar a algo que no existe no puede compilar");
    let msg = format!("{err:?}");
    assert!(
        msg.contains("ni_idea_de_esto"),
        "el error tiene que decir QUÉ falta, y dijo: {msg}"
    );
}

// ── La pieza 5: las cadenas ───────────────────────────────────────────
//
// Ningún ejemplo del repo llama a `strlen`, `strcpy`, `memset`, `strcmp`,
// `strchr`, `strncmp` ni `memcmp` — cero usos en los seis `.c`. Así que la
// conversión no movió ni un byte de lo que existe, y estos tests son la única
// prueba de que hace lo que dice. En un programa que las usa cuarenta veces
// cada una, el código pasa de 6268 a 4249 bytes (−32,2%).

/// El bucle de `bmo_lower::memoria::largo`: `mov cl,[rdi+rax]` + `test cl,cl`.
const BUCLE_LARGO: &[u8] = &[0x8A, 0x0C, 0x07, 0x84, 0xC9];

/// ★ Veinte `strlen`, UN cuerpo.
#[test]
fn veinte_strlen_emiten_un_solo_cuerpo() {
    let llamadas = "t = t + strlen(s); ".repeat(20);
    let fuente = format!(
        "int main() {{ char s[8]; int t; s[0]=104; s[1]=0; t=0; {llamadas} \
         printf(\"%d\", t); return 0; }}"
    );
    let bef = compile_source_to_bef(&fuente).expect("el programa debe compilar");
    assert_eq!(cuantas_veces(&bef, BUCLE_LARGO), 1, "veinte strlen, un cuerpo");
    assert_eq!(run_c(&fuente), "20", "y sigue midiendo: 20 veces largo(\"h\") = 20");
}

/// ★ `strcpy` es el único que COMPONE dos emisores, y el orden no es libre:
/// `largo` ensucia `cl`, así que la medida tiene que salir antes de cargar
/// `rcx` con ella. Al revés, `rcx` llegaría machacado al bucle de copia.
///
/// Se usa una cadena de varios caracteres a propósito: con una de un solo byte
/// un `rcx` equivocado podría acertar por casualidad.
#[test]
fn strcpy_sintetizado_copia_la_cadena_entera_y_la_cierra() {
    let fuente = "int main() { char d[16]; char *s; \
                  s = \"abcdef\"; \
                  strcpy(d, s); \
                  printf(\"%s|%d\", d, strlen(d)); return 0; }";
    assert_eq!(run_c(fuente), "abcdef|6");
}

/// `memset` devuelve el destino, igual que `memcpy`, y por la misma vía: un
/// `mov rax,[rbp+16]` en el epílogo.
#[test]
fn memset_sintetizado_devuelve_el_destino() {
    let fuente = "int main() { char a[8]; char *p; \
                  p = memset(a, 65, 3); \
                  printf(\"%d%d%d\", p[0], p[1], p[2]); return 0; }";
    assert_eq!(run_c(fuente), "656565");
}

/// `strncmp` y `memcmp` salen del MISMO emisor con un booleano distinto —si el
/// terminador corta o no—, y son dos entradas separadas de la tabla. Este test
/// existe porque una tabla con un solo nombre para las dos daría a `memcmp` la
/// semántica de `strncmp` y compilaría igual de bien.
#[test]
fn strncmp_y_memcmp_no_se_confunden_al_sintetizarse() {
    // Tras el terminador los bytes DIFIEREN. `strncmp` se para y dice "iguales";
    // `memcmp` sigue y encuentra la diferencia.
    let fuente = "int main() { char a[8]; char b[8]; \
                  a[0]=104; a[1]=0; a[2]=1; \
                  b[0]=104; b[1]=0; b[2]=9; \
                  printf(\"%d,%d\", strncmp(a, b, 4) == 0, memcmp(a, b, 4) == 0); \
                  return 0; }";
    assert_eq!(run_c(fuente), "1,0");
}
