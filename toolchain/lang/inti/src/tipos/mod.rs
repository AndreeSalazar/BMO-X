//! `tipos` -- que dos cosas se puedan operar juntas.
//!
//! ## Que problema resuelve, dicho con el sintoma
//!
//! Hasta F6a, esto compilaba y corria:
//!
//! ```text
//!    funcion f(a es flotante64, b es entero64) devuelve flotante64
//!        devuelve a + b
//! ```
//!
//! Y hacia aritmetica de coma flotante **sobre los bits de un entero**. `b`
//! valiendo 5 no entra ahi como `5.0`: entra como `2,47e-323`, que es un
//! flotante perfectamente valido. El programa no se rompe: **contesta otra
//! cosa**, y sigue.
//!
//! Es el mismo agujero que F5b cerro en los campos y F5d en la tabla de la
//! maquina, visto una vez mas: **algo que compila y no hace lo que dice**.
//!
//! ## ** Por que es un modulo aparte y no unas lineas en `disposicion`
//!
//! Por el criterio de siempre: son dos trabajos.
//!
//! ```text
//!    disposicion   dice QUE TIPO es cada cosa   -> una respuesta
//!    tipos         dice si eso CUADRA           -> avisos
//! ```
//!
//! `disposicion` contesta una pregunta y no juzga; este juzga y no calcula
//! nada. Si el que mide denunciara, cada medida nueva traeria una politica
//! nueva pegada -- y el dia que la politica cambie habria que tocar al que mide.
//!
//! ## ** LO QUE ESTE MODULO NO HACE, y es lo que lo hace util
//!
//! **No infiere.** Trabaja solo con lo que esta ESCRITO -- los parametros y las
//! declaraciones con tipo. Lo que no consta no se denuncia, y eso es una
//! decision, no una carencia:
//!
//! > Un aviso que salta de mas se desactiva en una semana, y entonces ya no
//! > vigila nada.
//!
//! Es la misma leccion que `tests/agnostico.rs` aprendio acusando a `conversion`
//! de nombrar un registro.
//!
//! ## Las tres cosas que si mira
//!
//! ```text
//!    1. mezclar clases        `flotante64 + entero64`   -> E0022
//!    2. condicion no logica   `si a` con `a` entero      -> E0040
//!    3. asignar a un tipo     `x es flotante64 = a`      -> E0022
//! ```
//!
//! ## OJO: Y lo que NO es un error, dicho por delante
//!
//! **Un literal no tiene tipo todavia.** `a * 2` con `a` de coma flotante es
//! correcto: el `2` no es "un entero que se convierte", es un numero que aun no
//! ha elegido forma. Eso no es la conversion implicita que prohibe el censo
//! `v05` -- alli hay DOS tipos escritos, y aqui hay uno y un literal.
//!
//! **Mezclar anchos dentro de la misma clase tampoco.** `entero64 + natural8`
//! pasa, y no por descuido: INTI opera en el ancho de la maquina y ninguno de
//! los dos deja de ser un numero por el camino. Lo que produce basura es
//! mezclar CLASES, porque entonces los mismos bytes se leen con dos alfabetos.
//! El dia que haya que apretar los anchos, es una fila mas en esta lista y no
//! un modulo nuevo.

use std::collections::HashMap;

use crate::arbol::{Bloque, Clase, Decl, Expr, Funcion, Modulo, Op, Repeticion, Sent, Tipo};
use crate::aviso::{codigos, Aviso, Cosecha, Sitio};
use crate::disposicion::{es_de_comparar, tipos_de, Plano};

/// Comprueba que lo que se opera junto se pueda operar junto.
pub fn comprobar(m: &Modulo, plano: &Plano) -> Cosecha<()> {
    let mut avisos = Vec::new();

    // ** Solo en `llano`, por lo mismo que `disposicion`: en `pleno` un valor
    // puede cambiar de forma en ejecucion, y medirlo con estas reglas
    // denunciaria programas correctos. El dia que `pleno` tenga su modelo, esta
    // puerta se abre; esta arriba y entera para que se vea, no repartida en
    // `if`s por dentro.
    if !matches!(m.perfil, crate::arbol::Perfil::Llano) {
        return Cosecha::con((), avisos);
    }

    for d in &m.declaraciones {
        match d {
            Decl::Funcion(f) => revisa(f, plano, &mut avisos),
            Decl::Operacion { funcion, .. } => revisa(funcion, plano, &mut avisos),
            Decl::Registro { operaciones, .. } => {
                for f in operaciones {
                    revisa(f, plano, &mut avisos);
                }
            }
            Decl::Constante { .. } => {}
        }
    }

    Cosecha::con((), avisos)
}

