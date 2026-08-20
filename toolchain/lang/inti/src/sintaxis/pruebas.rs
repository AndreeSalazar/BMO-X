//! Pruebas del parser.
//!
//! Mismo criterio que en el lexico: **cada prueba fija una frase de
//! `GRAMATICA.md`**, y el nombre de la prueba es esa frase.

use super::*;
use crate::lexico;
use crate::palabras::Vocabulario;

fn arbol(fuente: &str) -> Cosecha<Modulo> {
    let v = Vocabulario::por_defecto().unwrap();
    let piezas = lexico::barrer(fuente, &v);
    assert!(
        piezas.codigos().is_empty(),
        "el fuente de la prueba no lexa limpio: {:?}",
        piezas.codigos()
    );
    leer(&piezas.valor, &v)
}

fn cuerpo_de_principal(fuente: &str) -> Bloque {
    let m = arbol(fuente);
    assert!(
        !m.hay_errores(),
        "no deberia haber errores: {}",
        m.pintar("prueba.inti")
    );
    for d in m.valor.declaraciones {
        if let Decl::Funcion(f) = d {
            if f.nombre == "principal" {
                return f.cuerpo;
            }
        }
    }
    panic!("no hay `funcion principal`");
}

fn en_principal(sentencias: &str) -> Bloque {
    let mut f = String::from("perfil pleno\n\nfuncion principal\n");
    for l in sentencias.lines() {
        f.push_str("    ");
        f.push_str(l);
        f.push('\n');
    }
    cuerpo_de_principal(&f)
}

// ===================================================================
//  El modulo
// ===================================================================

#[test]
fn el_perfil_es_lo_primero() {
    let m = arbol("perfil llano\n");
    assert_eq!(m.valor.perfil, Perfil::Llano);
    assert!(!m.hay_errores());
}

/// No hay perfil por defecto: elegirlo seria elegirte el lenguaje.
#[test]
fn un_fichero_sin_perfil_lo_dice() {
    let m = arbol("funcion principal\n    escribe(\"hola\")\n");
    assert_eq!(m.codigos(), vec!["E0001"]);
    // Y aun asi se sigue leyendo, para poder dar el resto de avisos.
    assert_eq!(m.valor.declaraciones.len(), 1);
}

#[test]
fn un_perfil_que_no_existe_lo_dice() {
    let m = arbol("perfil medio\n");
    assert_eq!(m.codigos(), vec!["E0003"]);
}

#[test]
fn el_usa_se_recoge_en_orden() {
    let m = arbol("perfil pleno\nusa entrada\nusa superficie\n");
    let nombres: Vec<_> = m.valor.usa.iter().map(|(n, _)| n.as_str()).collect();
    assert_eq!(nombres, vec!["entrada", "superficie"]);
}

// ===================================================================
//  Declaraciones
// ===================================================================

#[test]
fn una_funcion_con_tipos_se_lee_entera() {
    let m = arbol(
        "perfil pleno\n\
         funcion media(numeros es lista de numero) devuelve numero\n\
         \x20   devuelve 0\n",
    );
    assert!(!m.hay_errores(), "{}", m.pintar("p.inti"));
    match &m.valor.declaraciones[0] {
        Decl::Funcion(f) => {
            assert_eq!(f.nombre, "media");
            assert_eq!(f.parametros.len(), 1);
            assert_eq!(
                f.parametros[0].tipo,
                Some(Tipo::Lista(Box::new(Tipo::Nombre("numero".into()))))
            );
            let r = f.retorno.as_ref().unwrap();
            assert_eq!(r.tipo, Tipo::Nombre("numero".into()));
            assert!(!r.puede_fallar);
            assert_eq!(f.cuerpo.len(), 1);
        }
        otra => panic!("no es una funcion: {:?}", otra),
    }
}

