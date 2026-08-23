//! El censo, barrido de verdad.
//!
//! ## Que puede y que no puede comprobar esto todavia
//!
//! `CENSO.md` declara 42 sondas con su veredicto escrito por delante. La
//! mayoria de esos veredictos son de fases que **no existen aun** (`E0030` es
//! del analisis de nombres, `E0080` del de tareas). Comprobarlos hoy seria
//! fingir.
//!
//! Lo que si se puede comprobar hoy, y es exactamente lo que hace este fichero:
//!
//! 1. **Que las 42 sondas se pueden leer**, y que ninguna lleva un fallo de
//!    escritura escondido -- margenes torcidos, comillas sin cerrar, signos de
//!    otro lenguaje.
//! 2. **Que las que declaran un veredicto LEXICO lo cumplen ya.**
//!
//! El primer punto ya se gano el sitio antes de correr: las primeras 38 estaban
//! escritas con **tres** espacios de sangria y la gramatica dice **cuatro**. El
//! documento y el corpus llevaban dos dias sin estar de acuerdo, y nadie lo
//! habria visto leyendo.

use std::path::{Path, PathBuf};

use bmo_inti_front::{barrer, Clase};

fn carpeta() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("censo")
}

fn sondas() -> Vec<(String, String)> {
    let mut v: Vec<(String, String)> = std::fs::read_dir(carpeta())
        .expect("no encuentro censo/")
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().map(|x| x == "inti").unwrap_or(false))
        .map(|p| {
            let nombre = p.file_stem().unwrap().to_string_lossy().to_string();
            let texto = std::fs::read_to_string(&p).expect("no puedo leer la sonda");
            (nombre, texto)
        })
        .collect();
    v.sort();
    v
}

/// El veredicto que la propia sonda declara en su primera linea.
fn veredicto(texto: &str) -> String {
    let primera = texto.lines().next().unwrap_or("");
    match primera.split("espera:").nth(1) {
        Some(t) => t.trim().to_string(),
        None => String::new(),
    }
}

#[test]
fn el_censo_tiene_las_sondas_que_dice() {
    assert_eq!(sondas().len(), 43, "el numero del censo y el de la carpeta");
}

/// Cada sonda declara su veredicto en la primera linea, para que la sonda y su
/// expectativa no se puedan separar.
#[test]
fn todas_las_sondas_declaran_su_veredicto() {
    for (nombre, texto) in sondas() {
        assert!(
            !veredicto(&texto).is_empty(),
            "{} no dice que espera en su primera linea",
            nombre
        );
    }
}

/// Ninguna sonda lleva un fallo de escritura escondido.
///
/// Se excluyen las dos que existen justamente para llevarlo: `s03` trae un
/// tabulador y `s05` una comilla simple. Que la lista de excepciones sea
/// EXACTA importa -- si manana otra sonda empieza a fallar en el barrido,
/// tiene que romper este test y no colarse en la excepcion de al lado.
#[test]
fn ninguna_sonda_lleva_un_fallo_de_escritura() {
    const LO_LLEVAN_A_PROPOSITO: &[(&str, &str)] =
        &[("s03_tabulador", "E0010"), ("s05_comilla_simple", "E0011")];

    for (nombre, texto) in sondas() {
        let c = barrer(&texto);
        let codigos = c.codigos();

        match LO_LLEVAN_A_PROPOSITO.iter().find(|(n, _)| *n == nombre) {
            Some((_, esperado)) => assert!(
                codigos.contains(esperado),
                "{} tenia que dar {} y dio {:?}",
                nombre,
                esperado,
                codigos
            ),
            None => assert!(
                codigos.is_empty(),
                "{} deberia barrerse limpia y dio {:?}\n{}",
                nombre,
                codigos,
                c.pintar(&format!("{}.inti", nombre))
            ),
        }
    }
}

