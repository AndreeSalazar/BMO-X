//! `texto` y `lista` en ejecucion: los dos objetos del monton.
//!
//! ** Salieron de `pruebas.rs` el 2026-08-23 por L6a. El corte es por TEMA: lo
//! que se prueba aqui es que un objeto se construye, se copia y se indexa **de
//! verdad** -- no que compile.
//!
//! Los ayudantes (`ejecuta_en`, `emitido`, `ir_de`) viven en el padre.

use super::*;
// ===================================================================
//  *** `texto + texto` PIDE MEMORIA DE VERDAD (2026-08-23)
// ===================================================================

/// Dos textos fabricados a mano en el monton, y el cuerpo con `a` y `b` a mano.
///
/// ** Se fabrican a mano y no con un literal a proposito: un literal vive en
/// `RoData`, es INMORTAL, y probar `junta` con dos inmortales no diria nada
/// sobre el caso que importa -- el del texto CONTADO que puede morir.
fn con_dos_textos(cuerpo: &str) -> String {
    format!(
        "perfil llano\nusa objetos\nusa monton\n\nfuncion prueba(base es natural64, cuantos es natural64) devuelve natural64\n    crudo\n        escribe_natural64(base, base + 32)\n        escribe_natural64(base + 8, base + 4096)\n        escribe_natural64(base + 16, 0)\n        a = pide(base, 26)\n        escribe_natural64(a, 1)\n        escribe_natural64(a + 16, 2)\n        escribe_natural8(a + 24, 104)\n        escribe_natural8(a + 25, 111)\n        b = pide(base, 27)\n        escribe_natural64(b, 1)\n        escribe_natural64(b + 16, 3)\n        escribe_natural8(b + 24, 108)\n        escribe_natural8(b + 25, 97)\n        escribe_natural8(b + 26, 33)\n{}",
        cuerpo
    )
}

/// ***JUNTAR DOS TEXTOS RESERVA, COPIA, Y EL RESULTADO ES VALIDO.***
///
/// `"ho" + "la!"` -> `"hola!"`, cinco bytes, un dueno.
///
/// ** Y reserva porque un texto es INMUTABLE: si `a + b` no puede tocar ni `a`
/// ni `b`, el resultado es un TERCER objeto. No es una torpeza que se arreglara
/// -- es la consecuencia de la linea que define el tipo.
#[test]
fn juntar_dos_textos_reserva_uno_nuevo_y_lo_copia_entero() {
    let f = con_dos_textos(
        "        c = junta(base, a, b)\n        si mide(c) no es 5\n            devuelve 0\n        si lee_natural8(c + 24) no es 104\n            devuelve 0\n        si lee_natural8(c + 25) no es 111\n            devuelve 0\n        si lee_natural8(c + 26) no es 108\n            devuelve 0\n        si lee_natural8(c + 27) no es 97\n            devuelve 0\n        si lee_natural8(c + 28) no es 33\n            devuelve 0\n        devuelve 1\n",
    );
    assert_eq!(ejecuta_en(&f, "prueba", 0x40000, 0), 1, "\"ho\" + \"la!\" no dio \"hola!\"");
}

