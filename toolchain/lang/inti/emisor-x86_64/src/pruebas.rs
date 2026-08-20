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
    let v = Vocabulario::por_defecto().expect("sin vocabulario");
    let piezas = lexico::barrer(fuente, &v);
    let arbol = sintaxis::leer(&piezas.valor, &v);
    assert!(
        !arbol.hay_errores(),
        "el fuente de la prueba no se lee: {}",
        arbol.pintar("prueba.inti")
    );
    let ir = ir::bajar(&arbol.valor).valor;
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
