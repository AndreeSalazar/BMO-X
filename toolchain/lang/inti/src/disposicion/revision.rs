//! `disposicion::revision` -- comprobar cada USO, que es otra pregunta.
//!
//! ## El corte (L6a, 2026-08-23)
//!
//! ```text
//!    disposicion   MEDIR: cuanto ocupa un tipo, donde cae cada campo
//!    revision      COMPROBAR: este `.campo` existe? este indice se sale?
//! ```
//!
//! ** Y las dos mitades tienen PERFILES DISTINTOS desde el 23-08, que es la
//! prueba de que son dos cosas: medir no infiere nada y vale en los dos
//! perfiles; comprobar un `.campo` necesita el tipo de quien lo pide, y en
//! `pleno` ese tipo se deduce.
//!
//! *** Confundirlas es lo que tenia parado a `pleno`: se juzgaba con las reglas
//! de `llano` un modulo que no lo era.

use super::*;

pub(super) fn revisa_funcion(f: &Funcion, plano: &Plano, avisos: &mut Vec<Aviso>, exige_tipo: bool) {
    let tipos = tipos_de_con(f, Some(plano));
    let mut v = Revision {
        plano,
        tipos: &tipos,
        avisos,
        dentro_de_crudo: false,
        exige_tipo,
    };
    v.bloque(&f.cuerpo);
}

struct Revision<'a> {
    plano: &'a Plano,
    tipos: &'a HashMap<String, Tipo>,
    avisos: &'a mut Vec<Aviso>,
    /// ** Indexar un bufer pide `crudo`, y hay que llevar la cuenta aqui porque
    /// `perfil` --que es quien la lleva para los nombres-- no sabe cuales de
    /// estos indices son bufers y cuales no. Saberlo pide el plano, y el plano
    /// es de este modulo.
    dentro_de_crudo: bool,
    /// **Puede este perfil exigir que un tipo este escrito?** (2026-08-23)
    ///
    /// `llano` si: alli los tipos son obligatorios y no decirlos ya es `E0020`.
    /// `pleno` **no**: son opcionales y lo que no se deduce hoy se deducira
    /// cuando la deduccion crezca.
    ///
    /// *** La diferencia no es de severidad, es de QUIEN TIENE LA CULPA. Un
    /// tipo que falta en `llano` es del programa. Uno que falta en `pleno` es
    /// del compilador, y acusar al usuario de una carencia propia es la forma
    /// mas facil de que un lenguaje parezca caprichoso.
    ///
    /// [!] Y lo que NO cambia con esto: si el tipo SI se sabe, `pleno` comprueba
    /// igual que `llano`. Se calla lo que no sabe, no lo que sabe.
    exige_tipo: bool,
}