/// `devuelve numero o error` -- quien llama tiene que mirar el resultado.
#[test]
fn una_funcion_puede_declarar_que_falla() {
    let m = arbol(
        "perfil pleno\n\
         funcion divide(a, b) devuelve numero o error\n\
         \x20   devuelve a / b\n",
    );
    match &m.valor.declaraciones[0] {
        Decl::Funcion(f) => assert!(f.retorno.as_ref().unwrap().puede_fallar),
        _ => panic!(),
    }
}

#[test]
fn un_registro_recoge_sus_campos() {
    let m = arbol(
        "perfil pleno\n\
         registro Alumno\n\
         \x20   nombre es texto\n\
         \x20   nota   es numero\n\
         \x20   activo es logico = cierto\n",
    );
    assert!(!m.hay_errores(), "{}", m.pintar("p.inti"));
    match &m.valor.declaraciones[0] {
        Decl::Registro { nombre, campos, .. } => {
            assert_eq!(nombre, "Alumno");
            assert_eq!(campos.len(), 3);
            assert_eq!(campos[2].nombre, "activo");
            assert!(campos[2].defecto.is_some());
        }
        _ => panic!(),
    }
}

/// La herencia se detecta a proposito para poder decir que NO existe, en vez de
/// soltar un error de sintaxis que no explica nada.
#[test]
fn la_herencia_se_explica_en_vez_de_dar_error_de_sintaxis() {
    let m = arbol(
        "perfil pleno\n\
         registro Persona\n\
         \x20   nombre es texto\n\
         registro Alumno de Persona\n\
         \x20   nota es numero\n",
    );
    assert!(m.codigos().contains(&"E0100"), "{:?}", m.codigos());
    let a = m.avisos.iter().find(|a| a.codigo.0 == "E0100").unwrap();
    assert!(a.que_habia.contains("Un registro son datos"));
}

#[test]
fn una_constante_del_modulo_se_lee() {
    let m = arbol("perfil pleno\nmaximo = 100\n");
    match &m.valor.declaraciones[0] {
        Decl::Constante { nombre, .. } => assert_eq!(nombre, "maximo"),
        _ => panic!(),
    }
}

// ===================================================================
//  Precedencia
// ===================================================================

fn op_de(e: &Expr) -> Op {
    match e {
        Expr::Binaria { op, .. } => *op,
        otra => panic!("no es binaria: {:?}", otra),
    }
}

fn izquierda(e: &Expr) -> &Expr {
    match e {
        Expr::Binaria { izquierda, .. } => izquierda,
        _ => panic!(),
    }
}

fn derecha(e: &Expr) -> &Expr {
    match e {
        Expr::Binaria { derecha, .. } => derecha,
        _ => panic!(),
    }
}

fn expr_de(sentencias: &str) -> Expr {
    match &en_principal(sentencias)[0] {
        Sent::Asigna { valor, .. } => valor.clone(),
        Sent::Expresion(e) => e.clone(),
        otra => panic!("no es una expresion: {:?}", otra),
    }
}

#[test]
fn el_producto_aprieta_mas_que_la_suma() {
    let e = expr_de("x = 1 + 2 * 3\n");
    assert_eq!(op_de(&e), Op::Suma);
    assert_eq!(op_de(derecha(&e)), Op::Por);
}

#[test]
fn la_comparacion_es_mas_floja_que_la_suma() {
    let e = expr_de("x = a + 1 < b\n");
    assert_eq!(op_de(&e), Op::Menor);
    assert_eq!(op_de(izquierda(&e)), Op::Suma);
}

#[test]
fn la_y_aprieta_mas_que_la_o() {
    let e = expr_de("x = a o b y c\n");
    assert_eq!(op_de(&e), Op::O);
    assert_eq!(op_de(derecha(&e)), Op::Y);
}

/// `elevado` asocia a la derecha, que es lo que dicen las matematicas.
#[test]
fn la_potencia_asocia_a_la_derecha() {
    let e = expr_de("x = 2 elevado 3 elevado 2\n");
    assert_eq!(op_de(&e), Op::Elevado);
    assert_eq!(op_de(derecha(&e)), Op::Elevado);
}

