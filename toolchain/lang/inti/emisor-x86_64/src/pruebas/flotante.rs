//! LA COMA FLOTANTE -- el numero que mide, no el que cuenta.
//!
//! Las cuatro operaciones, las seis comparaciones con el NaN correcto, y la
//! conversion. Y la Regla 11, que se comprueba en lo que NO se emite.

use super::*;
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
    // ** DOS: la del divisor cero (E1003) y la del cociente que no cabe
    // (E1001, y es la Regla 1 escondida dentro de una division). La segunda
    // se anadio el 2026-08-22; hasta entonces `-2^63 entre -1` mataba el
    // programa con una autopsia en vez de atrapar.
    assert_eq!(reglas_de(f), 2, "a los enteros les faltan reglas de la division");
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

/// *** LOS BITS VAN Y VUELVEN, Y ESE ES EL CONTRATO DE UNA FILA DE CERO BYTES.
///
/// ** `bits_de` y `flotante_de` no emiten una sola instruccion en esta maquina,
/// asi que el guardian de la tabla --que exige que los bytes de cada fila
/// aparezcan en el codigo-- no puede decir nada de ellas. Lo que se les exige
/// es otra cosa, y mas fuerte: **que el valor llegue de la entrada a la salida
/// sin cambiar ni un bit.**
///
/// Y se comprueba EJECUTANDO, que es el criterio de este banco: una
/// reinterpretacion que se equivocara de registro se veria igual en un volcado.
#[test]
fn los_bits_van_y_vuelven() {
    // 1.0 en IEEE-754 de 64 bits es 0x3FF0000000000000. Es el numero que hace
    // esta prueba legible: si algo convirtiera el VALOR en vez de los bits,
    // saldria un 1 y no un 0x3FF0...
    let f = "perfil llano\nusa matematica\n\nfuncion prueba() devuelve natural64\n    devuelve bits_de(1.0)\n";
    assert_eq!(
        ejecuta_en(&f, "prueba", 0, 0),
        0x3FF0_0000_0000_0000,
        "bits_de tiene que dar el PATRON, no el valor"
    );

    // Y la vuelta: los mismos bits leidos como flotante son 1.0 otra vez.
    let g = "perfil llano\nusa matematica\n\nfuncion prueba() devuelve natural64\n    x es flotante64 = flotante_de(4607182418800017408)\n    devuelve bits_de(x)\n";
    assert_eq!(ejecuta_en(&g, "prueba", 0, 0), 0x3FF0_0000_0000_0000);
}

/// [!] Y LA DIFERENCIA CON `entero64`, que es lo que el aviso `E0123` mandaba a
/// hacer y **no hacia lo que decia**.
///
/// ```text
///    entero64(3.5)   ->  3                      convierte el VALOR
///    bits_de(3.5)    ->  0x400C000000000000     entrega los BYTES
/// ```
///
/// ** El consejo de `E0123` decia *"convierte a entero primero si lo que quieres
/// son los bits"*, y eso da un numero pequeno donde se esperaba un patron. Se
/// arreglo el consejo el mismo dia que aparecio el camino de verdad.
#[test]
fn convertir_el_valor_y_leer_los_bits_no_son_lo_mismo() {
    let valor = "perfil llano\n\nfuncion prueba() devuelve entero64\n    devuelve entero64(3.5)\n";
    assert_eq!(ejecuta_en(&valor, "prueba", 0, 0), 3, "entero64 TRUNCA el valor");

    let bits = "perfil llano\nusa matematica\n\nfuncion prueba() devuelve natural64\n    devuelve bits_de(3.5)\n";
    assert_eq!(
        ejecuta_en(&bits, "prueba", 0, 0),
        0x400C_0000_0000_0000,
        "bits_de da el patron de 3,5"
    );
}

/// **Y con esto se construye `2^k` sin tabla ni bucle**, que es lo que hace
/// posible `exp`.
///
/// ** El exponente de IEEE-754 vive en los bits 62..52 con sesgo 1023. Poner
/// `k + 1023` ahi y llamarlo flotante da `2^k` exacto -- sin error, porque una
/// potencia de dos se representa clavada.
#[test]
fn dos_elevado_a_k_se_construye_con_los_bits() {
    let f = |k: i64| {
        format!(
            "perfil llano\nusa matematica\n\nfuncion prueba() devuelve natural64\n    e es natural64 = natural64({} + 1023)\n    devuelve bits_de(flotante_de(e desplaza izquierda 52))\n",
            k
        )
    };
    // 2^0 = 1.0
    assert_eq!(ejecuta_en(&f(0), "prueba", 0, 0), 0x3FF0_0000_0000_0000);
    // 2^1 = 2.0
    assert_eq!(ejecuta_en(&f(1), "prueba", 0, 0), 0x4000_0000_0000_0000);
    // 2^-1 = 0.5
    assert_eq!(ejecuta_en(&f(-1), "prueba", 0, 0), 0x3FE0_0000_0000_0000);
    // 2^10 = 1024.0
    assert_eq!(ejecuta_en(&f(10), "prueba", 0, 0), 0x4090_0000_0000_0000);
}

