//! **La resta de punteros, y la lista doblemente enlazada que la usa.**
//!
//! # De donde sale, y por que este eje estaba sin medir
//!
//! Metal del 2026-08-14: con el `Z_CheckHeap` asesino quitado, DOOM **carga el
//! nivel entero** (1590 bloques en la zona) y se muere en el primer fotograma
//! que dibujaria ese nivel:
//!
//! ```text
//!   #PF  PUNTERO NULO en 0+0x2c   ->  R_SortVisSprites+0x2c6
//!   48 63 00 = movsxd rax, [rax]      leer un int CON SIGNO por puntero
//! ```
//!
//! ** Y el `0x2c` ya descarta lo primero que uno miraria. En `vissprite_t`
//! --`prev`(0) `next`(8) `x1`(16) `x2`(20) `gx`(24) `gy`(28) `gz`(32)
//! `gzt`(36) `startfrac`(40) `scale`(44)-- el campo que cae en 44 es `scale`,
//! que es exactamente el que lee el bucle de ordenacion. **O sea que BMO C
//! coloco el struct BIEN**: si la disposicion estuviera mal, el offset seria
//! otro. La disposicion no es el sospechoso.
//!
//! Lo que queda es lo que hace la funcion, y son tres cosas que este arbol
//! nunca ha medido juntas:
//!
//! ```c
//!   count = vissprite_p - vissprites;     // PUNTERO MENOS PUNTERO
//!   for (ds=vissprites ; ds<vissprite_p ; ds++)
//!   { ds->next = ds+1; ds->prev = ds-1; } // ARITMETICA sobre un puntero a
//!                                         // struct AUTORREFERENTE
//!   for (ds=unsorted.next ; ds != &unsorted ; ds=ds->next)
//!       if (ds->scale < bestscale)        // <- aqui muere, con ds == NULL
//! ```
//!
//! `p - p` tiene brazo propio en el codegen (`codegen/mod.rs`, `Expr::Sub` con
//! los dos lados escalados: resta y luego `idiv` por el tamano del elemento) y
//! lo destapo **la sonda del lenguaje, no un arranque**. Pero ese brazo depende
//! de que `pointer_scale` sepa el tamano: si contesta `None`, la resta cae en
//! el `_ =>` y devuelve **bytes pelados**. Con un elemento de 80 bytes eso es
//! un `count` ochenta veces mas grande, el bucle de fuera sigue sacando
//! sprites de una lista ya vacia, y acaba en NULL. Encaja con el sintoma
//! entero.
//!
//! [!] El struct de aqui **mide 80 bytes igual que `vissprite_t`** y tiene su
//! misma forma (dos punteros a si mismo delante, enteros detras, un puntero
//! suelto que obliga a rellenar). Un struct de juguete de 8 bytes no probaria
//! nada: la division por el tamano acierta por casualidad cuando el tamano es
//! una potencia de dos pequena.

use super::*;

/// El mismo molde en todas las casillas: la forma de `vissprite_t`.
///
/// [!] Lleva el `typedef` **porque DOOM lo lleva**: `vissprite_t` es un
/// `typedef struct vissprite_s vissprite_t`, y sus globales se declaran con el
/// alias (`vissprite_t *vissprite_p;`). Medir la forma sin alias seria medir
/// otro programa -- y de hecho las dos formas NO se comportan igual, que es lo
/// que dice la ultima casilla de este fichero.
const MOLDE: &str = "
struct vs {
    struct vs *prev;
    struct vs *next;
    int x1; int x2;
    int gx; int gy;
    int gz; int gzt;
    int startfrac;
    int scale;
    int xiscale;
    int texturemid;
    int patch;
    char *colormap;
    int mobjflags;
};
typedef struct vs vs_t;
";

