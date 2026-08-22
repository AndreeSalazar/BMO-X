//! EL METAL -- la tabla de la maquina deja de ser decorativa.
//!
//! La matriz de conformidad --cada nombre de la tabla emite sus bytes-- y hasta
//! donde llega el emulador. Las dos preguntan lo mismo desde dos sitios: **lo
//! que la tabla promete, se cumple?**

use super::*;
// ===================================================================
//  ** F5d -- EL METAL. La tabla de la maquina deja de ser decorativa.
// ===================================================================
//
//  Lo que estaba roto, dicho sin adornos: `Instr::Metal { .. } => {}`.
//
//  La tabla de x86-64 llevaba desde F2b con setenta y tantos nombres --los
//  puertos, los registros de control, las atomicas, las cuentas de bits-- y
//  NINGUNO llegaba a un byte. Peor: el descenso ni siquiera generaba
//  `Instr::Metal`, asi que `lee_reloj()` se bajaba a una LLAMADA a un simbolo
//  que no existe. Compilaba, pasaba nombres, pasaba perfiles, pasaba el gate.
//
//  Es el fallo que este proyecto persigue desde el principio, y esta vez con
//  cuatro capas de por medio: la pieza que se calcula bien y no la lee nadie.


/// **LA MATRIZ DE CONFORMIDAD**: cada nombre de la tabla de la maquina se
/// compila, y se exige que salgan bytes.
///
/// ## ** Por que esto tenia que existir, y lo dice la propia tabla
///
/// El comentario de `Intrinsics::names()` lo lleva pidiendo desde que se
/// escribio: *"sin poder recorrerla, una fila con el nombre de un registro mal
/// escrito no falla hasta que alguien la usa -- y 'alguien la usa' en una tabla
/// de driver puede ser dentro de seis meses y en metal"*.
///
/// Esto es eso, recorrido entero. Y no comprueba que los bytes sean los
/// correctos --eso lo hacen las pruebas de abajo, una a una-- sino algo mas
/// basico y que nadie miraba: **que salga alguno**.
#[test]
fn cada_nombre_de_la_maquina_emite_bytes() {
    let taller = Taller::nuevo();
    let maquina = taller.maquina.as_ref().expect("sin tabla de maquina");
    let intrinsecos = taller.intrinsecos.as_ref().expect("sin tabla de bytes");

    let mut mudos: Vec<String> = Vec::new();
    let mut probados = 0usize;

    for nombre in maquina.nombres_que_trae() {
        let Some(instruccion) = maquina.instruccion(&nombre) else {
            mudos.push(format!("{}: la maquina no dice que instruccion es", nombre));
            continue;
        };
        let Some(def) = intrinsecos.get(instruccion) else {
            mudos.push(format!(
                "{} -> `{}`: no esta en intrinsics.toml",
                nombre, instruccion
            ));
            continue;
        };
        let e = emitido(&llamada_a(&nombre, def.args.len()));
        if !e.sin_emitir.is_empty() {
            mudos.extend(e.sin_emitir.iter().cloned());
            continue;
        }
        // Y que los bytes de la instruccion esten DE VERDAD dentro. Sin esto,
        // un camino que no emitiera nada y tampoco se quejara pasaria.
        assert!(
            e.codigo
                .windows(def.bytes.len())
                .any(|w| w == def.bytes.as_slice()),
            "`{}` compila y sus bytes no aparecen: {:02X?}",
            nombre,
            def.bytes
        );
        probados += 1;
    }

    assert!(
        mudos.is_empty(),
        "{} nombre(s) de la tabla no llegan a un byte:\n  {}",
        mudos.len(),
        mudos.join("\n  ")
    );
    assert!(probados >= 60, "solo se probaron {} nombres", probados);
}