// ===================================================================
//  `exp`, escrita en INTI. Ver `runtime/matematica/exp.inti`
// ===================================================================

/// Corre `exp(x)` de verdad y devuelve lo que salio, ya como flotante.
///
/// ** Se devuelven los BITS por la puerta y se rehacen aqui: `ejecuta_en` da un
/// `u64`, y convertir el valor en vez de los bits perderia la parte decimal --
/// que es justamente lo que esta prueba mira.
fn exp_de(x: &str) -> f64 {
    let f = format!(
        "perfil llano\nusa matematica\n\nfuncion prueba() devuelve natural64\n    devuelve bits_de(exp({}))\n",
        x
    );
    f64::from_bits(ejecuta_en(&f, "prueba", 0, 0))
}

/// **`exp(0)` es UNO CLAVADO, y eso no es una casualidad del redondeo.**
///
/// ** Con `x = 0`: `k` sale 0, `r` sale 0, y el polinomio de Horner devuelve
/// exactamente 1,0 porque todos los terminos con `r` se anulan. Si esto diera
/// `0,9999999` habria un error en la reduccion, no en el polinomio.
#[test]
fn exp_de_cero_es_uno_clavado() {
    assert_eq!(exp_de("0.0"), 1.0, "sin margen: tiene que ser exacto");
}

/// Los valores que se pueden comprobar de memoria.
#[test]
fn exp_da_los_numeros_que_tiene_que_dar() {
    let casos: [(&str, f64); 6] = [
        ("1.0", core::f64::consts::E),
        ("-1.0", 1.0 / core::f64::consts::E),
        ("2.0", 7.389056098930650),
        ("-2.0", 0.135335283236612),
        ("10.0", 22026.465794806718),
        ("-10.0", 0.0000453999297624848),
    ];
    for (entrada, esperado) in casos {
        let salida = exp_de(entrada);
        let error = ((salida - esperado) / esperado).abs();
        // *** SIETE CIFRAS es lo que este `exp` promete en su cabecera, y esta
        // linea es lo que convierte esa promesa en algo que se puede romper.
        // Si alguien baja el grado del polinomio, aqui se entera.
        assert!(
            error < 1e-7,
            "exp({entrada}) dio {salida} y tenia que dar {esperado} (error {error:e})"
        );
    }
}

/// *** LA REDUCCION FUNCIONA EN TODO EL RANGO, y esta es la prueba que la mira.
///
/// ** El polinomio solo es bueno para `|r| <= ln2/2`. Todo lo demas lo hace la
/// reduccion `x = k*ln2 + r`, y un fallo ahi **no se ve con numeros pequenos**:
/// con `x = 1` el `k` vale 1 y casi cualquier cosa acierta. Con `x = 700` el
/// `k` vale 1010, y ahi es donde el `ln2` partido en dos gana su sitio.
#[test]
fn los_extremos_del_rango_siguen_dando_siete_cifras() {
    for (entrada, esperado) in [("700.0", 1.0142320547350045e304), ("-700.0", 9.859676543759770e-305)] {
        let salida = exp_de(entrada);
        let error = ((salida - esperado) / esperado).abs();
        assert!(
            error < 1e-7,
            "exp({entrada}) dio {salida} y tenia que dar {esperado} (error {error:e})"
        );
    }
}

/// **Fuera de rango SATURA, no atrapa.**
///
/// ** Y es lo correcto: por encima de 709,78 el resultado no cabe en un
/// `flotante64` y la respuesta de IEEE-754 es el infinito; por debajo de
/// -745,13 es el cero. Los dos son valores DEFINIDOS.
///
/// *** Y para un softmax importa que sea asi: un valor muy negativo quiere un
/// cero, no una tarea muerta. Atrapar aqui haria que la funcion no sirviera
/// para lo unico que la hizo falta.
#[test]
fn fuera_de_rango_satura_en_vez_de_atrapar() {
    assert!(exp_de("800.0").is_infinite(), "por arriba, infinito");
    assert!(exp_de("800.0") > 0.0, "y positivo");
    assert_eq!(exp_de("-800.0"), 0.0, "por abajo, cero");
}