/// ** LA MEDIDA DE CONTROL: el struct mide 80 y `scale` cae en 44.
///
/// Va primera a proposito. Si esta sale roja, todo lo demas de este fichero
/// esta midiendo otra cosa -- y ademas querria decir que el `0x2c` del metal no
/// era `scale`, o sea que la lectura del fallo estaba mal desde el principio.
#[test]
fn el_molde_mide_ochenta_y_scale_cae_en_cuarenta_y_cuatro() {
    let salida = run_c(&format!(
        "{MOLDE}
int main() {{
    struct vs v;
    char *base;
    char *campo;
    base = (char *)&v;
    campo = (char *)&v.scale;
    printf(\"%d %d\", (int)sizeof(struct vs), (int)(campo - base));
    return 0;
}}"
    ));
    assert_eq!(
        salida, "80 44",
        "si el molde no mide lo que `vissprite_t`, esta sonda no prueba nada"
    );
}

/// ** PUNTERO MENOS PUNTERO DA UN INDICE, NO UNOS BYTES.
///
/// Esta es la casilla del `count = vissprite_p - vissprites` de DOOM: dos
/// punteros al mismo array y su diferencia. Con 80 bytes por elemento, la
/// respuesta equivocada no es un poco distinta: es **ochenta veces** la buena.
#[test]
fn la_resta_de_dos_punteros_a_struct_da_elementos() {
    let salida = run_c(&format!(
        "{MOLDE}
struct vs arr[16];
vs_t *fin;
int main() {{
    fin = &arr[5];
    printf(\"%d\", (int)(fin - arr));
    return 0;
}}"
    ));
    assert_eq!(salida, "5", "cinco elementos, no 400 bytes");
}

/// Y la resta al reves es NEGATIVA, no un numero gigante.
///
/// El brazo del codegen divide **con signo** (`cqo` + `idiv`) justo por esto:
/// una division sin signo convertiria un -5 legal en 230.584.300.921.369.395.
///
/// **ROJA, y reproduce un bug ABIERTO.** Da **-679168**. Y ojo al contraste
/// con la casilla de arriba, que sale verde: restar dos punteros GUARDADOS EN
/// VARIABLES acierta; restar el ARRAY (que decae) menos `&arr[5]` no. O sea
/// que no falla la division por el tamano -- falla lo que se le da a restar.
/// Ni siquiera son bytes: -400 seria "se olvido de dividir", y -679168 no es
/// eso. Es un operando que no es el que se pide.
#[test]
#[ignore = "reproduce un bug abierto: `arr - &arr[5]` da -679168 en vez de -5"]
fn la_resta_al_reves_sale_negativa() {
    let salida = run_c(&format!(
        "{MOLDE}
struct vs arr[16];
int main() {{
    printf(\"%d\", (int)(arr - &arr[5]));
    return 0;
}}"
    ));
    assert_eq!(salida, "-5", "restar hacia atras es legal en C y da negativo");
}

/// ** `p + 1` SOBRE UN PUNTERO A STRUCT AUTORREFERENTE.
///
/// `ds->next = ds+1` es como DOOM construye la lista. El tipo se declara
/// **dentro de si mismo** (`struct vs *next;` cuando `struct vs` aun no esta
/// cerrado), que es justo la forma en la que una tabla de tamanos puede no
/// tener todavia la respuesta. Si el paso sale 1 en vez de 80, la lista queda
/// hecha de direcciones desalineadas y el primer `->next` ya lee basura.
#[test]
fn avanzar_un_puntero_a_struct_autorreferente_salta_el_struct_entero() {
    let salida = run_c(&format!(
        "{MOLDE}
struct vs arr[16];
int main() {{
    struct vs *a;
    struct vs *b;
    a = arr;
    b = a + 1;
    printf(\"%d\", (int)((char *)b - (char *)a));
    return 0;
}}"
    ));
    assert_eq!(salida, "80", "un elemento son 80 bytes");
}

/// ** LA RECONSTRUCCION: `R_SortVisSprites` en miniatura.
///
/// Esta es la casilla que de verdad se queria. Hace lo mismo que DOOM --enlazar
/// el array en una lista circular con un centinela LOCAL, y luego recorrerla
/// hasta volver al centinela-- y cuenta las vueltas.
///
/// Lo que se comprueba no es un numero bonito: es que **el recorrido TERMINA y
/// no pasa por un puntero nulo**. Si `ds+1`, `&centinela` o la comparacion
/// `ds != &centinela` fallan, esto se cuelga o se sale del array, que es
/// exactamente lo que le pasa a DOOM en el Ryzen.
///
/// [!] El centinela es una LOCAL, como en DOOM (`vissprite_t unsorted;`), y no
/// un global: `&local` de un struct y `&global` de un struct son dos brazos
/// distintos de `Expr::AddrOf`, y el que usa DOOM es el primero.
///
/// **ROJA, Y ES LA MUERTE DE DOOM REPRODUCIDA SIN ENCENDER LA MAQUINA.**
///
/// Da `8 101`. El `8` es `count`, correcto. El `101` es el tope de seguridad
/// de este fichero: **el recorrido no vuelve nunca al centinela**. En DOOM no
/// hay tope, asi que sigue andando fuera del array hasta que un `next` vale 0
/// y `ds->scale` --offset 0x2c-- revienta. Es exactamente
/// `R_SortVisSprites+0x2c6`.
///
/// ** Y la casilla de abajo, que SI pasa, dice por donde NO va. Aquella
/// compara `mejor == &centinela` y desenlaza, y acierta; o sea que **coger la
/// direccion del centinela local y compararla funciona**. Lo que no funciona
/// es que la CADENA lleve hasta el.
///
/// El sospechoso que queda, y es una forma muy concreta:
///
/// ```c
///     (tope - 1)->next = &centinela;
/// ```
///
/// una flecha sobre un puntero CALCULADO. Si esa escritura cae en el sitio
/// equivocado, el ultimo elemento conserva su `next = ds + 1`, que apunta una
/// posicion MAS ALLA del final -- y el recorrido se va del array sin tocar el
/// centinela. Encaja con el sintoma sin dejar cabos.
#[test]
#[ignore = "reproduce la muerte de DOOM: el recorrido de la lista no vuelve al centinela"]
fn la_lista_circular_con_centinela_local_se_recorre_entera() {
    let salida = run_c(&format!(
        "{MOLDE}
struct vs arr[8];
vs_t *tope;
int main() {{
    struct vs centinela;
    struct vs *ds;
    int count;
    int vueltas;

    tope = &arr[8];
    count = tope - arr;

    /* Igual que DOOM: enlazar cada uno con su vecino... */
    for (ds = arr; ds < tope; ds++) {{
        ds->next = ds + 1;
        ds->prev = ds - 1;
        ds->scale = 100 - (int)(ds - arr);
    }}
    /* ...y cerrar los dos extremos contra el centinela. */
    arr[0].prev = &centinela;
    centinela.next = &arr[0];
    (tope - 1)->next = &centinela;
    centinela.prev = tope - 1;

    vueltas = 0;
    for (ds = centinela.next; ds != &centinela; ds = ds->next) {{
        vueltas = vueltas + 1;
        if (vueltas > 100) {{ break; }}   /* no colgar el banco de pruebas */
    }}
    printf(\"%d %d\", count, vueltas);
    return 0;
}}"
    ));
    assert_eq!(
        salida, "8 8",
        "ocho elementos y ocho vueltas: si el segundo no es 8, la lista de \
         `R_SortVisSprites` no se puede recorrer"
    );
}

/// Y el bucle de fuera, que es el que puede pasarse de largo.
///
/// DOOM saca `count` sprites de la lista, uno por vuelta. Si `count` viniera
/// inflado --el fallo que esta sonda persigue-- seguiria sacando de una lista
/// ya vacia. Aqui se comprueba que **el numero de extracciones y el de
/// elementos son el mismo**, que es la condicion que hace que ese bucle sea
/// seguro.
#[test]
fn sacar_count_elementos_vacia_la_lista_exactamente() {
    let salida = run_c(&format!(
        "{MOLDE}
struct vs arr[8];
vs_t *tope;
int main() {{
    struct vs centinela;
    struct vs *ds;
    struct vs *mejor;
    int count;
    int i;
    int sacados;

    tope = &arr[8];
    count = tope - arr;
    for (ds = arr; ds < tope; ds++) {{
        ds->next = ds + 1;
        ds->prev = ds - 1;
        ds->scale = 100 - (int)(ds - arr);
    }}
    arr[0].prev = &centinela;
    centinela.next = &arr[0];
    (tope - 1)->next = &centinela;
    centinela.prev = tope - 1;

    sacados = 0;
    for (i = 0; i < count; i++) {{
        mejor = centinela.next;
        if (mejor == &centinela) {{ break; }}   /* la lista se vacio antes */
        mejor->next->prev = mejor->prev;
        mejor->prev->next = mejor->next;
        sacados = sacados + 1;
    }}
    printf(\"%d %d\", sacados, count);
    return 0;
}}"
    ));
    assert_eq!(
        salida, "8 8",
        "si `count` viene inflado, `sacados` se queda corto y el bucle de DOOM \
         sigue pidiendo de una lista vacia"
    );
}

// ---------------------------------------------------------------------------
// ** EL ESTRECHAMIENTO, 2026-08-23: de un barrio a una linea.
//
// La casilla circular de arriba acusaba a una forma --`(tope - 1)->next = X`,
// una flecha sobre un puntero CALCULADO-- pero esa linea hace tres cosas a la
// vez: calcular la direccion, elegir el campo, y guardar. Un sospechoso con
// tres partes no es un sospechoso: es un barrio.
//
// Las cinco de abajo lo parten, y la respuesta salio en la primera vuelta:
//
//     calcular la direccion    VERDE
//     con variable intermedia  VERDE
//     guardar en linea         ROJA  -- y cae en el campo 0, no en el suyo
//
// *** **EL CULPABLE, LOCALIZADO**: `parser/types.rs`,
// `resolve_arrow_expr_offset`, que acaba en **`.unwrap_or(0)`**. Cuando el
// tipo de la base no se puede deducir --y `tope - 1` es una binaria, que
// `resolve_expr_type` no sabe tipar-- el offset del campo se convierte en
// CERO en silencio. No hay error, no hay aviso: la escritura cae en el primer
// campo del struct.
//
// En `vissprite_t` el campo 0 es `prev`. O sea que `(tope-1)->next = &unsorted`
// escribe en `prev` y **`next` conserva `ds + 1`**, una posicion mas alla del
// final. El recorrido se va del array, y en `+0x2c` --`scale`-- revienta.
// Es `R_SortVisSprites+0x2c6` sin un solo cabo suelto.
//
// [!] Y tiene un hermano en el mismo camino: `field_type_via_pointer` termina
// en `.unwrap_or(TypeSpec::Long)`, o sea que el ANCHO de la escritura tambien
// se inventa cuando el tipo no se deduce.
// ---------------------------------------------------------------------------

/// **PARTE 1: `tope - 1` calcula la direccion buena?** VERDE.
///
/// Puntero global menos entero, sin tocar memoria. Absuelve a la aritmetica.
#[test]
fn la_resta_de_uno_a_un_puntero_global_retrocede_un_elemento() {
    let salida = run_c(&format!(
        "{MOLDE}
struct vs arr[8];
vs_t *tope;
int main() {{
    tope = &arr[8];
    printf(\"%d\", (int)((char *)(tope - 1) - (char *)arr));
    return 0;
}}"
    ));
    assert_eq!(salida, "560", "siete elementos de 80 bytes");
}

/// **PARTE 2: con el puntero en una VARIABLE, la escritura cae bien.** VERDE.
///
/// Es la mitad que absuelve al `->` en general: guardar por una flecha
/// funciona. Lo que no funciona es cuando la base va EN LINEA.
#[test]
fn guardar_por_una_flecha_sobre_una_variable_puntero_cae_en_su_sitio() {
    let salida = run_c(&format!(
        "{MOLDE}
struct vs arr[8];
vs_t *tope;
int main() {{
    vs_t *p;
    tope = &arr[8];
    arr[7].next = 0;
    p = tope - 1;
    p->next = &arr[0];
    printf(\"%d\", (int)(arr[7].next == &arr[0]));
    return 0;
}}"
    ));
    assert_eq!(salida, "1", "la variable intermedia si tiene tipo declarado");
}

/// **PARTE 3: y EN LINEA no.** ROJA.
///
/// La unica diferencia con la casilla de arriba es que el puntero no pasa por
/// una variable. Ahi se pierde el tipo, y con el tipo se pierde el offset.
#[test]
#[ignore = "bug abierto: la flecha sobre un puntero calculado resuelve offset 0"]
fn guardar_por_una_flecha_sobre_un_puntero_calculado_cae_en_el_ultimo() {
    let salida = run_c(&format!(
        "{MOLDE}
struct vs arr[8];
vs_t *tope;
int main() {{
    tope = &arr[8];
    arr[7].next = 0;
    (tope - 1)->next = &arr[0];
    printf(\"%d\", (int)(arr[7].next == &arr[0]));
    return 0;
}}"
    ));
    assert_eq!(salida, "1", "la escritura tiene que caer en arr[7].next");
}

/// **PARTE 4: DONDE cae, que es lo que nombra al culpable.** ROJA.
///
/// Da `1 0`: la escritura fue a `prev` --offset 0-- en vez de a `next`
/// --offset 8--. Eso no es una direccion mal calculada: es un OFFSET DE CAMPO
/// que vale cero. Ver la cabecera de esta seccion.
#[test]
#[ignore = "bug abierto: da `1 0`, o sea que el campo resuelto fue el offset 0"]
fn la_flecha_calculada_no_escribe_en_el_campo_cero() {
    let salida = run_c(&format!(
        "{MOLDE}
struct vs arr[8];
vs_t *tope;
int main() {{
    tope = &arr[8];
    arr[7].prev = 0;
    arr[7].next = 0;
    (tope - 1)->next = &arr[3];
    printf(\"%d %d\", (int)(arr[7].prev == &arr[3]), (int)(arr[7].next == &arr[3]));
    return 0;
}}"
    ));
    assert_eq!(salida, "0 1", "si sale `1 0`, la flecha resolvio offset 0");
}

/// **PARTE 5: la forma exacta de DOOM** -- guardar la direccion de una LOCAL.
/// ROJA por lo mismo, y se conserva porque es la linea literal del juego
/// (`unsorted` es una local de `R_SortVisSprites`).
#[test]
#[ignore = "bug abierto: misma causa que las dos de arriba"]
fn guardar_la_direccion_de_una_local_por_un_puntero_calculado() {
    let salida = run_c(&format!(
        "{MOLDE}
struct vs arr[8];
vs_t *tope;
int main() {{
    struct vs centinela;
    tope = &arr[8];
    arr[7].next = 0;
    (tope - 1)->next = &centinela;
    printf(\"%d\", (int)(arr[7].next == &centinela));
    return 0;
}}"
    ));
    assert_eq!(salida, "1", "la cadena tiene que llegar al centinela");
}
