//! Pruebas del emisor.
//!
//! ## El criterio: se EJECUTA
//!
//! Es el mismo del banco de BMO C, y la razon esta escrita alli: *un `si` que
//! no bifurca se ve igual que uno que si en un volcado; solo se distinguen
//! corriendolos*.
//!
//! Asi que estas pruebas compilan un fuente de INTI, emiten los bytes, los
//! meten en el emulador y **miran el resultado**. Un volcado que "parece bien"
//! no prueba nada.

use super::*;
use bmo_inti_front::{ir, lexico, palabras::Vocabulario, sintaxis};
use bmo_abi::syscalls::surface::{CURRENT_TASK, TASK_OP_EXIT};
use bmo_lower::emu::{run, Machine};

/// Compila un fuente de INTI hasta bytes.
fn emitido(fuente: &str) -> Emitido {
    // ** Por `armar` y no por `sintaxis::leer` a pelo: es lo que hace el
    // compilador de verdad, y es lo unico que trae las piezas que el fuente
    // pidio con un `usa`. Un banco que compila por otro camino prueba otro
    // compilador.
    let arbol = bmo_inti_front::armar(fuente);
    assert!(
        !arbol.hay_errores(),
        "el fuente de la prueba no se lee: {}",
        arbol.pintar("prueba.inti")
    );
    let raices = bmo_mods::Roots::find();
    let modulos = bmo_inti_front::tablas::Modulos::cargar(&raices);
    // ** El plano tambien se COMPRUEBA aqui, no solo se calcula: si una prueba
    // escribe `p.x` sobre algo sin tipo, tiene que enterarse en la prueba y no
    // ver un cero raro al final.
    let plano = bmo_inti_front::disposicion::comprobar(
        &arbol.valor,
        bmo_inti_front::disposicion::Medidas::cargar(&raices),
    );
    assert!(
        !plano.hay_errores(),
        "la disposicion no cuadra: {}",
        plano.pintar("prueba.inti")
    );
    // ** Y el METAL: los nombres que son una instruccion y no una funcion.
    //
    // Sin esta linea, `lee_reloj()` se baja a una llamada a un simbolo que no
    // existe -- que es exactamente lo que hacia el compilador entero antes de
    // F5d. Un banco que compila por otro camino prueba otro compilador.
    let metal = ir::metal_que_declara(&arbol.valor, &raices, &modulos);
    let ir = ir::bajar_con(&arbol.valor, &modulos, &plano.valor, &metal).valor;
    emitir(&ir)
}

/// Compila, ejecuta la primera funcion con dos argumentos, y devuelve lo que
/// dejo en el registro de retorno.
fn ejecuta(fuente: &str, a: u64, b: u64) -> u64 {
    let e = emitido(fuente);
    // La PRIMERA FUNCION, que desde F4a ya no es el byte cero: si el modulo
    // trae `principal`, el byte cero es el arranque. Suponerlo llamaria al
    // `crt0` en vez de a la funcion, y el test moriria contando un fallo que
    // no es suyo.
    let primera = e.inicios.first().map(|(_, off)| *off).unwrap_or(0);

    // Un `crt0` de diez bytes. Hace falta porque una funcion acaba en `ret`, y
    // un `ret` sin nadie que la haya llamado devuelve a cualquier sitio.
    //
    // La forma es la que es por como para el emulador --*"hasta caer del final
    // del codigo"*--, asi que la direccion de retorno tiene que quedar FUERA:
    //
    //     0:  jmp   L          salta por encima de la funcion
    //     5:  <la funcion>
    //     L:  call  5          y el retorno cae en L+5 = el final
    //
    // Cuando exista el `crt0` de verdad esto se tira: alli quien llama a
    // `principal` es el sistema, y lo que devuelve se entrega por la puerta.
    let largo = e.codigo.len() as i32;
    let mut codigo = Vec::new();
    codigo.push(0xE9); // jmp rel32
    codigo.extend_from_slice(&largo.to_le_bytes());
    codigo.extend_from_slice(&e.codigo);
    codigo.push(0xE8); // call rel32
    let desde = codigo.len() as i32 + 4;
    codigo.extend_from_slice(&((primera as i32 + 5) - desde).to_le_bytes());

    let mut m = Machine::new(codigo);
    // Los dos primeros argumentos, donde la convencion dice.
    m.regs[7] = a; // rdi
    m.regs[6] = b; // rsi
    let m = run(m, 10_000);
    m.regs[0] // rax
}

/// Como [`ejecuta`], pero arrancando por la funcion que se diga.
///
/// Hace falta desde que hay llamadas: el modulo ya no tiene una sola funcion, y
/// arrancar siempre por la primera probaria la de arriba en vez de la que
/// interesa.
fn ejecuta_en(fuente: &str, nombre: &str, a: u64, b: u64) -> u64 {
    let e = emitido(fuente);
    let inicio = e
        .inicios
        .iter()
        .find(|(n, _)| n == nombre)
        .map(|(_, off)| *off)
        .unwrap_or_else(|| panic!("no encuentro la funcion {}", nombre));

    // El mismo `crt0` de diez bytes, pero llamando a la de dentro.
    let largo = e.codigo.len() as i32;
    let mut codigo = Vec::new();
    codigo.push(0xE9); // jmp por encima del modulo
    codigo.extend_from_slice(&largo.to_le_bytes());
    codigo.extend_from_slice(&e.codigo);
    codigo.push(0xE8); // call a la funcion pedida
    let desde = codigo.len() as i32 + 4;
    codigo.extend_from_slice(&((inicio as i32 + 5) - desde).to_le_bytes());

    let mut m = Machine::new(codigo);

    // *** Y LAS TABLAS CONGELADAS SE CARGAN, que hasta hoy NO PASABA.
    //
    // `Instr::Direccion` deja un inmediato a CERO y una reubicacion, porque la
    // direccion de `RoData` la elige el cargador. Este banco no tenia cargador,
    // asi que el cero se quedaba y **una tabla congelada se leia de la direccion
    // cero**: numeros al azar, con el codigo correcto.
    //
    // ** Ninguna prueba unitaria habia visto nunca una tabla de verdad. Las que
    // existian miran los BYTES del `.ibex` --que es otra pregunta, y sigue
    // siendo suya-- y ninguna la EJECUTABA. Lo destapo el decimal: `POTENCIAS`
    // daba basura y el fichero estaba bien.
    //
    // [!] La disposicion sale de `rodata_de`, la MISMA funcion que usa
    // `empaquetar`. Copiarla aqui habria dado dos layouts que se separan el dia
    // que uno cambie -- que es el fallo que este proyecto lleva todo el dia
    // cazando.
    let (rodata, donde) = crate::rodata_de(&e);
    if !rodata.is_empty() {
        let base = m.load_data(&rodata);
        for (off, i) in &e.reubicaciones {
            if let Some(d) = donde.get(*i as usize) {
                let dir = (base + d).to_le_bytes();
                // +5 por el `jmp` del `crt0` que va delante del modulo.
                let hueco = *off + 5;
                m.code[hueco..hueco + 8].copy_from_slice(&dir);
            }
        }
    }

    m.regs[7] = a;
    m.regs[6] = b;
    let m = run(m, 100_000);
    m.regs[0]
}

const SUMA: &str = "\
perfil llano

funcion suma(a es entero64, b es entero64) devuelve entero64
    devuelve a + b
";

// ===================================================================
//  ** Que corre de verdad
// ===================================================================

#[test]
fn una_suma_de_inti_corre_y_da_la_suma() {
    assert_eq!(ejecuta(SUMA, 3, 4), 7);
    assert_eq!(ejecuta(SUMA, 100, 23), 123);
}

#[test]
fn la_resta_y_el_producto_tambien() {
    let resta = "perfil llano\n\nfuncion f(a es entero64, b es entero64) devuelve entero64\n    devuelve a - b\n";
    assert_eq!(ejecuta(resta, 10, 4), 6);

    let por = "perfil llano\n\nfuncion f(a es entero64, b es entero64) devuelve entero64\n    devuelve a * b\n";
    assert_eq!(ejecuta(por, 6, 7), 42);
}

/// La precedencia sobrevive al viaje entero: texto -> arbol -> IR -> bytes.
#[test]
fn la_precedencia_llega_hasta_los_bytes() {
    let f = "perfil llano\n\nfuncion f(a es entero64, b es entero64) devuelve entero64\n    devuelve a + b * 2\n";
    // 3 + 4*2 = 11, no (3+4)*2 = 14.
    assert_eq!(ejecuta(f, 3, 4), 11);
}

#[test]
fn una_local_guarda_y_se_lee() {
    let f = "\
perfil llano

funcion f(a es entero64, b es entero64) devuelve entero64
    cambiante t = a + b
    t = t + 1
    devuelve t
";
    assert_eq!(ejecuta(f, 10, 5), 16);
}

/// Un `si` que bifurca de verdad. Este es el que un volcado no distingue.
#[test]
fn un_si_bifurca() {
    let f = "\
perfil llano

funcion mayor(a es entero64, b es entero64) devuelve entero64
    si a > b
        devuelve a
    devuelve b
";
    assert_eq!(ejecuta(f, 9, 4), 9);
    assert_eq!(ejecuta(f, 4, 9), 9);
    assert_eq!(ejecuta(f, 7, 7), 7);
}

