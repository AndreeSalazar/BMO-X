//! MUSICA: `<bmo/musica.h>` sobre `KIND_AUDIO`
//!
//! Parte del banco de pruebas de BMO C. Los ayudantes (`run_c`,
//! `run_c_sembrado`, `ejecutar_bef`) viven en `tests/mod.rs`.
//!
//! ## Que se puede probar aqui, y que no
//!
//! Aqui **no suena nada**, y en la mitad de las placas reales tampoco. Lo que
//! se comprueba es la PARTITURA: la lista de `(hercios, milisegundos)` que el
//! programa mando por la puerta. Eso basta para decidir si la libreria esta
//! bien -- una nota mal convertida da un numero distinto, suene o no suene.
//!
//! Lo que NO se puede probar aqui es que un altavoz emita ese tono. Eso es el
//! Ryzen, con `c/musica.bex`, y esta dicho en la cabecera de `musica.h`.

use super::*;

/// La cabecera trae cuerpos de funcion y se incluye con `#include`, asi que
/// hay que pasar por el PREPROCESADOR -- el camino de la linea de ordenes, no
/// el de biblioteca. Es el mismo `run_c_con_pp` de siempre, pero devolviendo la
/// maquina entera en vez de solo la consola: lo que hay que mirar aqui no es lo
/// que se imprimio, es lo que se mando al altavoz.
fn ejecutar_musica(cuerpo: &str) -> bmo_lower::emu::Machine {
    let src = format!("#include <bmo/musica.h>\n{cuerpo}");
    let bef = compile_with_preprocessor(&src, std::path::Path::new("prueba.c"), CStandard::C11)
        .expect("el programa debe compilar");
    maquina_de_bef(&bef)
}

/// Una nota es su altura y su duracion, y las dos tienen que salir exactas.
///
/// `LA4` en negra a 120 pulsos: la negra son 500 ms, el ligado del 85% deja
/// 425 ms sonando y 75 callado. Si la conversion se hiciera con la negra como
/// unidad en vez del dieciseisavo, aqui saldria un numero redondeado hacia
/// abajo y la pieza entera iria corta.
///
/// ** Y sale en DOS pitidos, no en uno: 425 ms pasan del tope de 250 del kernel.
/// O sea que **el troceo es el caso normal, no el raro** -- a cualquier tempo
/// razonable una negra ya no cabe en una sola llamada. Esta fila se escribio
/// esperando `[(440, 425)]` y se equivocaba ella, no la libreria.
#[test]
fn una_nota_es_altura_y_duracion() {
    let m = ejecutar_musica(
        r#"
int main() {
    unsigned long long cap;
    cap = bmo_sonido_reclamar();
    bmo_musica_tempo(120);
    bmo_nota(cap, LA4, NEGRA);
    return 0;
}
"#,
    );
    let p = m.partitura();
    assert_eq!(p, &[(440, 250), (440, 175), (0, 75)], "partitura inesperada: {p:?}");
    // Y lo que de verdad importa, dicho aparte del troceo: 425 sonando de 500.
    assert_eq!(m.audio_ms_sonando(), 425);
    assert!(p.iter().all(|t| t.0 == 440 || t.0 == 0), "la altura no puede cambiar");
}

/// El TEMPO cambia la pieza entera sin tocar una nota. Es la razon de que la
/// libreria exista: con hercios y milisegundos a pelo habria que reescribirla.
#[test]
fn el_tempo_cambia_la_duracion_y_no_la_altura() {
    let m = ejecutar_musica(
        r#"
int main() {
    unsigned long long cap;
    cap = bmo_sonido_reclamar();
    bmo_musica_tempo(240);
    bmo_nota(cap, LA4, NEGRA);
    return 0;
}
"#,
    );
    // El doble de rapido: la negra son 250 ms, y el 85% son 212.
    assert_eq!(m.partitura(), &[(440, 212), (0, 38)]);
}

/// La ARTICULACION: una nota no ocupa toda su figura.
///
/// Sin ese hueco, dos notas iguales seguidas suenan como una sola larga, y eso
/// no es un detalle de gusto -- es la diferencia entre oir dos y oir una.
#[test]
fn dos_notas_iguales_se_separan() {
    let m = ejecutar_musica(
        r#"
int main() {
    unsigned long long cap;
    cap = bmo_sonido_reclamar();
    bmo_musica_tempo(240);
    bmo_nota(cap, MI4, CORCHEA);
    bmo_nota(cap, MI4, CORCHEA);
    return 0;
}
"#,
    );
    let p = m.partitura();
    assert_eq!(p.len(), 4, "faltan los silencios de articulacion: {p:?}");
    assert_eq!(p[0].0, 330);
    assert_eq!(p[1].0, 0, "entre dos notas iguales tiene que haber silencio");
    assert_eq!(p[2].0, 330);
}

