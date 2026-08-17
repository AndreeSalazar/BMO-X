//! **EL MONTON DE RING 3** -- `malloc` de verdad, sobre UN bloque del kernel.
//!
//! Parte del banco de pruebas de BMO C. Cada fila EJECUTA: un asignador que
//! reparte mal no da un error, da un puntero que pisa a otro -- y eso se ve
//! escribiendo en los dos y releyendo.
//!
//! ## Lo que separa estos tests de los de `memoria.rs`
//!
//! Los de alli usan `run_c`, que **no pasa por el preprocesador**, asi que no
//! hay `#include` y `malloc` sigue siendo el empotrado del codegen: una
//! peticion a `KIND_MEMORIA` por llamada, con tope de cuatro. Eso es el
//! CONTRATO DEL KERNEL y se sigue probando alli.
//!
//! Aqui se incluye `<stdlib.h>`, y entonces `malloc` es el monton. Las dos
//! cosas conviven a proposito y el corte es el `#include`.

use super::*;

fn corre(cuerpo: &str) -> String {
    let src = format!("#include <stdlib.h>\n{cuerpo}");
    let bef = compile_with_preprocessor(&src, std::path::Path::new("prueba.c"), CStandard::C11)
        .expect("el programa debe compilar");
    ejecutar_bef(&bef)
}

/// Cuantas veces cruzo la puerta una peticion de memoria.
///
/// Se cuenta por las llamadas que vio el emulador y no preguntandole al
/// programa: un asignador que dijera "solo pedi una vez" seria un testigo, no
/// una prueba.
fn peticiones_al_kernel(bef: &[u8]) -> usize {
    use bmo_abi::syscalls::surface::TASK_OP_MEMORIA_PEDIR;
    maquina_de_bef(bef)
        .syscalls
        .iter()
        .filter(|s| s.operation == TASK_OP_MEMORIA_PEDIR)
        .count()
}

/// ** LA FILA QUE JUSTIFICA TODO EL FICHERO: mas de cuatro `malloc`.
///
/// Con el `malloc` empotrado el quinto devuelve 0, porque el kernel da cuatro
/// bloques por proceso. Es exactamente lo que mataba a DOOM, que llama a
/// `malloc` una docena de veces solo en el arranque -- siete de ellas desde
/// `I_AtExit`.
#[test]
fn veinte_mallocs_seguidos_y_todos_dan_memoria() {
    let out = corre(
        r#"
int main() {
    int i;
    int nulos;
    char *p;
    nulos = 0;
    for (i = 0; i < 20; i = i + 1) {
        p = (char *)malloc(64);
        if (p == 0) { nulos = nulos + 1; }
    }
    printf("nulos=%d\n", nulos);
    return 0;
}
"#,
    );
    assert_eq!(out, "nulos=0\n");
}

/// Dos bloques son dos sitios distintos. Se escribe en los dos y se releen los
/// dos DESPUES: releer uno a uno no distinguiria dos bloques de uno repetido.
#[test]
fn dos_bloques_del_monton_no_se_pisan() {
    let out = corre(
        r#"
int main() {
    char *a;
    char *b;
    int i;
    int malos;
    a = (char *)malloc(100);
    b = (char *)malloc(100);
    for (i = 0; i < 100; i = i + 1) { a[i] = 1; }
    for (i = 0; i < 100; i = i + 1) { b[i] = 2; }
    malos = 0;
    for (i = 0; i < 100; i = i + 1) { if (a[i] != 1) { malos = malos + 1; } }
    for (i = 0; i < 100; i = i + 1) { if (b[i] != 2) { malos = malos + 1; } }
    printf("malos=%d,orden=%d\n", malos, b > a);
    return 0;
}
"#,
    );
    assert_eq!(out, "malos=0,orden=1\n");
}