/// Toda sonda empieza por `perfil`, porque el lenguaje no tiene perfil por
/// defecto -- salvo `s02`, que existe para probar justamente que falta.
#[test]
fn toda_sonda_declara_su_perfil() {
    for (nombre, texto) in sondas() {
        if nombre == "s02_sin_perfil" {
            continue;
        }
        let piezas = barrer(&texto).valor;
        let primera = piezas
            .iter()
            .find(|p| !matches!(p.clase, Clase::FinLinea | Clase::Sangra | Clase::Desangra))
            .expect("una sonda vacia");
        assert!(
            primera.es(bmo_inti_front::Simbolo::Perfil),
            "{} no empieza por `perfil`, empieza por {}",
            nombre,
            primera.como_se_llama()
        );
    }
}

/// Las dos familias del corpus tienen que estar representadas: si algun dia se
/// borran todas las sondas de `llano`, el lenguaje habria dejado de tener dos
/// perfiles sin que nadie lo dijera.
#[test]
fn el_corpus_cubre_los_dos_perfiles() {
    let mut llano = 0;
    let mut pleno = 0;
    for (_, texto) in sondas() {
        let piezas = barrer(&texto).valor;
        for (i, p) in piezas.iter().enumerate() {
            if p.es(bmo_inti_front::Simbolo::Perfil) {
                match piezas.get(i + 1) {
                    Some(q) if q.es(bmo_inti_front::Simbolo::Llano) => llano += 1,
                    Some(q) if q.es(bmo_inti_front::Simbolo::Pleno) => pleno += 1,
                    _ => {}
                }
                break;
            }
        }
    }
    assert!(llano >= 5, "pocas sondas de `llano`: {}", llano);
    assert!(pleno >= 10, "pocas sondas de `pleno`: {}", pleno);
}

/// Las sondas, pasadas por la GRAMATICA y no solo por el barrido.
///
/// Aqui solo se comprueban las que declaran `COMPILA`: las que esperan un
/// codigo de una fase que aun no existe (`E0030` es del analisis de nombres)
/// no se pueden juzgar todavia, y fingir que si seria peor que no mirarlas.
#[test]
fn las_sondas_que_dicen_compila_se_leen_enteras() {
    for (nombre, texto) in sondas() {
        let v = veredicto(&texto);
        if !v.starts_with("COMPILA") {
            continue;
        }
        let c = bmo_inti_front::leer(&texto);
        assert!(
            !c.hay_errores(),
            "{} dice COMPILA y no se lee:\n{}",
            nombre,
            c.pintar(&format!("{}.inti", nombre))
        );
    }
}

/// Las sondas que YA se pueden juzgar, comprobadas de verdad.
///
/// Estas ya no son promesas: declaran un codigo que alguna fase sabe dar hoy,
/// asi que se exige. Las demas siguen esperando a la suya, y la lista crece
/// cada vez que una fase aprende algo -- que es la forma de que el censo mida

/// Y las que dicen COMPILA siguen sin dar ni un aviso al pasar por el perfil.
#[test]
fn las_sondas_que_compilan_pasan_el_perfil() {
    for (nombre, texto) in sondas() {
        if !veredicto(&texto).starts_with("COMPILA") {
            continue;
        }
        let c = bmo_inti_front::comprobar(&texto);
        // ** Los codigos del COMPILADOR no cuentan: `E0073` dice que no sabe
        // bajar `pleno` a bytes, y eso no rechaza el fuente -- lo rechaza a el.
        let del_programa: Vec<&str> = c
            .codigos()
            .into_iter()
            .filter(|x| x.starts_with('E') && !del_compilador(x))
            .collect();
        assert!(
            del_programa.is_empty(),
            "{} dice COMPILA y el perfil la rechaza:
{}",
            nombre,
            c.pintar(&format!("{}.inti", nombre))
        );
    }
}

