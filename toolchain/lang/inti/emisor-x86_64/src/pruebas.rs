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
    let bex = empaquetar(&e).expect("el gate lo rechazo");
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

// ===================================================================
//  *** F3: los temporales viven en registros
// ===================================================================

/// Cuantas veces el codigo toca el marco.
///
/// Un `mov` contra `[rbp+disp32]` empieza por `48 8B` (leer) o `48 89`
/// (escribir), y el tercer byte lleva el registro metido dentro. Contarlos es
/// contar **las veces que el programa baja a memoria**, que es exactamente lo
/// que F3 existe para reducir.
fn accesos_al_marco(codigo: &[u8]) -> usize {
    let modrm: Vec<u8> = (0u8..8).map(|r| 0x85 | (r << 3)).collect();
    codigo
        .windows(3)
        .filter(|w| w[0] == 0x48 && (w[1] == 0x8B || w[1] == 0x89) && modrm.contains(&w[2]))
        .count()
}

/// *** Con tres registros, una suma no baja a memoria mas que para recoger sus
/// parametros al entrar.
#[test]
fn los_temporales_ya_no_bajan_a_memoria() {
    let e = emitido(SUMA);
    assert!(e.en_registros >= 1, "algun temporal tiene que ir a registro");
    assert_eq!(e.en_pila, 0, "y ninguno deberia quedarse en la pila");

    // Los dos unicos accesos son los dos parametros, que el prologo copia al
    // marco. Todo lo demas vive en registros.
    assert_eq!(
        accesos_al_marco(&e.codigo),
        4,
        "dos escrituras de parametros y sus dos lecturas: el resto es registro"
    );
}

/// Y una expresion con dos operaciones tampoco.
#[test]
fn una_expresion_encadenada_se_queda_en_registros() {
    let f = "perfil llano\n\nfuncion f(a es entero64, b es entero64) devuelve entero64\n    devuelve a + b * 2\n";
    let e = emitido(f);
    assert_eq!(e.en_pila, 0);
    // Y sigue dando lo que tiene que dar.
    assert_eq!(ejecuta(f, 3, 4), 11);
}

/// ** El numero que se puede seguir en el tiempo, como los `crudo`.
#[test]
fn el_emisor_dice_cuantos_temporales_salvo() {
    let e = emitido(SUMA);
    assert_eq!(e.en_registros + e.en_pila, 1, "la suma usa un temporal");
    assert_eq!(e.en_registros, 1);
}

/// OJO: Y lo que NO cambio: sigue atrapando. Optimizar no puede quitar una
/// comprobacion que el lenguaje promete -- y este test es el que lo impide.
#[test]
fn con_registros_la_comprobacion_sigue_estando() {
    let e = emitido(SUMA);
    assert_eq!(e.comprobaciones, 1);
    assert_eq!(ejecuta(SUMA, i64::MAX as u64, 1), 1001);
    assert_eq!(ejecuta(SUMA, 3, 4), 7);
}

// ===================================================================
//  *** Las llamadas: lo que desbloquea todo lo demas
// ===================================================================

/// Una funcion de INTI llama a otra de INTI, y sale el numero bueno.
///
/// Es la pieza que faltaba para que exista un runtime: **todo runtime son
/// llamadas**. Sin esto, `pleno` no podia empezar.
#[test]
fn una_funcion_llama_a_otra_y_corre() {
    let f = "\
perfil llano

funcion doble(x es entero64) devuelve entero64
    devuelve x + x

funcion principal(a es entero64, b es entero64) devuelve entero64
    devuelve doble(a) + b
";
    // doble(3) + 4 = 10
    assert_eq!(ejecuta_en(f, "principal", 3, 4), 10);
    assert_eq!(ejecuta_en(f, "principal", 10, 1), 21);
}

/// Y hacia ATRAS: una funcion puede llamar a otra declarada mas abajo.
///
/// ** Por eso los destinos se resuelven al final del modulo y no sobre la
/// marcha: resolverlos segun se emite obligaria a ordenar las funciones por
/// quien llama a quien, y eso es imposible en cuanto dos se llaman entre si.
#[test]
fn se_puede_llamar_a_una_funcion_declarada_mas_abajo() {
    let f = "\
perfil llano

funcion principal(a es entero64, b es entero64) devuelve entero64
    devuelve mas_tarde(a)

funcion mas_tarde(x es entero64) devuelve entero64
    devuelve x + 100
";
    assert_eq!(ejecuta_en(f, "principal", 5, 0), 105);
}

/// ** El freno del asignador, ejercitado por fin.
///
/// Una funcion con llamadas no reparte registros -- los tres se los puede pisar
/// la funcion llamada. Y lo que importa es que **sigue dando el resultado
/// correcto**: el freno no rompe nada, solo deja de optimizar.
#[test]
fn con_llamadas_no_se_reparten_registros_y_sigue_bien() {
    let con = "\
perfil llano

funcion uno(x es entero64) devuelve entero64
    devuelve x

funcion principal(a es entero64, b es entero64) devuelve entero64
    devuelve uno(a) + uno(b)
";
    let e = emitido(con);
    assert!(e.en_registros == 0, "con llamadas, nada a registros");
    assert!(e.en_pila > 0, "y los temporales van al marco");
    assert_eq!(ejecuta_en(con, "principal", 6, 7), 13);
}

/// La pila queda alineada a 16 antes de cada llamada. Si no lo estuviera, el
/// fallo aparece DENTRO de la funcion llamada -- el peor sitio posible.
#[test]
fn el_marco_deja_la_pila_alineada_para_llamar() {
    let f = "\
perfil llano

funcion uno(x es entero64) devuelve entero64
    devuelve x + 1

funcion principal(a es entero64, b es entero64) devuelve entero64
    cambiante t = uno(a)
    t = t + uno(b)
    devuelve t
";
    assert_eq!(ejecuta_en(f, "principal", 1, 2), 5);
}


// ===================================================================
//  ** F4a -- EL ARRANQUE. Un programa que empieza y termina solo.
// ===================================================================
//
//  Hasta aqui, cada prueba de este banco envolvia el modulo en un `crt0` de
//  diez bytes escrito a mano, porque una funcion que acaba en `ret` sin nadie
//  que la haya llamado devuelve a cualquier sitio.
//
//  ** Eso se acabo. El modulo trae el suyo, y estas pruebas ejecutan el codigo
//  TAL CUAL SALE DEL EMISOR, desde el byte cero, igual que hara el kernel.
//
//  Y la parte que importa: no comprueban que "no explota". Comprueban que el
//  programa **termino hablando con el kernel**, y con que le hablo.

/// Corre lo que salio del emisor, desde el principio y sin envoltorio.
fn arranca(fuente: &str) -> Machine {
    let e = emitido(fuente);
    assert!(e.arranca, "este fuente tiene `principal` y deberia arrancar solo");
    run(Machine::new(e.codigo), 100_000)
}

/// ** LA PRUEBA DE F4a: un programa de INTI arranca, corre y sale por la
/// puerta con SU codigo.
///
/// Las tres cosas de golpe, y ninguna se puede fingir:
///
///   - `exited` solo se pone si algo cruzo la puerta con `TASK_OP_EXIT`.
///   - `arg0` es lo que `principal` devolvio, asi que el valor viajo desde el
///     `devuelve` hasta el kernel.
///   - y el emulador **para**: no se cae del final del codigo.
#[test]
fn un_programa_arranca_solo_y_sale_por_la_puerta_con_su_codigo() {
    let m = arranca("perfil llano\n\nfuncion principal devuelve entero32\n    devuelve 7\n");

    assert!(m.exited, "el programa no salio por la puerta");
    let ultima = m.syscalls.last().expect("ninguna llamada al sistema");
    assert_eq!(ultima.capability, CURRENT_TASK, "sale sobre su propia tarea");
    assert_eq!(ultima.operation, TASK_OP_EXIT);
    assert_eq!(ultima.arg0, 7, "el codigo de salida es lo que devolvio");
}

/// Y el codigo viaja: no es un cero disfrazado.
#[test]
fn el_codigo_de_salida_es_el_que_devuelve_y_no_otro() {
    for n in [0u64, 1, 42, 255] {
        let f = format!(
            "perfil llano\n\nfuncion principal devuelve entero32\n    devuelve {}\n",
            n
        );
        assert_eq!(arranca(&f).syscalls.last().unwrap().arg0, n);
    }
}