/// Y la otra mitad: `usa binarios`, que es la que SE PORTA.
///
/// ** La diferencia con la de arriba no es tecnica --las dos acaban en la misma
/// instruccion en esta maquina-- es de DECLARACION: quien escribe `usa
/// binarios` dice *"esto se porta"*, y quien escribe `usa x86_64` dice *"esto
/// no"*. El compilador no elige por ti cual de las dos cosas quieres decir.
///
/// Que los seis salgan aqui es lo que hace verdad esa frase. Si uno no saliera,
/// `usa binarios` seria una promesa de portabilidad sobre algo que no compila.
#[test]
fn los_binarios_portables_emiten_todos() {
    let taller = Taller::nuevo();
    let modulos = bmo_inti_front::tablas::Modulos::cargar(&bmo_mods::Roots::find());
    let nombres = modulos.trae("binarios");
    assert!(!nombres.is_empty(), "el modulo `binarios` esta vacio");

    let maquina = taller.maquina.as_ref().expect("sin tabla de maquina");
    let intrinsecos = taller.intrinsecos.as_ref().expect("sin tabla de bytes");
    let mut mudos = Vec::new();
    for n in nombres {
        let cuantos = maquina
            .instruccion(n)
            .and_then(|i| intrinsecos.get(i))
            .map(|d| d.args.len())
            .unwrap_or(1);
        let e = emitido(&llamada_a(n, cuantos));
        mudos.extend(e.sin_emitir.iter().cloned());
    }
    assert!(
        mudos.is_empty(),
        "`usa binarios` promete portabilidad sobre algo que no emite:\n  {}",
        mudos.join("\n  ")
    );
}

// -------------------------------------------------------------------
//  Y que hagan lo que dicen, no solo que salgan bytes
// -------------------------------------------------------------------

/// `cuenta_unos` cuenta bits de verdad, corriendo.
///
/// ** Es la prueba que la matriz NO puede dar. La matriz dice que los bytes
/// estan; esta dice que son los bytes correctos. Las dos hacen falta: una fila
/// con los bytes de otra instruccion pasa la primera y falla esta.
#[test]
fn cuenta_unos_cuenta_bits() {
    let f = "\
perfil llano
usa binarios

funcion f(a es entero64, b es entero64) devuelve entero64
    crudo
        devuelve cuenta_unos(a)
";
    assert_eq!(ejecuta(f, 0, 0), 0);
    assert_eq!(ejecuta(f, 1, 0), 1);
    assert_eq!(ejecuta(f, 0xFF, 0), 8);
    assert_eq!(ejecuta(f, 0b1010_1010, 0), 4);
}

/// `ceros_detras` sobre una potencia de dos da el exponente.
#[test]
fn ceros_detras_encuentra_el_bit_bajo() {
    let f = "\
perfil llano
usa binarios

funcion f(a es entero64, b es entero64) devuelve entero64
    crudo
        devuelve ceros_detras(a)
";
    assert_eq!(ejecuta(f, 1, 0), 0);
    assert_eq!(ejecuta(f, 8, 0), 3);
    assert_eq!(ejecuta(f, 256, 0), 8);
}

/// ** Y la que demuestra que la lectura del valor esta bien hecha: `lee_reloj`
/// devuelve SESENTA Y CUATRO bits, partidos en dos registros de 32 por el
/// silicio.
///
/// Recogerlo de uno solo da la mitad baja -- y la mitad baja de un contador de
/// ciclos parece un numero perfectamente razonable. Es exactamente el fallo que
/// `invoca_valor` tuvo el primer dia, en otro sitio.
#[test]
fn lee_reloj_junta_las_dos_mitades() {
    let f = "\
perfil llano
usa x86_64

funcion f(a es entero64, b es entero64) devuelve entero64
    crudo
        devuelve lee_reloj()
";
    let e = emitido(f);
    assert!(e.sin_emitir.is_empty(), "{:?}", e.sin_emitir);
    // Los dos bytes de la instruccion, y detras el desplazamiento que junta las
    // mitades. Sin el segundo, la instruccion esta y el numero es la mitad.
    assert!(
        e.codigo.windows(2).any(|w| w == [0x0F, 0x31]),
        "no se emitio la instruccion"
    );
    assert!(
        e.codigo
            .windows(4)
            .any(|w| w == [0x48, 0xC1, 0xE2, 0x20]),
        "la mitad alta no se junta: el reloj devolveria solo 32 bits"
    );
}