fn revisa(f: &Funcion, plano: &Plano, avisos: &mut Vec<Aviso>) {
    let tipos = tipos_de(f);
    let mut v = Revision {
        plano,
        tipos: &tipos,
        avisos,
    };
    v.bloque(&f.cuerpo);
}

struct Revision<'a> {
    plano: &'a Plano,
    tipos: &'a HashMap<String, Tipo>,
    avisos: &'a mut Vec<Aviso>,
}

impl Revision<'_> {
    fn bloque(&mut self, b: &Bloque) {
        for s in b {
            self.sentencia(s);
        }
    }

    fn sentencia(&mut self, s: &Sent) {
        match s {
            Sent::Asigna {
                destino,
                tipo,
                valor,
                sitio,
                ..
            } => {
                self.expresion(destino);
                self.expresion(valor);
                self.mira_asignacion(destino, tipo.as_ref(), valor, *sitio);
            }
            Sent::Si { ramas, sino, .. } => {
                for (cond, cuerpo) in ramas {
                    self.expresion(cond);
                    self.mira_condicion(cond);
                    self.bloque(cuerpo);
                }
                if let Some(c) = sino {
                    self.bloque(c);
                }
            }
            Sent::Repite { forma, cuerpo, .. } => {
                if let Repeticion::Mientras(cond) = forma {
                    self.expresion(cond);
                    self.mira_condicion(cond);
                }
                self.bloque(cuerpo);
            }
            Sent::Devuelve { valor: Some(e), .. } => self.expresion(e),
            Sent::Expresion(e) => self.expresion(e),
            Sent::Crudo { cuerpo, .. } => self.bloque(cuerpo),
            Sent::Paralelo { cuerpo, .. } => self.bloque(cuerpo),
            Sent::ParaCada { cuerpo, .. } => self.bloque(cuerpo),
            _ => {}
        }
    }

    fn expresion(&mut self, e: &Expr) {
        match e {
            Expr::Binaria {
                op,
                izquierda,
                derecha,
                sitio,
            } => {
                self.expresion(izquierda);
                self.expresion(derecha);
                self.mira_operacion(*op, izquierda, derecha, *sitio);
            }
            Expr::Unaria { valor, .. } => self.expresion(valor),
            Expr::Llamada { que, argumentos, .. } => {
                self.expresion(que);
                for a in argumentos {
                    self.expresion(&a.valor);
                }
            }
            Expr::Indice { que, indice, .. } => {
                self.expresion(que);
                self.expresion(indice);
            }
            Expr::Campo { que, .. } => self.expresion(que),
            _ => {}
        }
    }

    /// **LA COMPROBACION QUE IMPORTA**: los dos lados de una operacion tienen
    /// que ser de la misma aritmetica.
    ///
    /// ** Y solo se denuncia cuando los DOS constan. Si de uno no se sabe nada
    /// --un literal, un nombre sin tipo escrito-- no hay dos tipos que
    /// discrepen: hay uno.
    fn mira_operacion(&mut self, op: Op, izq: &Expr, der: &Expr, sitio: Sitio) {
        // Comparar mezclado es el mismo problema: `a < b` con uno de cada lee
        // los mismos bytes con dos alfabetos.
        if matches!(op, Op::Y | Op::O | Op::EsUn) {
            return;
        }
        let (Some(a), Some(b)) = (
            self.plano.clase_de(izq, self.tipos),
            self.plano.clase_de(der, self.tipos),
        ) else {
            return;
        };
        if a == b {
            return;
        }
        self.avisos.push(
            Aviso::nuevo(
                codigos::SIN_CONVERSION,
                "Aqui se mezclan un numero de coma flotante y uno entero.".to_string(),
                sitio,
            )
            .con_habia(
                "INTI no convierte por su cuenta. Los ocho bytes de un flotante y los de un \
                 entero son los mismos ocho bytes leidos de dos formas distintas, asi que \
                 operarlos juntos no da un numero raro: da OTRO numero, y el programa sigue."
                    .to_string(),
            )
            .con_hacer(if matches!(a, Clase::Flotante) {
                "pide la conversion por su nombre: `flotante64(...)` sobre el otro lado"
            } else {
                "pide la conversion por su nombre: `flotante64(...)` sobre este lado"
            }),
        );
    }

    /// **SIN VERACIDAD**: una condicion es un `logico`, no "algo que no es cero".
    ///
    /// ** Es la sorpresa de Python que INTI quita a proposito. Alli `si lista`
    /// pregunta si esta vacia, `si 0` es falso y `si "0"` es cierto -- tres
    /// reglas distintas que hay que recordar. Aqui una condicion es una
    /// pregunta, y una pregunta se escribe.
    fn mira_condicion(&mut self, e: &Expr) {
        if self.es_logico(e) || self.no_consta(e) {
            return;
        }
        self.avisos.push(
            Aviso::nuevo(
                codigos::CONDICION_NO_LOGICA,
                "Esto no es una pregunta, es un numero.".to_string(),
                sitio_de(e),
            )
            .con_habia(
                "En INTI una condicion es un `logico`. No hay valores que cuenten como \
                 ciertos por no ser cero: eso son tres reglas que recordar y una sorpresa \
                 en el sitio donde mas caro sale."
                    .to_string(),
            )
            .con_hacer("escribe la pregunta entera: `si x no es 0`"),
        );
    }

    /// `x es flotante64 = a` con `a` entero.
    fn mira_asignacion(
        &mut self,
        destino: &Expr,
        tipo: Option<&Tipo>,
        valor: &Expr,
        sitio: Sitio,
    ) {
        // El tipo del destino: el que se escribio aqui, o el que ya tenia.
        let esperado = match tipo {
            Some(t) => self.plano.clase_del_tipo(t),
            None => self.plano.clase_de(destino, self.tipos),
        };
        let (Some(esperado), Some(dado)) = (esperado, self.plano.clase_de(valor, self.tipos))
        else {
            return;
        };
        if esperado == dado {
            return;
        }
        let (que, como) = if matches!(esperado, Clase::Flotante) {
            (
                "Aqui se guarda un entero donde va un numero de coma flotante.",
                "pide la conversion por su nombre: `flotante64(...)`",
            )
        } else {
            (
                "Aqui se guarda un numero de coma flotante donde va un entero.",
                "pide la conversion por su nombre: `entero64(...)`, que ademas atrapa si no cabe",
            )
        };
        self.avisos.push(
            Aviso::nuevo(codigos::SIN_CONVERSION, que.to_string(), sitio)
                .con_habia(
                    "INTI no convierte por su cuenta. Los mismos bytes leidos con la otra \
                     aritmetica son otro numero, no un numero aproximado."
                        .to_string(),
                )
                .con_hacer(como),
        );
    }

    /// Esto produce un `logico`?
    fn es_logico(&self, e: &Expr) -> bool {
        match e {
            Expr::Logico(_, _) => true,
            Expr::Binaria { op, .. } => es_de_comparar(*op),
            Expr::Unaria { op, .. } => matches!(op, crate::arbol::OpUno::No),
            _ => matches!(
                self.plano.tipo_de(e, self.tipos),
                Some(Tipo::Nombre(ref n)) if n == "logico"
            ),
        }
    }

    /// De esto no se sabe el tipo, asi que no se denuncia.
    ///
    /// ** Una llamada a una funcion del usuario cae aqui: su tipo de retorno no
    /// se resuelve todavia. Denunciar `si hay_tecla()` seria denunciar un
    /// programa correcto, que es como se desactiva un aviso.
    fn no_consta(&self, e: &Expr) -> bool {
        match e {
            Expr::Llamada { .. } => true,
            _ => self.plano.tipo_de(e, self.tipos).is_none(),
        }
    }
}

/// El sitio de una expresion, para poder senalarla.
fn sitio_de(e: &Expr) -> Sitio {
    match e {
        Expr::Numero(_, s)
        | Expr::Texto(_, s)
        | Expr::Logico(_, s)
        | Expr::Nada(s)
        | Expr::Nombre(_, s)
        | Expr::Tipo(_, s)
        | Expr::Lista(_, s)
        | Expr::Tabla(_, s) => *s,
        Expr::Binaria { sitio, .. }
        | Expr::Unaria { sitio, .. }
        | Expr::Llamada { sitio, .. }
        | Expr::Indice { sitio, .. }
        | Expr::Campo { sitio, .. } => *sitio,
        _ => Sitio::default(),
    }
}

#[cfg(test)]
mod pruebas;