/// El arranque no se salta el trabajo: llama de verdad, y lo que la funcion
/// calcula es lo que sale.
#[test]
fn el_arranque_llama_a_principal_de_verdad() {
    let f = "\
perfil llano

funcion doble(x es entero64) devuelve entero64
    devuelve x + x

funcion principal devuelve entero32
    devuelve doble(21)
";
    assert_eq!(arranca(f).syscalls.last().unwrap().arg0, 42);
}

/// ** Y lo contrario, que es la mitad que se olvida: un modulo SIN `principal`
/// no arranca solo.
///
/// Es la diferencia entre un programa y una biblioteca, y la decide el fuente.
/// Un `.bex` de biblioteca que arrancara al cargarse haria lo que le diera la
/// gana la primera vez que alguien lo abriera para leerle una funcion.
#[test]
fn una_biblioteca_no_arranca_sola() {
    let e = emitido(SUMA);
    assert!(!e.arranca);
    assert!(
        !e.codigo.windows(2).any(|w| w == [0x0F, 0x05]),
        "una biblioteca no cruza ninguna puerta"
    );
}

// ===================================================================
//  ** LA PUERTA -- `invoca` no es un `call`
// ===================================================================

/// La puerta se cruza de verdad, y **no** por una llamada.
///
/// Un `call` a `invoca` habria compilado igual de bien y habria saltado a la
/// direccion cero, porque no existe ninguna funcion con ese nombre. Compilar no
/// prueba nada aqui: hay que verla cruzar.
#[test]
fn invoca_cruza_la_puerta_en_vez_de_llamar() {
    let f = "\
perfil llano
usa bmo

funcion principal devuelve entero32
    respuesta = invoca(7, 3, 0, 0, 0)
    devuelve 0
";
    let m = arranca(f);
    // Dos: la del programa y la de su salida.
    assert_eq!(m.syscalls.len(), 2, "{:?}", m.syscalls);
    assert_eq!(m.syscalls[0].capability, 7, "la capability que se pidio");
    assert_eq!(m.syscalls[0].operation, 3, "y la operacion");
}

/// ** El cuarto argumento de la puerta NO es el cuarto de una llamada.
///
/// Es la fila que justifica que `[puerta]` sea una tabla y no seis lineas de
/// Rust: `syscall` machaca `rcx` con la direccion de vuelta **en el silicio**,
/// asi que un argumento puesto ahi se pierde entre la instruccion y el kernel.
///
/// Y el fallo no se veria: el programa correria, cruzaria la puerta, y el
/// kernel recibiria un numero que no es. Por eso se comprueba aqui y no se
/// confia en haberlo escrito bien.
#[test]
fn el_cuarto_argumento_de_la_puerta_no_es_el_de_una_llamada() {
    let t = Taller::nuevo();
    assert_eq!(t.puerta.argumentos[3], 10, "r10, y no rcx");
    assert_ne!(
        t.puerta.argumentos[3], ARGUMENTOS[3],
        "la puerta y la llamada no pueden coincidir en el cuarto"
    );
    // Los otros cinco si coinciden, y eso tambien hay que fijarlo: si algun dia
    // dejan de hacerlo, sera por un cambio y no por un despiste.
    for i in [0, 1, 2, 4, 5] {
        assert_eq!(t.puerta.argumentos[i], ARGUMENTOS[i], "argumento {}", i);
    }
}

/// Los nombres que abren la puerta salen de `modulos.toml`, no de este crate.
///
/// ** Es la condicion que Eddi puso dos veces --*"la puerta no vive en el
/// lenguaje"*--, comprobada donde se rompe: si alguien escribiera la lista a
/// mano dentro del emisor, este test seguiria pasando y la promesa estaria
/// rota. Por eso lo que se comprueba es que `invoca` esta Y que un nombre
/// cualquiera no.
#[test]
fn los_nombres_de_la_puerta_salen_de_la_tabla() {
    let t = Taller::nuevo();
    assert!(t.abre_la_puerta("invoca"));
    assert!(t.abre_la_puerta("espera_a"));
    assert!(!t.abre_la_puerta("suma"));
    assert!(!t.abre_la_puerta("lee_reloj"), "eso es metal, no la puerta");
}

/// Y el `.bex` de un programa entero pasa el gate.
#[test]
fn el_bex_de_un_programa_con_arranque_pasa_el_gate() {
    let e = emitido("perfil llano\n\nfuncion principal devuelve entero32\n    devuelve 0\n");
    assert!(e.arranca);
    let bytes = empaquetar(&e).expect("el gate lo rechazo");
    assert!(!bytes.is_empty());
}

/// ** CUANTO PESA EL ARRANQUE. El numero, no la impresion.
///
/// La seccion 13c del maestro dice que el punto 1 de los cinco runtimes son
/// *"unas decenas de bytes"* y lo marca como estimacion, porque cuando se
/// escribio no existia. Ya existe, asi que aqui esta medido -- y este test es
/// el que se entera el dia que alguien lo engorde sin darse cuenta.
///
/// ```text
///    call principal          5     y quien lo llama es esto, no una biblioteca
///    mov  <arg2>, <retorno>  3     el codigo de salida, antes de tocar nada
///    mov  <arg0>, imm64      10    sobre quien: la propia tarea
///    mov  <arg1>, imm32      5     que: salir
///    mov  <numero>, imm32    5     por que puerta: la unica que hay
///    syscall                 2
///    jmp  -2                 2     si la puerta devuelve, no se sigue
/// ```
///
/// Comparalo con lo que trae Go dentro de cada binario --~1,5 MB-- y la
/// diferencia no es que aqui se escriba mejor: es que los puntos 4 y 5 **no
/// estan**, y esos son los que pesan.
#[test]
fn el_arranque_cabe_en_treinta_y_dos_bytes() {
    let e = emitido("perfil llano\n\nfuncion principal devuelve entero32\n    devuelve 0\n");
    let arranque = e.inicios.first().map(|(_, off)| *off).expect("sin funciones");
    assert_eq!(arranque, 32, "el arranque de INTI, en bytes");
}


// ===================================================================
//  ** F4b -- LA MEMORIA. La puerta se abrio y al otro lado no habia manos.
// ===================================================================
//
//  F4a dejo a INTI pidiendole un bloque al kernel y **sin poder tocarlo**: no
//  habia forma de leer ni escribir una direccion. Un lenguaje de sistema al que
//  le falta eso no es un lenguaje de sistema, es una calculadora con syscalls.

/// Escribir y volver a leer. Lo minimo, y lo que no estaba.
#[test]
fn una_direccion_se_escribe_y_se_lee() {
    let f = "\
perfil llano
usa memoria

funcion principal devuelve entero32
    crudo
        escribe_natural64(0x200000, 12345)
        devuelve lee_natural64(0x200000)
";
    assert_eq!(arranca(f).syscalls.last().unwrap().arg0, 12345);
}

/// ** Un byte se lee ENTERO y sin basura detras.
///
/// Se lee con `movzx` y no con un `mov` de 8 bits, y la diferencia importa: el
/// `mov` dejaria intactos los 56 bits de arriba, asi que el resultado traeria
/// lo que hubiera antes en el registro. **Y funcionaria casi siempre** -- solo
/// fallaria cuando el registro viniera sucio, que es cuando ya nadie mira.
///
/// Por eso el test ensucia el registro a proposito antes de leer: escribe un
/// numero grande, lo lee, y luego lee un byte.
#[test]
fn un_byte_se_lee_entero_y_sin_arrastrar_lo_de_antes() {
    let f = "\
perfil llano
usa memoria

funcion principal devuelve entero32
    crudo
        escribe_natural64(0x200000, 0x1122334455667788)
        sucio = lee_natural64(0x200000)
        escribe_natural8(0x300000, 200)
        devuelve lee_natural8(0x300000) + sucio - sucio
";
    assert_eq!(arranca(f).syscalls.last().unwrap().arg0, 200);
}