/// Una instruccion sin argumentos y sin resultado deja un CERO, no basura.
///
/// ** Entre dos cosas mal, la que no cambia entre ejecuciones. `x = para()` no
/// tiene sentido y se puede escribir; con basura el programa sigue con un
/// numero que parece valido y distinto cada vez.
#[test]
fn lo_que_no_devuelve_nada_deja_cero() {
    let f = "\
perfil llano
usa x86_64

funcion f(a es entero64, b es entero64) devuelve entero64
    crudo
        devuelve nada_que_hacer()
";
    assert_eq!(ejecuta(f, 0xDEAD, 0xBEEF), 0);
}

/// ** Y LA PRUEBA DE QUE LA MATRIZ MUERDE: un nombre que no esta en la tabla no
/// se emite en silencio, se apunta.
///
/// Una lista de fallos que nunca se llena no vigila nada. Aqui se llena a
/// proposito, pidiendo una aridad que la instruccion no tiene.
#[test]
fn lo_que_no_se_puede_emitir_se_apunta() {
    let f = "\
perfil llano
usa x86_64

funcion f(a es entero64, b es entero64) devuelve entero64
    crudo
        devuelve lee_reloj(1, 2, 3)
";
    let e = emitido(f);
    assert!(
        !e.sin_emitir.is_empty(),
        "una llamada con la aridad mal tenia que apuntarse"
    );
    assert!(
        e.sin_emitir[0].contains("lee_reloj"),
        "el apunte no dice de quien es: {:?}",
        e.sin_emitir
    );
}


// ===================================================================
//  ** HASTA DONDE LLEGA EL EMULADOR -- y donde empieza el metal
// ===================================================================
//
//  Peticion de Eddi (2026-08-21): *"intenta completar todo lo que el emulador
//  llegue hasta donde pueda... y si el emulador no deja claro el metal lo
//  pruebe"*.
//
//  ** La trampa que esto evita es concreta y este proyecto ya la ha pisado: un
//  banco que solo prueba lo que el emulador sabe **se lee como si probara
//  todo**. Las instrucciones que el emulador no conoce simplemente no tienen
//  test, y no tener test se parece mucho a estar bien.
//
//  Asi que se recorre la tabla ENTERA, se ejecuta cada una, y las que el
//  emulador no sabe se quedan escritas AQUI con su nombre. La lista es una
//  constante y se compara entera, igual que el censo: si crece sin que nadie lo
//  decida, el test lo dice.
//
//  == Y por que el emulador no las sabe, que no es un descuido suyo ==
//
//  Porque no hay nada que emular. `wbinvd` tira una cache que aqui no existe;
//  `cpuid` describe un silicio que aqui no hay; `lgdt` carga una tabla que solo
//  significa algo con una MMU detras. Emularlas seria inventarse una respuesta,
//  y una respuesta inventada en un banco es peor que no tener banco.
//
//  ** Lo que estas van a necesitar es el Ryzen. Y ahora estan contadas, que era
//  la diferencia entre "pendiente" y "olvidado".

