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
            // ** La 2 no se puede provocar y ESO es el dato: indexar un `bufer`
            // pide `crudo` justamente porque no hay contra que comprobar.
            Comprobacion::Indice => None,
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