#[test]
fn las_comparaciones_dan_cero_o_uno() {
    let f = "perfil llano\n\nfuncion f(a es entero64, b es entero64) devuelve entero64\n    devuelve a < b\n";
    assert_eq!(ejecuta(f, 1, 2), 1);
    assert_eq!(ejecuta(f, 2, 1), 0);
}

// ===================================================================
//  *** La regla, en bytes
// ===================================================================

/// El `jo` que hace que "sin comportamiento indefinido" sea una instruccion y
/// no una frase.
#[test]
fn una_suma_emite_su_comprobacion_de_desborde() {
    let e = emitido(SUMA);
    assert_eq!(e.comprobaciones, 1);
    // `0F 80` es el salto que mira la bandera que la suma dejo puesta.
    assert!(
        e.codigo.windows(2).any(|w| w == [0x0F, 0x80]),
        "no esta el salto de desbordamiento"
    );
}

/// ** Y cuando desborda de verdad, ATRAPA: no da la vuelta en silencio.
///
/// Es la diferencia entera con C, y aqui se ve corriendo.
#[test]
fn cuando_desborda_atrapa_en_vez_de_dar_la_vuelta() {
    let maximo = i64::MAX as u64;
    // En C esto daria un numero negativo sin decir nada.
    let r = ejecuta(SUMA, maximo, 1);
    assert_eq!(r, 1001, "tenia que atrapar con el codigo de desbordamiento");
    // Y sin desbordar, la suma normal.
    assert_eq!(ejecuta(SUMA, 2, 2), 4);
}

/// Comparar no puede salirse, asi que no paga nada. El coste del "sin UB" se
/// paga **donde hace falta** y en ningun otro sitio.
#[test]
fn comparar_no_emite_comprobacion() {
    let f = "perfil llano\n\nfuncion f(a es entero64, b es entero64) devuelve entero64\n    devuelve a < b\n";
    assert_eq!(emitido(f).comprobaciones, 0);
}

// ===================================================================
//  El gate
// ===================================================================

/// *** Ningun `.bex` del sistema se escribe sin pasar por `bmo-verify`. INTI no
/// abre un quinto camino que lo esquive.
#[test]
fn el_bex_que_sale_pasa_el_gate() {
    let e = emitido(SUMA);
    let bex = empaquetar(&e, None).expect("el gate lo rechazo");
    assert!(bex.len() > 64, "un .bex de verdad tiene cabecera");
    assert_eq!(&bex[0..4], b"BEF1", "la marca del contenedor");
}

#[test]
fn cada_funcion_sabe_donde_empieza() {
    let dos = "\
perfil llano

funcion uno(a es entero64) devuelve entero64
    devuelve a

funcion dos(a es entero64) devuelve entero64
    devuelve a
";
    let e = emitido(dos);
    assert_eq!(e.inicios.len(), 2);
    assert_eq!(e.inicios[0].0, "uno");
    assert_eq!(e.inicios[1].0, "dos");
    assert!(e.inicios[1].1 > e.inicios[0].1, "la segunda va detras");
}


/// Los bits de un `f64`, que es lo que devuelve la funcion, leidos como numero.
fn como_numero(u: u64) -> f64 {
    f64::from_bits(u)
}

/// Cuantas comprobaciones de las doce reglas trae la IR de este fuente.
///
/// ** Contarlas es lo que separa *"INTI no tiene comportamiento indefinido"* de
/// una frase, y por eso la IR las lleva como instrucciones y no como bytes
/// sueltos dentro del emisor: lo que es una instruccion se puede contar.
fn reglas_de(fuente: &str) -> usize {
    let arbol = bmo_inti_front::armar(fuente);
    assert!(!arbol.hay_errores(), "{}", arbol.pintar("prueba.inti"));
    let raices = bmo_mods::Roots::find();
    let modulos = bmo_inti_front::tablas::Modulos::cargar(&raices);
    let plano = bmo_inti_front::disposicion::comprobar(
        &arbol.valor,
        bmo_inti_front::disposicion::Medidas::cargar(&raices),
    );
    let metal = ir::metal_que_declara(&arbol.valor, &raices, &modulos);
    ir::bajar_con(&arbol.valor, &modulos, &plano.valor, &metal)
        .valor
        .funciones
        .iter()
        .flat_map(|f| f.instrucciones.iter())
        .filter(|i| matches!(i, bmo_inti_front::ir::Instr::Comprueba { .. }))
        .count()
}

/// Cuantas llamadas de verdad hay en la IR de este fuente.
fn llamadas_de(fuente: &str) -> usize {
    let arbol = bmo_inti_front::armar(fuente);
    assert!(!arbol.hay_errores(), "{}", arbol.pintar("prueba.inti"));
    let raices = bmo_mods::Roots::find();
    let modulos = bmo_inti_front::tablas::Modulos::cargar(&raices);
    let plano = bmo_inti_front::disposicion::comprobar(
        &arbol.valor,
        bmo_inti_front::disposicion::Medidas::cargar(&raices),
    );
    let metal = ir::metal_que_declara(&arbol.valor, &raices, &modulos);
    ir::bajar_con(&arbol.valor, &modulos, &plano.valor, &metal)
        .valor
        .funciones
        .iter()
        .flat_map(|f| f.instrucciones.iter())
        .filter(|i| matches!(i, bmo_inti_front::ir::Instr::Llama { .. }))
        .count()
}

/// Corre lo que salio del emisor, desde el principio y sin envoltorio.
fn arranca(fuente: &str) -> Machine {
    let e = emitido(fuente);
    assert!(e.arranca, "este fuente tiene `principal` y deberia arrancar solo");
    run(Machine::new(e.codigo), 100_000)
}

/// Un fuente que llama a `nombre` con la aridad que la tabla dice.
///
/// ** Todo dentro de `crudo`, y da igual que el nombre no lo pida: escribirlo
/// de mas no cambia lo que se emite, y asi la matriz no tiene que llevar una
/// segunda lista de cuales lo piden. Una lista que hay que mantener a mano al
/// lado de una tabla es la forma de que las dos discrepen.
fn llamada_a(nombre: &str, cuantos: usize) -> String {
    let args: Vec<String> = (0..cuantos).map(|i| format!("{}", i + 1)).collect();
    format!(
        "perfil llano\nusa x86_64\nusa binarios\n\n\
         funcion f devuelve entero64\n    crudo\n        devuelve {}({})\n",
        nombre,
        args.join(", ")
    )
}

/// El mismo `crt0` de diez bytes que usa `ejecuta`, sacado aparte para poder
/// pasarlo a un `catch_unwind`.
fn con_arranque(e: &Emitido) -> Vec<u8> {
    let primera = e.inicios.first().map(|(_, off)| *off).unwrap_or(0);
    let largo = e.codigo.len() as i32;
    let mut codigo = Vec::new();
    codigo.push(0xE9);
    codigo.extend_from_slice(&largo.to_le_bytes());
    codigo.extend_from_slice(&e.codigo);
    codigo.push(0xE8);
    let desde = codigo.len() as i32 + 4;
    codigo.extend_from_slice(&((primera as i32 + 5) - desde).to_le_bytes());
    codigo
}

/// El fuente de `cpu.inti` SIN su `principal`, para poder ponerle otro.
fn maquinaria_de_cpu() -> String {
    let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("sondas")
        .join("cpu.inti");
    let texto = std::fs::read_to_string(&p)
        .unwrap_or_else(|e| panic!("no puedo leer {}: {}", p.display(), e));
    let corte = texto
        .find("funcion principal")
        .expect("cpu.inti tiene que tener un `principal`");
    texto[..corte].to_string()
}

/// Lo que el programa escribio en la consola, ya desempaquetado.
///
/// ** El kernel corta en el primer byte cero, y aqui se hace igual: si esta
/// prueba leyera los ocho bytes siempre, aprobaria una palabra a medias que en
/// la maquina saldria cortada.
fn lo_escrito(m: &Machine) -> Vec<String> {
    m.syscalls
        .iter()
        .filter(|s| s.operation == 0x06)
        .map(|s| {
            let b = s.arg0.to_le_bytes();
            let fin = b.iter().position(|x| *x == 0).unwrap_or(8);
            String::from_utf8_lossy(&b[..fin]).to_string()
        })
        .collect()
}

// ===================================================================
//  ** EL REPARTO DE ESTE BANCO (L6b)
// ===================================================================
//
//  Era UN fichero de 2.409 lineas y el guardian L6a lo tumbo. El corte NO es por
//  tamano: cada trozo contesta una pregunta distinta, y el nombre la dice.
//
//     marco        donde vive un valor, y como se sale
//     memoria      tocar memoria: el monton, los anchos, el framebuffer
//     disposicion  `p.x` y `a[i]` leen lo que dicen leer?
//     flotante     el numero que mide, no el que cuenta
//     metal        lo que la tabla de la maquina promete, se cumple?
//     lenguaje     las tres frases con las que se define INTI
//     reglas       las doce, en bytes y corriendo
//     sonda        el instrumento del Ryzen, calibrado
//
//  ** El andamio se queda AQUI --`emitido`, `ejecuta`, `arranca`-- porque lo usan
//  los ocho. Un fichero de pruebas que ademas tiene que traerse su forma de
//  compilar es dos ficheros disfrazados de uno.

