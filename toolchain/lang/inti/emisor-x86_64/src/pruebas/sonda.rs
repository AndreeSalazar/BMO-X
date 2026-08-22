//! LA SONDA DEL RYZEN, calibrada antes de medir con ella.
//!
//! El instrumento que va a sacar numeros de una maquina que no podemos mirar.
//! Si el formateador estuviera mal, la culpa pareceria del CPU.

use super::*;
// ===================================================================
//  ** LA SONDA DEL RYZEN, calibrada antes de medir con ella
// ===================================================================
//
//  `sondas/cpu.inti` va a correr en un procesador de verdad y sacar numeros por
//  la consola. Si el FORMATEADOR estuviera mal, esos numeros serian basura y la
//  culpa pareceria del CPU.
//
//  ** Un instrumento se calibra antes de medir con el. Eso es esto.
//
//  Y se calibra con EL FICHERO DE VERDAD, no con una copia de sus funciones: se
//  lee `cpu.inti`, se le quita su `principal` y se le pone otro que solo
//  ejercita la maquinaria. Una copia se separaria del original en la primera
//  correccion, y entonces esta prueba aprobaria un formateador que ya no es el
//  que va al metal.



/// **LA CALIBRACION**: un numero conocido tiene que salir con sus dieciseis
/// digitos, en orden y en minusculas.
///
/// ** El orden es la parte que se rompe sin avisar. Los caracteres van
/// empaquetados little-endian --el primero en el byte BAJO-- porque asi es como
/// el kernel lee la palabra. Al reves saldria el numero del reves, y un numero
/// del reves parece un fallo del procesador.
#[test]
fn la_sonda_del_cpu_imprime_el_numero_que_es() {
    let fuente = format!(
        "{}funcion principal devuelve entero32\n    hex(81985529216486895)\n    devuelve 0\n",
        maquinaria_de_cpu()
    );
    let m = arranca(&fuente);
    let dice = lo_escrito(&m);
    assert_eq!(
        dice,
        vec!["01234567", "89abcdef"],
        "0x0123456789ABCDEF tiene que salir en hexadecimal y en orden"
    );
}

/// Los bordes: el cero y el mas grande.
///
/// ** El cero es el que caza un formateador que "optimiza" quitando los ceros de
/// delante: aqui NO se quitan, porque un informe con columnas de ancho fijo se
/// lee de un vistazo y uno de ancho variable no.
#[test]
fn los_dos_bordes_salen_enteros() {
    let de = |v: &str| {
        let fuente = format!(
            "{}funcion principal devuelve entero32\n    hex({})\n    devuelve 0\n",
            maquinaria_de_cpu(),
            v
        );
        lo_escrito(&arranca(&fuente))
    };
    assert_eq!(de("0"), vec!["00000000", "00000000"], "el cero no se encoge");
    assert_eq!(
        de("18446744073709551615"),
        vec!["ffffffff", "ffffffff"],
        "el mas grande sale entero"
    );
}

/// ** LAS CUENTAS DE BITS DAN CERO, que es lo unico del informe que se puede
/// aprobar o suspender sin mirar un manual.
///
/// Las demas lineas dicen lo que el CPU diga --y no hay contra que compararlas--
/// pero estas tienen respuesta conocida: si sale algo distinto de cero, el CPU y
/// el compilador no estan de acuerdo, y cada bit dice en cual.
///
/// Que se compruebe AQUI y no solo en el metal es lo que hace util la linea del
/// informe: si el emulador ya dice cero, un cero en el Ryzen confirma; y un
/// numero distinto en el Ryzen senala al silicio y no a la sonda.
#[test]
fn las_cuentas_de_bits_de_la_sonda_dan_cero_en_el_emulador() {
    let fuente = format!(
        "{}funcion principal devuelve entero32\n    devuelve entero32(prueba_bits())\n",
        maquinaria_de_cpu()
    );
    let m = arranca(&fuente);
    let ultima = m.syscalls.last().expect("no salio por la puerta");
    assert_eq!(
        ultima.arg0, 0,
        "una cuenta de bits no cuadra; cada bit del numero dice cual"
    );
}