/// ** Lo que INTI le cuenta a CABINA de una sonda de verdad.
///
/// Es la prueba de que el puente no es un tipo suelto: una sonda del censo
/// entra por un lado y por el otro sale la foto que el sistema puede seguir en
/// el tiempo.
#[test]
fn una_sonda_le_cuenta_su_parte_a_cabina() {
    let (_, texto) = sondas()
        .into_iter()
        .find(|(n, _)| n == "p01_llano")
        .expect("falta p01_llano");

    let (parte, eventos) = bmo_inti_front::informar(&texto, "p01_llano.inti");

    assert_eq!(parte.perfil, "llano");
    assert_eq!(parte.arquitecturas, vec!["x86_64".to_string()]);
    assert_eq!(parte.bloques_crudo, 1, "la sonda tiene un `crudo`");

    // Todo va en la capa de los lenguajes, con el nombre de INTI.
    assert!(!eventos.is_empty());
    for e in &eventos {
        assert_eq!(e.module_str(), "inti");
    }
    // Y una sonda que compila no manda ni un fallo.
    assert!(
        !eventos
            .iter()
            .any(|e| e.severity == cabina_core::Severity::Fault),
        "p01_llano compila: no deberia haber fallos"
    );
}

// ===================================================================
//  ** EL CENSO ENTERO, y no diez elegidas a mano
// ===================================================================
//
//  Hasta el 2026-08-21 este fichero comprobaba **diez** sondas contra su
//  veredicto, escritas en una lista. Las otras treinta y dos declaraban un
//  veredicto que no miraba nadie.
//
//  Y una lista a mano tiene un fallo que no se ve: **no crece sola**. Cada vez
//  que el compilador aprendia algo nuevo, la sonda que ya se podia comprobar
//  seguia sin comprobarse hasta que alguien se acordara de anadirla. Nadie se
//  acordaba, porque nada fallaba.
//
//  ** Lo que costo: `v04_sin_veracidad` llevaba desde F0 SIN COMPILAR --usaba
//  `lista` como nombre de variable, y `lista` es palabra clave-- y daba tres
//  errores de sintaxis. Es el mismo fallo que `para` en la tabla de la maquina,
//  y sobrevivio por el mismo motivo: el corpus se miraba por encima (margenes,
//  comillas) y **nunca se le preguntaba si se LEE**.
//
//  Ahora se recorren las 42 y la lista es de EXENCIONES, no de inclusiones. Una
//  exencion hay que justificarla; una inclusion que falta no la echa de menos
//  nadie.

/// Las sondas cuyo veredicto pide una fase que todavia no existe.
///
/// ** Cada una con su motivo, y el motivo tiene que ser una FASE, no una excusa.
/// Si algun dia una de estas empieza a cumplir su veredicto, el test lo dice:
/// una exencion que ya no hace falta es tan mala como una que falta.
const TODAVIA_NO: &[(&str, &str)] = &[
    (
        "p05_paralelo_mutable",
        "E0080 pide el analisis de tareas, que nace con `paralelo` (F7)",
    ),
    (
        "r02_indice",
        "E0090 pide saber la LONGITUD, y un `bufer` no la lleva. Nace con `lista de T` de `pleno`",
    ),
    (
        "v03_sin_nulo",
        "E0021 pide `quiza T`, que no esta construido",
    ),
    (
        "v04_sin_veracidad",
        "E0040 existe y funciona en `llano` (ver v07). Aqui es `pleno`, donde `tipos` no entra todavia",
    ),
    (
        "v05_sin_conversion",
        "E0022 existe y funciona en `llano` (ver v06). Aqui es texto + numero, que es el modelo de `pleno`",
    ),
];