mod disposicion;
mod flotante;
mod lenguaje;
mod marco;
mod memoria;
mod metal;
mod reglas;
mod sonda;

// ===================================================================
//  ** QUE LAS DOS LISTAS NO SE SEPAREN
// ===================================================================

/// **`Comprobacion::llega_a_bytes()` y el `match` del emisor dicen lo mismo.**
///
/// ## Por que hace falta esta prueba
///
/// Desde que existe `--puedo`, la respuesta a *"que reglas sabes emitir?"* vive
/// en DOS sitios: la tabla de `ir::forma` --que es la que se le ensena a una
/// persona-- y el `match` de `Instr::Comprueba` --que es la que emite bytes.
///
/// *** Dos listas que dicen lo mismo se separan el dia que alguien toca una. Y
/// esta se separaria hacia el peor lado posible: el compilador anunciando una
/// regla que no emite. Un binario saldria firmado, sin la comprobacion dentro,
/// y con el propio compilador habiendo dicho que si.
///
/// Es el mismo fallo del censo de las diez sondas y el de los cinco analisis de
/// la linea de ordenes, la tercera vez.
///
/// ## Como lo comprueba, y por que asi
///
/// No lee el `match`: **emite**. Para cada regla se compila un fuente que la
/// provoca y se mira si salio su bloque de trampa a la mesa de katanas. Leer el
/// codigo fuente del emisor seria comprobar lo que esta escrito; emitir
/// comprueba lo que hace.
#[test]
fn lo_que_inti_dice_que_emite_es_lo_que_emite() {
    use bmo_inti_front::ir::Comprobacion;

    // Un fuente por regla, elegido para que la IR pida ESA comprobacion.
    let provoca = |c: Comprobacion| -> Option<&'static str> {
        match c {
            Comprobacion::Desborde => Some("perfil llano\n\nfuncion f devuelve natural64\n    cambiante x es entero64 = 4000000000\n    devuelve x * x\n"),
            Comprobacion::EntreCero => Some("perfil llano\n\nfuncion f devuelve natural64\n    cambiante c es entero64 = 0\n    devuelve 10 entre c\n"),
            Comprobacion::Conversion(_) => Some("perfil llano\n\nfuncion f devuelve natural64\n    devuelve entero32(1e30)\n"),
            // ** La del cociente: `-2^63 entre -1` no cabe en 64 bits. Se
            // provoca con constantes para que la IR no tenga que adivinar
            // nada -- y aun asi la comprobacion se emite, porque hoy no hay
            // eliminacion de comprobaciones y el dia que la haya, esta
            // prueba dira que ha empezado.
            Comprobacion::Cociente => Some(
                "perfil llano\n\nfuncion f devuelve entero64\n    cambiante a es entero64 = -9223372036854775808\n    cambiante b es entero64 = -1\n    devuelve a entre b\n",
            ),
            // *** LA 2 YA SE PUEDE PROVOCAR (2026-08-23), y ese es el dato.
            //
            // Hasta hoy esta rama devolvia `None` con el motivo *"indexar un
            // `bufer` pide `crudo` justamente porque no hay contra que
            // comprobar"*. Sigue siendo verdad **del bufer** -- y `lista de T`
            // si lleva su longitud, asi que indexarla se comprueba.
            //
            // ** El fuente es `pleno` a proposito: un `lista de T` no cabe en
            // `llano` --crece, pide monton-- asi que la unica forma de escribir
            // esta regla es en el perfil que la admite. `emitido` no pasa por el
            // gate de `[bytes] llegan`, que es otro peldano.
            //
            // Y es un PARAMETRO y no un literal: `[1, 2]` todavia no baja a
            // `lista_nueva`, y una prueba que dependiera de eso estaria midiendo
            // dos cosas.
            Comprobacion::Indice => Some(
                "perfil pleno
usa objetos
usa monton

funcion f(notas es lista de entero64) devuelve entero64
    devuelve notas[5]
",
            ),
        }
    };

    for c in Comprobacion::TODAS {
        let codigo: u32 = c.codigo()[1..].parse().expect("el codigo no es un numero");
        match provoca(c) {
            Some(fuente) => {
                assert!(
                    c.llega_a_bytes(),
                    "{} se puede provocar y la tabla dice que no llega a bytes",
                    c.codigo()
                );
                let e = emitido(fuente);
                assert!(
                    e.katanas.iter().any(|(k, _, _)| *k as u32 == codigo),
                    "la tabla dice que {} ({}) llega a bytes y el emisor no saco su bloque: {:?}",
                    c.codigo(),
                    c.nombre(),
                    e.katanas
                );
            }
            None => {
                assert!(
                    !c.llega_a_bytes(),
                    "{} no se puede provocar y la tabla dice que llega a bytes",
                    c.codigo()
                );
                assert!(
                    !c.por_que_no().is_empty(),
                    "{} no llega a bytes y no dice por que -- un `no` sin motivo manda                      a buscar al codigo",
                    c.codigo()
                );
            }
        }
    }
}

// ===================================================================
//  ** LA MATEMATICA QUE PIDE UN MOTOR (2026-08-22)
// ===================================================================

/// **`raiz` da la raiz cuadrada, y se comprueba EJECUTANDOLA.**
///
/// ## Por que esta prueba tuvo que esperar al emulador
///
/// La matriz de conformidad ya decia que `raiz` emite bytes. Eso prueba que
/// **sale algo**, no que salga lo correcto -- y son dos cosas distintas que este
/// proyecto ya ha confundido antes.
///
/// *** Para probar lo segundo hay que ejecutarlo, y el emulador no conocia
/// `sqrtsd`: manejaba `0F 58..5F` de la aritmetica y `0x51` se le quedaba fuera.
/// O sea que INTI podia emitir una instruccion **que ninguna prueba podia
/// ejecutar**. Se le enseno al emulador antes de escribir esto.
#[test]
fn la_raiz_cuadrada_da_la_raiz_cuadrada() {
    let f = "perfil llano\nusa matematica\n\nfuncion r(x es flotante64) devuelve flotante64\n    devuelve raiz(x)\n";
    for v in [0.0f64, 1.0, 2.0, 9.0, 1e30] {
        let salio = como_numero(ejecuta(f, v.to_bits(), 0));
        assert_eq!(
            salio.to_bits(),
            v.sqrt().to_bits(),
            "raiz({}) dio {} y tenia que dar {}",
            v,
            salio,
            v.sqrt()
        );
    }
}

/// **La longitud de un vector: `raiz(x*x + y*y)`.**
///
/// *** Es la primitiva de la que cuelga un motor grafico entero -- normalizar,
/// distancia, interseccion de un rayo con una esfera. Y hasta hoy **no se podia
/// escribir en INTI**: los operadores estaban, la raiz no.
///
/// Se comprueba con un triangulo 3-4-5, que da un entero exacto y por eso no
/// necesita margen: si sale 5.0 clavado, la cadena entera --cargar, multiplicar,
/// sumar, cruzar al banco SSE, volver-- esta bien.
#[test]
fn la_longitud_de_un_vector_sale_exacta() {
    let f = "perfil llano\nusa matematica\n\nfuncion largo(x es flotante64, y es flotante64) devuelve flotante64\n    devuelve raiz(x * x + y * y)\n";
    let salio = como_numero(ejecuta(f, 3.0f64.to_bits(), 4.0f64.to_bits()));
    assert_eq!(salio, 5.0, "el 3-4-5 no da 5");
}

/// **`minimo` y `maximo`, que es lo que recorta un color a su rango.**
#[test]
fn minimo_y_maximo_recortan() {
    let mn = "perfil llano\nusa matematica\n\nfuncion m(a es flotante64, b es flotante64) devuelve flotante64\n    devuelve minimo(a, b)\n";
    let mx = "perfil llano\nusa matematica\n\nfuncion m(a es flotante64, b es flotante64) devuelve flotante64\n    devuelve maximo(a, b)\n";
    assert_eq!(como_numero(ejecuta(mn, 2.0f64.to_bits(), 7.0f64.to_bits())), 2.0);
    assert_eq!(como_numero(ejecuta(mn, 7.0f64.to_bits(), 2.0f64.to_bits())), 2.0);
    assert_eq!(como_numero(ejecuta(mx, 2.0f64.to_bits(), 7.0f64.to_bits())), 7.0);
    assert_eq!(como_numero(ejecuta(mx, 7.0f64.to_bits(), 2.0f64.to_bits())), 7.0);
}

/// **`absoluto` quita el signo sin tocar la coma flotante.**
///
/// No hay instruccion de "valor absoluto de un double" en x86-64. Lo que hay es
/// apagar el bit 63, y como INTI lleva los flotantes en registros generales eso
/// son dos instrucciones ENTERAS -- que caben en una fila de la tabla porque esa
/// columna se llama `bytes`, no `instruccion`.
#[test]
fn absoluto_quita_el_signo_y_deja_el_cero_negativo_en_cero() {
    let f = "perfil llano\nusa matematica\n\nfuncion a(x es flotante64) devuelve flotante64\n    devuelve absoluto(x)\n";
    for v in [-3.5f64, 3.5, -0.0, 0.0, -1e300] {
        assert_eq!(como_numero(ejecuta(f, v.to_bits(), 0)), v.abs(), "absoluto({})", v);
    }
}