/// ** LA PRUEBA DE F4b: el programa le PIDE memoria al kernel y la USA.
///
/// El camino entero, y cada paso es uno que no existia hace dos commits:
///
///   1. cruza la puerta para pedir un bloque       (F4a)
///   2. recoge el HANDLE, no el codigo             (el fallo que destapo esto)
///   3. vuelve a cruzar para preguntar por su base
///   4. escribe en esa direccion                   (F4b)
///   5. la lee
///   6. y sale por la puerta con lo que leyo       (F4a)
///
/// ** Y `mi_tarea` es un nombre, no un `-2`. Un programa que escribiera el
/// numero crudo compilaria igual y no se entenderia nunca mas.
#[test]
fn el_programa_pide_memoria_al_kernel_y_la_usa() {
    let f = "\
perfil llano
usa bmo
usa memoria

funcion principal devuelve entero32
    crudo
        bloque = invoca_valor(mi_tarea, 0x15, 4096, 0, 0)
        base = invoca_valor(bloque, 0x01, 0, 0, 0)
        escribe_natural64(base, 4321)
        devuelve lee_natural64(base)
";
    let m = arranca(f);
    assert_eq!(
        m.syscalls.last().unwrap().arg0,
        4321,
        "lo que se escribio en la memoria del kernel es lo que se leyo"
    );
    assert_eq!(m.memoria_entregada(), 4096, "y el kernel entrego lo pedido");
}

/// ** `invoca_valor` recoge el VALOR, no el codigo. Corriendo, no leyendo.
///
/// Este es el fallo que F4a se llevo puesto sin enterarse: la puerta contesta
/// DOS cosas a la vez --el codigo en un registro y el valor en otro-- y el
/// emisor leia el mismo para los dos.
///
/// El sintoma habria sido perfecto para no encontrarlo nunca: `invoca_valor`
/// devolvia el codigo, que en el caso bueno vale CERO. O sea que todo puntero
/// pedido al kernel habria valido cero, que es exactamente lo que devuelve un
/// kernel que dice que no.
#[test]
fn invoca_valor_recoge_el_valor_y_no_el_codigo() {
    let comun = "\
perfil llano
usa bmo

funcion principal devuelve entero32
    devuelve ";

    // El codigo de una peticion que sale bien es 0.
    let codigo = format!("{}invoca(mi_tarea, 0x15, 4096, 0, 0)\n", comun);
    assert_eq!(arranca(&codigo).syscalls.last().unwrap().arg0, 0);

    // El valor es un handle, y un handle no es cero.
    let valor = format!("{}invoca_valor(mi_tarea, 0x15, 4096, 0, 0)\n", comun);
    let h = arranca(&valor).syscalls.last().unwrap().arg0;
    assert_ne!(h, 0, "un handle de memoria no puede ser cero");
}

/// Y los dos leen de registros distintos, dicho donde se decide.
#[test]
fn la_puerta_tiene_dos_registros_de_respuesta() {
    let t = Taller::nuevo();
    assert_ne!(
        t.puerta.codigo, t.puerta.valor,
        "el codigo y el valor no pueden volver por el mismo sitio"
    );
    assert_eq!(t.puerta.recogida(Some("valor")), t.puerta.valor);
    assert_eq!(t.puerta.recogida(Some("codigo")), t.puerta.codigo);
    // Lo desconocido se trata como codigo: es lo unico seguro.
    assert_eq!(t.puerta.recogida(None), t.puerta.codigo);
}

// ===================================================================
//  ** F4c -- EL MONTON, y en piezas
// ===================================================================
//
//  Peticion de Eddi: *"si MONTON es monolitico = modular, para poder evitar
//  problemas o choques. INTI como siempre modular"*.
//
//  Y la primera consecuencia de tomarselo en serio fue **descubrir que yo me
//  habia equivocado**: dije que el monton estaba bloqueado por las variables de
//  modulo. Lo esta un monton MONOLITICO, el de C, que guarda su estado en una
//  global escondida.
//
//  Uno modular no lo necesita: **el estado del monton vive DENTRO del monton**.
//
//      monton + 0   libre   la primera direccion sin repartir
//      monton + 8   fin     la primera que ya no es suya
//      monton + 16  ...     desde aqui se reparte
//
//  ** Y eso no es un apano para esquivar una funcionalidad que falta: es mejor.
//  Un `malloc` con estado global es autoridad ambiente -- cualquiera reparte de
//  lo mismo sin haberlo pedido. `pide(monton, n)` tiene la forma de una
//  capability: **para repartir de un monton hay que tenerlo**.
//
//  Las piezas, y la unica frontera entre ellas es la tabla de arriba:
//
//      origen.inti    habla con el kernel   y NO sabe repartir
//      reparto.inti   sabe repartir         y NO habla con el kernel

const CON_MONTON: &str = "\
perfil llano
usa monton

funcion principal devuelve entero32
";

/// ** LA PRUEBA DE F4c: el monton se pide, se reparte, y las cuentas salen.
#[test]
fn el_monton_reparte_y_los_trozos_no_se_pisan() {
    let f = format!(
        "{}{}{}{}",
        CON_MONTON,
        "    m = monton_nuevo(4096)
",
        "    a = pide(m, 8)
    b = pide(m, 8)
",
        "    devuelve b - a
"
    );
    // Ocho bytes pedidos, dieciseis de distancia: alineado, y sin solaparse.
    assert_eq!(arranca(&f).syscalls.last().unwrap().arg0, 16);
}

/// Lo repartido se puede USAR. Que es de lo que iba todo esto.
#[test]
fn en_lo_que_reparte_el_monton_se_puede_escribir() {
    let f = format!(
        "{}{}{}{}{}",
        CON_MONTON,
        "    m = monton_nuevo(4096)
",
        "    a = pide(m, 8)
    b = pide(m, 8)
",
        "    crudo
        escribe_natural64(a, 111)
",
        "        escribe_natural64(b, 222)
        devuelve lee_natural64(a) + lee_natural64(b)
"
    );
    // Si `a` y `b` se solaparan, esto daria 444.
    assert_eq!(arranca(&f).syscalls.last().unwrap().arg0, 333);
}

/// ** Un monton que se acaba dice que NO, y no reparte lo que no tiene.
///
/// Es la mitad que se olvida de todo asignador, y la que convierte un fallo de
/// memoria en una corrupcion silenciosa cuando falta: sin esta comprobacion,
/// `pide` devolveria una direccion **fuera del bloque** y el programa
/// escribiria en la memoria de otro.
#[test]
fn un_monton_lleno_contesta_cero() {
    let f = format!(
        "{}    m = monton_nuevo(4096)\n    devuelve pide(m, 100000)\n",
        CON_MONTON
    );
    assert_eq!(arranca(&f).syscalls.last().unwrap().arg0, 0);
}

/// Y lo que queda baja segun se reparte, que es como se comprueba que reparte
/// de verdad en vez de devolver direcciones sueltas.
#[test]
fn lo_que_queda_baja_segun_se_reparte() {
    let antes = format!("{}    m = monton_nuevo(4096)\n    devuelve queda_en(m)\n", CON_MONTON);
    let despues = format!(
        "{}    m = monton_nuevo(4096)\n    a = pide(m, 8)\n    devuelve queda_en(m)\n",
        CON_MONTON
    );
    let a = arranca(&antes).syscalls.last().unwrap().arg0;
    let d = arranca(&despues).syscalls.last().unwrap().arg0;
    assert_eq!(a, 4096 - 16, "la cabecera del monton ocupa 16");
    assert_eq!(d, a - 16, "y un trozo de 8 se lleva 16 por la alineacion");
}

/// El monton pide su memoria al KERNEL, no a una zona inventada.
#[test]
fn el_monton_sale_de_la_puerta() {
    let f = format!("{}    m = monton_nuevo(4096)\n    devuelve 0\n", CON_MONTON);
    let m = arranca(&f);
    assert_eq!(m.memoria_entregada(), 4096);
}