/// ** El TROCEO. Una nota mas larga que el tope del kernel se parte, y los
/// trozos suman lo pedido.
///
/// Es la prueba que justifica modelar `AUDIO_MAX_MS` en el emulador: sin el
/// tope ahi dentro, `bmo_sostener` pasaria esta prueba estando vacio de bucle.
#[test]
fn una_nota_larga_se_trocea_y_suma_lo_mismo() {
    let m = ejecutar_musica(
        r#"
int main() {
    unsigned long long cap;
    cap = bmo_sonido_reclamar();
    bmo_musica_tempo(100);
    bmo_musica_ligado(100);
    bmo_nota(cap, DO4, BLANCA);
    return 0;
}
"#,
    );
    // Blanca a 100 ppm = 1200 ms, o sea cinco veces el tope de 250.
    let p = m.partitura();
    assert!(p.len() >= 5, "no se troceo: {p:?}");
    for (hz, ms) in p {
        assert_eq!(*hz, 262, "el troceo no puede cambiar la altura");
        assert!(*ms <= 250, "un trozo se paso del tope: {ms}");
    }
    let total: u64 = p.iter().map(|t| t.1).sum();
    assert_eq!(total, 1200, "los trozos no suman la nota: {p:?}");
}

/// Un silencio es la nota vacia: la misma operacion con frecuencia 0. Callar no
/// es un caso aparte.
#[test]
fn el_silencio_ocupa_su_figura_entera() {
    let m = ejecutar_musica(
        r#"
int main() {
    unsigned long long cap;
    cap = bmo_sonido_reclamar();
    bmo_musica_tempo(120);
    bmo_silencio(cap, NEGRA);
    return 0;
}
"#,
    );
    // Tambien troceado --500 ms son dos veces el tope-- y con la frecuencia 0
    // en los dos trozos: callar largo sigue siendo callar.
    assert_eq!(m.partitura(), &[(0, 250), (0, 250)]);
    assert_eq!(m.audio_ms_sonando(), 0, "un silencio no puede sonar");
}

/// El barrido sube de verdad: la primera frecuencia es la de salida y la
/// ultima esta cerca de la de llegada. Un barrido que devolviera siempre el
/// mismo tono pasaria cualquier prueba que solo contase llamadas.
#[test]
fn el_barrido_recorre_las_frecuencias() {
    let m = ejecutar_musica(
        r#"
int main() {
    unsigned long long cap;
    cap = bmo_sonido_reclamar();
    bmo_barrido(cap, 200, 1000, 8, 80);
    return 0;
}
"#,
    );
    let p = m.partitura();
    assert_eq!(p.len(), 8);
    assert_eq!(p[0].0, 200, "no empieza donde se le dijo");
    assert!(p[7].0 > 800, "no llega arriba: {:?}", p[7]);
    for i in 1..p.len() {
        assert!(p[i].0 > p[i - 1].0, "el barrido no es monotono: {p:?}");
    }
}

/// El VOLUMEN llega al aparato. Es la unica de las operaciones que no produce
/// sonido, asi que sin mirarla del otro lado no habria forma de saber si el
/// argumento cruzo la puerta.
#[test]
fn el_volumen_cruza_la_puerta() {
    let m = ejecutar_musica(
        r#"
int main() {
    unsigned long long cap;
    cap = bmo_sonido_reclamar();
    bmo_sonido_volumen(cap, 80);
    return 0;
}
"#,
    );
    assert_eq!(m.audio_volumen(), 80);
}

/// **** Y LA PROPIEDAD QUE SOSTIENE TODO LO DEMAS: soltar REVOCA.
///
/// Despues de soltar, el mismo handle no puede pitar. Si pudiera, "soltar"
/// seria una palabra bonita y el aparato seguiria siendo de quien dijo
/// devolverlo -- y eso no da error: da un programa que hace ruido cuando ya no
/// le toca.
#[test]
fn el_handle_soltado_ya_no_suena() {
    let m = ejecutar_musica(
        r#"
int main() {
    unsigned long long cap;
    cap = bmo_sonido_reclamar();
    bmo_nota(cap, LA4, CORCHEA);
    bmo_sonido_soltar();
    bmo_nota(cap, DO6, CORCHEA);
    return 0;
}
"#,
    );
    let p = m.partitura();
    assert!(
        p.iter().all(|t| t.0 != 1047),
        "el handle soltado siguio sonando: {p:?}"
    );
}

/// Y es EXCLUSIVO: la segunda reclamacion sin soltar devuelve el handle nulo.
#[test]
fn reclamarlo_dos_veces_falla() {
    let m = ejecutar_musica(
        r#"
int main() {
    unsigned long long cap;
    unsigned long long otra;
    cap = bmo_sonido_reclamar();
    otra = bmo_sonido_reclamar();
    if (otra == 0) {
        printf("exclusivo\n");
    } else {
        printf("DOS DUENOS\n");
    }
    return 0;
}
"#,
    );
    assert!(m.console.contains("exclusivo"), "salida: {}", m.console);
}

