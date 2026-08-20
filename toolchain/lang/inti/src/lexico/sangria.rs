//! `lexico::sangria` -- la regla del margen, aparte de todo lo demas.
//!
//! ## Por que esto merece un fichero para si
//!
//! Porque **es el sitio exacto donde se rompen los lenguajes con bloques por
//! sangria**, y porque es la unica parte del lexer que tiene ESTADO. El resto
//! del barrido mira caracteres y decide; esto mantiene una pila y decide segun
//! lo que paso antes. Mezclar las dos cosas en un bucle es como se acaba con un
//! lexer al que nadie se atreve a tocar el margen.
//!
//! Metido aparte, la regla entera se puede leer de una vez y probar sin lexer:
//! entra una lista de anchos y salen `Sangra`/`Desangra`.
//!
//! ## La regla, dicha entera
//!
//! ```text
//!    ancho de un nivel .... CUATRO espacios, exactos
//!    tabulador ............ E0010, y no es negociable
//!    ancho no multiplo .... E0012
//!    entrar dos de golpe .. E0012 (una sangria salta UN nivel)
//!    salir a un ancho que
//!      nadie abrio ........ E0012
//! ```
//!
//! Lo de los tabuladores no es tiquismiquis: dos anchos que se ven iguales y
//! valen distinto es una sorpresa, y `INTI_MAESTRO.md` P2 dice que si dos cosas
//! se ven iguales tienen que comportarse igual. La forma barata de cumplirlo es
//! que solo haya una forma de sangrar.

use crate::aviso::{codigos, Aviso, Sitio};
use crate::lexico::pieza::Clase;

/// Los espacios que vale un nivel. Constante y no ajuste: dos proyectos con
/// anchos distintos serian dos lenguajes que se leen distinto.
pub const ANCHO: usize = 4;

/// La pila de margenes abiertos.
#[derive(Debug, Clone)]
pub struct Sangrador {
    /// Anchos abiertos, siempre creciendo. El 0 esta desde el principio y no
    /// se saca nunca: es el margen del fichero.
    niveles: Vec<usize>,
}

impl Default for Sangrador {
    fn default() -> Self {
        Self::nuevo()
    }
}

impl Sangrador {
    pub fn nuevo() -> Self {
        Self { niveles: vec![0] }
    }

    /// El margen abierto ahora mismo.
    pub fn margen(&self) -> usize {
        *self.niveles.last().unwrap_or(&0)
    }

    /// Cuantos bloques hay abiertos. Sirve para cerrar al final del fichero.
    pub fn profundidad(&self) -> usize {
        self.niveles.len() - 1
    }

    /// Mide la sangria de una linea con contenido y dice que piezas salen.
    ///
    /// `linea_fuente` entra para poder poner el dedo en el aviso: sin ella el
    /// mensaje cumpliria tres de las cuatro partes, y tres no valen.
    pub fn medir(
        &mut self,
        ancho: usize,
        sitio: Sitio,
        linea_fuente: &str,
    ) -> (Vec<Clase>, Vec<Aviso>) {
        let mut piezas = Vec::new();
        let mut avisos = Vec::new();
        let actual = self.margen();

        if ancho == actual {
            return (piezas, avisos);
        }

        if ancho > actual {
            // Entrar. Solo se entra UN nivel, y solo del ancho exacto.
            if ancho != actual + ANCHO {
                avisos.push(
                    Aviso::nuevo(
                        codigos::SANGRIA_RARA,
                        "Esta linea entra mas de un nivel de golpe.",
                        sitio,
                    )
                    .con_linea(linea_fuente)
                    .con_habia(format!(
                        "Tiene {} espacios y el bloque de fuera va por {}: el siguiente nivel son {}.",
                        ancho,
                        actual,
                        actual + ANCHO
                    ))
                    .con_hacer(format!("deja {} espacios al principio de la linea", actual + ANCHO)),
                );
            }
            // Se entra igual, con el ancho que traiga: seguir contando desde el
            // ancho real evita que un solo error de margen convierta el resto
            // del fichero en un rio de avisos.
            self.niveles.push(ancho);
            piezas.push(Clase::Sangra);
            return (piezas, avisos);
        }

        // Salir. Se cierran tantos bloques como haga falta.
        while self.margen() > ancho {
            self.niveles.pop();
            piezas.push(Clase::Desangra);
        }

        if self.margen() != ancho {
            // Se salio a un ancho que nadie abrio: entre dos niveles.
            avisos.push(
                Aviso::nuevo(
                    codigos::SANGRIA_RARA,
                    "Esta linea no vuelve a ningun bloque abierto.",
                    sitio,
                )
                .con_linea(linea_fuente)
                .con_habia(format!(
                    "Tiene {} espacios, y los bloques abiertos van por {}.",
                    ancho,
                    self.margen()
                ))
                .con_hacer(format!("deja {} espacios al principio de la linea", self.margen())),
            );
            // Se acepta el ancho raro para no arrastrar el fallo.
            self.niveles.push(ancho);
            piezas.push(Clase::Sangra);
        }

        (piezas, avisos)
    }

    /// Cierra todo lo que quede abierto al acabar el fichero.
    pub fn cerrar(&mut self) -> Vec<Clase> {
        let mut piezas = Vec::new();
        while self.niveles.len() > 1 {
            self.niveles.pop();
            piezas.push(Clase::Desangra);
        }
        piezas
    }
}