/// Codigos que hablan del COMPILADOR y no del programa.
///
/// `E0073` dice *"no se bajar esto a bytes todavia"*. No es un fallo del fuente:
/// una sonda que declara `COMPILA` y usa `numero` sigue siendo correcta **como
/// programa**, y lo que no esta es el descenso.
///
/// Mezclarlos haria una de dos cosas, las dos malas: o el censo entero se pone
/// rojo por una limitacion temporal, o se afloja el criterio y deja de mirar lo
/// que si es del programa.
///
/// ## ** LO QUE CAMBIO EL 2026-08-23, y casi me lleva a borrar esta lista
///
/// `E0073` decia *"no se bajar este PERFIL"* -- una etiqueta -- y al hacerse el
/// gate ATOMICO paso a decir *"no se bajar esta PIEZA"*. La prueba que vigilaba
/// la exencion se puso roja, como estaba escrito que pasaria, y la primera
/// lectura fue *"caduco, fuera"*.
///
/// **Y era media verdad.** `perfil pleno` a secas ya no da `E0073` -- eso si
/// caduco. Pero `numero` y `tabla` siguen sin bajar, y una sonda que los use
/// sigue siendo un programa correcto contra un compilador incompleto.
///
/// *** Asi que la exencion se queda, y lo que mejora es su PRECISION: antes
/// tapaba un perfil entero; ahora tapa exactamente las piezas que faltan, y el
/// dia que bajen deja de aparecer sola.
const DEL_COMPILADOR: &[&str] = &["E0073"];

fn del_compilador(codigo: &str) -> bool {
    DEL_COMPILADOR.contains(&codigo)
}

fn exenta(nombre: &str) -> Option<&'static str> {
    TODAVIA_NO
        .iter()
        .find(|(n, _)| *n == nombre)
        .map(|(_, por_que)| *por_que)
}

/// **LA MATRIZ ENTERA**: cada sonda contra el veredicto que ella misma declara.
///
/// ## Los tres tipos de veredicto, y que se puede exigir de cada uno
///
/// ```text
///    COMPILA          no puede salir ni un error
///    Exxxx            ese codigo tiene que estar
///    Exxxx ejecucion  tiene que COMPILAR limpio; que atrape lo prueba el
///                     banco del emisor, que es quien puede ejecutar
/// ```
///
/// ** La tercera fila es la que mas se presta a enganar. Exigir aqui que atrape
/// seria imposible --este test no ejecuta nada-- pero **no exigir nada la
/// dejaria pasar aunque no compilara**, que es exactamente lo que le pasaba a
/// `r12_conversion`: escribia `escribe(...)` en `perfil llano`, donde no existe,
/// asi que daba E0070 al compilar y no llegaba nunca a la parte que probaba.
#[test]
fn cada_sonda_cumple_el_veredicto_que_declara() {
    let mut fallan: Vec<String> = Vec::new();
    let mut sobran: Vec<String> = Vec::new();
    let mut miradas = 0usize;

    for (nombre, texto) in sondas() {
        let v = veredicto(&texto);
        let c = bmo_inti_front::comprobar(&texto);
        let cods = c.codigos();
        let errores: Vec<&str> = cods
            .iter()
            .copied()
            .filter(|x| x.starts_with('E') && !del_compilador(x))
            .collect();

        let cumple = if v.starts_with("COMPILA") {
            errores.is_empty()
        } else if v.contains("ejecucion") {
            // Compila limpio; atrapar se prueba donde se puede ejecutar.
            errores.is_empty()
        } else if v.starts_with('E') || v.starts_with('A') {
            cods.contains(&&v[..5])
        } else {
            true
        };

        match (cumple, exenta(&nombre)) {
            (true, None) => miradas += 1,
            (true, Some(por_que)) => sobran.push(format!(
                "{} ya cumple `{}` y sigue exenta.\n      motivo escrito: {}",
                nombre, v, por_que
            )),
            (false, Some(_)) => {}
            (false, None) => fallan.push(format!(
                "{} dice `{}` y da {:?}\n{}",
                nombre,
                v,
                cods,
                c.pintar(&format!("{}.inti", nombre))
            )),
        }
    }

    assert!(
        fallan.is_empty(),
        "{} sonda(s) no cumplen lo que declaran:\n\n{}",
        fallan.len(),
        fallan.join("\n")
    );
    // ** Y la otra direccion, que es la que impide que la lista se pudra: una
    // exencion que ya no hace falta miente sobre lo que el compilador sabe
    // hacer, y ademas tapa el caso el dia que se rompa.
    assert!(
        sobran.is_empty(),
        "{} exencion(es) ya no hacen falta -- quitalas de TODAVIA_NO:\n  {}",
        sobran.len(),
        sobran.join("\n  ")
    );

    assert!(
        miradas >= 37,
        "solo se comprobaron {} sondas de {}",
        miradas,
        sondas().len()
    );
}

