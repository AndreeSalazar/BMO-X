//! Lo que esta tabla promete, comprobado.

use super::*;

#[test]
fn la_tabla_incrustada_se_lee_entera() {
    let n = Necesidades::por_defecto();
    assert_eq!(n.monton_por_defecto(), 4096, "la decision de Eddi: una pagina");
    assert!(n.monton_maximo() > 4096, "el techo tiene que dejar sitio");
    assert_eq!(n.unidad("bytes"), Some(1));
    assert_eq!(n.unidad("megas"), Some(1024 * 1024));
    assert_eq!(n.unidad("gigas"), Some(1024 * 1024 * 1024));
}

/// [!] LA PRUEBA QUE COMPRUEBA QUE ESTOS NUMEROS SON LOS DEL ABI **NO ESTA
/// AQUI**, y no es un olvido.
///
/// `bmo-inti-front` no enlaza `bmo-abi` a proposito --lo dice su `Cargo.toml`
/// en su primera decision: *"F1 no emite bytes"*-- asi que aqui no hay con que
/// comparar. La prueba vive en el emisor, que si lo enlaza y que es ademas
/// quien escribe la seccion: `pruebas/necesita.rs`.

/// `memoria` NO se puede pedir, y su ausencia esta escrita en la tabla.
///
/// Es lo que tiene que existir antes de la primera instruccion, y eso lo sabe
/// el cargador mirando el fichero mejor que el programa. Dejarlo declarar seria
/// dejar que un programa mienta sobre su propio tamano.
#[test]
fn la_memoria_del_proceso_no_se_declara() {
    let n = Necesidades::por_defecto();
    assert!(n.clase("memoria").is_none());
}

/// Una tabla rota no revienta el compilador: deja los mapas vacios.
#[test]
fn una_tabla_rota_no_tumba_nada() {
    let n = Necesidades::desde_texto("esto ][ no es toml");
    assert_eq!(n.monton_por_defecto(), 4096, "se cae al valor de dentro");
    assert!(n.clase("monton").is_none());
    assert!(n.clases_conocidas().is_empty());
}

#[test]
fn una_clase_que_no_existe_se_contesta_con_la_lista() {
    let n = Necesidades::por_defecto();
    assert!(n.clase("mont0n").is_none());
    assert!(n.clases_conocidas().contains(&"monton"));
}