/// `/` divide de verdad y `entre` da cociente entero: **nombres distintos**, no
/// dos simbolos que se parecen.
#[test]
fn dividir_y_el_cociente_entero_son_dos_operadores() {
    assert_eq!(op_de(&expr_de("x = 5 / 2\n")), Op::Divide);
    assert_eq!(op_de(&expr_de("x = 5 entre 2\n")), Op::Entre);
}

#[test]
fn no_es_y_es_un_se_distinguen() {
    assert_eq!(op_de(&expr_de("x = a no es b\n")), Op::NoEs);
    assert_eq!(op_de(&expr_de("x = a es un numero\n")), Op::EsUn);
    assert_eq!(op_de(&expr_de("x = a es b\n")), Op::Igual);
}

#[test]
fn los_bits_son_palabras() {
    assert_eq!(op_de(&expr_de("x = a bits_y b\n")), Op::BitsY);
    assert_eq!(
        op_de(&expr_de("x = a desplaza izquierda 3\n")),
        Op::DesplazaIzquierda
    );
}

// ===================================================================
//  La azucar de `de`
// ===================================================================

/// `cuenta de lista` y `cuenta(lista)` salen **el mismo nodo**: `de` no es un
/// operador, es la forma de llamar con un argumento escrita como una frase.
#[test]
fn de_y_los_parentesis_dan_el_mismo_arbol() {
    let con_de = expr_de("x = cuenta de notas\n");
    let con_parentesis = expr_de("x = cuenta(notas)\n");
    match (&con_de, &con_parentesis) {
        (
            Expr::Llamada {
                que: q1,
                argumentos: a1,
                ..
            },
            Expr::Llamada {
                que: q2,
                argumentos: a2,
                ..
            },
        ) => {
            assert!(matches!(**q1, Expr::Nombre(ref n, _) if n == "cuenta"));
            assert!(matches!(**q2, Expr::Nombre(ref n, _) if n == "cuenta"));
            assert_eq!(a1.len(), 1);
            assert_eq!(a2.len(), 1);
        }
        otro => panic!("no son dos llamadas: {:?}", otro),
    }
}

#[test]
fn valor_y_motivo_son_llamadas_de_biblioteca() {
    let e = expr_de("x = valor de r\n");
    assert!(matches!(&e, Expr::Llamada { que, .. } if matches!(&**que, Expr::Nombre(n, _) if n == "valor")));
}

// ===================================================================
//  Sentencias
// ===================================================================

#[test]
fn el_si_recoge_sus_tres_ramas() {
    let b = en_principal(
        "si a > 30\n    escribe(\"mucho\")\nsino si a > 15\n    escribe(\"normal\")\nsino\n    escribe(\"poco\")\n",
    );
    match &b[0] {
        Sent::Si { ramas, sino, .. } => {
            assert_eq!(ramas.len(), 2);
            assert!(sino.is_some());
        }
        otra => panic!("no es un si: {:?}", otra),
    }
}

#[test]
fn las_tres_formas_de_repite() {
    let b = en_principal("repite 10 veces\n    escribe(\".\")\n");
    assert!(matches!(&b[0], Sent::Repite { forma: Repeticion::Veces(_), .. }));

    let b = en_principal("repite mientras quedan\n    escribe(\".\")\n");
    assert!(matches!(&b[0], Sent::Repite { forma: Repeticion::Mientras(_), .. }));

    let b = en_principal("repite\n    corta\n");
    assert!(matches!(&b[0], Sent::Repite { forma: Repeticion::Siempre, .. }));
}

/// El rango vive en el bucle y **no es un valor**: no se puede guardar en una
/// variable, asi que no necesita un tipo que el lenguaje no tiene.
#[test]
fn el_rango_vive_dentro_del_bucle() {
    let b = en_principal("para cada i en 0 hasta 10\n    escribe(i)\n");
    match &b[0] {
        Sent::ParaCada { nombre, hasta, .. } => {
            assert_eq!(nombre, "i");
            assert!(hasta.is_some());
        }
        otra => panic!("no es un para cada: {:?}", otra),
    }
}

