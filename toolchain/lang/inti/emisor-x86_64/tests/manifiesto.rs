//! **P1: el perfil viaja dentro del `.bex`.**
//!
//! ## Que se comprueba aqui y no en el banco de dentro
//!
//! El banco del frontend comprueba que el manifiesto se escribe y se relee.
//! Eso prueba el TEXTO. Lo que se prueba aqui es lo otro: que el texto llega al
//! FICHERO, que el header lo anuncia, que el gate lo acepta, y que **nada de
//! esto toca un solo byte del codigo**.
//!
//! ** Y la ultima es la que de verdad importa: `cpu.bex` es el fichero que va
//! al Ryzen. Si el manifiesto cambiara la emision, la medida del 22-08 dejaria
//! de comparar con lo mismo y no lo diria nadie.

use std::path::PathBuf;
use std::process::Command;

use bmo_abi::bmo_abi::bef::paquete;
use bmo_abi::bmo_abi::bef::sections::SectionKind;
use bmo_inti_front::manifiesto::Manifiesto;

fn caja(nombre: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("inti-manifiesto-{}", nombre));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).expect("no puedo crear la caja");
    d
}

fn compila(fuente: &PathBuf) -> Vec<u8> {
    let s = Command::new(env!("CARGO_BIN_EXE_inti"))
        .arg(fuente.to_str().unwrap())
        .output()
        .expect("no puedo ejecutar el compilador");
    assert!(
        s.status.success(),
        "no compilo:\n{}{}",
        String::from_utf8_lossy(&s.stdout),
        String::from_utf8_lossy(&s.stderr)
    );
    std::fs::read(fuente.with_extension("ibex")).expect("no hay `.bex`")
}

/// El fuente trae `usa monton`, que es lo unico que hoy mete piezas de verdad.
const CON_PIEZAS: &str = "\
perfil llano
usa bmo
usa x86_64
usa monton

funcion principal devuelve entero32
    crudo
        m = monton_nuevo(4096)
    devuelve 0
";

/// **SE PUEDE SABER QUE ES UN `.bex` SIN VER UNA LINEA DE FUENTE.**
///
/// Es el criterio de aprobado de P1, escrito como una prueba.
#[test]
fn el_bex_dice_su_perfil_su_crudo_y_de_que_esta_hecho() {
    let d = caja("declara");
    let fuente = d.join("prog.inti");
    std::fs::write(&fuente, CON_PIEZAS).unwrap();
    let bex = compila(&fuente);

    let seccion = paquete::seccion(&bex, SectionKind::Manifest)
        .expect("el `.bex` no trae seccion Manifest");
    let texto = std::str::from_utf8(seccion).expect("el manifiesto no es UTF-8");
    let m = Manifiesto::de_toml(texto).unwrap_or_else(|| panic!("no se parsea:\n{}", texto));

    assert_eq!(m.lenguaje, "inti");
    assert_eq!(m.perfil, "llano");
    // *** CUATRO, y el autor escribio UNO. Los otros tres vinieron dentro de
    // las piezas del monton.
    //
    // Ese es el numero honrado y es justo el que no se podia ver antes: el
    // medidor no dice *"cuantas ventanas sin comprobar abriste"*, dice
    // **cuantas trae este binario** -- y un `usa` mete las suyas. Se fija aqui
    // para que el dia que cambie, cambie a proposito.
    //
    // *** ERAN 4 Y SON 6 (2026-08-23). El monton crecio dos bloques `crudo`:
    // `queda_suelto`, que es nuevo, y `suelta`, que antes era un `devuelve 0`
    // sin tocar memoria y ahora enhebra la lista de huecos.
    //
    // ** Que este numero suba al hacer el monton mas capaz **es la propiedad, no
    // el problema**: `crudo` cuenta los sitios donde nadie comprueba por ti, y un
    // repartidor de memoria es exactamente eso. Lo que no puede pasar es que suba
    // sin que nadie se entere -- y por eso esta fijado aqui.
    assert_eq!(
        m.crudo, 6,
        "uno del fuente y cinco de las piezas del monton: {:?}",
        m
    );
    assert!(
        m.arquitecturas.iter().any(|a| a == "x86_64"),
        "declaro `usa x86_64` y el manifiesto no lo dice: {:?}",
        m.arquitecturas
    );
    // ** Y las costuras: de que esta hecho, con el perfil de cada trozo.
    assert!(
        !m.piezas.is_empty(),
        "`usa monton` trae piezas y el manifiesto no las declara"
    );
    assert!(
        m.piezas.iter().all(|p| p.usa == "monton"),
        "las piezas no dicen quien las trajo: {:?}",
        m.piezas
    );
    assert!(
        m.piezas.iter().all(|p| !p.perfil.is_empty()),
        "una pieza sin perfil declarado no sirve para la regla del mezclado"
    );
}

