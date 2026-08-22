//! EL MARCO Y LA PUERTA -- donde vive un valor, y como se sale.
//!
//! Como reparte el marco los registros, como se llama a otra funcion, como
//! arranca un programa solo, y como cruza la puerta. Cuatro cosas y una sola
//! pregunta detras: **donde esta cada cosa cuando el codigo corre**.

use super::*;
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
