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
    let ir = ir::bajar_con(&arbol.valor, &modulos, &plano.valor).valor;
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