#[test]
fn para_cada_sobre_una_lista_no_tiene_hasta() {
    let b = en_principal("para cada a en alumnos\n    escribe(a)\n");
    match &b[0] {
        Sent::ParaCada { hasta, .. } => assert!(hasta.is_none()),
        _ => panic!(),
    }
}

#[test]
fn cambiante_se_recoge_y_el_tipo_tambien() {
    let b = en_principal("cambiante x es entero32 = 0\n");
    match &b[0] {
        Sent::Asigna {
            cambiante, tipo, ..
        } => {
            assert!(cambiante);
            assert_eq!(tipo.as_ref(), Some(&Tipo::Nombre("entero32".into())));
        }
        _ => panic!(),
    }
}

/// **Asignar es una sentencia y nunca una expresion.** Es lo que deja que `=`
/// signifique igual en los dos sitios sin ambiguedad.
#[test]
fn asignar_no_es_una_expresion() {
    // Dentro de un `si`, el `=` solo puede ser una comparacion.
    let b = en_principal("si x = 5\n    escribe(\"cinco\")\n");
    match &b[0] {
        Sent::Si { ramas, .. } => assert_eq!(op_de(&ramas[0].0), Op::Igual),
        _ => panic!(),
    }
}

/// En INTI no se declara sin valor, y por eso leer una variable sin
/// inicializar --el comportamiento indefinido mas viejo de C-- **no se puede
/// ni escribir**.
#[test]
fn declarar_sin_valor_no_se_puede_escribir() {
    let m = arbol("perfil llano\nfuncion principal\n    cambiante x es entero32\n");
    assert!(m.codigos().contains(&"E0031"), "{:?}", m.codigos());
}

#[test]
fn a_una_llamada_no_se_le_asigna() {
    let m = arbol("perfil pleno\nfuncion principal\n    f(1) = 2\n");
    assert!(m.hay_errores());
}

#[test]
fn crudo_y_paralelo_son_bloques() {
    let b = en_principal("en paralelo\n    f(1)\n");
    assert!(matches!(&b[0], Sent::Paralelo { .. }));

    let b = en_principal("crudo\n    f(1)\n");
    assert!(matches!(&b[0], Sent::Crudo { .. }));
}

#[test]
fn devuelve_puede_no_llevar_valor() {
    let b = en_principal("devuelve\n");
    assert!(matches!(&b[0], Sent::Devuelve { valor: None, .. }));
}

// ===================================================================
//  Errores como datos
// ===================================================================

#[test]
fn o_si_no_con_un_valor() {
    let e = expr_de("x = abrir(\"n.txt\") o si no \"\"\n");
    assert!(matches!(
        &e,
        Expr::OSiNo {
            respaldo: Respaldo::Valor(_),
            ..
        }
    ));
}

#[test]
fn o_si_no_con_un_bloque() {
    let b = en_principal("x = abrir(\"n.txt\") o si no\n    devuelve\n");
    match &b[0] {
        Sent::Asigna { valor, .. } => assert!(matches!(
            valor,
            Expr::OSiNo {
                respaldo: Respaldo::Bloque(_),
                ..
            }
        )),
        otra => panic!("{:?}", otra),
    }
}

/// El `o` de `o si no` y el `o` logico no se pisan.
#[test]
fn el_o_logico_sigue_funcionando_al_lado_de_o_si_no() {
    let e = expr_de("x = a o b\n");
    assert_eq!(op_de(&e), Op::O);
}

// ===================================================================
//  Recuperacion
// ===================================================================

/// Parar en el primer error convertiria arreglar un fichero en adivinar cuantos
/// quedan.
#[test]
fn un_fallo_no_se_lleva_el_resto_del_fichero() {
    let m = arbol(
        "perfil pleno\n\
         funcion principal\n\
         \x20   x = \n\
         \x20   y = 2\n\
         funcion otra\n\
         \x20   devuelve 1\n",
    );
    assert!(m.hay_errores());
    assert_eq!(
        m.valor.declaraciones.len(),
        2,
        "la funcion de despues tiene que seguir leyendose"
    );
}