impl Revision<'_> {
    fn bloque(&mut self, b: &Bloque) {
        for s in b {
            self.sentencia(s);
        }
    }

    fn sentencia(&mut self, s: &Sent) {
        match s {
            Sent::Asigna { destino, valor, .. } => {
                self.expresion(destino);
                self.expresion(valor);
            }
            Sent::Si { ramas, sino, .. } => {
                for (cond, cuerpo) in ramas {
                    self.expresion(cond);
                    self.bloque(cuerpo);
                }
                if let Some(c) = sino {
                    self.bloque(c);
                }
            }
            Sent::Repite { forma, cuerpo, .. } => {
                match forma {
                    Repeticion::Mientras(c) | Repeticion::Veces(c) => self.expresion(c),
                    Repeticion::Siempre => {}
                }
                self.bloque(cuerpo);
            }
            Sent::ParaCada {
                desde,
                hasta,
                cuerpo,
                ..
            } => {
                self.expresion(desde);
                if let Some(h) = hasta {
                    self.expresion(h);
                }
                self.bloque(cuerpo);
            }
            Sent::Crudo { cuerpo, .. } => {
                let antes = self.dentro_de_crudo;
                self.dentro_de_crudo = true;
                self.bloque(cuerpo);
                self.dentro_de_crudo = antes;
            }
            Sent::Paralelo { cuerpo, .. } => self.bloque(cuerpo),
            Sent::Devuelve { valor: Some(e), .. } => self.expresion(e),
            Sent::Expresion(e) => self.expresion(e),
            Sent::Falla { motivo, .. } => self.expresion(motivo),
            _ => {}
        }
    }

    fn expresion(&mut self, e: &Expr) {
        match e {
            Expr::Campo { que, nombre, sitio } => {
                self.expresion(que);
                self.mira_campo(que, nombre, *sitio);
            }
            Expr::Indice { que, indice, sitio } => {
                self.expresion(que);
                self.expresion(indice);
                self.mira_indice(que, *sitio);
            }
            Expr::Binaria {
                op,
                izquierda,
                derecha,
                sitio,
            } => {
                self.expresion(izquierda);
                self.expresion(derecha);
                self.mira_operacion(*op, e, *sitio);
            }
            Expr::Unaria { valor, .. } => self.expresion(valor),
            Expr::Llamada { que, argumentos, .. } => {
                self.expresion(que);
                for a in argumentos {
                    self.expresion(&a.valor);
                }
            }
            _ => {}
        }
    }

    fn tipo_de(&self, e: &Expr) -> Option<Tipo> {
        self.plano.tipo_de(e, self.tipos)
    }

    /// Esta operacion, existe para lo que se le esta dando?
    ///
    /// ** Solo hay una familia que no: **los bits sobre un flotante**. Y no es
    /// una carencia del emisor que ya se anadira -- es que la pregunta no tiene
    /// sentido. Los ocho bytes de un `flotante64` son signo, exponente y
    /// mantisa; `f | 1` no enciende el bit de las unidades de nada, toca el
    /// exponente y devuelve un numero que no se parece a ninguno de los dos.
    ///
    /// El resto SI existen: sumar, restar, multiplicar, dividir y las seis
    /// comparaciones estan todas en IEEE-754 con su resultado escrito.
    fn mira_operacion(&mut self, op: crate::arbol::Op, e: &Expr, sitio: Sitio) {
        use crate::arbol::Op;
        let de_bits = matches!(
            op,
            Op::BitsY
                | Op::BitsO
                | Op::BitsXor
                | Op::DesplazaIzquierda
                | Op::DesplazaDerecha
                | Op::Resto
                | Op::Entre
        );
        if !de_bits || !self.plano.es_flotante(e, self.tipos) {
            return;
        }
        self.avisos.push(
            Aviso::nuevo(
                codigos::FLOTANTE_SIN_BITS,
                "Esta operacion no existe para un numero de coma flotante.".to_string(),
                sitio,
            )
            .con_habia(
                "Los ocho bytes de un flotante son signo, exponente y mantisa, no un                  numero en binario. Operarlos a bits no toca lo que parece que toca."
                    .to_string(),
            )
            // *** ESTE CONSEJO ERA FALSO HASTA EL 2026-08-24.
            //
            // Decia *"convierte a entero primero si lo que quieres son los
            // bits"*, y `entero64(3.5)` da **3**: convierte el VALOR. Los bits
            // no se podian obtener por ningun camino, asi que el aviso mandaba
            // a hacer algo que no hace lo que dice.
            //
            // ** Un consejo equivocado es peor que ninguno: manda a buscar por
            // donde no es, y quien lo siga obtiene un numero pequeno donde
            // esperaba un patron -- sin que nada se queje.
            .con_hacer(
                "usa `/` para dividir. Y si lo que quieres son los OCHO BYTES,                  eso es `bits_de(x)` -- no `entero64(x)`, que convierte el valor",
            ),
        );
    }

    fn mira_campo(&mut self, que: &Expr, nombre: &str, sitio: Sitio) {
        let Some(t) = self.tipo_de(que) else {
            // ** En `pleno`, no saberlo NO es una acusacion. Ver `exige_tipo`.
            if !self.exige_tipo {
                return;
            }
            self.avisos.push(
                Aviso::nuevo(
                    codigos::SIN_MEDIDA,
                    format!("No se sabe de que tipo es esto, asi que `.{}` no se puede resolver.", nombre),
                    sitio,
                )
                .con_habia(
                    "Un campo es una direccion mas un desplazamiento, y el desplazamiento sale \
                     del tipo. Sin el tipo escrito no hay desplazamiento que valga."
                        .to_string(),
                )
                .con_hacer("declara el tipo: `p es Punto`"),
            );
            return;
        };
        let Tipo::Nombre(r) = &t else {
            self.avisos.push(
                Aviso::nuevo(
                    codigos::CAMPO_DESCONOCIDO,
                    format!("Esto no es un registro, asi que no tiene `.{}`.", nombre),
                    sitio,
                )
                .con_hacer("los campos solo existen en lo que declara `registro`"),
            );
            return;
        };
        let Some(reg) = self.plano.registro(r) else {
            self.avisos.push(
                Aviso::nuevo(
                    codigos::CAMPO_DESCONOCIDO,
                    format!("`{}` no es un registro declarado en este fichero.", r),
                    sitio,
                )
                .con_hacer("declaralo con `registro`, o revisa el nombre"),
            );
            return;
        };
        if reg.campo(nombre).is_none() {
            let tiene: Vec<&str> = reg.campos().iter().map(|(n, _)| n.as_str()).collect();
            self.avisos.push(
                Aviso::nuevo(
                    codigos::CAMPO_DESCONOCIDO,
                    format!("`{}` no tiene ningun campo `{}`.", r, nombre),
                    sitio,
                )
                .con_habia(format!("Tiene: {}.", tiene.join(", ")))
                .con_hacer("revisa el nombre del campo"),
            );
        }
    }

    fn mira_indice(&mut self, que: &Expr, sitio: Sitio) {
        let Some(t) = self.tipo_de(que) else {
            self.avisos.push(
                Aviso::nuevo(
                    codigos::SIN_MEDIDA,
                    "No se sabe de que tipo es esto, asi que no se puede indexar.".to_string(),
                    sitio,
                )
                .con_habia(
                    "Un indice es una direccion mas el numero por LA MEDIDA DEL ELEMENTO. Sin \
                     saber que hay dentro, no hay medida."
                        .to_string(),
                )
                .con_hacer("declara el tipo: `pantalla es bufer de natural32`"),
            );
            return;
        };
        // ** Un bufer NO lleva su longitud, asi que no hay contra que
        // comprobar el indice. No es que la comprobacion se haya olvidado: no
        // existe informacion para hacerla, y por eso esto tiene que ir dentro
        // de `crudo` -- que es justo lo que `crudo` significa.
        //
        // `lista de T` si lleva longitud, y por eso `pleno` la comprueba y no
        // pide `crudo`. La misma regla de siempre: al otro lado, hay alguien
        // que comprueba?
        if self.plano.elemento(&t).is_some() && !self.dentro_de_crudo {
            self.avisos.push(
                Aviso::nuevo(
                    codigos::METAL_SIN_CRUDO,
                    "Indexar un `bufer` tiene que ir dentro de un bloque `crudo`.".to_string(),
                    sitio,
                )
                .con_habia(
                    "Un `bufer` no lleva su longitud dentro, asi que no hay contra que \
                     comprobar el indice. `lista de <tipo>` si la lleva, y esa no pide \
                     `crudo` -- pero es de `pleno`."
                        .to_string(),
                )
                .con_hacer("mete la linea dentro de un bloque `crudo`"),
            );
        }
        // *** Y UNA `lista de T` SE INDEXA, desde el 2026-08-23.
        //
        // Este aviso decia *"`lista de <tipo>` lleva su longitud dentro y es de
        // `pleno`"* -- describiendo un futuro. Ese futuro llego:
        // `runtime/objetos/lista.inti` existe, `sitio_de` compara el indice
        // contra `cuantos`, y el descenso lo usa.
        //
        // ** Se distingue por la FORMA del tipo y no por el perfil: lo que se
        // puede indexar es lo que SABE DONDE ACABA. Un `bufer` no lo sabe --por
        // eso pide `crudo`-- y una lista si.
        let indexable = self.plano.elemento(&t).is_some() || matches!(t, Tipo::Lista(_));
        if !indexable {
            self.avisos.push(
                Aviso::nuevo(
                    codigos::CAMPO_DESCONOCIDO,
                    "Esto no se puede indexar.".to_string(),
                    sitio,
                )
                .con_habia(
                    "Lo que se indexa es un `bufer de <tipo>` --con `crudo`, porque no sabe                      donde acaba-- o una `lista de <tipo>`, que si lo sabe y por eso se                      comprueba."
                        .to_string(),
                )
                .con_hacer("declaralo como `bufer de <tipo>` o como `lista de <tipo>`"),
            );
        }
    }
}
