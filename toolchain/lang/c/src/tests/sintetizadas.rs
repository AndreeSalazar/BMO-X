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
