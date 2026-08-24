//! **EL MONTON GRANDE**: que un programa pueda decir cuanta memoria necesita.
//!
//! ## Lo que estas pruebas fijan, y por que aqui y no en el frontend
//!
//! `necesita monton 64 megas` tiene que llegar a DOS sitios, y los dos estan de
//! este lado:
//!
//! ```text
//!    el inmediato del arranque    lo que se le pide al kernel al empezar
//!    la seccion `Requisitos`      lo que el CARGADOR lee antes de arrancar
//! ```
//!
//! ** Comprobar solo el primero dejaria pasar el fallo caro: un programa que
//! pide bien y **no lo declara**, o sea que arranca donde no cabe y muere en su
//! primera reserva -- exactamente lo que hacia antes de existir la palabra.

use super::*;
use bmo_abi::bef::{requisitos, SectionKind};

/// Un programa de `pleno` que junta dos textos: el minimo que pide monton.
///
/// ** La forma es la de `objetos::el_arranque_monta_el_monton` y no una
/// parecida: es la que se sabe que emite `Instr::MontonDeLaTarea`, que es lo
/// unico que hace que el arranque monte monton. Una prueba del monton escrita
/// sobre un programa que no lo pide no prueba nada -- y ademas **pasa**, que es
/// lo que la hace peligrosa.
fn con(cabecera: &str) -> String {
    format!(
        "perfil pleno\nusa objetos\nusa monton\n{cabecera}\n\n\
         funcion principal\n    a = \"ho\"\n    b = \"la\"\n    c = a + b\n"
    )
}

fn bytes_de(fuente: &str) -> Vec<u8> {
    let e = emitido(fuente);
    empaquetar(&e, None).expect("el paquete tiene que salir")
}

/// Saca los bytes de una seccion por su tipo, leyendo la tabla como la lee el
/// cargador. Sin esto estas pruebas mirarian offsets a mano.
fn seccion(bytes: &[u8], kind: SectionKind) -> Option<&[u8]> {
    use bmo_abi::bef::SectionEntry;
    let count = u32::from_le_bytes(bytes[40..44].try_into().ok()?) as usize;
    let tabla = u64::from_le_bytes(bytes[32..40].try_into().ok()?) as usize;
    for i in 0..count {
        let e = tabla + i * SectionEntry::SIZE;
        if bytes[e] != kind as u8 {
            continue;
        }
        let off = u64::from_le_bytes(bytes[e + 8..e + 16].try_into().ok()?) as usize;
        let len = u64::from_le_bytes(bytes[e + 16..e + 24].try_into().ok()?) as usize;
        return bytes.get(off..off + len);
    }
    None
}

/// *** SIN DECIR NADA, SIGUE SIENDO 4096 -- y eso es lo que no podia romperse.
///
/// ** El monton grande vale de poco si el precio lo paga todo el mundo. Un
/// driver de `llano` que no toca un objeto no monta monton siquiera, y un
/// programa de `pleno` que no pide nada tiene que seguir pidiendo una pagina.
#[test]
fn quien_no_dice_nada_sigue_pidiendo_una_pagina() {
    let e = emitido(&con(""));
    assert!(
        e.codigo
            .windows(4)
            .any(|w| w == (arranque::MONTON_POR_DEFECTO as u32).to_le_bytes()),
        "el 4096 tiene que seguir ahi cuando nadie pide otra cosa"
    );
    assert!(
        e.necesita.is_empty(),
        "y no se declara nada que el cargador tenga que contestar"
    );
}

/// **LO DECLARADO LLEGA AL INMEDIATO.** 64 megas son 67.108.864 bytes.
#[test]
fn lo_que_se_pide_es_lo_que_se_pide() {
    let e = emitido(&con(
        "necesita monton 64 megas \"los pesos del modelo viven en RAM\"",
    ));
    let cuanto: u32 = 64 * 1024 * 1024;
    assert!(
        e.codigo.windows(4).any(|w| w == cuanto.to_le_bytes()),
        "no aparece el tamano declarado en el arranque"
    );
    assert!(
        !e.codigo
            .windows(4)
            .any(|w| w == (arranque::MONTON_POR_DEFECTO as u32).to_le_bytes()),
        "y el 4096 ya no puede estar: seria pedir dos veces cosas distintas"
    );
}