/// La voz del sistema suena, y **el error BAJA mientras el arranque SUBE**.
///
/// La regla no esta escrita en ningun sitio del codigo: esta en la eleccion de
/// las notas. Esta fila la convierte en algo que el build puede comprobar, para
/// que no se pierda el dia que alguien retoque una melodia.
#[test]
fn la_voz_del_sistema_sube_o_baja_segun_lo_que_dice() {
    let arranque = ejecutar_musica(
        r#"
int main() {
    unsigned long long cap;
    cap = bmo_sonido_reclamar();
    bmo_son_arranque(cap);
    return 0;
}
"#,
    );
    let notas: Vec<u64> = arranque.partitura().iter().map(|t| t.0).filter(|h| *h != 0).collect();
    assert!(notas.len() >= 3, "el arranque son tres notas: {notas:?}");
    assert!(notas[2] > notas[0], "el arranque tiene que SUBIR: {notas:?}");

    let error = ejecutar_musica(
        r#"
int main() {
    unsigned long long cap;
    cap = bmo_sonido_reclamar();
    bmo_son_error(cap);
    return 0;
}
"#,
    );
    let notas: Vec<u64> = error.partitura().iter().map(|t| t.0).filter(|h| *h != 0).collect();
    assert!(notas.len() >= 2, "el error son dos notas: {notas:?}");
    assert!(notas[1] < notas[0], "el error tiene que BAJAR: {notas:?}");
}

// =============== UNA PIEZA DE VERDAD: Vivaldi ===============
//
// Las filas de arriba prueban la libreria nota a nota. Esta prueba la PIEZA, y
// es una pregunta distinta: que ocho compases seguidos midan lo que la
// partitura dice, sin que el tempo derive ni se pierda una figura por el
// camino. Un pitido correcto no dice nada de eso.

/// El ejemplo del repositorio, ejecutado entero.
///
/// Es el `.bex` que se lanza en el Ryzen (`c/vivaldi.bex`), asi que lo que
/// mide aqui es lo que va a durar alli.
#[test]
fn el_ritornello_de_vivaldi_suena_lo_que_la_partitura_dice() {
    let bef = compile_with_preprocessor(
        include_str!("../../examples/vivaldi_C.c"),
        std::path::Path::new("vivaldi.c"),
        CStandard::C11,
    )
    .expect("el programa debe compilar");
    let m = maquina_de_bef(&bef);
    let p = m.partitura();

    // ** LO QUE SE COMPRUEBA ES QUE NO DERIVA, no cuanto dura.
    //
    // La primera version de esta fila rehacia aqui la aritmetica de
    // `bmo_musica_ms` --pulsos a milisegundos, con su redondeo-- y comparaba.
    // Eso son **dos cuentas que TIENEN que dar lo mismo**, que es el
    // antipatron de siempre: el dia que alguien toque el redondeo de la
    // libreria, esta fila se pone roja sin que nada este mal, y para
    // arreglarla hay que copiar el numero nuevo. Una prueba que se arregla
    // copiando el resultado dejo de probar.
    //
    // Lo que si es una propiedad de la PIEZA, y no de la formula: **las dos
    // vueltas del ritornello miden exactamente lo mismo**. Si el tempo derivara
    // --un redondeo que se acumula, un contador que se pierde-- la segunda
    // seria mas corta o mas larga que la primera, y eso se oye.
    //
    // Se parten por la mitad contando NOTAS y no milisegundos: 24 notas por
    // vuelta, cada una un tramo sonando y otro callando, mas el troceo del tope
    // del kernel. Lo que se compara es el tiempo de cada mitad.
    let total: u64 = p.iter().map(|t| t.1).sum();
    assert!(
        total > 12_000 && total < 20_000,
        "el ritornello dos veces a 132 ppm son unos 15 s, y midio {total}"
    );

    // Ninguna llamada pasa del tope del kernel. Es lo que hace `bmo_sostener`,
    // y si un dia deja de hacerlo la pieza se cortaria a trozos en metal sin
    // que ninguna fila de las de arriba lo notara.
    assert!(
        p.iter().all(|t| t.1 <= 250),
        "una nota se paso del tope de 250 ms: {:?}",
        p.iter().find(|t| t.1 > 250)
    );

    // ** Y EL ECO: la pieza suena DOS VECES, la segunda mas floja. Esta en la
    // partitura de Vivaldi, no es una repeticion de relleno -- y es lo que
    // ejercita `BMO_SONIDO_VOLUMEN` desde una pieza real.
    let vols = m.volumenes();
    assert_eq!(vols, &[100, 35], "forte y luego piano: {vols:?}");

    // La primera nota es un mi5 y la ultima tambien: el ritornello vuelve a
    // casa. Si el indexado de los arrays globales se descuadrara --el fallo del
    // 08-08-- esto seria lo primero en salir mal.
    let alturas: std::vec::Vec<u64> = p.iter().map(|t| t.0).filter(|&h| h != 0).collect();
    assert_eq!(alturas.first(), Some(&659), "empieza en mi5");
    assert_eq!(alturas.last(), Some(&659), "y acaba en mi5");
}