/// ***Y EL RESULTADO LO VALIDA EL ABI DE RUST, no INTI.***
///
/// ** El programa deja el texto en memoria del emulador; aqui se sacan sus
/// bytes y se le pasan a `bmo_abi::dynobj::texto::revisar`, **que es otro
/// codigo**. Si `junta` validara su propio resultado estaria comprobando su
/// aritmetica contra si misma -- que es la trampa que `png.rs` ya dejo escrita.
///
/// `revisar` mira lo que INTI no mira: que el UTF-8 sea valido de verdad.
#[test]
fn el_texto_que_junta_construye_lo_acepta_el_abi() {
    use bmo_abi::dynobj::texto as abi;
    let f = con_dos_textos("        devuelve junta(base, a, b)\n");
    let e = emitido(&f);
    let inicio = e
        .inicios
        .iter()
        .find(|(n, _)| n == "prueba")
        .map(|(_, o)| *o)
        .expect("sin `prueba`");

    // El mismo `crt0` de diez bytes que usa `ejecuta`, para poder leer la
    // memoria DESPUES en vez de solo el registro de retorno.
    let largo = e.codigo.len() as i32;
    let mut codigo = Vec::new();
    codigo.push(0xE9);
    codigo.extend_from_slice(&largo.to_le_bytes());
    codigo.extend_from_slice(&e.codigo);
    codigo.push(0xE8);
    let desde = codigo.len() as i32 + 4;
    codigo.extend_from_slice(&((inicio as i32 + 5) - desde).to_le_bytes());

    let mut m = Machine::new(codigo);
    m.regs[7] = 0x40000;
    m.regs[6] = 0;
    let m = run(m, 200_000);
    let donde = m.regs[0];
    assert_ne!(donde, 0, "`junta` no devolvio nada");

    // Los bytes del objeto, tal cual quedaron en la memoria del emulador.
    let mut bytes = Vec::new();
    for i in 0..(abi::CABECERA_LEN as u64 + 5) {
        bytes.push(m.read_u8_pub(donde + i));
    }

    let t = abi::revisar(&bytes).expect("el ABI rechazo el texto que construyo INTI");
    assert_eq!(t.bytes, 5, "cinco bytes");
    assert_eq!(t.refs, 1, "nace con UN dueno, no inmortal: lo construyo alguien");
    assert_eq!(abi::contenido(&bytes).unwrap(), b"hola!");
}

/// *** LA CABECERA MIDE 24 EN LOS DOS SITIOS, y hay que exigirlo.
///
/// El 24 esta escrito en `texto.inti` a mano y en `dynobj::texto::CABECERA_LEN`
/// como constante. **Dos escrituras del mismo numero se separan el dia que
/// alguien toca una** -- es la misma razon por la que existe la prueba que hace
/// coincidir el contador de INTI con el del ABI.
///
/// Se comprueba de la unica forma que se puede desde fuera: si INTI usara otro
/// numero, los bytes del contenido no caerian donde el ABI los busca.
#[test]
fn el_desplazamiento_del_contenido_coincide_con_el_del_abi() {
    use bmo_abi::dynobj::texto as abi;
    let f = con_dos_textos(
        &format!("        c = junta(base, a, b)\n        devuelve lee_natural8(c + {})\n", abi::CABECERA_LEN),
    );
    // 104 es la `h` de "ho": el PRIMER byte del contenido.
    assert_eq!(ejecuta_en(&f, "prueba", 0x40000, 0), 104);
}

/// ***UNA CABECERA MENTIROSA NO DESBORDA EL BUFER: ATRAPA.***
///
/// ## Y esta prueba es la respuesta a una pregunta de Eddi
///
/// > *"TODOS los componentes deberian tener detector de UB?"*
///
/// Si `na + nb` diera la vuelta: `total` sale pequeno, `pide` devuelve un bloque
/// pequeno, y los dos bucles escriben **fuera** -- un desbordamiento de bufer,
/// el fallo mas caro de los ultimos veinte anos.
///
/// ** Y LO PARA EL PROPIO LENGUAJE. Aqui hubo una guardia escrita a mano con el
/// motivo *"la que `crudo` apago"*, y era falso: **`crudo` no apaga las reglas**.
/// Es un permiso para tocar el metal, no un interruptor.
///
/// *** Asi que la respuesta a la pregunta es esta: no hace falta que cada
/// componente traiga su detector. Hace falta que **el lenguaje no deje agujeros
/// que detectar** -- y que los sitios donde uno pide permiso para salirse SE
/// CUENTEN. El bloque `crudo` sale en el manifiesto del `.bex` con un numero; en
/// C todo el fichero es `crudo` y no hay numero que mirar.
#[test]
fn una_cabecera_mentirosa_atrapa_en_vez_de_desbordar() {
    // `a` dice medir casi 2^64. `na + nb` da la vuelta y `total` sale ridiculo.
    let f = con_dos_textos(
        "        escribe_natural64(a + 16, 18446744073709551615)\n        devuelve junta(base, a, b)\n",
    );
    assert_eq!(
        ejecuta_en(&f, "prueba", 0x40000, 0),
        1001,
        "la suma dio la vuelta y nadie la paro: eso escribe fuera del bloque"
    );
}

