//! Pruebas del barrido.
//!
//! El criterio: **cada prueba fija una frase de `GRAMATICA.md`**, y el nombre
//! de la prueba es esa frase. Si alguien cambia el lenguaje, aqui se ve cual de
//! las decisiones escritas dejo de ser verdad.

use super::*;
use crate::aviso::codigos;
use crate::palabras::{Simbolo, Vocabulario};

fn vocab() -> Vocabulario {
    Vocabulario::por_defecto().expect("la tabla de palabras no carga")
}

fn piezas(fuente: &str) -> Vec<Clase> {
    barrer(fuente, &vocab())
        .valor
        .into_iter()
        .map(|p| p.clase)
        .collect()
}

fn codigos_de(fuente: &str) -> Vec<&'static str> {
    barrer(fuente, &vocab()).codigos()
}

fn nombre(n: &str) -> Clase {
    Clase::Nombre(n.to_string())
}

// ===================================================================
//  Lo basico
// ===================================================================

#[test]
fn un_fichero_vacio_no_es_un_error() {
    let c = barrer("", &vocab());
    assert_eq!(c.valor.len(), 1);
    assert_eq!(c.valor[0].clase, Clase::Fin);
    assert!(c.avisos.is_empty());
}

#[test]
fn la_primera_linea_se_lee_entera() {
    assert_eq!(
        piezas("perfil pleno"),
        vec![
            Clase::Palabra(Simbolo::Perfil),
            Clase::Palabra(Simbolo::Pleno),
            Clase::FinLinea,
            Clase::Fin,
        ]
    );
}

#[test]
fn un_nombre_no_es_una_palabra_clave() {
    assert_eq!(
        piezas("alumno = 5"),
        vec![
            nombre("alumno"),
            Clase::Signo(Signo::Igual),
            Clase::Numero(Numero {
                texto: "5".into(),
                base: Base::Diez,
                con_punto: false
            }),
            Clase::FinLinea,
            Clase::Fin,
        ]
    );
}

/// `GRAMATICA.md` sec. 3: los tipos empiezan por mayuscula.
#[test]
fn la_mayuscula_inicial_hace_un_tipo() {
    let p = piezas("Alumno");
    assert_eq!(p[0], Clase::Tipo("Alumno".into()));
}

#[test]
fn el_comentario_llega_hasta_el_final_de_la_linea() {
    assert_eq!(
        piezas("x = 1 # esto no cuenta"),
        vec![
            nombre("x"),
            Clase::Signo(Signo::Igual),
            Clase::Numero(Numero {
                texto: "1".into(),
                base: Base::Diez,
                con_punto: false
            }),
            Clase::FinLinea,
            Clase::Fin,
        ]
    );
}

/// Una linea de solo comentario **no toca el margen**. Si lo tocara, un
/// comentario pegado al borde cerraria el bloque en el que esta escrito.
#[test]
fn un_comentario_pegado_al_borde_no_cierra_el_bloque() {
    let fuente = "funcion f\n    x = 1\n# comentario al borde\n    y = 2\n";
    let p = piezas(fuente);
    let desangres = p.iter().filter(|c| **c == Clase::Desangra).count();
    assert_eq!(desangres, 1, "solo el del final del fichero");
}

// ===================================================================
//  Sangria
// ===================================================================

#[test]
fn el_bloque_abre_y_cierra_con_el_margen() {
    let p = piezas("funcion f\n    x = 1\n");
    assert!(p.contains(&Clase::Sangra));
    assert!(p.contains(&Clase::Desangra));
    assert_eq!(p[p.len() - 1], Clase::Fin);
    assert_eq!(p[p.len() - 2], Clase::Desangra, "el fichero cierra lo abierto");
}

#[test]
fn el_tabulador_es_un_error() {
    assert_eq!(codigos_de("funcion f\n\tx = 1\n"), vec!["E0010"]);
}

// ===================================================================
//  Textos
// ===================================================================

#[test]
fn un_texto_pierde_las_comillas_y_gana_sus_escapes() {
    let p = piezas("x = \"hola\\nadios\"");
    assert_eq!(p[2], Clase::Texto("hola\nadios".into()));
}

#[test]
fn la_llave_escapada_es_una_llave() {
    let p = piezas("x = \"una \\{ literal\"");
    assert_eq!(p[2], Clase::Texto("una { literal".into()));
}

/// `GRAMATICA.md` sec. 3: la comilla simple no existe. Una forma menos.
#[test]
fn la_comilla_simple_no_existe() {
    assert_eq!(codigos_de("escribe 'hola'"), vec!["E0011"]);
}

#[test]
fn un_texto_no_sigue_en_la_linea_de_abajo() {
    assert_eq!(codigos_de("x = \"se me olvido"), vec!["E0013"]);
}