/// ** TODA SONDA SE TIENE QUE PODER LEER, incluidas las que declaran un error.
///
/// Es la prueba que faltaba y la que habria cazado `v04` el primer dia. Una
/// sonda que declara `E0040` y muere con tres `E0017` de sintaxis **no esta
/// probando la regla que dice probar**: esta probando que el parser se queja, y
/// eso ya lo prueba otro.
///
/// La distincion es fina y es la que importa: un error de SINTAXIS en una sonda
/// que espera un error SEMANTICO significa que la sonda esta mal escrita, no que
/// el lenguaje funcione.
#[test]
fn ninguna_sonda_muere_en_el_parser_salvo_las_que_lo_buscan() {
    // Las que SI buscan un error de sintaxis, por su codigo declarado.
    const DE_SINTAXIS: &[&str] = &["E0001", "E0010", "E0011", "E0017"];

    for (nombre, texto) in sondas() {
        let v = veredicto(&texto);
        if DE_SINTAXIS.iter().any(|c| v.starts_with(c)) {
            continue;
        }
        let c = bmo_inti_front::comprobar(&texto);
        for codigo in c.codigos() {
            assert!(
                !DE_SINTAXIS.contains(&codigo),
                "{} declara `{}` y no se lee: da {} en el parser.\n{}\n\
                 Una sonda que muere en el parser no prueba la regla que dice probar.",
                nombre,
                v,
                codigo,
                c.pintar(&format!("{}.inti", nombre))
            );
        }
    }
}

/// *** LA EXENCION DE `E0073` CADUCO, Y ESTA PRUEBA LO CELEBRA (2026-08-23).
///
/// Se llamaba `la_excusa_de_pleno_sigue_haciendo_falta` y exigia lo contrario:
/// que `perfil pleno` siguiera dando `E0073`, para que el dia que dejara de
/// darlo **se enterara alguien**. Se entero: se puso roja sola.
///
/// ## Lo que cambio, y es el gate atomico
///
/// `E0073` decia *"no se bajar este PERFIL"* -- una etiqueta, y con ella se
/// rechazaba el programa entero. Ahora dice *"no se bajar esta PIEZA"*, y solo
/// para a quien la use.
///
/// ** Asi que un `pleno` que solo toca `texto` y `lista` **compila**, y uno que
/// toca `tabla` se para con el nombre de `tabla` y la linea donde aparece.
///
/// [!] Y la prueba se queda, del reves: si `perfil pleno` a secas volviera a dar
/// `E0073`, seria que el gate ha vuelto a mirar el pasaporte.
#[test]
fn pleno_a_secas_ya_no_se_rechaza_por_su_etiqueta() {
    let c = bmo_inti_front::comprobar("perfil pleno

funcion principal
    devuelve 0
");
    assert!(
        !c.codigos().contains(&"E0073"),
        "el gate volvio a mirar el perfil en vez de las piezas: {:?}",
        c.codigos()
    );
}

/// Y lo que SI sigue parando: usar una pieza que no baja.
///
/// ** Es la otra mitad del gate atomico, y la que lo hace MAS estricto que el
/// viejo: el dia que se abriera por perfil, esto habria pasado con `tabla`
/// devolviendo ceros.
#[test]
fn una_pieza_que_no_baja_para_el_programa_con_su_nombre() {
    let c = bmo_inti_front::comprobar("perfil pleno

funcion f(indice es tabla de texto a entero64)
    devuelve
");
    assert!(c.codigos().contains(&"E0073"), "{:?}", c.codigos());
    let texto = c.pintar("prueba.inti");
    assert!(texto.contains("`tabla`"), "el aviso no dice que pieza es:
{}", texto);
}