/// *** Y LLEGA AL `.bex`, que es la mitad que de verdad cambia el trato.
#[test]
fn lo_declarado_se_escribe_en_el_paquete() {
    let bytes = bytes_de(&con(
        "necesita monton 64 megas \"los pesos del modelo viven en RAM\"",
    ));
    let sec = seccion(&bytes, SectionKind::Requisitos)
        .expect("un programa que declara tiene que traer su seccion");
    let t = requisitos::Tabla::abrir(sec).expect("la tabla tiene que abrirse");

    // ** DOS, y el otro no es mio. El escritor del `.bex` deduce solo lo que
    // puede --`CLASE_MEMORIA`: codigo, datos y ceros de la imagen-- y deja
    // entrar por `requerir()` lo que no puede saber. Lo dice en su propio
    // comentario, y el monton es exactamente de los segundos: cuanto va a
    // repartir una tarea en ejecucion no se ve mirando el fichero.
    assert_eq!(t.cuantos(), 2, "la memoria de la imagen, y mi monton");
    assert!(
        t.iter().any(|r| r.clase == requisitos::CLASE_MEMORIA),
        "la que deduce el escritor tiene que seguir estando"
    );

    let r = t
        .iter()
        .find(|r| r.clase == requisitos::CLASE_MONTON)
        .expect("el monton declarado tiene que estar");
    assert_eq!(r.unidad, requisitos::UNIDAD_BYTES);
    assert_eq!(r.cantidad, 64 * 1024 * 1024);
    assert!(r.es_obligatorio(), "sin el monton el programa no funciona");
    // ** EL MOTIVO VIAJA. Es lo unico que convierte un rechazo del cargador en
    // algo que se puede contestar, y por eso el ABI se niega a escribir un
    // requisito obligatorio sin el.
    assert_eq!(t.motivo(&r), "los pesos del modelo viven en RAM");
}

/// [!] Y UN PROGRAMA QUE NO DECLARA NADA **SIGUE TRAYENDO LA SECCION**, con lo
/// que el escritor deduce solo.
///
/// ** Se comprueba para que quede escrito de que lado esta cada numero. La
/// memoria de la imagen --codigo, datos, ceros-- la sabe el fichero mirandose,
/// y por eso lleva declarandose desde antes de que existiera `necesita`. Lo que
/// la palabra nueva anade es lo que el fichero NO puede saber de si mismo.
#[test]
fn quien_no_declara_nada_trae_solo_lo_que_el_escritor_deduce() {
    let bytes = bytes_de(&con(""));
    let sec = seccion(&bytes, SectionKind::Requisitos).expect("siempre esta");
    let t = requisitos::Tabla::abrir(sec).unwrap();
    assert_eq!(t.cuantos(), 1);
    assert_eq!(t.requisito(0).unwrap().clase, requisitos::CLASE_MEMORIA);
}

/// *** LOS NUMEROS DE `necesidades.toml` SON LOS DEL ABI.
///
/// ** Esta prueba vive aqui y no en el frontend porque `bmo-inti-front` **no
/// enlaza `bmo-abi` a proposito** -- lo dice su `Cargo.toml`: *"F1 no emite
/// bytes"*. Alli no hay con que comparar; aqui si.
///
/// Son dos sitios diciendo el mismo numero --uno es contrato de fichero, el
/// otro es el nombre del lenguaje-- y dos sitios que dicen lo mismo se separan
/// el dia que alguien toca uno.
#[test]
fn la_tabla_del_lenguaje_y_el_abi_dicen_los_mismos_numeros() {
    let n = bmo_inti_front::necesidades::Necesidades::por_defecto();
    let esperado = [
        ("monton", requisitos::CLASE_MONTON, requisitos::UNIDAD_BYTES),
        ("recursos", requisitos::CLASE_RECURSOS, requisitos::UNIDAD_BYTES),
        ("pantalla", requisitos::CLASE_PANTALLA, requisitos::UNIDAD_UNIDADES),
        ("sonido", requisitos::CLASE_AUDIO, requisitos::UNIDAD_UNIDADES),
        ("entrada", requisitos::CLASE_ENTRADA, requisitos::UNIDAD_UNIDADES),
        ("procesos", requisitos::CLASE_PROCESOS, requisitos::UNIDAD_UNIDADES),
    ];
    for (nombre, numero, unidad) in esperado {
        let c = n
            .clase(nombre)
            .unwrap_or_else(|| panic!("falta la clase `{nombre}` en necesidades.toml"));
        assert_eq!(c.numero, numero, "el numero de `{nombre}` se separo del ABI");
        assert_eq!(c.unidad, unidad, "la unidad de `{nombre}` se separo del ABI");
        assert!(
            requisitos::clase_conocida(c.numero),
            "el ABI no reconoce la clase `{nombre}`"
        );
    }
}

/// Y el respaldo del arranque vale lo mismo que la tabla.
///
/// ** Dos sitios con el mismo numero otra vez, y el mismo remedio. El de
/// `arranque.rs` es el que se usa si nadie carga la tabla; el dia que dejen de
/// coincidir, una tarea pediria una cosa u otra segun por donde se compilara.
#[test]
fn el_respaldo_del_arranque_es_el_de_la_tabla() {
    let n = bmo_inti_front::necesidades::Necesidades::por_defecto();
    assert_eq!(arranque::MONTON_POR_DEFECTO, n.monton_por_defecto());
}