/// ***LA PRIMERA LIBRERIA ESCRITA EN INTI QUE NO ES EL MONTON.***
///
/// `potencia` no es una instruccion: **ningun procesador tiene "eleva este
/// double a la enesima"**. Asi que esta escrita en INTI, en
/// `runtime/matematica/potencias.inti`, y la trae `usa matematica` por el mismo
/// camino que el monton.
///
/// ** Hasta hoy, lo que INTI no podia pedirle al silicio se lo pedia a Rust.
/// Esto es lo primero que se escribe a si mismo -- y es un paso del camino
/// largo: el dia que INTI se compile solo, todo lo que hoy es Rust sera esto.
#[test]
fn potencia_por_cuadrados_sucesivos() {
    let f = "perfil llano\nusa matematica\n\nfuncion p(b es flotante64, n es natural64) devuelve flotante64\n    devuelve potencia(b, n)\n";
    for (base, exp) in [(2.0f64, 10u64), (3.0, 0), (1.5, 4), (10.0, 3), (0.5, 8)] {
        let salio = como_numero(ejecuta(f, base.to_bits(), exp));
        assert_eq!(salio, base.powi(exp as i32), "{}^{}", base, exp);
    }
}

/// **La distancia al cuadrado, que es la funcion mas usada de un motor.**
///
/// Sin la raiz a proposito: para saber cual de dos cosas esta mas cerca la raiz
/// no hace falta, porque conserva el orden.
#[test]
fn la_distancia_al_cuadrado_no_saca_la_raiz() {
    let f = "perfil llano\nusa matematica\n\nfuncion d(ax es flotante64, ay es flotante64) devuelve flotante64\n    devuelve distancia2(ax, ay, 0.0, 0.0)\n";
    assert_eq!(como_numero(ejecuta(f, 3.0f64.to_bits(), 4.0f64.to_bits())), 25.0);
}

/// **`mezcla` devuelve los extremos CLAVADOS.**
///
/// *** Es la prueba que justifica como esta escrita. `a + (b - a) * t` y
/// `a*(1-t) + b*t` son la misma formula en el papel y **no en coma flotante**:
/// la segunda no devuelve `a` exacto cuando `t` vale 0. En un degradado no se
/// nota; en el extremo de una animacion, si.
#[test]
fn mezcla_clava_los_extremos() {
    let f = "perfil llano\nusa matematica\n\nfuncion m(t es flotante64) devuelve flotante64\n    devuelve mezcla(0.1, 0.7, t)\n";
    assert_eq!(como_numero(ejecuta(f, 0.0f64.to_bits(), 0)), 0.1, "en t=0 tiene que dar `a` clavado");
    assert_eq!(como_numero(ejecuta(f, 1.0f64.to_bits(), 0)), 0.7, "en t=1 tiene que dar `b` clavado");
}

/// ***EL AGUJERO DE LA REGLA 1 QUE VIVIA DENTRO DE LA 3.***
///
/// `-2^63 entre -1` no cabe en 64 bits: es un DESBORDE. Pero se escribe como una
/// division, y de la division solo se comprobaba el divisor.
///
/// Hasta el 2026-08-22 ese fuente compilaba limpio, salia firmado, y en el Ryzen
/// **moria con una autopsia del kernel** en vez de atrapar con E1001 -- porque
/// `idiv` levanta `#DE`, el mismo vector que dividir entre cero.
///
/// ** No era comportamiento indefinido: la muerte esta definida. Pero no era lo
/// que `REGLAS.md` promete, y esa distancia es la que no se puede permitir.
#[test]
fn el_cociente_que_no_cabe_atrapa_con_e1001() {
    let f = "perfil llano\n\nfuncion d(a es entero64, b es entero64) devuelve entero64\n    devuelve a entre b\n";
    assert_eq!(
        ejecuta(f, i64::MIN as u64, (-1i64) as u64),
        1001,
        "`-2^63 entre -1` tenia que atrapar con E1001"
    );
}

/// **Y las divisiones normales siguen dividiendo.**
///
/// ** Sin esta prueba, la guardia podria estar atrapando SIEMPRE y la de arriba
/// seguiria en verde. Una comprobacion que dice que si a todo no comprueba nada.
#[test]
fn la_guardia_del_cociente_no_estorba_a_las_demas() {
    let f = "perfil llano\n\nfuncion d(a es entero64, b es entero64) devuelve entero64\n    devuelve a entre b\n";
    assert_eq!(ejecuta(f, 100u64, 7u64), 14);
    assert_eq!(ejecuta(f, (-100i64) as u64, (-1i64) as u64), 100u64, "-100/-1 = 100, y CABE");
    assert_eq!(ejecuta(f, i64::MIN as u64, 2u64), (i64::MIN / 2) as u64);
    // El divisor cero sigue siendo la Regla 3, no la 1.
    assert_eq!(ejecuta(f, 5u64, 0u64), 1003);
}

/// **El resto tambien**: `-2^63 resto -1` desborda por el mismo motivo.
#[test]
fn el_resto_lleva_la_misma_guardia() {
    let f = "perfil llano\n\nfuncion r(a es entero64, b es entero64) devuelve entero64\n    devuelve a resto b\n";
    assert_eq!(ejecuta(f, i64::MIN as u64, (-1i64) as u64), 1001);
    assert_eq!(ejecuta(f, 100u64, 7u64), 2);
}