/// Comparar es BYTE A BYTE, y se dice en vez de insinuar otra cosa.
///
/// ** Dos textos que se ven iguales en pantalla pueden tener bytes distintos
/// --normalizacion Unicode-- y el maestro deja eso FUERA por escrito.
#[test]
fn dos_textos_se_comparan_por_sus_bytes() {
    let iguales = con_dos_textos(
        "        c = junta(base, a, b)\n        d = junta(base, a, b)\n        devuelve iguales(c, d)\n",
    );
    assert_eq!(ejecuta_en(&iguales, "prueba", 0x40000, 0), 1);

    let distintos = con_dos_textos("        devuelve iguales(a, b)\n");
    assert_eq!(ejecuta_en(&distintos, "prueba", 0x40000, 0), 0, "miden distinto");
}

// ===================================================================
//  *** `texto + texto` BAJA A UNA LLAMADA, no a un `add` (2026-08-23)
// ===================================================================


const DOS_TEXTOS: &str = "perfil pleno\n\nfuncion principal\n    a = \"ho\"\n    b = \"la\"\n    c = a + b\n";

/// ***SUMAR DOS TEXTOS NO ES SUMAR: ES RESERVAR Y COPIAR.***
///
/// Bajarlo a un `add` sumaria las DOS DIRECCIONES y devolveria un numero que no
/// apunta a ningun sitio. **Compilaria, correria, y daria basura** -- la misma
/// familia que el signo: una respuesta equivocada, en silencio.
///
/// ** Y hace falta reservar porque un `texto` es INMUTABLE: si `a + b` no puede
/// tocar ni `a` ni `b`, el resultado es un TERCER objeto. Eso es `junta`, y esta
/// escrita en INTI en `runtime/objetos/texto.inti`.
#[test]
fn sumar_dos_textos_baja_a_junta_y_no_a_una_suma() {
    let m = ir_de(DOS_TEXTOS);
    let instrs: Vec<&Instr> = m.funciones.iter().flat_map(|f| f.instrucciones.iter()).collect();

    let llama_a_junta = instrs.iter().any(|i| matches!(
        i,
        Instr::Llama { que: Valor::Nombre(n), .. } if n == "junta"
    ));
    assert!(llama_a_junta, "`a + b` de textos no llamo a `junta`");

    // Y NO hay ninguna suma entera de por medio: eso seria sumar punteros.
    let suma_entera = instrs.iter().any(|i| matches!(
        i,
        Instr::Binaria { op: bmo_inti_front::arbol::Op::Suma, .. }
    ));
    assert!(!suma_entera, "quedo un `add`: eso suma dos direcciones");
}

/// Y el monton llega por `Instr::MontonDeLaTarea`, no por la expresion.
///
/// ** Un operador no tiene hueco donde llevarlo: nadie escribe `a +(monton) b`.
/// Por eso el monton de la tarea es AMBIENTE, como en cualquier lenguaje con
/// objetos, y esta instruccion es por donde se coge.
#[test]
fn el_monton_llega_por_su_instruccion_y_es_el_primer_argumento() {
    let m = ir_de(DOS_TEXTOS);
    let instrs: Vec<&Instr> = m.funciones.iter().flat_map(|f| f.instrucciones.iter()).collect();

    let el_monton = instrs.iter().find_map(|i| match i {
        Instr::MontonDeLaTarea { destino } => Some(*destino),
        _ => None,
    });
    let el_monton = el_monton.expect("nadie pidio el monton de la tarea");

    let args = instrs.iter().find_map(|i| match i {
        Instr::Llama { que: Valor::Nombre(n), argumentos, .. } if n == "junta" => Some(argumentos),
        _ => None,
    }).expect("sin llamada a `junta`");

    assert_eq!(args.len(), 3, "junta(monton, a, b)");
    assert_eq!(
        args[0],
        Valor::Temporal(el_monton),
        "el monton tiene que ser el PRIMER argumento"
    );
}

