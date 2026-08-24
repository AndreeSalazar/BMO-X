//! El decimal exacto: `0.1 + 0.2` da `0.3`.
//!
//! ** Salio de `pruebas.rs` el 2026-08-23 por L6a. Aqui esta la promesa de la
//! portada ejecutada, y el descenso que la conecta -- que trajo consigo la
//! primera local de mas de una palabra.

use super::*;
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