/// ** Y las piezas siguen siendo piezas: `usa monton` trae DOS ficheros, y el
/// orden en que llegan no lo elige el sistema de ficheros.
///
/// Sin el orden fijo, dos compilaciones del mismo fuente darian dos binarios
/// distintos -- y entonces "este .bex es el que audite" deja de poder decirse.
#[test]
fn el_monton_llega_en_piezas_y_en_orden() {
    let piezas = bmo_inti_front::tablas::Runtime::traer(&bmo_mods::Roots::find(), "monton");
    let nombres: Vec<&str> = piezas.iter().map(|(n, _)| n.as_str()).collect();
    assert_eq!(nombres, vec!["origen.inti", "reparto.inti"]);
}

/// Un `usa` que no es una pieza no trae nada, y eso no es un error.
#[test]
fn un_usa_que_no_es_una_pieza_no_trae_nada() {
    let r = bmo_mods::Roots::find();
    assert!(bmo_inti_front::tablas::Runtime::traer(&r, "x86_64").is_empty());
    assert!(bmo_inti_front::tablas::Runtime::traer(&r, "bmo").is_empty());
    // Y un nombre que intente salirse del sitio no busca en ningun lado.
    assert!(bmo_inti_front::tablas::Runtime::traer(&r, "../monton").is_empty());
}

// ===================================================================
//  ** F5a -- LOS CUATRO ANCHOS, y el primer framebuffer
// ===================================================================
//
//  Faltaban el de 16 y el de 32. Estaba declarado en la tabla con su motivo
//  --`bmo_lower` no traia los ayudantes-- y se anadieron ALLI, que es donde
//  tenian que estar.
//
//  El de 32 no es uno mas: **es el que escribe un pixel**.