/// Las que el emulador NO sabe ejecutar, y por eso su prueba es el metal.
///
/// ** Ordenada y comparada ENTERA. Si el dia de manana una que hoy corre deja de
/// correr, o una nueva se cuela, el test no dice "algo cambio": dice cual.
const SOLO_EN_METAL: &[&str] = &[
    "azar", "azar_de_verdad", "cambia_gs", "carga_gdt", "carga_idt",
    "carga_ldt", "carga_tr", "duerme_hasta", "entrada_puerto",
    "entrada_puerto16", "entrada_puerto32", "escribe_banderas",
    "escribe_cr0", "escribe_cr3", "escribe_cr4", "escribe_msr",
    "escribe_puerto", "escribe_puerto16", "escribe_puerto32",
    "escribe_xcr", "lee_banderas", "lee_cr0", "lee_cr2", "lee_cr3",
    "lee_cr4", "lee_gdt", "lee_idt", "lee_msr", "lee_reloj",
    "lee_reloj_serio", "lee_xcr", "olvida_pagina", "que_cpu_eres",
    "tira_cache_sin_escribir", "tira_la_cache", "vigila",
];

/// Emite una llamada a cada nombre de la maquina y **la ejecuta**.
///
/// Devuelve `(las que corrieron, las que el emulador no supo)`.
fn hasta_donde_llega() -> (Vec<String>, Vec<String>) {
    let taller = Taller::nuevo();
    let maquina = taller.maquina.as_ref().expect("sin tabla de maquina");
    let intrinsecos = taller.intrinsecos.as_ref().expect("sin tabla de bytes");

    let mut corren = Vec::new();
    let mut no = Vec::new();

    // El emulador se queja con un panico cuando no conoce un opcode, que es lo
    // correcto para el --callarse seria ejecutar otra cosa-- pero aqui hay que
    // recogerlo. Se silencia el mensaje: setenta panicos esperados en la salida
    // esconden el uno que no lo era.
    let anterior = std::panic::take_hook();
    std::panic::set_hook(Box::new(|_| {}));

    for nombre in maquina.nombres_que_trae() {
        let Some(def) = maquina.instruccion(&nombre).and_then(|i| intrinsecos.get(i)) else {
            continue;
        };
        let fuente = llamada_a(&nombre, def.args.len());
        let e = emitido(&fuente);
        if !e.sin_emitir.is_empty() {
            continue;
        }
        let codigo = con_arranque(&e);
        let salio = std::panic::catch_unwind(move || {
            let m = Machine::new(codigo);
            let m = run(m, 10_000);
            m.regs[0]
        });
        match salio {
            Ok(_) => corren.push(nombre.clone()),
            Err(_) => no.push(nombre.clone()),
        }
    }

    std::panic::set_hook(anterior);
    corren.sort();
    no.sort();
    (corren, no)
}


/// **EL MAPA**: que parte de la tabla de la maquina se puede probar aqui, y que
/// parte hay que llevar al Ryzen.
#[test]
fn el_emulador_llega_hasta_donde_dice_la_lista() {
    let (corren, no) = hasta_donde_llega();

    assert_eq!(
        no,
        SOLO_EN_METAL
            .iter()
            .map(|s| s.to_string())
            .collect::<Vec<_>>(),
        "\nla frontera entre el emulador y el metal se movio.\n\
         corren aqui: {}\n\
         solo en metal: {:?}\n\
         Si el cambio es a proposito, actualiza SOLO_EN_METAL. Si no, alguien \
         emitio bytes distintos.",
        corren.len(),
        no
    );

    // ** Y un SUELO, para que la parte probable no se encoja sin que nadie lo
    // decida. Hoy son 25 de 61 -- menos de la mitad, y esa es la cifra
    // incomoda de este fichero: **la mayor parte de la libreria de la maquina
    // no se puede verificar aqui**.
    //
    // No por un fallo del emulador: por su regla. Devolver un cero como si
    // fuera el valor de un registro de control seria inventarse un dato, y un
    // emulador que inventa datos es peor que uno que no los tiene.
    //
    // Sin este suelo, alguien podria romper 20 de las 25 y el test seguiria en
    // verde: la lista de arriba solo vigila las que FALLAN.
    assert!(
        corren.len() >= 25,
        "solo {} nombres se pueden probar aqui, de {}",
        corren.len(),
        corren.len() + no.len()
    );
}