/// Cuenta el margen de una linea y denuncia el tabulador.
///
/// Devuelve `(ancho, resto, avisos)`. El tabulador **no se convierte a
/// espacios**: convertirlo seria elegir un ancho por el autor, que es
/// exactamente la sorpresa que la regla evita.
pub fn medir_margen(linea: &str, numero_de_linea: usize) -> (usize, &str, Vec<Aviso>) {
    let mut avisos = Vec::new();
    let mut ancho = 0usize;
    let mut corte = 0usize;

    for (i, c) in linea.char_indices() {
        match c {
            ' ' => {
                ancho += 1;
                corte = i + 1;
            }
            '\t' => {
                if avisos.is_empty() {
                    avisos.push(
                        Aviso::nuevo(
                            codigos::TABULADOR,
                            "Hay un tabulador donde va la sangria.",
                            Sitio::nuevo(numero_de_linea, ancho + 1),
                        )
                        .con_linea(linea)
                        .con_habia(
                            "La sangria de INTI son cuatro espacios, siempre. Un tabulador se ve \
                             igual en tu editor y distinto en el mio."
                                .to_string(),
                        )
                        .con_hacer("cambia el tabulador por cuatro espacios"),
                    );
                }
                // Se cuenta como cuatro para poder seguir leyendo el fichero y
                // dar el resto de avisos de una pasada.
                ancho += ANCHO;
                corte = i + 1;
            }
            _ => break,
        }
    }

    (ancho, &linea[corte..], avisos)
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn clases(v: &[Clase]) -> Vec<&'static str> {
        v.iter()
            .map(|c| match c {
                Clase::Sangra => "entra",
                Clase::Desangra => "sale",
                _ => "otra",
            })
            .collect()
    }

    #[test]
    fn entrar_y_salir_un_nivel() {
        let mut s = Sangrador::nuevo();
        let (p, a) = s.medir(4, Sitio::default(), "    x");
        assert_eq!(clases(&p), vec!["entra"]);
        assert!(a.is_empty());

        let (p, a) = s.medir(0, Sitio::default(), "x");
        assert_eq!(clases(&p), vec!["sale"]);
        assert!(a.is_empty());
        assert_eq!(s.profundidad(), 0);
    }

    /// Salir de tres niveles de golpe cierra los tres. Es lo que pasa al final
    /// de una funcion con un `si` dentro de un `para`.
    #[test]
    fn salir_de_varios_a_la_vez() {
        let mut s = Sangrador::nuevo();
        s.medir(4, Sitio::default(), "");
        s.medir(8, Sitio::default(), "");
        s.medir(12, Sitio::default(), "");
        let (p, a) = s.medir(0, Sitio::default(), "");
        assert_eq!(clases(&p), vec!["sale", "sale", "sale"]);
        assert!(a.is_empty());
    }

    #[test]
    fn la_misma_sangria_no_dice_nada() {
        let mut s = Sangrador::nuevo();
        s.medir(4, Sitio::default(), "");
        let (p, a) = s.medir(4, Sitio::default(), "");
        assert!(p.is_empty());
        assert!(a.is_empty());
    }

    #[test]
    fn entrar_dos_niveles_de_golpe_se_denuncia() {
        let mut s = Sangrador::nuevo();
        let (p, a) = s.medir(8, Sitio::default(), "        x");
        assert_eq!(clases(&p), vec!["entra"], "tiene que entrar igual");
        assert_eq!(a.len(), 1);
        assert_eq!(a[0].codigo, codigos::SANGRIA_RARA);
        assert!(a[0].que_hacer.contains('4'), "la sugerencia dice el ancho");
    }

    /// Volver a un margen intermedio que nadie abrio.
    #[test]
    fn salir_a_un_ancho_que_nadie_abrio() {
        let mut s = Sangrador::nuevo();
        s.medir(4, Sitio::default(), "");
        s.medir(8, Sitio::default(), "");
        let (_, a) = s.medir(6, Sitio::default(), "      x");
        assert_eq!(a.len(), 1);
        assert_eq!(a[0].codigo, codigos::SANGRIA_RARA);
    }

    #[test]
    fn el_final_cierra_lo_que_quede() {
        let mut s = Sangrador::nuevo();
        s.medir(4, Sitio::default(), "");
        s.medir(8, Sitio::default(), "");
        assert_eq!(clases(&s.cerrar()), vec!["sale", "sale"]);
        assert_eq!(s.profundidad(), 0);
    }

    #[test]
    fn el_tabulador_se_denuncia_una_vez_y_se_sigue() {
        let (ancho, resto, avisos) = medir_margen("\t\tescribe", 7);
        assert_eq!(ancho, 8, "se cuenta como cuatro para poder seguir");
        assert_eq!(resto, "escribe");
        assert_eq!(avisos.len(), 1, "un aviso por linea, no uno por tabulador");
        assert_eq!(avisos[0].codigo, codigos::TABULADOR);
        assert_eq!(avisos[0].sitio.linea, 7);
    }

    #[test]
    fn el_margen_de_espacios_se_mide_bien() {
        let (ancho, resto, avisos) = medir_margen("      x = 1", 1);
        assert_eq!(ancho, 6);
        assert_eq!(resto, "x = 1");
        assert!(avisos.is_empty(), "medir no juzga: eso lo hace el Sangrador");
    }
}