/// *** PEDIR MAS DEL TECHO SE DICE AL COMPILAR, no se recorta en silencio.
///
/// ** Recortarlo habria sido lo comodo: el programa arranca y "va tirando". Y
/// entonces un programa que necesita 100 GiB corre con lo que le den y falla
/// mucho mas tarde, sin relacion visible con su causa -- que es el fallo que
/// este proyecto persigue desde el primer dia.
#[test]
fn pedir_mas_del_techo_se_denuncia() {
    let fuente = con("necesita monton 900 gigas \"un modelo enorme\"");
    let c = bmo_inti_front::comprobar(&fuente);
    assert!(
        c.codigos().contains(&"E0133"),
        "tenia que denunciarse: {:?}",
        c.codigos()
    );
}

/// Un `necesita` sin motivo **no compila**, y el aviso sale en su linea.
#[test]
fn sin_motivo_no_hay_requisito() {
    let fuente = con("necesita monton 64 megas");
    let c = bmo_inti_front::comprobar(&fuente);
    assert!(
        c.codigos().contains(&"E0132"),
        "un requisito sin motivo no se puede contestar: {:?}",
        c.codigos()
    );
}

/// Una clase que no existe se contesta **con la lista de las que si**.
#[test]
fn una_clase_inventada_trae_la_lista() {
    let fuente = con("necesita mont0n 64 megas \"con un cero\"");
    let c = bmo_inti_front::comprobar(&fuente);
    assert!(c.codigos().contains(&"E0130"), "{:?}", c.codigos());
    let texto = c.pintar("prueba.inti");
    assert!(
        texto.contains("monton"),
        "el aviso tiene que decir cuales valen: {texto}"
    );
}

/// Y una unidad que no existe, igual.
#[test]
fn una_unidad_inventada_tambien() {
    let fuente = con("necesita monton 64 toneladas \"pesa mucho\"");
    let c = bmo_inti_front::comprobar(&fuente);
    assert!(c.codigos().contains(&"E0131"), "{:?}", c.codigos());
}

/// **DOS LINEAS PARA LA MISMA CLASE NO SE SUMAN.**
///
/// ** Sumarlas seria decidir por el programa; quedarse con una seria decidir
/// por el ORDEN, que es peor porque no se ve al leer.
#[test]
fn la_misma_clase_dos_veces_se_denuncia() {
    let fuente = con(
        "necesita monton 4 megas \"uno\"\nnecesita monton 8 megas \"otro\"",
    );
    let c = bmo_inti_front::comprobar(&fuente);
    assert!(c.codigos().contains(&"E0134"), "{:?}", c.codigos());
}

/// El orden entre `usa` y `necesita` **no significa nada**, asi que no puede
/// cambiar el resultado.
///
/// ** Con una funcion por palabra --que es como estaba escrito-- un fichero que
/// pusiera `necesita` antes que `usa` habria dejado el `usa` sin importar y sin
/// un aviso, porque el segundo lector no habria llegado a mirarlo.
#[test]
fn el_orden_de_la_cabecera_da_igual() {
    let a = "perfil pleno\nusa memoria\nnecesita monton 8 megas \"uno\"\n\
             funcion principal() devuelve entero64\n    s es texto = \"a\" + \"b\"\n    devuelve 0\n";
    let b = "perfil pleno\nnecesita monton 8 megas \"uno\"\nusa memoria\n\
             funcion principal() devuelve entero64\n    s es texto = \"a\" + \"b\"\n    devuelve 0\n";
    assert_eq!(emitido(a).codigo, emitido(b).codigo);
    assert_eq!(emitido(a).necesita, emitido(b).necesita);
}

/// *** UN MONTON POR ENCIMA DE 4 GiB CABE, y cambia la instruccion.
///
/// ** Es el caso que obligo al arranque a elegir el inmediato: hasta 4 GiB va
/// uno de 32 bits --cinco bytes, y se extiende con ceros, que es justo lo que
/// quiere un tamano-- y por encima hace falta uno de 64. Emitir siempre el
/// largo costaria cinco bytes en TODA tarea del sistema.
#[test]
fn por_encima_de_cuatro_gigas_el_inmediato_crece() {
    let cuanto: u64 = 8 * 1024 * 1024 * 1024;
    let e = emitido(&con("necesita monton 8 gigas \"no cabe en 32 bits\""));
    assert!(
        e.codigo.windows(8).any(|w| w == cuanto.to_le_bytes()),
        "los 8 GiB tienen que aparecer enteros, en un inmediato de 64 bits"
    );
}