/// *** EL SOFTMAX, QUE ES PARA LO QUE SE ESCRIBIO.
///
/// ** La forma que se usa de verdad resta el maximo antes de exponenciar --
/// `exp(x - max)`-- justamente para que ningun termino se salga por arriba. Asi
/// que todas las entradas de `exp` son <= 0, y el caso que importa es que los
/// muy negativos den cero sin romper la suma.
///
/// Esta prueba hace ese softmax entero en INTI y comprueba que **suma uno**.
#[test]
fn un_softmax_entero_en_inti_suma_uno() {
    // Tres valores ya con el maximo restado: 0, -1, -2.
    let f = "perfil llano\nusa matematica\n\nfuncion prueba() devuelve natural64\n    \
             a es flotante64 = exp(0.0)\n    \
             b es flotante64 = exp(-1.0)\n    \
             c es flotante64 = exp(-2.0)\n    \
             total es flotante64 = a + b + c\n    \
             p0 es flotante64 = a / total\n    \
             p1 es flotante64 = b / total\n    \
             p2 es flotante64 = c / total\n    \
             devuelve bits_de(p0 + p1 + p2)\n";
    let suma = f64::from_bits(ejecuta_en(f, "prueba", 0, 0));
    assert!(
        (suma - 1.0).abs() < 1e-12,
        "las probabilidades de un softmax suman uno, y dieron {suma}"
    );

    // Y la mayor es la del valor mas alto, que es la otra mitad del contrato.
    let g = "perfil llano\nusa matematica\n\nfuncion prueba() devuelve natural64\n    \
             a es flotante64 = exp(0.0)\n    \
             b es flotante64 = exp(-1.0)\n    \
             c es flotante64 = exp(-2.0)\n    \
             total es flotante64 = a + b + c\n    \
             devuelve bits_de(a / total)\n";
    let p0 = f64::from_bits(ejecuta_en(g, "prueba", 0, 0));
    // e^0 / (e^0 + e^-1 + e^-2) = 0,66524...
    //
    // [!] La tolerancia es 1e-7 y NO menos, y eso no es aflojar: es lo que
    // `exp` promete en su cabecera. Una prueba mas estricta que el contrato de
    // lo que prueba no mide mejor -- mide otra cosa, y se pone roja el dia que
    // alguien haga un cambio perfectamente correcto.
    assert!((p0 - 0.665240955774822).abs() < 1e-7, "p0 dio {p0}");
}

/// *** NEGAR UN FLOTANTE NO ES NEGAR SUS BITS -- y esto acertaba a veces.
///
/// ## El fallo, y por que sobrevivio
///
/// `Instr::Unaria` no llevaba la clase, asi que el emisor bajaba `neg rax` para
/// todo. Sobre un `flotante64` eso hace el **complemento a dos del patron de
/// bits**, y un patron de flotante es signo + exponente + mantisa: el
/// complemento a dos revuelve los tres.
///
/// *** Y lo caro es que **funcionaba en la mitad de los casos**:
///
/// ```text
///    -2.0   bits 0x4000000000000000   complemento a dos -> 0xC000000000000000
///                                     sign-flip         -> 0xC000000000000000   IGUAL
///
///    -1.0   bits 0x3FF0000000000000   complemento a dos -> 0xC010000000000000  = -4,0
///                                     sign-flip         -> 0xBFF0000000000000  = -1,0
/// ```
///
/// Un fallo que acierta a veces es el que sobrevive: nadie escribio `-1.0` en un
/// programa de INTI hasta que `exp` lo necesito, el 2026-08-24.
///
/// ** Y ninguna de las doce reglas podia cazarlo. No era comportamiento
/// indefinido: era **una respuesta equivocada, en silencio** -- la misma familia
/// que el signo de `setl` y que sumar dos direcciones en `texto + texto`.
#[test]
fn negar_un_flotante_no_es_negar_sus_bits() {
    let bits = |lit: &str| {
        let f = format!(
            "perfil llano
usa matematica

funcion prueba() devuelve natural64
    x es flotante64 = {}
    devuelve bits_de(x)
",
            lit
        );
        ejecuta_en(&f, "prueba", 0, 0)
    };

    // El que fallaba, y el numero exacto que daba.
    assert_eq!(bits("-1.0"), (-1.0f64).to_bits(), "daba -4,0");
    // El que acertaba por casualidad: tiene que seguir acertando.
    assert_eq!(bits("-2.0"), (-2.0f64).to_bits());
    // Y unos cuantos mas, porque el fallo dependia del patron.
    for v in [-0.5f64, -3.5, -0.1, -1e-9, -1e300] {
        let lit = format!("{:?}", v);
        assert_eq!(bits(&lit), v.to_bits(), "fallo con {lit}");
    }

    // [!] Y el entero NO cambia: sigue siendo complemento a dos.
    let e = "perfil llano

funcion prueba() devuelve entero64
    x es entero64 = -7
    devuelve x
";
    assert_eq!(ejecuta_en(e, "prueba", 0, 0) as i64, -7);
}

/// Y negar DOS VECES devuelve el mismo numero, bit a bit.
///
/// ** Con el `neg` de enteros esto tambien pasaba --el complemento a dos es su
/// propia inversa-- asi que por si sola no habria cazado nada. Esta aqui porque
/// es la propiedad que un lector espera, y una prueba que solo vale acompanada
/// vale mas escrita que supuesta.
#[test]
fn negar_dos_veces_deja_el_flotante_como_estaba() {
    let f = "perfil llano
usa matematica

funcion prueba() devuelve natural64
    x es flotante64 = 3.75
    y es flotante64 = -x
    devuelve bits_de(-y)
";
    assert_eq!(ejecuta_en(f, "prueba", 0, 0), (3.75f64).to_bits());
}