/// **UN FUENTE QUE SE PORTA LO DICE, Y NO CON UN HUECO.**
///
/// Vacio es una respuesta, no una ausencia: este binario no se ata a ninguna
/// maquina. Sin la prueba, un dia la lista saldria vacia por un fallo y se
/// leeria igual.
#[test]
fn un_fuente_portable_declara_la_lista_vacia() {
    let d = caja("portable");
    let fuente = d.join("puro.inti");
    std::fs::write(
        &fuente,
        "perfil llano\n\nfuncion principal devuelve entero32\n    devuelve 0\n",
    )
    .unwrap();
    let bex = compila(&fuente);
    let texto =
        std::str::from_utf8(paquete::seccion(&bex, SectionKind::Manifest).unwrap()).unwrap();
    let m = Manifiesto::de_toml(texto).unwrap();
    assert!(m.arquitecturas.is_empty(), "{:?}", m.arquitecturas);
    assert_eq!(m.crudo, 0);
    assert!(m.piezas.is_empty());
}

/// **EL HEADER LO ANUNCIA, Y NO PORQUE ALGUIEN SE ACORDARA.**
///
/// `HAS_MANIFEST` la enciende `BefBuilder::build()` al ver la seccion. Sin la
/// bandera el binario seria correcto por dentro y mudo por fuera: *"un
/// consumidor que se fie de la bandera no la mirara"*.
#[test]
fn el_header_anuncia_el_manifiesto_y_el_gate_no_se_queja() {
    let d = caja("bandera");
    let fuente = d.join("prog.inti");
    std::fs::write(&fuente, CON_PIEZAS).unwrap();
    let bex = compila(&fuente);

    // `flags` vive en el byte 8: magic(4) + major(2) + minor(2).
    const HAS_MANIFEST: u32 = 1 << 2;
    let flags = u32::from_le_bytes(bex[8..12].try_into().unwrap());
    assert!(
        flags & HAS_MANIFEST != 0,
        "el header no anuncia el manifiesto: flags = {:#010x}",
        flags
    );

    let (veredicto, avisos) = bmo_verify::verify_verbose(&bex);
    assert!(veredicto.is_ok(), "el gate lo rechaza: {:?}", veredicto);
    assert!(
        !avisos.iter().any(|a| a.contains("no lo anuncia")),
        "el validador se queja del manifiesto: {:?}",
        avisos
    );
}

/// **Y LO MISMO SOBRE LA SONDA DE VERDAD, que es la que vuela.**
///
/// *** `cpu.bex` es el fichero que se lleva al Ryzen. La prueba de al lado usa
/// un fuente de test; esta usa **el que se entrega**, porque una garantia sobre
/// un fuente parecido no es una garantia sobre el fichero que arranca.
#[test]
fn la_sonda_del_ryzen_emite_los_mismos_bytes_que_antes_de_p1() {
    // La ruta es relativa a la raiz del paquete, que es donde cargo pone el CWD.
    let cpu = PathBuf::from("../sondas/cpu.inti");
    let texto = std::fs::read_to_string(&cpu).expect("no encuentro la sonda");

    let arbol = bmo_inti_front::armar(&texto);
    let raices = bmo_mods::Roots::find();
    let modulos = bmo_inti_front::tablas::Modulos::cargar(&raices);
    let plano = bmo_inti_front::disposicion::comprobar(
        &arbol.valor,
        bmo_inti_front::disposicion::Medidas::cargar(&raices),
    );
    let metal = bmo_inti_front::ir::metal_que_declara(&arbol.valor, &raices, &modulos);
    let ir = bmo_inti_front::ir::bajar_con(&arbol.valor, &modulos, &plano.valor, &metal).valor;
    let emitido = bmo_inti_x86_64::emitir(&ir);

    let sin = bmo_inti_x86_64::empaquetar(&emitido, None).expect("el gate lo rechazo");
    let manifiesto = bmo_inti_front::manifiesto::de(
        &arbol.valor,
        &bmo_inti_front::comprobar(&texto).valor,
        "cpu.inti",
    )
    .a_toml();
    let con = bmo_inti_x86_64::empaquetar(&emitido, Some(&manifiesto)).expect("el gate lo rechazo");

    assert_eq!(
        paquete::seccion(&sin, SectionKind::Code),
        paquete::seccion(&con, SectionKind::Code),
        "el manifiesto cambio el codigo de la sonda"
    );
    // ** LA LINEA BASE, Y SE MOVIO A PROPOSITO EL 2026-08-22.
    //
    //     8.752   hasta que se anadio la regla del cociente
    //     8.856   con ella: +104 bytes, la guardia de `-2^63 entre -1` en cada
    //             division que la sonda hace
    //
    // *** Este numero NO se toca para que un test pase. Se movio porque el
    // binario lleva una regla mas de verdad, y la diferencia esta contada:
    // `reglas emitidas` subio en el informe.
    //
    // ** Y tiene una consecuencia que hay que decir: `cpu.ibex` ya no es el
    // fichero que corrio en el Ryzen el 22-08. Hace lo mismo y una cosa mas, y
    // la proxima medida se compara contra ESTE.
    //
    //     10.432  con el monton que SUELTA de verdad (2026-08-23)
    //
    // *** Y ESTOS +1.576 BYTES SON EL PRECIO DE "INCLUSION, NO ENLAZADO",
    // medido por primera vez en algo real.
    //
    // La sonda escribe `usa monton`, asi que **lleva el monton entero dentro**.
    // Hacer que `suelta` suelte le anadio dos bucles y una funcion, y eso
    // engorda A CADA PROGRAMA que use el monton -- no solo a los que sueltan.
    //
    // ** `MONTON.md` ya lo tenia escrito en su seccion 5: *"diez programas que
    // usen el monton llevan diez copias, que es literalmente lo que la seccion
    // 13c del maestro le critica a Go"*. Esto es esa frase con un numero
    // detras, y la respuesta tambien esta escrita: el runtime es codigo que no
    // cambia, o sea CONGELADO, y lo congelado en BMO-X **se presta en vez de
    // copiarse**. El dia que exista compilacion separada, este numero baja.
    //
    // [!] Y la consecuencia de siempre: `cpu.ibex` vuelve a no ser el fichero
    // que corrio en el Ryzen. La proxima medida se compara contra ESTE.
    assert_eq!(sin.len(), 10432, "la emision de la sonda cambio de tamano");
}