#[test]
fn un_escape_que_no_existe_se_dice() {
    let c = barrer("x = \"a\\q\"", &vocab());
    assert_eq!(c.codigos(), vec!["E0014"]);
    assert!(c.avisos[0].que_habia.contains("los escapes son cinco"));
}

// ===================================================================
//  Numeros
// ===================================================================

/// El lexer **no convierte**: `numero` es decimal exacto y pasarlo por `f64`
/// aqui perderia en el primer paso justo lo que el lenguaje promete.
#[test]
fn el_numero_se_guarda_tal_y_como_se_escribio() {
    let p = piezas("x = 0.1");
    assert_eq!(
        p[2],
        Clase::Numero(Numero {
            texto: "0.1".into(),
            base: Base::Diez,
            con_punto: true
        })
    );
}

#[test]
fn el_hexadecimal_se_recuerda_como_hexadecimal() {
    let p = piezas("x = 0x60");
    assert_eq!(
        p[2],
        Clase::Numero(Numero {
            texto: "0x60".into(),
            base: Base::Dieciseis,
            con_punto: false
        })
    );
}

/// `p.x` no es un numero con punto: el punto solo es decimal si detras hay un
/// digito.
#[test]
fn el_punto_de_un_campo_no_es_un_decimal() {
    assert_eq!(
        piezas("p.x"),
        vec![
            nombre("p"),
            Clase::Signo(Signo::Punto),
            nombre("x"),
            Clase::FinLinea,
            Clase::Fin,
        ]
    );
}

#[test]
fn un_numero_con_dos_puntos_se_denuncia() {
    assert_eq!(codigos_de("x = 1.2.3"), vec!["E0016"]);
}

#[test]
fn un_cero_equis_sin_digitos_se_denuncia() {
    assert_eq!(codigos_de("x = 0x"), vec!["E0016"]);
}

// ===================================================================
//  Signos, y los que vienen de otros lenguajes
// ===================================================================

#[test]
fn menor_igual_es_un_signo_y_no_dos() {
    let p = piezas("a <= b");
    assert_eq!(p[1], Clase::Signo(Signo::MenorIgual));
    assert_eq!(p.len(), 5);
}

/// La deuda de venir de C o de Python: la persona escribio lo que sabia, y el
/// mensaje tiene que reconocerlo en vez de decir "caracter no valido".
#[test]
fn el_punto_y_coma_se_explica_en_vez_de_rechazarse() {
    let c = barrer("x = 1;", &vocab());
    assert_eq!(c.codigos(), vec!["E0015"]);
    assert!(c.avisos[0].que_habia.contains("acaba donde acaba la linea"));
    assert_eq!(c.avisos[0].que_hacer, "borra el `;`");
}

#[test]
fn los_operadores_de_c_apuntan_a_su_palabra() {
    let c = barrer("si a && b", &vocab());
    assert!(c.avisos[0].que_habia.contains("`y`, `o`, `no`"));
    assert_eq!(c.avisos[0].que_hacer, "escribe `y`");
}

#[test]
fn el_porcentaje_apunta_a_resto() {
    let c = barrer("x = 5 % 2", &vocab());
    assert_eq!(c.avisos[0].que_hacer, "escribe `resto`");
}

// ===================================================================
//  Parejas y continuacion de linea
// ===================================================================

/// La unica continuacion de linea que existe: dentro de una pareja abierta.
#[test]
fn dentro_de_una_pareja_la_linea_sigue() {
    let p = piezas("x = [1,\n     2,\n     3]\n");
    let fines = p.iter().filter(|c| **c == Clase::FinLinea).count();
    assert_eq!(fines, 1, "las tres lineas son una sola sentencia");
}

/// Y por eso mismo, dentro de una pareja el margen no significa nada.
#[test]
fn dentro_de_una_pareja_el_margen_no_cuenta() {
    let p = piezas("x = [1,\n        2]\n");
    assert!(!p.contains(&Clase::Sangra), "no puede abrir un bloque");
}

#[test]
fn una_pareja_sin_cerrar_dice_donde_empezo() {
    let c = barrer("x = (1 + 2\n", &vocab());
    assert_eq!(c.codigos(), vec!["E0017"]);
    assert!(c.avisos[0].que_habia.contains("linea 1"));
}

#[test]
fn cerrar_con_la_pareja_equivocada_se_dice() {
    let c = barrer("x = (1 + 2]\n", &vocab());
    assert_eq!(c.codigos(), vec!["E0017"]);
    assert!(c.avisos[0].que_paso.contains("Aqui va `)`"));
}

// ===================================================================
//  El idioma
// ===================================================================

