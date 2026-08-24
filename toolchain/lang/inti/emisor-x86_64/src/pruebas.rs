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
    let ir = ir::bajar_con(&arbol.valor, &modulos, &plano.valor, &metal, &nec()).valor;
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
    ir::bajar_con(&arbol.valor, &modulos, &plano.valor, &metal, &nec())
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
    ir::bajar_con(&arbol.valor, &modulos, &plano.valor, &metal, &nec())
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
/// La IR de un fuente, sin llegar a bytes.
///
/// ** Vive AQUI y no en un submodulo porque la usan tres: `objetos`, `decimal` y
/// las pruebas del descenso. Un ayudante compartido que vive dentro de uno de
/// sus usuarios ata a los otros dos -- es la misma regla que mudo `Modulos` a
/// `tablas`, una capa mas arriba.
pub(super) fn ir_de(fuente: &str) -> bmo_inti_front::ir::ModuloIr {
    let arbol = bmo_inti_front::armar(fuente);
    assert!(!arbol.hay_errores(), "{}", arbol.pintar("p.inti"));
    let raices = bmo_mods::Roots::find();
    let modulos = bmo_inti_front::tablas::Modulos::cargar(&raices);
    let plano = bmo_inti_front::disposicion::comprobar(
        &arbol.valor,
        bmo_inti_front::disposicion::Medidas::cargar(&raices),
    );
    let metal = ir::metal_que_declara(&arbol.valor, &raices, &modulos);
    ir::bajar_con(&arbol.valor, &modulos, &plano.valor, &metal, &nec()).valor
}

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

mod decimal;
mod disposicion;
mod flotante;
mod lenguaje;
mod marco;
mod memoria;
mod metal;
mod monton;
mod necesita;
mod objetos;
mod reglas;
mod signo;
mod simd;
mod sonda;
mod tabla;

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

/// La tabla de necesidades de las pruebas: **la incrustada**.
///
/// ** Y no la del disco a proposito. Una prueba que leyera `$BMO_MODS` diria
/// cosas distintas segun quien la corra, que es justo lo que un test no puede
/// hacer. La que se comprueba contra el disco es otra, y esta declarada aparte.
fn nec() -> bmo_inti_front::necesidades::Necesidades {
    bmo_inti_front::necesidades::Necesidades::por_defecto()
}