/// *** LOS TEXTOS LLEGAN A BYTES (2026-08-23), y lo que decia antes.
///
/// Esta prueba se llamaba `los_textos_que_no_llegan_a_bytes_se_dicen` y exigia
/// que el emisor CONFESARA que el pozo se perdia:
///
/// ```text
///    `ir::bajar` interna cada literal en `ModuloIr::textos`... y el emisor no
///    lo mira ni una vez. `Const::Texto(i)` baja a un cero.
/// ```
///
/// Era la firma de fallo de siempre --la pieza que se calcula bien y no lee
/// nadie-- y el aviso se dejo escrito porque no habia adonde llevarlos. Ahora
/// si: **un literal ES un congelado**, va a `RoData` con su cabecera de objeto,
/// y el codigo llega a el por una reubicacion.
///
/// ** El pozo nunca fue un segundo mecanismo: existia porque `RoData` no
/// existia.
#[test]
fn los_textos_llegan_a_bytes_y_ya_no_se_confiesan() {
    let e = emitido(
        "perfil pleno

funcion f
    escribe(\"hola\")
    escribe(\"adios\")
",
    );
    assert!(
        !e.sin_emitir.iter().any(|x| x.contains("pozo")),
        "el pozo ya no se pierde, asi que no hay nada que confesar: {:?}",
        e.sin_emitir
    );
    // Dos literales distintos -> dos congelados, y los dos son textos.
    let textos: Vec<_> = e
        .congelados
        .iter()
        .filter(|c| matches!(c.clase, bmo_inti_front::ir::ClaseCongelada::Texto))
        .collect();
    assert_eq!(textos.len(), 2, "{:?}", e.congelados);
    assert_eq!(textos[0].bytes, b"hola");
    assert_eq!(textos[1].bytes, b"adios");
}

/// El pozo no repite, y por tanto los congelados tampoco: el mismo literal dos
/// veces es UN objeto congelado, no dos.
///
/// ** Que se comparta es exactamente lo que la inmortalidad permite. Un objeto
/// contado no se podria compartir asi sin tocar su contador en cada sitio.
#[test]
fn el_mismo_texto_dos_veces_es_un_solo_congelado() {
    let e = emitido(
        "perfil pleno

funcion f
    escribe(\"hola\")
    escribe(\"hola\")
",
    );
    let textos = e
        .congelados
        .iter()
        .filter(|c| matches!(c.clase, bmo_inti_front::ir::ClaseCongelada::Texto))
        .count();
    assert_eq!(textos, 1, "el pozo deduplica, y el congelado hereda eso");
}

/// Y un fuente sin textos no crea ningun congelado de texto.
#[test]
fn un_fuente_sin_textos_no_crea_congelados_de_texto() {
    let e = emitido(
        "perfil llano

funcion f devuelve entero32
    devuelve 7
",
    );
    assert!(
        !e.sin_emitir.iter().any(|x| x.contains("pozo")),
        "aviso de pozo sin textos: {:?}",
        e.sin_emitir
    );
    assert!(e
        .congelados
        .iter()
        .all(|c| !matches!(c.clase, bmo_inti_front::ir::ClaseCongelada::Texto)));
}

// ===================================================================
//  ** LAS CONSTANTES CONGELADAS (2026-08-22)
// ===================================================================

/// ***`maximo = 100` VALIA CERO, y compilaba limpio.***
///
/// ## Lo que estaba pasando
///
/// `Decl::Constante` existia en la gramatica --con su ejemplo, `MAXIMO = 100`--,
/// en el arbol, y en el analisis de perfiles, que recorria su valor. Y en la IR
/// se tiraba con un `Decl::Constante { .. } => {}`.
///
/// *** El nombre llegaba suelto al emisor, `carga` lo bajaba a un `zero_r32`, y
/// salia un `.ibex` que compilaba limpio, pasaba el gate, salia FIRMADO **y
/// devolvia cero**. Tres capas con la pieza puesta y ninguna que la construyera.
#[test]
fn una_constante_del_modulo_vale_lo_que_dice() {
    let f = "perfil llano\n\nmaximo = 100\n\nfuncion cuanto devuelve entero32\n    devuelve maximo\n";
    assert_eq!(ejecuta_en(f, "cuanto", 0, 0), 100);
    assert!(emitido(f).sin_emitir.is_empty(), "y sin quejarse de nada");
}

/// **Una constante declarada DEBAJO de quien la usa vale igual.**
///
/// ** En el nivel superior no hay orden: todo se congela cuando el modulo acaba
/// de cargarse. Por eso se recogen en una pasada propia antes de bajar las
/// funciones -- resolverlas sobre la marcha obligaria a ordenar el fichero por
/// quien usa a quien, que es imposible en cuanto dos se usan entre si.
#[test]
fn una_constante_vale_aunque_se_declare_despues() {
    let f = "perfil llano\n\nfuncion cuanto devuelve entero32\n    devuelve tope\n\ntope = 42\n";
    assert_eq!(ejecuta_en(f, "cuanto", 0, 0), 42);
}

/// **Y el menos de un numero se congela**: `-1` es una constante, no una resta.
#[test]
fn una_constante_negativa_se_congela() {
    let f = "perfil llano\n\nfondo = 0 - 7\n\nfuncion cuanto devuelve entero64\n    devuelve fondo\n";
    // ** `0 - 7` es una OPERACION, no un literal: no se congela, y por eso el
    // emisor lo dice en vez de bajarlo a cero. Es la mitad honesta del arreglo.
    let e = emitido(f);
    assert!(
        e.sin_emitir.iter().any(|x| x.contains("`fondo`")),
        "una constante que no se puede congelar tiene que decirse: {:?}",
        e.sin_emitir
    );
}

/// ***Y ESTA ES LA QUE CIERRA LA CLASE, no el caso.***
///
/// Arreglar las constantes quita el sintoma. Lo que quita la FAMILIA es que un
/// nombre que el emisor no sabe resolver **deje de convertirse en un numero en
/// silencio**.
///
/// ** `carga()` acaba en `Valor::Nombre(_) => zero_r32`, y es lo unico que puede
/// hacer. Lo que no puede es callarselo: cualquier cosa que se cuele por ahi
/// --hoy, o dentro de un ano con una construccion nueva-- sale por `sin_emitir`
/// con su nombre delante.
#[test]
fn un_nombre_que_el_emisor_no_resuelve_no_se_baja_a_cero_en_silencio() {
    // `fondo` no se congela porque su valor es una operacion.
    let f = "perfil llano\n\nfondo = 0 - 7\n\nfuncion f devuelve entero64\n    devuelve fondo\n";
    let e = emitido(f);
    assert!(
        e.sin_emitir.iter().any(|x| x.contains("lo baja a un CERO")),
        "el aviso no dice lo que pasa: {:?}",
        e.sin_emitir
    );
}

// ===================================================================
//  *** EL MONTON RECIBE DE VERDAD (2026-08-23)
// ===================================================================
//
//  `MONTON.md` llevaba desde que existe diciendo la verdad incomoda en su
//  seccion 3: *"`suelta` existe, se puede llamar, y NO devuelve nada al
//  monton"*. Ya devuelve.
//
//  ** Y estas pruebas EJECUTAN. Un asignador que compila no dice nada: lo unico
//  que decide si un trozo vuelve es pedirlo, soltarlo, y volver a pedir.

/// El montaje comun: un monton fabricado a mano sobre una direccion cualquiera.
///
/// *** No pasa por `monton_nuevo`, y es a proposito: `reparto.inti` **no habla
/// con el kernel** --lo dice su primera linea-- asi que probarlo a traves de la
/// puerta probaria las dos piezas juntas y no diria cual falla. Aqui se le da la
/// disposicion escrita a mano, que es todo lo que esa pieza sabe de un monton.
fn con_monton(cuerpo: &str) -> String {
    format!(
        "perfil llano\nusa monton\n\nfuncion prueba(base es natural64, cuantos es natural64) devuelve natural64\n    crudo\n        escribe_natural64(base, base + 32)\n        escribe_natural64(base + 8, base + cuantos)\n        escribe_natural64(base + 16, 0)\n{}",
        cuerpo
    )
}

/// ***UN TROZO SOLTADO VUELVE, Y EL SIGUIENTE `pide` LO REUTILIZA.***
///
/// Es la prueba entera en una linea: `c` tiene que ser **la misma direccion**
/// que `a`. Si el reparto siguiera siendo solo de avance, `c` estaria mas
/// adelante y el monton se habria comido cien bytes que ya no usaba nadie.
#[test]
fn un_trozo_soltado_se_reutiliza_en_el_siguiente_pide() {
    let f = con_monton(
        "        a = pide(base, 100)\n        b = pide(base, 100)\n        suelta(base, a)\n        c = pide(base, 100)\n        si c no es a\n            devuelve 0\n        si b = a\n            devuelve 0\n        devuelve 1\n",
    );
    assert_eq!(ejecuta_en(&f, "prueba", 0x40000, 4096), 1);
}

/// **`suelta` devuelve CUANTOS bytes vuelven**, y el numero sale de la cabecera
/// del trozo, no de quien suelta.
///
/// 100 bytes pedidos se redondean a 112 --el monton reparte a 16-- y eso es lo
/// que vuelve. Que el numero no sea 100 es la prueba de que sale de la cabecera.
#[test]
fn suelta_dice_cuantos_bytes_devuelve_y_salen_de_la_cabecera() {
    let f = con_monton(
        "        a = pide(base, 100)\n        devuelve suelta(base, a)\n",
    );
    assert_eq!(ejecuta_en(&f, "prueba", 0x40000, 4096), 112);
}

/// *** EL MONTON SE PUEDE AUDITAR: `queda_suelto` recorre la lista y suma.
///
/// ** Sin este numero, *"suelta de verdad"* seria una afirmacion sin forma de
/// comprobarla desde fuera. Es la costumbre de esta casa: el dato sale del
/// propio monton, no de quien lo usa.
#[test]
fn lo_suelto_se_puede_contar_y_vuelve_a_cero_al_reutilizarlo() {
    // Dos trozos sueltos: 112 + 112.
    let dos = con_monton(
        "        a = pide(base, 100)\n        b = pide(base, 100)\n        suelta(base, a)\n        suelta(base, b)\n        devuelve queda_suelto(base)\n",
    );
    assert_eq!(ejecuta_en(&dos, "prueba", 0x40000, 4096), 224);

    // Y al reutilizar uno, la cuenta baja: el hueco sale de la lista.
    let uno = con_monton(
        "        a = pide(base, 100)\n        b = pide(base, 100)\n        suelta(base, a)\n        suelta(base, b)\n        c = pide(base, 100)\n        devuelve queda_suelto(base)\n",
    );
    assert_eq!(ejecuta_en(&uno, "prueba", 0x40000, 4096), 112);
}

/// [!] Y EL CURSOR NO SE MUEVE al soltar: un trozo vuelve por la lista, no
/// desandando el camino.
///
/// ** Desandar solo se podria con el ULTIMO trozo, y una regla que funciona a
/// veces es peor que una que no funciona nunca -- porque la primera se aprende
/// mal y se usa donde no vale.
#[test]
fn soltar_no_baja_el_cursor() {
    let f = con_monton(
        "        a = pide(base, 100)\n        antes = queda_en(base)\n        suelta(base, a)\n        si queda_en(base) no es antes\n            devuelve 0\n        devuelve 1\n",
    );
    assert_eq!(ejecuta_en(&f, "prueba", 0x40000, 4096), 1);
}

/// Un hueco demasiado pequeno NO se reutiliza: se sigue mirando, y si ninguno
/// cabe, avanza el cursor.
///
/// ** Sin esta, `pide` podria estar devolviendo el primer hueco de la lista sin
/// mirar su medida -- y la prueba de arriba seguiria en verde, porque alli todos
/// los trozos miden lo mismo.
#[test]
fn un_hueco_que_no_cabe_no_se_reutiliza() {
    let f = con_monton(
        "        pequeno = pide(base, 16)\n        suelta(base, pequeno)\n        grande = pide(base, 500)\n        si grande = pequeno\n            devuelve 0\n        devuelve 1\n",
    );
    assert_eq!(ejecuta_en(&f, "prueba", 0x40000, 4096), 1);
}

/// Soltar la direccion cero no toca nada, y no es una comprobacion de adorno:
/// `pide` devuelve 0 cuando no cabe, asi que **el cero llega aqui por el camino
/// normal** el dia que un programa no mire lo que le dieron.
#[test]
fn soltar_un_cero_no_rompe_la_lista() {
    let f = con_monton(
        "        suelta(base, 0)\n        devuelve queda_suelto(base)\n",
    );
    assert_eq!(ejecuta_en(&f, "prueba", 0x40000, 4096), 0);
}

// ===================================================================
//  *** EL CONTADOR DE REFERENCIAS (2026-08-23)
// ===================================================================
//
//  La pieza que le faltaba a `pleno` para tener objetos con vida propia. Vive
//  en `runtime/objetos/contador.inti` y **esta escrita en INTI `llano`**, que es
//  como este proyecto demuestra que `llano` sirve para escribir el sistema en
//  vez de repetirlo.

/// Un monton fabricado a mano, un objeto pedido, y su contador puesto a `refs`.
///
/// El cuerpo recibe `o` --la direccion del objeto, que es la que devolvio
/// `pide`-- y `refs` como segundo argumento de la funcion.
fn con_objeto(cuerpo: &str) -> String {
    format!(
        "perfil llano\nusa objetos\nusa monton\n\nfuncion prueba(base es natural64, refs es natural64) devuelve natural64\n    crudo\n        escribe_natural64(base, base + 32)\n        escribe_natural64(base + 8, base + 4096)\n        escribe_natural64(base + 16, 0)\n        o = pide(base, 64)\n        escribe_natural64(o, refs)\n{}",
        cuerpo
    )
}

const INMORTAL: u64 = 1 << 63;

/// ***`retiene` DICE LO MISMO QUE EL ABI, valor por valor.***
///
/// ** Esta es la prueba que importa de las cuatro. `bmo_abi::dynobj::header`
/// declara la semantica --`retain`, `release`, `is_last`-- y `contador.inti` la
/// vuelve a escribir en otro lenguaje. **Dos escrituras de la misma regla se
/// separan el dia que alguien toca una**, y este proyecto ya se comio ese fallo
/// con la tabla de intrinsecos esta misma manana.
///
/// Asi que no se comprueba "que suba": se comprueba que **coincida**.
#[test]
fn retiene_dice_lo_mismo_que_el_abi() {
    use bmo_abi::dynobj::header;
    let f = con_objeto("        devuelve retiene(o)\n");
    for refs in [0u64, 1, 2, 7, 1000, INMORTAL, INMORTAL | 5] {
        assert_eq!(
            ejecuta_en(&f, "prueba", 0x40000, refs),
            header::retain(refs),
            "INTI y el ABI no dicen lo mismo de retain({refs:#x})"
        );
    }
}

/// Y `libera` tambien: **el que muere es el que el ABI llama `is_last`.**
#[test]
fn libera_mata_exactamente_a_quien_el_abi_llama_el_ultimo() {
    use bmo_abi::dynobj::header;
    let f = con_objeto("        devuelve libera(base, o)\n");
    for refs in [0u64, 1, 2, 7, 1000, INMORTAL, INMORTAL | 5] {
        let murio = ejecuta_en(&f, "prueba", 0x40000, refs) == 1;
        assert_eq!(
            murio,
            header::is_last(refs),
            "INTI y el ABI no dicen lo mismo de is_last({refs:#x})"
        );
    }
}

/// ***CUANDO MUERE, EL TROZO VUELVE AL MONTON.*** Que es de lo que iba todo.
///
/// 64 bytes pedidos, 64 sueltos. Sin esto, "contador de referencias" seria un
/// numero que baja y una memoria que no vuelve -- que es exactamente lo que
/// `dynobj::lista` avisa de si mismo: *"el contador cuenta y no libera"*.
#[test]
fn al_morir_el_trozo_vuelve_al_monton() {
    let f = con_objeto("        libera(base, o)\n        devuelve queda_suelto(base)\n");
    assert_eq!(ejecuta_en(&f, "prueba", 0x40000, 1), 64);

    // Y con dos duenos NO vuelve: solo baja el contador.
    assert_eq!(ejecuta_en(&f, "prueba", 0x40000, 2), 0);
}

/// *** A UN INMORTAL NO SE LE TOCA: ni el contador, ni el monton.
///
/// ** Y no basta con dejar el numero igual: **escribir los mismos bytes en una
/// pagina de solo lectura falla igual**. Un literal de texto vive en `RoData`
/// desde esta misma manana, asi que esto dejo de ser teorico hoy.
#[test]
fn un_inmortal_ni_se_cuenta_ni_se_suelta() {
    let f = con_objeto(
        "        libera(base, o)\n        si queda_suelto(base) no es 0\n            devuelve 100\n        devuelve referencias(o)\n",
    );
    // El contador vuelve intacto, con su bit 63 y su parte baja.
    assert_eq!(ejecuta_en(&f, "prueba", 0x40000, INMORTAL | 5), INMORTAL | 5);
}

/// ***EL DOBLE `libera` NO DA LA VUELTA, Y ESA ES LA REGLA CARA.***
///
/// Si un contador en cero diera la vuelta valdria `0xFFFF_FFFF_FFFF_FFFF`, que
/// **tiene el bit 63 puesto**: un doble-`libera` convertiria el objeto en
/// INMORTAL en silencio -- una fuga que no se denuncia a si misma jamas.
///
/// ** `header.rs` lo dice con esas palabras --*"it would be the `unwrap_or(0)`
/// failure in a new costume"*-- y se comprueba aqui porque **aqui es donde se
/// puede romper**: en el ABI es una funcion pura; en `contador.inti` es una
/// escritura a memoria.
#[test]
fn un_doble_libera_no_convierte_el_objeto_en_inmortal() {
    let f = con_objeto(
        "        libera(base, o)\n        libera(base, o)\n        devuelve referencias(o)\n",
    );
    let quedo = ejecuta_en(&f, "prueba", 0x40000, 1);
    assert_eq!(quedo, 0, "satura en cero");
    assert_eq!(quedo & INMORTAL, 0, "y sobre todo: NO se volvio inmortal");
}

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
//  *** EL SIGNO (2026-08-23) -- cuatro familias, y ninguna fallaba
// ===================================================================
//
//  Se destapo escribiendo A MANO una guardia de desbordamiento dentro de un
//  bloque `crudo`. La guardia estaba bien y no saltaba, porque el emisor bajaba
//  TODA comparacion con `setl` -- la version con signo.
//
//  ** No era comportamiento indefinido. Era peor de encontrar: **una respuesta
//  equivocada, en silencio, sin que ninguna de las doce reglas saltara.** Las
//  reglas vigilan lo que C deja SIN DEFINIR; esto estaba definido, y mal.

/// ***`2 < 18446744073709551615` EN `natural64` ES CIERTO.***
///
/// Con `setl` daba 0: leidos con signo, 2^64-1 es -1. Y este no es un caso de
/// laboratorio -- son **direcciones**: `si nuevo > fin` dentro del propio
/// monton compara punteros, y el dia que uno pase del bit 63 la comparacion
/// contesta al reves.
#[test]
fn los_naturales_se_comparan_sin_signo() {
    let f = "perfil llano\n\nfuncion prueba(a es natural64, b es natural64) devuelve natural64\n    si a < b\n        devuelve 1\n    devuelve 0\n";
    assert_eq!(ejecuta_en(f, "prueba", 2, u64::MAX), 1, "2 < 2^64-1");
    assert_eq!(ejecuta_en(f, "prueba", u64::MAX, 2), 0, "y al reves, no");
}

/// Y los enteros SIGUEN con signo, que es la otra mitad y la que se podia
/// romper al arreglar la primera.
#[test]
fn los_enteros_se_siguen_comparando_con_signo() {
    let f = "perfil llano\n\nfuncion prueba(a es entero64, b es entero64) devuelve natural64\n    si a < b\n        devuelve 1\n    devuelve 0\n";
    assert_eq!(ejecuta_en(f, "prueba", (-1i64) as u64, 2), 1, "-1 < 2");
    assert_eq!(ejecuta_en(f, "prueba", 2, (-1i64) as u64), 0);
}

/// ***DIVIDIR: `div` para los naturales, `idiv` para los enteros.***
///
/// Con `idiv`, dividir 2^63 entre 2 no da 2^62: da una **excepcion del
/// procesador**, porque el cociente no cabe en un `entero64` con signo.
#[test]
fn dividir_un_natural_grande_no_revienta() {
    let f = "perfil llano\n\nfuncion prueba(a es natural64, b es natural64) devuelve natural64\n    devuelve a entre b\n";
    assert_eq!(ejecuta_en(f, "prueba", 1u64 << 63, 2), 1u64 << 62);
    assert_eq!(ejecuta_en(f, "prueba", u64::MAX, 2), u64::MAX / 2);
}

/// Y la division con signo sigue dando negativo.
#[test]
fn dividir_enteros_sigue_llevando_el_signo() {
    let f = "perfil llano\n\nfuncion prueba(a es entero64, b es entero64) devuelve entero64\n    devuelve a entre b\n";
    assert_eq!(ejecuta_en(f, "prueba", (-8i64) as u64, 2), (-4i64) as u64);
}

/// ***DESPLAZAR A LA DERECHA: el fallo AL REVES, y del mismo dia.***
///
/// Aqui el emisor emitia `shr` SIEMPRE --metiendo ceros por arriba-- que es lo
/// correcto para un natural y falso para un entero negativo:
///
/// ```text
///    -8 desplaza derecha 1     con `sar`  ->  -4
///                              con `shr`  ->  9.223.372.036.854.775.804
/// ```
///
/// ** El propio `x86::shr_r64_cl` predijo el dia: *"el dia que INTI distinga el
/// desplazamiento con signo sera otra fila de la tabla y otra instruccion"*.
#[test]
fn desplazar_un_entero_negativo_arrastra_el_signo() {
    let f = "perfil llano\n\nfuncion prueba(a es entero64, b es entero64) devuelve entero64\n    devuelve a desplaza derecha b\n";
    assert_eq!(ejecuta_en(f, "prueba", (-8i64) as u64, 1), (-4i64) as u64);

    // Y un natural sigue metiendo ceros, que es lo suyo.
    let g = "perfil llano\n\nfuncion prueba(a es natural64, b es natural64) devuelve natural64\n    devuelve a desplaza derecha b\n";
    assert_eq!(ejecuta_en(g, "prueba", u64::MAX, 1), u64::MAX >> 1);
}

/// [!] Y LA ARITMETICA DE DIRECCIONES ES SIN SIGNO, aunque nadie escriba un
/// tipo: `p.x` y `a[i]` suman bytes a una direccion, y una direccion no puede
/// ser negativa.
///
/// ** Sin esto, un registro colocado por encima del bit 63 se indexaria al
/// reves. Hoy no pasa porque el monton vive bajo, y "hoy no pasa" es
/// exactamente como se escriben los fallos que aparecen dentro de dos anos.
#[test]
fn la_aritmetica_de_direcciones_no_lleva_signo() {
    // Se mira en la IR y no en los bytes a proposito: un `add` es el mismo byte
    // lleve signo o no. Lo que cambia es lo que se emite DESPUES --la
    // comparacion, el desplazamiento, la guardia-- y eso sale de esta marca.
    let fuente = concat!(
        "perfil llano\n\n",
        "registro Punto\n    x es entero64\n    y es entero64\n\n",
        "funcion prueba(p es Punto) devuelve entero64\n    devuelve p.y\n"
    );
    let arbol = bmo_inti_front::armar(fuente);
    assert!(!arbol.hay_errores(), "{}", arbol.pintar("p.inti"));
    let raices = bmo_mods::Roots::find();
    let modulos = bmo_inti_front::tablas::Modulos::cargar(&raices);
    let plano = bmo_inti_front::disposicion::comprobar(
        &arbol.valor,
        bmo_inti_front::disposicion::Medidas::cargar(&raices),
    );
    let metal = ir::metal_que_declara(&arbol.valor, &raices, &modulos);
    let m = ir::bajar_con(&arbol.valor, &modulos, &plano.valor, &metal).valor;

    let sumas: Vec<bool> = m
        .funciones
        .iter()
        .flat_map(|f| f.instrucciones.iter())
        .filter_map(|i| match i {
            Instr::Binaria { sin_signo, .. } => Some(*sin_signo),
            _ => None,
        })
        .collect();
    assert!(!sumas.is_empty(), "sin aritmetica de campos que mirar");
    assert!(
        sumas.iter().all(|s| *s),
        "la aritmetica de direcciones perdio la marca de sin signo: {sumas:?}"
    );
}

// ===================================================================
//  *** `texto + texto` BAJA A UNA LLAMADA, no a un `add` (2026-08-23)
// ===================================================================

fn ir_de(fuente: &str) -> bmo_inti_front::ir::ModuloIr {
    let arbol = bmo_inti_front::armar(fuente);
    assert!(!arbol.hay_errores(), "{}", arbol.pintar("p.inti"));
    let raices = bmo_mods::Roots::find();
    let modulos = bmo_inti_front::tablas::Modulos::cargar(&raices);
    let plano = bmo_inti_front::disposicion::comprobar(
        &arbol.valor,
        bmo_inti_front::disposicion::Medidas::cargar(&raices),
    );
    let metal = ir::metal_que_declara(&arbol.valor, &raices, &modulos);
    ir::bajar_con(&arbol.valor, &modulos, &plano.valor, &metal).valor
}

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
            .any(|w| w == arranque::MONTON_DE_LA_TAREA.to_le_bytes()),
        "no aparece el 4096 del monton de la tarea"
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

// ===================================================================
//  *** EL DECIMAL EXACTO (2026-08-23) -- la promesa de la portada
// ===================================================================

/// Tres numeros a mano: `a` en `base`, `b` en `base+16`, el resultado en `+32`.
fn con_decimales(cuerpo: &str) -> String {
    format!(
        "perfil llano\nusa decimal\nusa memoria\n\nfuncion prueba(base es natural64, x es natural64) devuelve natural64\n    crudo\n        a = base\n        b = base + 16\n        c = base + 32\n{}",
        cuerpo
    )
}

/// ***`0.1 + 0.2` DA `0.3`.*** Es la frase de la portada, ejecutada.
///
/// ** En binario no existe un `0.1`. Lo que existe es **un entero y una
/// escala**: `0.1` es el par `(1, 1)`, `0.2` es `(2, 1)`, y sumarlos da `(3, 1)`
/// -- que es `0.3` EXACTO. No hay redondeo porque no hay conversion.
#[test]
fn cero_uno_mas_cero_dos_da_cero_tres() {
    let f = con_decimales(
        "        pon_numero(a, 1, 1)\n        pon_numero(b, 2, 1)\n        si suma(c, a, b) no es 1\n            devuelve 0\n        si escala(c) no es 1\n            devuelve 0\n        devuelve natural64(coeficiente(c))\n",
    );
    assert_eq!(ejecuta_en(&f, "prueba", 0x40000, 0), 3, "(1,1) + (2,1) = (3,1)");
}

/// ***LAS ESCALAS SE IGUALAN SUBIENDO, NUNCA BAJANDO.***
///
/// `1.5 + 0.25` = `(15,1) + (25,2)`. Subir `15` a escala 2 da `150`, y
/// `150 + 25 = 175` -> `1.75`, exacto.
///
/// ** Bajar seria dividir, y dividir pierde: `0.25` a escala 1 seria `0.2` o
/// `0.3` **y habria que elegir**. Subir no pierde nada, asi que la suma de dos
/// exactos sigue siendo exacta -- que es lo que separa esto de un flotante.
#[test]
fn las_escalas_se_igualan_subiendo_y_no_se_pierde_nada() {
    let f = con_decimales(
        "        pon_numero(a, 15, 1)\n        pon_numero(b, 25, 2)\n        si suma(c, a, b) no es 1\n            devuelve 0\n        si escala(c) no es 2\n            devuelve 0\n        devuelve natural64(coeficiente(c))\n",
    );
    assert_eq!(ejecuta_en(&f, "prueba", 0x40000, 0), 175, "1.5 + 0.25 = 1.75");
}

/// Y con los negativos igual: `(-1,1) + (2,1)` = `0.1`.
#[test]
fn los_negativos_suman_con_su_signo() {
    let f = con_decimales(
        "        pon_numero(a, -1, 1)\n        pon_numero(b, 2, 1)\n        suma(c, a, b)\n        devuelve natural64(coeficiente(c))\n",
    );
    assert_eq!(ejecuta_en(&f, "prueba", 0x40000, 0), 1, "-0.1 + 0.2 = 0.1");
}

/// ***MULTIPLICAR SUMA LAS ESCALAS, y no iguala nada.***
///
/// `0.5 * 0.25` = `(5,1) * (25,2)` = `(125, 3)` = `0.125`. Sale exacto sin tocar
/// nada: es la operacion BARATA de este formato, al reves que en coma flotante.
#[test]
fn multiplicar_suma_las_escalas() {
    let f = con_decimales(
        "        pon_numero(a, 5, 1)\n        pon_numero(b, 25, 2)\n        si multiplica(c, a, b) no es 1\n            devuelve 0\n        si escala(c) no es 3\n            devuelve 0\n        devuelve natural64(coeficiente(c))\n",
    );
    assert_eq!(ejecuta_en(&f, "prueba", 0x40000, 0), 125, "0.5 * 0.25 = 0.125");
}

/// ***Y CUANDO NO CABE, LO DICE. No da un numero equivocado.***
///
/// Dentro de `crudo` la Regla 1 esta APAGADA, asi que la guardia se escribe a
/// mano: la suma de dos con signo desborda cuando los dos sumandos tienen el
/// mismo signo y el resultado tiene otro.
///
/// [!] Y esa comprobacion mira CON SIGNO. Funciona desde esta misma manana:
/// hasta hoy el emisor bajaba toda comparacion con `setl` mirase lo que mirase,
/// y esta guardia habria acertado por casualidad -- que es peor que fallar.
#[test]
fn una_suma_que_no_cabe_atrapa_con_la_regla_1() {
    let f = con_decimales(
        "        pon_numero(a, 9223372036854775807, 0)\n        pon_numero(b, 1, 0)\n        devuelve suma(c, a, b)\n",
    );
    assert_eq!(
        ejecuta_en(&f, "prueba", 0x40000, 0),
        1001,
        "el maximo mas uno tenia que atrapar con la Regla 1"
    );
}

/// ***UNA TRAMPA DENTRO DE UNA LIBRERIA SE CONVIERTE EN UN NUMERO.***
///
/// # Esta prueba fija un DEFECTO, no una virtud. Es P4.
///
/// `sube(1e18, 18)` desborda y **atrapa de verdad**: llamada a pelo devuelve
/// `1001`. Pero llamada desde `suma` no para nada, y `suma` contesta `1` como si
/// hubiera salido bien.
///
/// ## Por que, y es una linea del emisor
///
/// El bloque de atrapar hace esto:
///
/// ```text
///    mov  <retorno>, 1001
///    <epilogo>
///    ret
/// ```
///
/// **Pone el codigo en el registro de retorno y VUELVE.** No mata la tarea. Asi
/// que para quien llamo, atrapar y devolver un numero **son la misma cosa** -- y
/// `1001` es un coeficiente perfectamente valido.
///
/// *** Y ESTO YA ESTABA EN EL PLAN, con nombre y con la frase justa:
/// `PLAN_EL_SILICIO.md`, **P4 -- EL CAMINO DE VUELTA: atrapar deja de ser
/// devolver un numero**, descrito como *"el peldano que sostiene todo lo
/// demas"*. Lo que anade esta prueba es que deja de ser una prevision: es un
/// caso, con su numero.
///
/// [!] Y es el fallo silencioso mas grande que hay hoy en INTI. Las cuatro
/// reglas atrapan --eso es cierto y esta comprobado-- pero **una trampa dentro
/// de una libreria no llega a nadie**. Cuanto mas runtime se escriba en INTI,
/// mas caro sale.
///
/// El dia que P4 entre, esta prueba se pone roja. Es lo que se quiere.
#[test]
fn hoy_una_trampa_en_una_libreria_vuelve_como_un_numero() {
    // A pelo: atrapa, y se ve.
    let sola = con_decimales("        devuelve natural64(sube(1000000000000000000, 18))\n");
    assert_eq!(
        ejecuta_en(&sola, "prueba", 0x40000, 0),
        1001,
        "`sube` tiene que atrapar: `1e18 * 1e18` no cabe"
    );

    // Desde dentro: la misma trampa, y el llamante no se entera.
    let dentro = con_decimales(
        "        pon_numero(a, 1000000000000000000, 0)\n        pon_numero(b, 1, 18)\n        devuelve suma(c, a, b)\n",
    );
    assert_eq!(
        ejecuta_en(&dentro, "prueba", 0x40000, 0),
        1,
        "P4: `suma` recibio 1001 como si fuera un coeficiente y siguio"
    );
}

/// Comparar tambien iguala escalas: `0.5` y `0.50` son el MISMO numero.
#[test]
fn comparar_iguala_escalas_antes_de_mirar() {
    let f = con_decimales(
        "        pon_numero(a, 5, 1)\n        pon_numero(b, 50, 2)\n        si menor(a, b) no es 0\n            devuelve 0\n        si menor(b, a) no es 0\n            devuelve 0\n        devuelve 1\n",
    );
    assert_eq!(ejecuta_en(&f, "prueba", 0x40000, 0), 1, "0.5 no es menor que 0.50");

    let g = con_decimales(
        "        pon_numero(a, 5, 1)\n        pon_numero(b, 51, 2)\n        devuelve menor(a, b)\n",
    );
    assert_eq!(ejecuta_en(&g, "prueba", 0x40000, 0), 1, "0.50 < 0.51");
}

/// [!] Y LA ESCALA TIENE TECHO: 18, porque `10^19` no cabe en un `entero64`.
///
/// ** Que la tabla acabe donde acaba el tipo no es casualidad: es el limite del
/// coeficiente dicho de otra forma. Pedir mas contesta 0 -- **no la ultima
/// potencia**, porque quien pide `10^25` tiene un problema que no se arregla
/// dandole `10^18`: se convertiria en un numero equivocado en vez de en un no.
#[test]
fn la_escala_tiene_techo_y_pasarse_no_devuelve_lo_mas_parecido() {
    let f = con_decimales("        devuelve natural64(potencia(19))\n");
    assert_eq!(ejecuta_en(&f, "prueba", 0x40000, 0), 0);

    let g = con_decimales("        devuelve natural64(potencia(18))\n");
    assert_eq!(ejecuta_en(&g, "prueba", 0x40000, 0), 1_000_000_000_000_000_000);
}

/// ***`crudo` NO APAGA LAS REGLAS.*** Y esto hubo que comprobarlo (2026-08-23).
///
/// Durante todo el dia se escribio lo contrario en tres ficheros del runtime:
/// *"dentro de `crudo` la Regla 1 esta APAGADA, porque tocar memoria cruda no
/// puede pagar una guardia por operacion"*. **Es falso.**
///
/// `ir::mod.rs` baja `Sent::Crudo` con `self.bloque(cuerpo)` y nada mas: las
/// comprobaciones se emiten igual. Lo destapo una prueba del decimal que
/// esperaba un `0` y recibio **1001** -- el codigo de la Regla 1.
///
/// ## *** Y lo que `crudo` SI significa, que es otra cosa y mejor
///
/// Es un permiso, no un interruptor: **"aqui se toca el metal, y al otro lado no
/// hay nadie que compruebe"**. Lo vigila `perfil`, que sin `crudo` no deja
/// llamar a `lee_natural64`. Las reglas del LENGUAJE siguen puestas.
///
/// ** O sea que el runtime de INTI esta protegido por las reglas de INTI incluso
/// donde toca memoria cruda. Es mejor de lo que yo estaba escribiendo.
#[test]
fn crudo_no_apaga_las_reglas_del_lenguaje() {
    let e = emitido("perfil llano

funcion f(a es entero64, b es entero64) devuelve entero64
    crudo
        devuelve a + b
");
    assert!(
        e.katanas.iter().any(|(k, _, _)| *k as u32 == 1001),
        "una suma dentro de `crudo` se quedo sin su Regla 1: {:?}",
        e.katanas
    );
}

// ===================================================================
//  *** `numero + numero` EN EL DESCENSO (2026-08-23)
// ===================================================================

/// ***`a + b` DE DOS `numero` LLAMA A `suma`, no emite un `add`.***
///
/// ** Un `numero` mide 16 bytes --coeficiente `entero64` mas escala-- asi que
/// **no cabe en un registro**. Un `add` sumaria los ocho bytes bajos de cada uno
/// --los coeficientes-- **ignorando las escalas**:
///
/// ```text
///    1.5 + 0.25   con `add`    ->  (40, ?)   los coeficientes 15 y 25
///                 con `suma`   ->  (175, 2)  = 1.75
/// ```
///
/// Compilaria, correria, y daria otro numero. La familia de siempre.
#[test]
fn sumar_dos_numeros_llama_al_decimal_y_no_emite_un_add() {
    let m = ir_de("perfil pleno\nusa decimal\n\nfuncion f(a es numero, b es numero) devuelve numero\n    devuelve a + b\n");
    let f = m.funciones.iter().find(|f| f.nombre == "f").expect("sin `f`");
    assert!(
        f.instrucciones.iter().any(|i| matches!(
            i,
            Instr::Llama { que: Valor::Nombre(n), .. } if n == "suma"
        )),
        "`a + b` de numeros no llamo a `suma`: {:?}",
        f.instrucciones
    );
    assert!(
        !f.instrucciones.iter().any(|i| matches!(
            i,
            Instr::Binaria { op: bmo_inti_front::arbol::Op::Suma, .. }
        )),
        "quedo un `add`: eso suma coeficientes e ignora las escalas"
    );
}

/// ***Y EL RESULTADO VIVE EN UNA LOCAL DE 16 BYTES, no en un temporal.***
///
/// ** Un temporal es UNA PALABRA, y esa es toda su definicion. El resultado de
/// una suma decimal no cabe, asi que el descenso pide una local ANONIMA y pasa
/// su direccion. Es la primera vez que INTI necesita una local de mas de una
/// palabra -- y la que obligo al marco a repartir por MEDIDA en vez de por
/// cuenta.
#[test]
fn el_resultado_decimal_vive_en_una_local_de_dieciseis_bytes() {
    let m = ir_de("perfil pleno\nusa decimal\n\nfuncion f(a es numero, b es numero) devuelve numero\n    devuelve a + b\n");
    let f = m.funciones.iter().find(|f| f.nombre == "f").unwrap();
    assert!(
        f.medidas_locales.iter().any(|x| *x == 16),
        "no hay ninguna local de 16 bytes: {:?}",
        f.medidas_locales
    );
    assert!(
        f.instrucciones
            .iter()
            .any(|i| matches!(i, Instr::DireccionDeLocal { .. })),
        "nadie pidio la direccion de la local"
    );
}

/// ***EL MARCO NO PISA NADA.*** Dos `numero` seguidos ocupan 32 bytes, y el
/// tercero cae DETRAS -- no encima de la segunda mitad del segundo.
///
/// ** Antes del 2026-08-23 el marco daba **una palabra a cada local**:
/// `local(l) = -((l+1) * PALABRA)`. Con un `numero` de 16 bytes, la local de al
/// lado caia dentro. En silencio, y con la direccion bien puesta.
#[test]
fn dos_numeros_seguidos_no_se_pisan_en_el_marco() {
    let m = ir_de("perfil pleno\nusa decimal\n\nfuncion f devuelve entero32\n    a es numero = 1\n    b es numero = 2\n    c es entero64 = 7\n    devuelve 0\n");
    let f = m.funciones.iter().find(|f| f.nombre == "f").unwrap();
    let marco = crate::marco::Marco::de(f);

    let sitios: Vec<i32> = (0..f.locales).map(|i| marco.local(bmo_inti_front::ir::Local(i))).collect();
    // Cada local tiene que caber entre su sitio y el de la anterior.
    for (i, medida) in f.medidas_locales.iter().enumerate() {
        let m_i = if *medida == 0 { 8 } else { *medida as i32 };
        let mio = sitios[i];
        for (j, otro) in sitios.iter().enumerate() {
            if i == j {
                continue;
            }
            let m_j = f.medidas_locales.get(j).copied().unwrap_or(8).max(1) as i32;
            let solapa = mio < *otro + m_j && *otro < mio + m_i;
            assert!(!solapa, "la local {} y la {} se pisan: {} y {}", i, j, mio, otro);
        }
    }
}