/// Quien escribe con tildes no tropieza.
#[test]
fn una_palabra_clave_con_tilde_sigue_siendo_palabra_clave() {
    let p = piezas("funci\u{f3}n f");
    assert_eq!(p[0], Clase::Palabra(Simbolo::Funcion));
}

/// Y un nombre con tilde vale, con aviso: no es un error, es un dato.
#[test]
fn un_nombre_con_tilde_vale_y_se_avisa() {
    let c = barrer("a\u{f1}o = 2026", &vocab());
    assert_eq!(c.codigos(), vec!["A2010"]);
    assert!(!c.hay_errores(), "un aviso no impide compilar");
    assert!(c.avisos[0].que_hacer.contains("ano"));
}

/// La prueba de que el idioma es una columna: el mismo barrido, otro
/// vocabulario, y el lexer no se entera.
#[test]
fn el_mismo_lexer_lee_ingles_sin_cambiar_una_linea() {
    let ingles = Vocabulario::desde_texto(
        include_str!("../../../../forge/sem-asm/tables/lang/inti/palabras.toml"),
        Some("en"),
    )
    .expect("no carga el ingles");

    let p: Vec<Clase> = barrer("profile full\nfunction f\n    while x\n", &ingles)
        .valor
        .into_iter()
        .map(|x| x.clase)
        .collect();

    assert_eq!(p[0], Clase::Palabra(Simbolo::Perfil));
    assert_eq!(p[1], Clase::Palabra(Simbolo::Pleno));
    assert!(p.contains(&Clase::Palabra(Simbolo::Mientras)));
    // Y en ese dialecto, `mientras` es un nombre cualquiera.
    let q: Vec<Clase> = barrer("mientras = 1", &ingles)
        .valor
        .into_iter()
        .map(|x| x.clase)
        .collect();
    assert_eq!(q[0], nombre("mientras"));
}

// ===================================================================
//  Un fichero de verdad
// ===================================================================

/// La sonda `f01_funcion` del censo, barrida entera y sin un solo aviso.
#[test]
fn la_sonda_f01_se_barre_limpia() {
    let fuente = "\
perfil pleno

funcion media(numeros es lista de numero) devuelve numero
   si esta vacia numeros
      devuelve 0
   cambiante suma = 0
   para cada n en numeros
      suma = suma + n
   devuelve suma / cuenta de numeros

funcion principal
   escribe media([8, 6, 9])
";
    let c = barrer(fuente, &vocab());
    // La sonda esta escrita con tres espacios, que es lo que se leia bien en el
    // documento; el lenguaje pide cuatro. Que el censo y la gramatica no
    // estuvieran de acuerdo es EXACTAMENTE lo que estas pruebas existen para
    // encontrar -- ver `ARQUITECTURA.md`, "el censo manda".
    let sangrias: Vec<_> = c.codigos();
    assert!(
        sangrias.iter().all(|x| *x == "E0012"),
        "no deberia haber mas fallo que el del margen: {:?}",
        sangrias
    );
    assert!(c.valor.iter().any(|p| p.es(Simbolo::Funcion)));
    assert!(c.valor.iter().any(|p| p.es(Simbolo::Devuelve)));
    assert!(c.valor.iter().any(|p| p.es(Simbolo::De)));
}

/// ** El natural64 mas grande CABE, y el que no cabe se denuncia.
///
/// Durante un dia `0xFFFFFFFFFFFFFFFF` --que es un `natural64` perfectamente
/// valido-- no cabia en el `i64` de los literales, la conversion fallaba, y el
/// numero **se convertia en CERO sin una sola queja**.
///
/// Se encontro escribiendo una prueba de otra cosa, que es como se encuentran
/// estos: nadie escribe un test para el caso que cree que funciona.
#[test]
fn el_natural64_mas_grande_cabe() {
    assert_eq!(
        super::valor_entero("FFFFFFFFFFFFFFFF", super::Base::Dieciseis),
        Some(-1i64),
        "el patron de bits, que es lo que se guarda"
    );
    assert_eq!(codigos_de("x = 0xFFFFFFFFFFFFFFFF"), Vec::<&str>::new());
}

#[test]
fn un_numero_que_no_cabe_en_64_bits_se_dice() {
    assert_eq!(codigos_de("x = 0xFFFFFFFFFFFFFFFFF"), vec!["E0018"]);
}

/// Y el patron se lee como diga el tipo, no como diga el literal.
#[test]
fn el_literal_guarda_bits_y_no_una_interpretacion() {
    // El mismo hueco de 64 bits, escrito de las dos formas.
    assert_eq!(
        super::valor_entero("18446744073709551615", super::Base::Diez),
        super::valor_entero("FFFFFFFFFFFFFFFF", super::Base::Dieciseis)
    );
}