/// ***EL ARRANQUE MONTA EL MONTON DE LA TAREA (2026-08-23).***
///
/// Decisiones de Eddi: **4096 bytes, y si falla la tarea muere.**
///
/// ```text
///    mov  edi, 4096
///    call monton_nuevo
///    test rax, rax
///    jnz  hay            -- si no, `exit(1004)` y no se llega a `principal`
///    mov  rcx, <slot>    -- inmediato a cero + reubicacion a `Data`
///    mov  [rcx], rax
///    call principal
/// ```
///
/// ** Muere ANTES de `principal` a proposito. Si se dejara seguir, el primer
/// `texto + texto` reservaria sobre un monton cero, `junta` devolveria 0, y el
/// fallo aparecerria paginas mas adelante sin relacion visible con su causa.
#[test]
fn el_arranque_monta_el_monton_y_ya_no_se_confiesa() {
    let f = "perfil pleno\nusa objetos\nusa monton\n\nfuncion principal\n    a = \"ho\"\n    b = \"la\"\n    c = a + b\n";
    let e = emitido(f);
    assert!(
        !e.sin_emitir.iter().any(|x| x.contains("monton de la tarea")),
        "el monton ya se monta, no hay nada que confesar: {:?}",
        e.sin_emitir
    );
    // [!] Los huecos de llamada NO se pueden mirar aqui: `emitir` los VACIA al
    // parchearlos. Que la llamada a `monton_nuevo` se resolvio se sabe por lo
    // contrario -- si no se hubiera resuelto, estaria en `sin_emitir`.
    assert!(
        !e.sin_emitir.iter().any(|x| x.contains("monton_nuevo")),
        "la llamada a `monton_nuevo` se quedo sin destino: {:?}",
        e.sin_emitir
    );
    // Mas los huecos que alcanzan el slot: uno por cada `a + b`.
    assert!(
        !e.reubicaciones_del_monton.is_empty(),
        "nadie apunta al slot del monton"
    );
    // El tamano y el codigo de muerte, en los bytes.
    assert!(
        e.codigo
            .windows(4)
            .any(|w| w == (arranque::MONTON_POR_DEFECTO as u32).to_le_bytes()),
        "no aparece el 4096 del monton de una tarea que no pidio otra cosa"
    );
    assert!(
        e.codigo
            .windows(8)
            .any(|w| w == arranque::SIN_MONTON.to_le_bytes()),
        "no aparece el codigo con el que muere si no hay monton"
    );
}

/// [!] Y UN PROGRAMA QUE NO TOCA OBJETOS NO PAGA EL MONTON.
///
/// ** Montarlo cuesta DOS cruces de la puerta. Se decide mirando la IR --si
/// nadie emitio `MontonDeLaTarea`, no hace falta-- y no el perfil, que seria
/// adivinar: hay programas de `pleno` que no tocan un objeto en su vida.
#[test]
fn un_programa_sin_objetos_no_monta_ningun_monton() {
    let e = emitido("perfil llano\n\nfuncion principal devuelve entero32\n    devuelve 7\n");
    assert!(
        !e.huecos_de_llamada
            .iter()
            .any(|(_, n)| n == arranque::MONTON_NUEVO),
        "monto un monton que nadie pidio"
    );
    assert!(e.reubicaciones_del_monton.is_empty());
}

// ===================================================================
//  *** LA LISTA EN EJECUCION, Y LA REGLA 2 (2026-08-23)
// ===================================================================
//
//  `REGLAS.md` tiene la Regla 2 escrita desde F0 y
//  `Comprobacion::llega_a_bytes` contesta que NO por ella, con el motivo:
//  *"un `bufer` es una direccion y no lleva su longitud, asi que no hay contra
//  que comprobar. **Nace con `lista de T`**"*.
//
//  Es esta.

fn con_lista(cuerpo: &str) -> String {
    format!(
        "perfil llano\nusa objetos\nusa monton\n\nfuncion prueba(base es natural64, n es natural64) devuelve natural64\n    crudo\n        escribe_natural64(base, base + 32)\n        escribe_natural64(base + 8, base + 4096)\n        escribe_natural64(base + 16, 0)\n        l = lista_nueva(base, 4, 8)\n{}",
        cuerpo
    )
}

/// ***UNA LISTA NUEVA NACE CON SITIO Y SIN NADA, y con un dueno.***
#[test]
fn una_lista_nueva_tiene_capacidad_y_ningun_elemento() {
    let f = con_lista(
        "        si cuantos(l) no es 0\n            devuelve 0\n        si caben(l) no es 4\n            devuelve 0\n        si referencias(l) no es 1\n            devuelve 0\n        devuelve 1\n",
    );
    assert_eq!(ejecuta_en(&f, "prueba", 0x40000, 0), 1);
}