/// Y las etiquetas son texto legible, no numeros que parezcan texto.
///
/// ** Se escriben a mano en decimal --en `llano` no hay `texto`, porque un texto
/// crece y crecer pide monton-- y una constante mal calculada da ocho bytes
/// perfectamente validos que en la pantalla son basura. Esta prueba las lee.
#[test]
fn las_etiquetas_de_la_sonda_se_leen() {
    let fuente = format!(
        "{}funcion principal devuelve entero32\n    \
         palabra(et_vendor())\n    palabra(et_tsc())\n    palabra(et_bits())\n    \
         palabra(et_atom())\n    palabra(et_xcr0())\n    palabra(et_fin())\n    devuelve 0\n",
        maquinaria_de_cpu()
    );
    let dice = lo_escrito(&arranca(&fuente));
    assert_eq!(dice.len(), 6, "faltan etiquetas");
    for (i, e) in dice.iter().enumerate() {
        assert!(
            e.chars().all(|c| c.is_ascii_graphic() || c == ' '),
            "la etiqueta {} no es texto legible: {:?}",
            i,
            e
        );
        assert!(!e.is_empty(), "la etiqueta {} sale vacia", i);
    }
}

/// **Y LA SONDA ENTERA COMPILA Y PASA EL GATE.**
///
/// ** No se puede EJECUTAR aqui --llama a `lee_reloj` y a `que_cpu_eres`, que el
/// emulador no contesta a proposito-- pero que compile y pase el gate es lo que
/// separa "hay un fichero que llevar al Ryzen" de "hay un fichero".
#[test]
fn la_sonda_entera_compila_y_pasa_el_gate() {
    let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("sondas")
        .join("cpu.inti");
    let texto = std::fs::read_to_string(&p).expect("no puedo leer cpu.inti");
    let e = emitido(&texto);
    assert!(
        e.sin_emitir.is_empty(),
        "hay cosas que no llegan a un byte: {:?}",
        e.sin_emitir
    );
    assert!(e.arranca, "la sonda tiene que arrancar sola");
    let bytes = empaquetar(&e).expect("el `.bex` no pasa el gate");
    assert_eq!(&bytes[..4], b"BEF1");
}

/// ** M3: LAS TRES REGLAS ATRAPAN, y con SU codigo.
///
/// Es la linea que decide si *"INTI no tiene comportamiento indefinido"* es
/// verdad o es una frase. Aqui se comprueba en el emulador; en el Ryzen lo
/// contesta el silicio.
///
/// Que se compruebe en los DOS sitios es el punto: si el emulador dice cero y el
/// metal dice otra cosa, el sospechoso es el emulador -- y ya se ha equivocado
/// tres veces este mes. Si los dos dicen cero, la frase se sostiene.
#[test]
fn las_tres_reglas_de_la_sonda_atrapan_en_el_emulador() {
    let fuente = format!(
        "{}funcion principal devuelve entero32
    devuelve entero32(prueba_reglas())
",
        maquinaria_de_cpu()
    );
    let m = arranca(&fuente);
    let salio = m.syscalls.last().expect("no salio por la puerta").arg0;
    assert_eq!(
        salio, 0,
        "una regla no atrapo. bit 0 = desborde, bit 1 = entre cero, bit 2 = conversion"
    );
}

/// Y cada una por separado, para que el numero de arriba diga DONDE cuando
/// falle. Un cero agregado que se rompe sin decir cual no sirve de nada.
#[test]
fn cada_regla_de_la_sonda_devuelve_su_codigo() {
    for (fn_inti, codigo) in [
        ("desborda", 1001u64),
        ("entre_cero", 1003),
        ("convierte_de_mas", 1012),
    ] {
        let fuente = format!(
            "{}funcion principal devuelve entero32
    devuelve entero32({}())
",
            maquinaria_de_cpu(),
            fn_inti
        );
        let m = arranca(&fuente);
        assert_eq!(
            m.syscalls.last().unwrap().arg0,
            codigo,
            "`{}` tenia que atrapar con {}",
            fn_inti,
            codigo
        );
    }
}