/// ** `free` DEVUELVE MEMORIA DE VERDAD, y esto es lo que no existia.
///
/// El `free` empotrado era un no-op **dicho**: el bloque vivia hasta que moria
/// el proceso. Aqui se reserva y se suelta mil veces un bloque grande; si `free`
/// no devolviera nada, un monton de 1 MiB se agota a la decima vuelta.
#[test]
fn reservar_y_soltar_mil_veces_no_agota_el_monton() {
    let out = corre(
        r#"
int main() {
    int i;
    int nulos;
    char *p;
    nulos = 0;
    for (i = 0; i < 1000; i = i + 1) {
        p = (char *)malloc(100000);
        if (p == 0) { nulos = nulos + 1; }
        free(p);
    }
    printf("nulos=%d\n", nulos);
    return 0;
}
"#,
    );
    assert_eq!(out, "nulos=0\n");
}

/// Los huecos sueltos se FUSIONAN. Tres bloques seguidos, los tres liberados, y
/// luego uno que solo cabe si los tres se juntaron.
///
/// Sin fusion esto devuelve 0 y el monton se degrada solo: es el fallo que no
/// se nota hasta que un programa lleva rato corriendo.
#[test]
fn tres_huecos_seguidos_se_fusionan_en_uno() {
    let out = corre(
        r#"
int main() {
    char *a;
    char *b;
    char *c;
    char *grande;
    a = (char *)malloc(20000);
    b = (char *)malloc(20000);
    c = (char *)malloc(20000);
    if (a == 0 || b == 0 || c == 0) { printf("no se pudo reservar\n"); return 1; }
    free(a);
    free(b);
    free(c);
    grande = (char *)malloc(55000);
    printf("cabe=%d\n", grande != 0);
    return 0;
}
"#,
    );
    assert_eq!(out, "cabe=1\n");
}

/// ** `realloc` CONSERVA EL CONTENIDO. Devolvia 0 a proposito porque no sabia
/// cuanto media el bloque viejo; ahora lo lee de su cabecera.
#[test]
fn realloc_crece_y_conserva_lo_que_habia() {
    let out = corre(
        r#"
int main() {
    char *p;
    int i;
    int malos;
    p = (char *)malloc(50);
    for (i = 0; i < 50; i = i + 1) { p[i] = i + 1; }
    p = (char *)realloc(p, 5000);
    if (p == 0) { printf("realloc devolvio 0\n"); return 1; }
    malos = 0;
    for (i = 0; i < 50; i = i + 1) { if (p[i] != i + 1) { malos = malos + 1; } }
    p[4999] = 9;
    printf("malos=%d,fin=%d\n", malos, p[4999]);
    return 0;
}
"#,
    );
    assert_eq!(out, "malos=0,fin=9\n");
}

/// Los dos bordes del estandar, que son los que se olvidan: `realloc(0, n)` es
/// `malloc(n)` y `realloc(p, 0)` es `free(p)` devolviendo 0.
#[test]
fn realloc_con_puntero_nulo_o_tamano_cero() {
    let out = corre(
        r#"
int main() {
    char *a;
    char *b;
    a = (char *)realloc(0, 100);
    a[0] = 7;
    b = (char *)malloc(100);
    printf("%d,%d,%d\n", a != 0, a[0], realloc(b, 0) == 0);
    return 0;
}
"#,
    );
    assert_eq!(out, "1,7,1\n");
}

/// `calloc` entrega CEROS, y no porque las paginas del kernel vengan limpias:
/// aqui el bloque puede ser uno reutilizado, con la basura de quien lo tuvo
/// antes. Es justo el caso que un `calloc` que confia en el kernel se come.
#[test]
fn calloc_da_ceros_incluso_reutilizando_un_bloque_sucio() {
    let out = corre(
        r#"
int main() {
    char *sucio;
    char *limpio;
    int i;
    int malos;
    sucio = (char *)malloc(500);
    for (i = 0; i < 500; i = i + 1) { sucio[i] = 0x55; }
    free(sucio);
    limpio = (char *)calloc(500, 1);
    malos = 0;
    for (i = 0; i < 500; i = i + 1) { if (limpio[i] != 0) { malos = malos + 1; } }
    printf("malos=%d\n", malos);
    return 0;
}
"#,
    );
    assert_eq!(out, "malos=0\n");
}