/// Anadir guarda de verdad, y `sitio_de` devuelve donde esta.
#[test]
fn anadir_guarda_y_el_indice_lo_encuentra() {
    let f = con_lista(
        "        agrega(l, 11, 8)\n        agrega(l, 22, 8)\n        si cuantos(l) no es 2\n            devuelve 0\n        d = sitio_de(l, 1, 8)\n        devuelve lee_natural64(d)\n",
    );
    assert_eq!(ejecuta_en(&f, "prueba", 0x40000, 0), 22);
}

/// ***LA REGLA 2: UN INDICE QUE SE SALE NO DEVUELVE UNA DIRECCION.***
///
/// ** Y el limite esta **a un `mov` de distancia**, en un sitio fijo de la
/// cabecera. Eso es toda la diferencia entre los dos tipos, y por eso son dos:
///
/// ```text
///    bufer de T    una direccion cruda. Indexarlo pide `crudo`
///    lista de T    lleva su longitud. Indexarla se comprueba
/// ```
///
/// *** Un `bufer` no puede tener esta comprobacion. No es que se haya olvidado:
/// **no existe la informacion para hacerla.**
#[test]
fn un_indice_fuera_de_rango_no_da_una_direccion() {
    // Dos elementos: el 0 y el 1 valen, el 2 no.
    let f = con_lista(
        "        agrega(l, 11, 8)\n        agrega(l, 22, 8)\n        si sitio_de(l, 0, 8) = 0\n            devuelve 0\n        si sitio_de(l, 1, 8) = 0\n            devuelve 0\n        si sitio_de(l, 2, 8) no es 0\n            devuelve 0\n        devuelve 1\n",
    );
    assert_eq!(ejecuta_en(&f, "prueba", 0x40000, 0), 1);
}

/// [!] Y el limite es `cuantos`, NO `capacidad`. La lista tiene sitio para
/// cuatro y solo dos elementos: el indice 3 CABE en el bloque y **no existe**.
///
/// ** Sin esta prueba, comparar contra `capacidad` pasaria la de arriba y
/// dejaria leer memoria reservada y sin escribir -- que es basura con la
/// direccion bien puesta, el peor resultado posible.
#[test]
fn el_limite_es_cuantos_hay_y_no_cuantos_caben() {
    let f = con_lista(
        "        agrega(l, 11, 8)\n        agrega(l, 22, 8)\n        devuelve sitio_de(l, 3, 8)\n",
    );
    assert_eq!(
        ejecuta_en(&f, "prueba", 0x40000, 0),
        0,
        "el 3 cabe en el bloque y no existe en la lista"
    );
}

/// Una lista llena lo DICE en vez de escribir fuera.
///
/// ** No crece sola todavia, y se dice igual que `suelta` estuvo diciendo
/// durante meses que no soltaba. Crecer pide decidir quien manda cuando alguien
/// guardo la direccion antigua, y eso es diseno, no trabajo mecanico.
#[test]
fn una_lista_llena_contesta_que_no_cabe() {
    let f = con_lista(
        "        agrega(l, 1, 8)\n        agrega(l, 2, 8)\n        agrega(l, 3, 8)\n        agrega(l, 4, 8)\n        devuelve agrega(l, 5, 8)\n",
    );
    assert_eq!(ejecuta_en(&f, "prueba", 0x40000, 0), 0);
}