/// **EL CODIGO NO CAMBIA POR LLEVAR MANIFIESTO.**
///
/// *** La prueba que protege la medida del Ryzen. `cpu.bex` es el fichero que
/// se lleva a la maquina; si el manifiesto tocara la emision, las cifras del
/// 22-08 dejarian de comparar con lo mismo **y no lo diria nadie**.
#[test]
fn la_seccion_de_codigo_es_identica_con_manifiesto_y_sin_el() {
    let d = caja("codigo");
    let fuente = d.join("prog.inti");
    std::fs::write(&fuente, CON_PIEZAS).unwrap();
    let con = compila(&fuente);

    // El mismo modulo, empaquetado sin manifiesto: es lo que se escribia antes.
    let texto = std::fs::read_to_string(&fuente).unwrap();
    let arbol = bmo_inti_front::armar(&texto);
    let raices = bmo_mods::Roots::find();
    let modulos = bmo_inti_front::tablas::Modulos::cargar(&raices);
    let plano = bmo_inti_front::disposicion::comprobar(
        &arbol.valor,
        bmo_inti_front::disposicion::Medidas::cargar(&raices),
    );
    let metal = bmo_inti_front::ir::metal_que_declara(&arbol.valor, &raices, &modulos);
    let ir = bmo_inti_front::ir::bajar_con(&arbol.valor, &modulos, &plano.valor, &metal).valor;
    let emitido = bmo_inti_x86_64::emitir(&ir);
    let sin = bmo_inti_x86_64::empaquetar(&emitido, None).expect("el gate lo rechazo");

    let a = paquete::seccion(&con, SectionKind::Code).expect("sin seccion Code");
    let b = paquete::seccion(&sin, SectionKind::Code).expect("sin seccion Code");
    assert_eq!(
        a.len(),
        b.len(),
        "el manifiesto cambio el TAMANO del codigo: {} contra {}",
        a.len(),
        b.len()
    );
    assert_eq!(a, b, "el manifiesto cambio los BYTES del codigo");
}

/// **LO QUE SE CARGA SIGUE EMPEZANDO EN FRONTERA DE SECTOR.**
///
/// ** Es la condicion que deja al disco escribir una seccion directamente en
/// los marcos del proceso. Anadir una seccion mueve todos los offsets del
/// fichero, asi que es exactamente el invariante que este cambio podia romper
/// -- y el sintoma aparaceria lejos de aqui: un `.bex` que no carga desde
/// disco, meses despues.
#[test]
fn anadir_el_manifiesto_no_rompe_la_frontera_de_sector() {
    let d = caja("sector");
    let fuente = d.join("prog.inti");
    std::fs::write(&fuente, CON_PIEZAS).unwrap();
    let bex = compila(&fuente);

    for clase in [SectionKind::Code, SectionKind::RoData, SectionKind::Data] {
        if let Some((off, _)) = paquete::localizar(&bex, clase) {
            assert_eq!(
                off % 512,
                0,
                "la seccion {:?} empieza en {} y no es multiplo de 512",
                clase,
                off
            );
        }
    }
}