/// El desbordamiento del producto de `calloc`, que es el clasico. Sin la guarda
/// se reparte un bloque pequeno para una peticion enorme y quien lo recorra se
/// lleva por delante el monton.
#[test]
fn calloc_rechaza_un_producto_que_se_desborda() {
    let out = corre(
        r#"
int main() {
    printf("%d\n", calloc(0x100000001, 16) == 0);
    return 0;
}
"#,
    );
    assert_eq!(out, "1\n");
}

/// ** CUANDO NO CABE, SE DICE. Regla 2 de `docs/identidad/LA_RAM.md`: `malloc` no miente.
///
/// El monton por defecto es 1 MiB; pedir 4 MiB no puede colar. Y despues de
/// decir que no, el monton **sigue sirviendo**: un "no" no puede dejarlo roto.
#[test]
fn lo_que_no_cabe_devuelve_cero_y_el_monton_sigue_vivo() {
    let out = corre(
        r#"
int main() {
    char *enorme;
    char *normal;
    enorme = (char *)malloc(4 * 1024 * 1024);
    normal = (char *)malloc(64);
    if (normal != 0) { normal[63] = 3; }
    printf("enorme=%d,normal=%d\n", enorme == 0, normal != 0);
    return 0;
}
"#,
    );
    assert_eq!(out, "enorme=1,normal=1\n");
}

/// El tamano del monton **lo declara el programa**, y por eso lo de arriba no
/// es un techo del sistema: con el `#define` delante, los 4 MiB entran.
#[test]
fn el_programa_declara_cuanto_monton_quiere() {
    let src = "#define BMO_MONTON_BYTES (8 * 1024 * 1024)\n\
               #include <stdlib.h>\n\
               int main() { printf(\"%d\\n\", malloc(4 * 1024 * 1024) != 0); return 0; }";
    let bef = compile_with_preprocessor(src, std::path::Path::new("prueba.c"), CStandard::C11)
        .expect("el programa debe compilar");
    assert_eq!(ejecutar_bef(&bef), "1\n");
}

/// ** UN PROGRAMA QUE NO PIDE MEMORIA NO GASTA NI UNA PETICION.
///
/// La arena se pide en el primer `malloc` y no antes. Se mira por donde de
/// verdad se sabe --las llamadas que cruzaron la puerta-- y no preguntandoselo
/// al programa.
#[test]
fn sin_malloc_no_se_le_pide_nada_al_kernel() {
    let src = "#include <stdlib.h>\nint main() { printf(\"hola\"); return 0; }";
    let bef = compile_with_preprocessor(src, std::path::Path::new("prueba.c"), CStandard::C11)
        .expect("el programa debe compilar");
    assert_eq!(
        peticiones_al_kernel(&bef),
        0,
        "un programa que no llama a malloc no puede haber pedido memoria"
    );
}

/// Y en cuanto pide, es UNA sola peticion por muchos `malloc` que haga. Es la
/// frase entera de este cambio, dicha con el numero del kernel.
#[test]
fn cincuenta_mallocs_son_una_sola_peticion_al_kernel() {
    let src = "#include <stdlib.h>\n\
               int main() { int i; for (i = 0; i < 50; i = i + 1) { malloc(32); } return 0; }";
    let bef = compile_with_preprocessor(src, std::path::Path::new("prueba.c"), CStandard::C11)
        .expect("el programa debe compilar");
    assert_eq!(
        peticiones_al_kernel(&bef),
        1,
        "cincuenta malloc, un bloque del kernel"
    );
}