/// ***Y LO QUE CONSTRUYE INTI LO ACEPTA EL ABI DE RUST.***
///
/// ** Otro codigo, a proposito: `bmo_abi::dynobj::lista::revisar` mira lo que
/// INTI no mira --que no diga tener mas elementos de los que caben, que los
/// elementos quepan en el bloque, que no este viva con cero referencias-- y si
/// `lista.inti` validara su propio resultado estaria comprobando su aritmetica
/// contra si misma.
#[test]
fn la_lista_que_construye_inti_la_acepta_el_abi() {
    use bmo_abi::dynobj::lista as abi;
    let f = con_lista("        agrega(l, 11, 8)\n        agrega(l, 22, 8)\n        devuelve l\n");
    let e = emitido(&f);
    let inicio = e
        .inicios
        .iter()
        .find(|(n, _)| n == "prueba")
        .map(|(_, o)| *o)
        .expect("sin `prueba`");

    let largo = e.codigo.len() as i32;
    let mut codigo = Vec::new();
    codigo.push(0xE9);
    codigo.extend_from_slice(&largo.to_le_bytes());
    codigo.extend_from_slice(&e.codigo);
    codigo.push(0xE8);
    let desde = codigo.len() as i32 + 4;
    codigo.extend_from_slice(&((inicio as i32 + 5) - desde).to_le_bytes());

    let mut m = Machine::new(codigo);
    m.regs[7] = 0x40000;
    m.regs[6] = 0;
    let m = run(m, 200_000);
    let donde = m.regs[0];
    assert_ne!(donde, 0, "`lista_nueva` no devolvio nada");

    let mut bytes = Vec::new();
    for i in 0..(abi::CABECERA_LEN as u64 + 4 * 8) {
        bytes.push(m.read_u8_pub(donde + i));
    }
    let l = abi::revisar(&bytes, 8).expect("el ABI rechazo la lista que construyo INTI");
    assert_eq!(l.count, 2, "dos elementos");
    assert_eq!(l.capacidad, 4, "y sitio para cuatro");
    assert_eq!(l.refs, 1, "nace con UN dueno: la construyo alguien");
}

/// ***LA REGLA 2 SALE EN LOS BYTES, Y ATRAPA (2026-08-23).***
///
/// Era la unica de las cuatro que no llegaba: el emisor tenia
/// `Comprobacion::Indice => { comprobaciones -= 1; }` -- no emitia nada, y
/// encima se descontaba para que el recuento no mintiera.
///
/// ** Lo que faltaba no era el emisor: era CONTRA QUE comparar. `sitio_de`
/// compara el indice con `cuantos` --que vive a un `mov` en la cabecera de la
/// lista-- y devuelve 0 si se sale. El `Comprueba` convierte ese 0 en `E1002`.
#[test]
fn indexar_una_lista_emite_su_katana_de_regla_2() {
    let e = emitido(
        "perfil pleno\nusa objetos\nusa monton\n\nfuncion f(notas es lista de entero64) devuelve entero64\n    devuelve notas[5]\n",
    );
    assert!(
        e.katanas.iter().any(|(k, _, _)| *k as u32 == 1002),
        "la Regla 2 no saco su bloque: {:?}",
        e.katanas
    );
}

/// Y baja a `sitio_de`, no a una suma de direccion e indice.
///
/// ** Sumar a pelo daria una direccion **dentro del bloque** para cualquier
/// indice que quepa en el monton: basura con la direccion bien puesta, que es el
/// peor resultado posible. `sitio_de` es lo que hace que el 5 de una lista de
/// dos elementos no sea una direccion.
#[test]
fn indexar_una_lista_llama_a_sitio_de() {
    let m = ir_de(
        "perfil pleno\nusa objetos\nusa monton\n\nfuncion f(notas es lista de entero64) devuelve entero64\n    devuelve notas[5]\n",
    );
    let f = m.funciones.iter().find(|f| f.nombre == "f").expect("sin `f`");
    assert!(
        f.instrucciones.iter().any(|i| matches!(
            i,
            Instr::Llama { que: Valor::Nombre(n), .. } if n == "sitio_de"
        )),
        "`notas[5]` no llamo a `sitio_de`: {:?}",
        f.instrucciones
    );
}

/// [!] Y UN `bufer` SIGUE SIN COMPROBARSE, que es la otra mitad y no cambia.
///
/// No es que la comprobacion se haya olvidado: **no existe la informacion para
/// hacerla**. Por eso indexarlo pide `crudo`, y por eso son dos tipos.
#[test]
fn indexar_un_bufer_sigue_sin_llamar_a_nadie() {
    let m = ir_de(
        "perfil llano\nusa memoria\n\nfuncion f(p es bufer de entero64) devuelve entero64\n    crudo\n        devuelve p[5]\n",
    );
    let f = m.funciones.iter().find(|f| f.nombre == "f").expect("sin `f`");
    assert!(
        !f.instrucciones.iter().any(|i| matches!(
            i,
            Instr::Llama { que: Valor::Nombre(n), .. } if n == "sitio_de"
        )),
        "un `bufer` no tiene contra que comprobar y no puede llamar a `sitio_de`"
    );
}