/// Cada ancho guarda y devuelve lo suyo, ni un bit mas.
#[test]
fn los_cuatro_anchos_van_y_vuelven() {
    for (bits, valor) in [
        (8u32, 200u64),
        (16, 60000),
        (32, 4000000000),
        (64, 12345678901234),
    ] {
        let f = format!(
            "perfil llano
usa memoria

funcion principal devuelve entero32
{}{}{}",
            "    crudo
",
            format!("        escribe_natural{}(0x200000, {})
", bits, valor),
            format!("        devuelve lee_natural{}(0x200000)
", bits)
        );
        assert_eq!(
            arranca(&f).syscalls.last().unwrap().arg0,
            valor,
            "ancho de {} bits",
            bits
        );
    }
}

/// ** Y lo de al lado NO se toca. Es la mitad que se olvida de un `escribe`.
///
/// Un `escribe_natural8` que en realidad escribiera cuatro bytes pasaria el
/// test de arriba tan campante -- lee lo mismo que escribio-- y **se llevaria
/// por delante los tres bytes siguientes**. En un array eso es el elemento de
/// al lado, y el fallo aparece en otra parte del programa.
#[test]
fn escribir_un_ancho_no_pisa_lo_de_al_lado() {
    let f = "\
perfil llano
usa memoria

funcion principal devuelve entero32
    crudo
        escribe_natural64(0x200000, 0xFFFFFFFFFFFFFFFF)
        escribe_natural8(0x200000, 0)
        devuelve lee_natural64(0x200000)
";
    // Solo el byte bajo a cero: quedan siete bytes de unos.
    assert_eq!(
        arranca(f).syscalls.last().unwrap().arg0,
        0xFFFF_FFFF_FFFF_FF00
    );
}

#[test]
fn escribir_dos_bytes_no_pisa_los_otros_seis() {
    let f = "\
perfil llano
usa memoria

funcion principal devuelve entero32
    crudo
        escribe_natural64(0x200000, 0xFFFFFFFFFFFFFFFF)
        escribe_natural16(0x200000, 0)
        devuelve lee_natural64(0x200000)
";
    assert_eq!(
        arranca(f).syscalls.last().unwrap().arg0,
        0xFFFF_FFFF_FFFF_0000
    );
}

/// ** EL PRIMER FRAMEBUFFER DE INTI.
///
/// Pide memoria al kernel, la reparte con su propio monton, y **rellena
/// pixeles de 32 bits en un bucle**. Que es, quitando el nombre bonito, lo que
/// hace un motor grafico en su linea mas caliente.
///
/// Aqui se juntan las cuatro piezas de hoy y ninguna sobra:
///
///   F4a  arranca solo y sale por la puerta
///   F4b  toca memoria
///   F4c  el monton se la reparte
///   F5a  y el ancho de 32 es el que cabe un pixel
#[test]
fn inti_rellena_una_pantalla_de_pixeles() {
    let f = "\
perfil llano
usa monton
usa memoria

funcion pinta(pantalla es natural64, cuantos es natural64, color es natural64)
    crudo
        cambiante i = 0
        repite mientras i < cuantos
            escribe_natural32(pantalla + i * 4, color)
            i = i + 1

funcion principal devuelve entero32
    m = monton_nuevo(4096)
    p = pide(m, 64)
    pinta(p, 16, 65280)
    crudo
        devuelve lee_natural32(p + 40)
";
    // El pixel 10 de 16, y ninguno se escribio dos veces ni se quedo sin
    // escribir: si el bucle contara mal, este seria cero.
    assert_eq!(arranca(f).syscalls.last().unwrap().arg0, 65280);
}

/// Y el ultimo pixel se escribe, que es donde se ve si el bucle se queda corto.
#[test]
fn el_ultimo_pixel_tambien_se_pinta() {
    let f = "\
perfil llano
usa monton
usa memoria

funcion pinta(pantalla es natural64, cuantos es natural64, color es natural64)
    crudo
        cambiante i = 0
        repite mientras i < cuantos
            escribe_natural32(pantalla + i * 4, color)
            i = i + 1

funcion principal devuelve entero32
    m = monton_nuevo(4096)
    p = pide(m, 64)
    pinta(p, 16, 7)
    crudo
        devuelve lee_natural32(p + 60)
";
    assert_eq!(arranca(f).syscalls.last().unwrap().arg0, 7);
}

// ===================================================================
//  ** F5b -- CAMPOS Y BUFERES. Los dos agujeros que quedaban.
// ===================================================================
//
//  Hasta hoy `p.x` se bajaba a `p` --el campo se IGNORABA, sin una queja-- y
//  `a[i]` bajaba a la DIRECCION del elemento en vez de a su valor. Las dos
//  compilaban, corrian, y hacian otra cosa.
//
//  Las dos eran el mismo agujero: **INTI no sabia cuanto mide nada**. Un campo
//  es una direccion mas un desplazamiento, y el desplazamiento sale de las
//  medidas de los campos de antes.
//
//  ** Y las dos se arreglan con la MISMA cuenta, que es la senal de que el
//  arreglo es el correcto: `p.x`, `p.x = 3`, `a[i]` y `a[i] = 3` calculan
//  exactamente lo mismo y solo cambian la instruccion del final.

const CON_PUNTO: &str = "\
perfil llano
usa monton
usa memoria

registro Punto
    x es entero64
    y es entero64

";

/// ** Un campo se escribe y se lee, y cae donde dice el plano.
#[test]
fn un_campo_de_registro_se_escribe_y_se_lee() {
    let f = format!(
        "{}{}{}{}{}",
        CON_PUNTO,
        "funcion principal devuelve entero32\n",
        "    m = monton_nuevo(4096)\n",
        "    p es Punto = pide(m, 16)\n",
        "    p.x = 11\n    p.y = 31\n    devuelve p.x + p.y\n"
    );
    assert_eq!(arranca(&f).syscalls.last().unwrap().arg0, 42);
}

/// ** Y los dos campos NO son el mismo sitio.
///
/// Es la prueba que echa abajo el comportamiento viejo: cuando `p.x` se bajaba
/// a `p`, los dos campos eran la misma direccion, `p.x = 11` seguido de
/// `p.y = 31` dejaba 31 en las dos, y la suma daba 62. Compilaba igual.
#[test]
fn dos_campos_no_son_el_mismo_sitio() {
    let f = format!(
        "{}{}{}{}{}",
        CON_PUNTO,
        "funcion principal devuelve entero32\n",
        "    m = monton_nuevo(4096)\n",
        "    p es Punto = pide(m, 16)\n",
        "    p.x = 5\n    p.y = 9\n    devuelve p.x\n"
    );
    assert_eq!(arranca(&f).syscalls.last().unwrap().arg0, 5, "no 9");
}

/// El desplazamiento del segundo campo es 8, y se ve desde fuera del registro.
///
/// Se escribe por el campo y se lee a mano por la direccion cruda. Si el plano
/// mintiera, estos dos numeros no coincidirian.
#[test]
fn el_campo_esta_donde_el_plano_dice() {
    let f = format!(
        "{}{}{}{}{}{}",
        CON_PUNTO,
        "funcion principal devuelve entero32\n",
        "    m = monton_nuevo(4096)\n",
        "    p es Punto = pide(m, 16)\n",
        "    p.y = 77\n",
        "    crudo\n        devuelve lee_natural64(p + 8)\n"
    );
    assert_eq!(arranca(&f).syscalls.last().unwrap().arg0, 77);
}

/// ** Dos registros seguidos no se pisan: el segundo empieza en la medida del
/// primero, y por eso la medida se redondea a la alineacion.
#[test]
fn dos_registros_seguidos_no_se_pisan() {
    let f = format!(
        "{}{}{}{}{}{}",
        CON_PUNTO,
        "funcion principal devuelve entero32\n",
        "    m = monton_nuevo(4096)\n",
        "    a es Punto = pide(m, 16)\n",
        "    b es Punto = pide(m, 16)\n",
        "    a.x = 100\n    b.x = 1\n    devuelve a.x + b.x\n"
    );
    // Si se solaparan, `b.x = 1` habria pisado `a.x` y esto daria 2.
    assert_eq!(arranca(&f).syscalls.last().unwrap().arg0, 101);
}

// ===================================================================
//  ** `bufer de T` -- indexar de verdad
// ===================================================================

/// El indice multiplica por la MEDIDA DEL ELEMENTO, no por uno.
///
/// Con el comportamiento viejo, `a[2]` daba `a + 2`. Ahora da `a + 8` para un
/// bufer de `entero64`, y lo que devuelve es el VALOR.
#[test]
fn un_bufer_se_indexa_por_la_medida_de_su_elemento() {
    let f = "\
perfil llano
usa monton
usa memoria

funcion principal devuelve entero32
    m = monton_nuevo(4096)
    a es bufer de entero64 = pide(m, 64)
    crudo
        a[0] = 10
        a[1] = 20
        a[2] = 30
        devuelve a[2] - a[0]
";
    assert_eq!(arranca(f).syscalls.last().unwrap().arg0, 20);
}

/// Y los elementos no se pisan, que es lo que pasaria con la medida mal.
#[test]
fn los_elementos_de_un_bufer_no_se_pisan() {
    let f = "\
perfil llano
usa monton
usa memoria

funcion principal devuelve entero32
    m = monton_nuevo(4096)
    a es bufer de entero64 = pide(m, 64)
    crudo
        a[0] = 1
        a[1] = 2
        a[2] = 4
        devuelve a[0] + a[1] + a[2]
";
    assert_eq!(arranca(f).syscalls.last().unwrap().arg0, 7);
}

/// ** EL FRAMEBUFFER, otra vez -- pero escrito como se escribe de verdad.
///
/// Comparalo con `inti_rellena_una_pantalla_de_pixeles`, que es el mismo
/// programa de hace un rato:
///
/// ```text
///    antes   escribe_natural32(pantalla + i * 4, color)
///    ahora   pantalla[i] = color
/// ```
///
/// El `* 4` desaparece del fuente porque **lo sabe el tipo**. Y no es solo mas
/// corto: el `4` escrito a mano es un numero que hay que cambiar en todos los
/// sitios el dia que los pixeles sean de 16 bits, y el que no se cambie
/// compilara igual.
#[test]
fn un_framebuffer_escrito_con_un_bufer() {
    let f = "\
perfil llano
usa monton
usa memoria

funcion pinta(pantalla es bufer de natural32, cuantos es entero64, color es entero64)
    crudo
        cambiante i = 0
        repite mientras i < cuantos
            pantalla[i] = color
            i = i + 1

funcion principal devuelve entero32
    m = monton_nuevo(4096)
    p es bufer de natural32 = pide(m, 64)
    pinta(p, 16, 65280)
    crudo
        devuelve p[10]
";
    assert_eq!(arranca(f).syscalls.last().unwrap().arg0, 65280);
}

/// Y con pixeles de 32 bits, el ultimo de 16 esta en el byte 60 -- lo que
/// confirma que el paso fue de cuatro y no de ocho.
#[test]
fn el_paso_del_bufer_es_el_del_elemento_y_no_una_palabra() {
    let f = "\
perfil llano
usa monton
usa memoria

funcion principal devuelve entero32
    m = monton_nuevo(4096)
    p es bufer de natural32 = pide(m, 64)
    crudo
        p[15] = 123
        devuelve lee_natural32(p + 60)
";
    assert_eq!(arranca(f).syscalls.last().unwrap().arg0, 123);
}

// ===================================================================
//  ** F5c -- LA COMA FLOTANTE. El cuarto tipo de numero que se puede tocar.
// ===================================================================
//
//  Hasta hoy INTI sabia contar y no sabia medir. Un `natural32` cabe un pixel,
//  pero no cabe una posicion, ni un angulo, ni una escala -- y por eso F5a
//  llegaba a rellenar un framebuffer de un color y no a mover nada dentro.
//
//  ** Y el modelo esta escrito en `flotante()`: los valores viven en registros
//  normales como PATRON DE BITS y solo cruzan para la operacion. Estas pruebas
//  no lo saben ni les importa; miran numeros. Ese es el punto de mirarlos.

/// Los bits de un `f64`, que es lo que devuelve la funcion, leidos como numero.
fn como_numero(u: u64) -> f64 {
    f64::from_bits(u)
}

const SUMA_FLOTANTE: &str = "\
perfil llano

funcion f devuelve flotante64
    devuelve 2.5 + 1.25
";

#[test]
fn una_suma_de_coma_flotante_corre_y_da_el_numero() {
    let r = como_numero(ejecuta(SUMA_FLOTANTE, 0, 0));
    assert_eq!(r, 3.75, "salio {}", r);
}

/// Las cuatro, y una de ellas es la que no se puede hacer con enteros: `/`.
///
/// ** `5 / 2` da `2.5` y no `2`. Es la sorpresa 10 de Python contestada al
/// reves: en INTI el simbolo divide de verdad y el cociente entero tiene su
/// propia palabra (`entre`). Aqui se ve que no es una promesa de la gramatica.
#[test]
fn las_cuatro_operaciones() {
    let de = |e: &str| {
        como_numero(ejecuta(
            &format!(
                "perfil llano\n\nfuncion f devuelve flotante64\n    devuelve {}\n",
                e
            ),
            0,
            0,
        ))
    };
    assert_eq!(de("2.5 + 1.25"), 3.75);
    assert_eq!(de("2.5 - 1.25"), 1.25);
    assert_eq!(de("2.5 * 4.0"), 10.0);
    assert_eq!(de("5.0 / 2.0"), 2.5);
}

/// ** DIVIDIR ENTRE CERO NO ATRAPA, y es la prueba de que la Regla 3 esta bien
/// entendida.
///
/// La Regla 3 existe porque en los ENTEROS `1 / 0` no tiene respuesta: cualquier
/// bit que salga se lo invento el compilador. En IEEE-754 la tiene --infinito--
/// y esta escrita desde 1985. Atrapar aqui no anadiria seguridad: quitaria la
/// aritmetica.
#[test]
fn entre_cero_da_infinito_y_no_atrapa() {
    let f = "perfil llano\n\nfuncion f devuelve flotante64\n    devuelve 1.0 / 0.0\n";
    assert_eq!(como_numero(ejecuta(f, 0, 0)), f64::INFINITY);

    // Y la comprobacion no esta ni en la IR.
    //
    // ** Se cuenta AQUI y no en los bytes emitidos, y la diferencia importa:
    // el emisor todavia no materializa la de division --lo dice el mismo, con
    // su motivo, en `Instr::Comprueba`--, asi que contando bytes esta prueba
    // saldria verde igual si la regla estuviera puesta. La regla vive en la
    // IR; es ahi donde hay que preguntar si esta.
    assert_eq!(reglas_de(f), 0, "un flotante no lleva comprobacion detras");
}

/// ** Y EL CONTRASTE, que es lo que hace valer la prueba de arriba: la misma
/// division con enteros SI trae su comprobacion.
#[test]
fn la_misma_division_con_enteros_si_trae_su_regla() {
    let f = "perfil llano\n\nfuncion f(a es entero64, b es entero64) devuelve entero64\n    devuelve a / b\n";
    assert_eq!(reglas_de(f), 1, "la Regla 3 desaparecio de los enteros");
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

// -------------------------------------------------------------------
//  ** LAS COMPARACIONES, Y EL NaN
// -------------------------------------------------------------------

fn compara(e: &str) -> u64 {
    ejecuta(
        &format!(
            "perfil llano\n\nfuncion f devuelve logico\n    devuelve {}\n",
            e
        ),
        0,
        0,
    )
}

#[test]
fn las_seis_comparaciones() {
    assert_eq!(compara("1.5 < 2.5"), 1);
    assert_eq!(compara("2.5 < 1.5"), 0);
    assert_eq!(compara("2.5 > 1.5"), 1);
    assert_eq!(compara("1.5 > 2.5"), 0);
    assert_eq!(compara("1.5 <= 1.5"), 1);
    assert_eq!(compara("1.5 >= 2.5"), 0);
    assert_eq!(compara("1.5 = 1.5"), 1);
    assert_eq!(compara("1.5 no es 2.5"), 1);
}

/// ** ESTA ES LA PRUEBA QUE DECIDE SI LA COMA FLOTANTE ESTA BIEN HECHA.
///
/// Un NaN --lo que sale de `0.0 / 0.0`-- no es mayor, ni menor, ni igual a
/// nada. Y el silicio no lo regala: la comparacion enciende la bandera de
/// "iguales" A LA VEZ que la de "no comparables", asi que una igualdad escrita
/// de la forma obvia contesta **que si**.
///
/// Las cinco primeras tienen que salir falsas. Y la sexta, cierta -- porque
/// `x no es x` es exactamente como se pregunta si algo es NaN, y tiene que
/// poder contestarse.
#[test]
fn un_nan_pierde_las_cinco_comparaciones_y_gana_la_sexta() {
    assert_eq!(compara("0.0 / 0.0 < 1.0"), 0, "un NaN no es menor");
    assert_eq!(compara("0.0 / 0.0 > 1.0"), 0, "ni mayor");
    assert_eq!(compara("0.0 / 0.0 <= 1.0"), 0);
    assert_eq!(compara("0.0 / 0.0 >= 1.0"), 0);
    assert_eq!(compara("0.0 / 0.0 = 1.0"), 0, "ni igual");
    assert_eq!(
        compara("0.0 / 0.0 no es 1.0"),
        1,
        "y la desigualdad es la unica que un NaN hace CIERTA"
    );
}

/// El NaN contra si mismo, que es el caso que enganaria a la version ingenua.
#[test]
fn un_nan_no_es_igual_ni_a_si_mismo() {
    assert_eq!(compara("0.0 / 0.0 = 0.0 / 0.0"), 0);
    assert_eq!(compara("0.0 / 0.0 no es 0.0 / 0.0"), 1);
}

// -------------------------------------------------------------------
//  ** LA CONVERSION, que es la unica vez que los bits CAMBIAN
// -------------------------------------------------------------------

/// `flotante64(5)` da 5.0, no los bits de 5 mirados del reves.
///
/// ** Confundir las dos cosas da `2,47e-323` donde tiene que haber un `5.0`, y
/// no rompe nada: sigue siendo un flotante valido. Por eso hay una prueba.
#[test]
fn un_entero_se_convierte_de_verdad_y_no_se_reinterpreta() {
    let f = "perfil llano\n\nfuncion f(a es entero64, b es entero64) devuelve flotante64\n    devuelve flotante64(a)\n";
    assert_eq!(como_numero(ejecuta(f, 5, 0)), 5.0);
    assert_eq!(como_numero(ejecuta(f, 0, 0)), 0.0);
}

/// Con signo, que es la otra mitad: `-7` tiene que dar `-7.0` y no 1,8e19.
#[test]
fn la_conversion_es_con_signo() {
    let f = "perfil llano\n\nfuncion f(a es entero64, b es entero64) devuelve flotante64\n    devuelve flotante64(a)\n";
    assert_eq!(como_numero(ejecuta(f, (-7i64) as u64, 0)), -7.0);
}

/// Y de vuelta, TRUNCANDO. 2,9 da 2 y -2,9 da -2.
#[test]
fn de_flotante_a_entero_se_trunca_hacia_el_cero() {
    let f = "perfil llano\n\nfuncion f devuelve entero64\n    devuelve entero64(2.9)\n";
    assert_eq!(ejecuta(f, 0, 0), 2);
    let g = "perfil llano\n\nfuncion f devuelve entero64\n    devuelve entero64(0.0 - 2.9)\n";
    assert_eq!(ejecuta(g, 0, 0) as i64, -2, "hacia el cero, no hacia abajo");
}

/// Ida y vuelta por una variable declarada: el tipo escrito es lo que decide,
/// no el literal.
#[test]
fn el_tipo_declarado_manda_sobre_la_operacion() {
    let f = "\
perfil llano

funcion f(a es entero64, b es entero64) devuelve flotante64
    x es flotante64 = flotante64(a)
    devuelve x / 2.0
";
    assert_eq!(
        como_numero(ejecuta(f, 7, 0)),
        3.5,
        "si fuera entera, saldria 3"
    );
}

// -------------------------------------------------------------------
//  ** LA REGLA 11, que se comprueba en lo que NO se emite
// -------------------------------------------------------------------

/// **La Regla 11 no se puede probar mirando un resultado**: `a * b + c` da el
/// mismo numero con la operacion fundida y sin ella casi siempre. La diferencia
/// esta en el redondeo de en medio, y solo aparece en unos pocos valores de
/// cada millon.
///
/// Asi que se prueba mirando los BYTES: si no hay una instruccion de
/// multiplicar-y-sumar emitida, no hay forma de que el redondeo se salte.
///
/// ** Y esto es la portabilidad que C no da. Un compilador de C con las
/// banderas de siempre PUEDE fundir esas dos operaciones, y entonces el mismo
/// fuente da bits distintos en dos maquinas. INTI lo prohibe y paga el precio
/// en velocidad, porque el argumento de venta de este sistema es que se puede
/// verificar -- y no se verifica lo que no da el mismo resultado dos veces.
#[test]
fn la_regla_11_no_funde_la_multiplicacion_con_la_suma() {
    let f = "perfil llano\n\nfuncion f devuelve flotante64\n    devuelve 2.0 * 3.0 + 1.0\n";
    let e = emitido(f);
    // Las instrucciones de multiplicar-y-sumar viven todas detras de dos
    // prefijos concretos. Que no aparezca ninguno es la prueba.
    let fundida = e.codigo.iter().any(|b| *b == 0xC4 || *b == 0x62);
    assert!(!fundida, "se emitio una instruccion de multiplicar-y-sumar");
    // Y da el numero correcto, que sin esto seria una prueba que aprueba un
    // programa que no calcula nada.
    assert_eq!(como_numero(ejecuta(f, 0, 0)), 7.0);
}

/// El mismo fuente, los mismos bytes. Dos veces.
///
/// ** Parece tonto y no lo es: es la mitad comprobable de *"el mismo programa da
/// el mismo bit"*. Si el emisor tuviera cualquier cosa que dependiera del
/// entorno --el orden de un mapa, una direccion, la hora-- se veria aqui.
#[test]
fn el_mismo_fuente_emite_los_mismos_bytes() {
    let a = emitido(SUMA_FLOTANTE);
    let b = emitido(SUMA_FLOTANTE);
    assert_eq!(a.codigo, b.codigo);
}


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


// ===================================================================
//  ** LAS TRES FRASES CON LAS QUE SE DEFINE INTI, hechas assert
// ===================================================================
//
//  Eddi lo dice asi: *"INTI es inspiracion de Python en sintaxis, pero nivel de
//  rendimiento de ASM, y fuera del syscall"*.
//
//  Son tres afirmaciones distintas y **dos de ellas se pueden comprobar aqui
//  mismo**. La tercera --la sintaxis-- no se mide con un test: se mide leyendo,
//  y para eso esta el censo.
//
//  ** Por que estan juntas en un bloque en vez de repartidas: porque la frase se
//  usa entera para describir el lenguaje, y una frase que se usa entera tiene
//  que poder fallar entera. Si algun dia una de estas deja de ser verdad, lo
//  honesto es dejar de decirla.

/// Un programa que CALCULA no cruza la puerta ni una vez.
///
/// ## ** Que se esta comprobando de verdad
///
/// Que la aritmetica de INTI no pasa por ningun runtime que a su vez hable con
/// el kernel. En Python `2 + 2` recorre el despacho de objetos; aqui son dos
/// instrucciones y la puerta ni aparece en los bytes.
///
/// Y no es una perogrullada: el maestro tiene un numero para esto --**969
/// ciclos** cuesta cruzar la puerta contra **20** una llamada-- y toda la
/// arquitectura del lenguaje se decidio con el delante. Este test es lo que
/// impide que esa decision se erosione sin que nadie lo note.
#[test]
fn un_programa_que_calcula_no_cruza_la_puerta() {
    let f = "\
perfil llano

funcion media(a es entero64, b es entero64) devuelve entero64
    cambiante t = 0
    cambiante i = 0
    repite mientras i < 10
        t = t + a * i + b
        i = i + 1
    devuelve t entre 10
";
    let e = emitido(f);
    // `0F 05` es la puerta. En un programa que solo cuenta, no puede estar.
    assert!(
        !e.codigo.windows(2).any(|w| w == [0x0F, 0x05]),
        "un programa que solo calcula esta cruzando la puerta"
    );
    // Y ademas corre y da el numero: sin esto seria un test que aprueba un
    // binario vacio, que efectivamente no cruza ninguna puerta.
    assert_eq!(ejecuta(f, 2, 3), (0..10).map(|i| 2 * i + 3).sum::<u64>() / 10);
}

/// **EL BUCLE MAS CALIENTE QUE INTI SABE ESCRIBIR HOY, sin una sola llamada.**
///
/// ## ** Esta es la frase de "nivel de ASM", y aqui esta lo que significa
///
/// No significa *"va tan rapido como el ensamblador que escribiria un experto"*
/// -- eso es medible y todavia no esta medido. Significa algo mas estrecho y
/// que si se puede comprobar: **entre el fuente y la instruccion no hay nadie**.
/// Ni despacho, ni contador de referencias, ni una llamada por elemento.
///
/// Ese es exactamente el techo que Python no puede levantar, y no por lentitud
/// del interprete: `x + y` alli **es** una llamada, y lo seguiria siendo
/// compilado. Aqui el bucle entero son saltos y aritmetica.
#[test]
fn el_bucle_de_pixeles_no_llama_a_nadie() {
    let f = "\
perfil llano
usa memoria

funcion pinta(pantalla es bufer de natural32, cuantos es entero64, color es entero64)
    cambiante i = 0
    repite mientras i < cuantos
        crudo
            pantalla[i] = color
        i = i + 1
";
    let e = emitido(f);
    assert!(
        !e.codigo.windows(2).any(|w| w == [0x0F, 0x05]),
        "el bucle cruza la puerta"
    );
    // ** Y ninguna LLAMADA, que es la mitad que de verdad importa: un
    // rasterizador que llama una vez por pixel tiene un techo que ninguna
    // optimizacion posterior levanta.
    //
    // Se cuenta en la IR y NO buscando el byte de la instruccion. El primer
    // intento buscaba `E8` suelto en el codigo y fallaba: ese byte aparece
    // dentro de cualquier inmediato o desplazamiento que lo lleve. Un test que
    // da falsos positivos se desactiva en una semana, y entonces ya no vigila
    // nada -- es la misma leccion que `agnostico.rs` aprendio con `rsi` dentro
    // de `conversion`.
    assert_eq!(llamadas_de(f), 0, "el bucle llama a alguien por pixel");
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

/// ** Y LA MEDIDA, que es lo que convierte la frase en un numero.
///
/// Cuantas comprobaciones anti-UB lleva ese mismo bucle, y cuantas instrucciones
/// de maquina. Los dos numeros van a CABINA en cada compilacion, asi que **se
/// pueden seguir en el tiempo**: el dia que alguien anada una comprobacion de
/// mas en el sitio equivocado, el numero sube y se ve.
///
/// La seccion 6.3 del maestro dice que comprobar cuesta ~1%. Esto no lo mide
/// --medirlo pide el Ryzen-- pero dice **contra que** se va a medir, que es lo
/// unico que se puede saber hoy sin hardware.
#[test]
fn el_precio_de_no_tener_ub_esta_contado() {
    let f = "\
perfil llano

funcion suma(a es entero64, b es entero64) devuelve entero64
    devuelve a + b * 2
";
    // Dos operaciones que se pueden pasar de la cuenta, dos comprobaciones.
    // Ni una de mas: comparar no puede desbordar y no la lleva.
    assert_eq!(reglas_de(f), 2);

    let g = "perfil llano\n\nfuncion f(a es entero64, b es entero64) devuelve logico\n    devuelve a < b\n";
    assert_eq!(reglas_de(g), 0, "una comparacion no puede salirse");
}

/// La tercera pata: **fuera del syscall no quiere decir sin acceso al sistema**.
///
/// ** Es la distincion que el maestro llama "control no es privilegio", y aqui
/// se ve en bytes: el mismo compilador emite un programa sin puerta (arriba) y
/// uno con puerta (este), y **la diferencia es una linea del fuente** -- `usa
/// bmo` -- no una bandera del compilador ni una palabra clave.
///
/// Quitar esa fila de `modulos.toml` apaga la puerta sin tocar una linea de
/// Rust. Eso es lo que significa que la puerta no sea sintaxis.
#[test]
fn la_puerta_llega_por_una_linea_del_fuente_y_no_por_otra_via() {
    let sin = "perfil llano\n\nfuncion f(a es entero64, b es entero64) devuelve entero64\n    devuelve a + b\n";
    let con = "\
perfil llano
usa bmo

funcion f(a es entero64, b es entero64) devuelve entero64
    devuelve invoca(a, b, 0, 0, 0)
";
    let hay = |src: &str| emitido(src).codigo.windows(2).any(|w| w == [0x0F, 0x05]);
    assert!(!hay(sin), "sin `usa bmo` no puede haber puerta");
    assert!(hay(con), "con `usa bmo` tiene que haberla");
}


// ===================================================================
//  ** F5e -- LAS REGLAS QUE SE CALCULABAN Y NO LLEGABAN A UN BYTE
// ===================================================================
//
//  De las cuatro comprobaciones de la IR, **una sola llegaba al binario**. Las
//  otras tres estaban declaradas, contadas en la IR, documentadas... y el
//  emisor las descontaba y no emitia nada.
//
//  El motivo estaba escrito y era honesto -- *"piden mirar un operando ANTES de
//  la operacion"* -- pero era un diagnostico, no un arreglo. El arreglo era
//  mover la comprobacion al sitio donde sirve, y eso es de la IR, no del
//  emisor: por eso el fallo sobrevivio a que alguien lo entendiera.
//
//  ** Y la que sigue sin salir --la 2-- ahora esta sola y por OTRO motivo: no
//  hay contra que comprobar, porque un `bufer` no lleva su longitud. Esa espera
//  a `lista de T`. Un pendiente con su causa exacta vale mucho mas que tres
//  juntos con una causa que solo explicaba dos.

/// Los codigos con los que atrapa cada regla, tal como salen en el registro de
/// retorno.
const DESBORDE: u64 = 1001;
const ENTRE_CERO: u64 = 1003;
const CONVERSION: u64 = 1012;

// -------------------------------------------------------------------
//  REGLA 3 -- dividir entre cero
// -------------------------------------------------------------------

const DIVIDE: &str = "\
perfil llano

funcion f(a es entero64, b es entero64) devuelve entero64
    devuelve a entre b
";

/// ** LA PRUEBA QUE NO SE PODIA ESCRIBIR HASTA HOY.
///
/// Antes esto no daba 1003: **se llevaba el emulador por delante**, igual que
/// se lleva un procesador de verdad. Dividir entre cero en x86 no da un numero
/// raro -- levanta una excepcion antes de dejar nada.
///
/// Y por eso la comprobacion tenia que ir ANTES: despues de la division no hay
/// programa que mire el resultado.
#[test]
fn dividir_entre_cero_atrapa_con_su_codigo() {
    assert_eq!(ejecuta(DIVIDE, 10, 0), ENTRE_CERO);
}

/// Y dividir de verdad sigue dividiendo. Sin esto, una comprobacion que
/// atrapara SIEMPRE pasaria la prueba de arriba.
#[test]
fn dividir_entre_algo_sigue_dando_el_cociente() {
    assert_eq!(ejecuta(DIVIDE, 10, 2), 5);
    assert_eq!(ejecuta(DIVIDE, 7, 7), 1);
}

/// El resto tambien: es la misma instruccion y el mismo cero.
#[test]
fn el_resto_entre_cero_tambien_atrapa() {
    let f = "perfil llano\n\nfuncion f(a es entero64, b es entero64) devuelve entero64\n    devuelve a resto b\n";
    assert_eq!(ejecuta(f, 10, 0), ENTRE_CERO);
    assert_eq!(ejecuta(f, 10, 3), 1);
}

// -------------------------------------------------------------------
//  ** DOS REGLAS, DOS CODIGOS -- que es lo que el destino unico impedia
// -------------------------------------------------------------------

/// ** Con un solo sitio al que saltar, atrapar por dividir entre cero habria
/// devuelto **1001** -- el codigo de desbordar -- y el programa habria dicho
/// que le paso otra cosa.
///
/// No es un detalle de presentacion: un error como dato que miente sobre su
/// causa es peor que no tenerlo, porque quien lo lea va a buscar donde no es.
#[test]
fn cada_regla_atrapa_con_SU_codigo_y_no_con_el_de_otra() {
    let dos_reglas = "\
perfil llano

funcion f(a es entero64, b es entero64) devuelve entero64
    devuelve (a * a) entre b
";
    // Multiplicar sin pasarse, dividir entre cero -> 1003.
    assert_eq!(ejecuta(dos_reglas, 3, 0), ENTRE_CERO);
    // Multiplicar pasandose -> 1001, y en el MISMO binario.
    assert_eq!(ejecuta(dos_reglas, 1 << 40, 1), DESBORDE);
    // Y sin pasarse ni dividir entre cero, el numero.
    assert_eq!(ejecuta(dos_reglas, 6, 4), 9);
}

// -------------------------------------------------------------------
//  REGLA 12 -- convertir un flotante que no cabe
// -------------------------------------------------------------------

fn convierte(tipo: &str, expr: &str) -> u64 {
    ejecuta(
        &format!(
            "perfil llano\n\nfuncion f(a es entero64, b es entero64) devuelve entero64\n    devuelve {}({})\n",
            tipo, expr
        ),
        0,
        0,
    )
}

/// El caso de la sonda `r12_conversion`: 1e30 no es ningun `entero32`.
#[test]
fn un_flotante_que_no_cabe_atrapa() {
    assert_eq!(convierte("entero32", "1e30"), CONVERSION);
    assert_eq!(convierte("entero64", "1e30"), CONVERSION);
}

/// ** Y EL ANCHO IMPORTA, que es por lo que la comprobacion lo lleva dentro.
///
/// El mismo numero cabe en uno y no en el otro. Una comprobacion que no supiera
/// contra que mide seria una que aprueba todo.
#[test]
fn el_mismo_numero_cabe_en_uno_y_no_en_el_otro() {
    assert_eq!(convierte("entero64", "1e10"), 10_000_000_000);
    assert_eq!(convierte("entero32", "1e10"), CONVERSION);
}

/// Los tres anchos estrechos, cada uno en su borde.
#[test]
fn cada_ancho_atrapa_en_su_borde() {
    assert_eq!(convierte("entero8", "127.0"), 127);
    assert_eq!(convierte("entero8", "128.0"), CONVERSION);
    assert_eq!(convierte("entero16", "32767.0"), 32767);
    assert_eq!(convierte("entero16", "32768.0"), CONVERSION);
}

/// ** TRUNCAR NO ES REDONDEAR, y aqui es donde se ve que la comprobacion mide
/// lo correcto.
///
/// `-128.5` no cabe *como numero* en un `entero8`, pero **truncado si**: da
/// -128. Una comprobacion escrita contra el valor original y no contra el
/// truncado rechazaria este programa, que es correcto.
#[test]
fn lo_que_truncado_cabe_no_atrapa_aunque_el_original_no_quepa() {
    assert_eq!(convierte("entero8", "0.0 - 128.5") as i8, -128);
    assert_eq!(convierte("entero8", "127.9"), 127);
}

/// ** EL NaN, que es el que se cuela sin la bandera de "no comparable".
///
/// Truncar un NaN devuelve el mismo centinela que un desbordamiento. Al
/// compararlo con el limite sale "no comparable", que enciende la bandera de
/// igualdad **a la vez** que la de paridad -- asi que sin mirar la segunda,
/// esto pasaria por un numero legitimo.
#[test]
fn un_nan_no_es_ningun_entero() {
    assert_eq!(convierte("entero64", "0.0 / 0.0"), CONVERSION);
    assert_eq!(convierte("entero32", "0.0 / 0.0"), CONVERSION);
}

/// Y el infinito tampoco.
#[test]
fn el_infinito_no_es_ningun_entero() {
    assert_eq!(convierte("entero64", "1.0 / 0.0"), CONVERSION);
    assert_eq!(convierte("entero64", "0.0 - 1.0 / 0.0"), CONVERSION);
}

/// ** EL CASO QUE HACE FALTA EL SEGUNDO PASO: `-2^63` SI cabe.
///
/// Truncarlo devuelve el mismo centinela que un desbordamiento, asi que una
/// comprobacion que solo mirara el centinela rechazaria un numero perfectamente
/// legitimo -- el mas negativo que existe.
///
/// Por eso, cuando sale el centinela, se compara el ORIGINAL con `-2^63` exacto.
#[test]
fn el_entero_mas_negativo_no_es_un_desbordamiento() {
    let r = convierte("entero64", "0.0 - 9223372036854775808.0");
    assert_eq!(r, i64::MIN as u64, "el mas negativo se rechazo, y es valido");
}

/// Lo justo por debajo si desborda.
#[test]
fn justo_por_debajo_del_mas_negativo_atrapa() {
    assert_eq!(
        convierte("entero64", "0.0 - 9300000000000000000.0"),
        CONVERSION
    );
}

/// Y lo normal sigue funcionando, que es lo que una comprobacion mal puesta se
/// lleva por delante sin que nadie lo note hasta que un programa real falla.
#[test]
fn las_conversiones_normales_no_atrapan() {
    assert_eq!(convierte("entero64", "2.9"), 2);
    assert_eq!(convierte("entero32", "1000.0"), 1000);
    assert_eq!(convierte("entero64", "0.0"), 0);
    assert_eq!(convierte("entero8", "0.0 - 1.0") as i8, -1);
}

// -------------------------------------------------------------------
//  Y la cuenta, que es lo que se puede seguir en el tiempo
// -------------------------------------------------------------------

/// ** Las que la IR pide y las que el binario lleva ya CUADRAN para tres de las
/// cuatro reglas.
///
/// El `Emitido` cuenta las que SALIERON, no las que se pidieron, y esa
/// diferencia es a proposito: el dia que haya eliminacion de comprobaciones,
/// restar los dos numeros dara exactamente lo que el optimizador quito.
///
/// Hoy la unica diferencia es la Regla 2, y tiene su motivo escrito.
#[test]
fn lo_que_la_ir_pide_y_lo_que_el_binario_lleva_ya_cuadran() {
    let f = "\
perfil llano

funcion f(a es entero64, b es entero64) devuelve entero64
    devuelve (a + b) entre (a - b)
";
    // Dos sumas/restas (Regla 1) y una division (Regla 3).
    assert_eq!(reglas_de(f), 3);
    assert_eq!(
        emitido(f).comprobaciones,
        3,
        "la IR pide tres y el binario tiene que llevar tres"
    );
}


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