// ===================================================================
//  *** EL LITERAL DE LISTA SE CONSTRUYE (2026-08-23)
// ===================================================================

const LITERAL: &str = "perfil pleno\nusa objetos\nusa monton\n\nfuncion principal\n    notas es lista de entero64 = [11, 22, 33]\n";

/// ***`[11, 22, 33]` BAJA A `lista_nueva` Y TRES `agrega`.***
///
/// ** Y en ORDEN. `agrega` pone al final, asi que el orden de las llamadas ES el
/// orden de la lista -- y ademas es el orden en que la Regla 8 dice que se
/// evaluan los elementos. Las dos cosas coinciden aqui, y por eso hay que
/// mirarlas: el dia que dejen de coincidir, alguien tiene que verlo.
#[test]
fn un_literal_de_lista_se_construye_en_orden() {
    let m = ir_de(LITERAL);
    let f = m.funciones.iter().find(|f| f.nombre == "principal").expect("sin `principal`");
    let llamadas: Vec<&str> = f
        .instrucciones
        .iter()
        .filter_map(|i| match i {
            Instr::Llama { que: Valor::Nombre(n), .. } => Some(n.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(
        llamadas,
        vec!["lista_nueva", "agrega", "agrega", "agrega"],
        "el literal no se construyo, o no en orden"
    );
}

/// La capacidad es EXACTA: los elementos que hay, no un numero con holgura.
///
/// ** Un literal se escribe entero. Si despues crece, crecer es otra operacion y
/// ya tiene su propio "no cabe". Reservar de mas "por si acaso" seria una
/// politica de crecimiento metida donde no toca.
#[test]
fn la_capacidad_del_literal_es_la_que_tiene() {
    let m = ir_de(LITERAL);
    let f = m.funciones.iter().find(|f| f.nombre == "principal").unwrap();
    let args = f
        .instrucciones
        .iter()
        .find_map(|i| match i {
            Instr::Llama { que: Valor::Nombre(n), argumentos, .. } if n == "lista_nueva" => {
                Some(argumentos.clone())
            }
            _ => None,
        })
        .expect("sin `lista_nueva`");
    assert_eq!(args.len(), 3, "lista_nueva(monton, capacidad, ancho)");
    assert_eq!(args[1], Valor::Const(Const::Entero(3)), "tres elementos");
    assert_eq!(args[2], Valor::Const(Const::Entero(8)), "de entero64: ocho");
}

/// ***Y SIN TIPO ESCRITO NO SE CONSTRUYE, PERO SE DICE.***
///
/// El ancho del elemento sale del TIPO, y `[1, 2, 3]` a secas no dice si sus
/// elementos miden uno, cuatro u ocho. Deducirlo de los literales tiene reglas
/// propias que nadie ha escrito: **que mide `[1, 2.5]`?**
///
/// ** Lo que no puede pasar es que baje a `nada` en silencio. Es la leccion de
/// `Const::Texto`, que bajo a un cero durante meses: lo unico que impidio que se
/// olvidara fue que el emisor lo confesaba **con un numero**.
#[test]
fn un_literal_sin_tipo_escrito_no_se_construye_y_el_emisor_lo_dice() {
    let e = emitido("perfil pleno\nusa objetos\nusa monton\n\nfuncion principal\n    notas = [1, 2, 3]\n");
    assert!(
        e.sin_emitir.iter().any(|x| x.contains("literal(es) de lista sin construir")),
        "se cayo callandose: {:?}",
        e.sin_emitir
    );
    // Y dice CUANTOS, que es lo que lo convierte en un aviso seguible.
    assert!(
        e.sin_emitir.iter().any(|x| x.contains("1 literal")),
        "no dice cuantos: {:?}",
        e.sin_emitir
    );
}

/// Y un programa sin literales de lista no se queja de nada. Un aviso que sale
/// siempre es ruido que entrena a no mirar.
#[test]
fn un_programa_sin_literales_de_lista_no_avisa() {
    let e = emitido("perfil llano\n\nfuncion principal devuelve entero32\n    devuelve 7\n");
    assert!(!e.sin_emitir.iter().any(|x| x.contains("literal(es) de lista")));
}
