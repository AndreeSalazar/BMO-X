//! `ir::expresion` -- de una EXPRESION a un valor.
//!
//! ## El tercer corte del mismo modulo, y el criterio no cambia
//!
//! ```text
//!    forma       QUE es una instruccion
//!    descenso    COMO se recorre una FUNCION: sentencias, bloques, saltos
//!    expresion   COMO se calcula un VALOR
//! ```
//!
//! ** La diferencia no es de tamano: una sentencia **no devuelve nada** y una
//! expresion **siempre devuelve un `Valor`**. Esa firma es la frontera, y por
//! eso el corte cae aqui y no en cualquier sitio que sumara mil lineas.
//!
//! *** Y es donde vive lo caro: cada tipo que baja a bytes anade un brazo aqui
//! --`texto + texto` llama a `junta`, `numero + numero` a `suma`, `a[i]` de una
//! lista a `sitio_de`-- asi que este fichero crece con el lenguaje. Tenerlo
//! aparte es lo que hace que crecer no arrastre al recorrido.

use super::*;

impl Descenso<'_> {
    pub(super) fn expresion(&mut self, e: &Expr) -> Valor {
        match e {
            Expr::Numero(n, _) => {
                if n.con_punto {
                    // ** LA MISMA ESCRITURA, DOS VALORES, y lo decide el perfil.
                    //
                    // `llano` no tiene `decimal` --lo prohibe `biblioteca.toml`
                    // por no decir su medida--, asi que alli `3.5` es binario y
                    // se convierte AQUI, una vez, al bajar. `pleno` lo deja en
                    // texto porque su `numero` es decimal exacto y pasarlo por
                    // un binario intermedio lo estropearia sin avisar.
                    match (self.perfil, n.texto.parse::<f64>()) {
                        (crate::arbol::Perfil::Llano, Ok(f)) => {
                            Valor::Const(Const::Flotante(f.to_bits()))
                        }
                        // Si no se deja convertir, se queda como estaba en vez
                        // de inventarle un valor. Un cero aqui compilaria y
                        // daria otra cosa, que es el fallo que F5b acaba de
                        // cerrar en los campos.
                        _ => Valor::Const(Const::Decimal(n.texto.clone())),
                    }
                } else {
                    match parse_entero(&n.texto, n.base) {
                        Some(v) => Valor::Const(Const::Entero(v)),
                        // Un numero que no cabe en `i64` se deja como decimal:
                        // el lenguaje no tiene precision arbitraria, pero
                        // perder el valor aqui seria mentir.
                        None => Valor::Const(Const::Decimal(n.texto.clone())),
                    }
                }
            }
            // *** UN LITERAL DE TEXTO ES UN CONGELADO, y siempre lo fue.
            //
            // Hasta hoy vivia en un "pozo de textos" aparte, y **no eran dos
            // mecanismos**: el pozo existia porque `RoData` no existia. El
            // emisor lo confesaba en su lista de "sin emitir" --*"N texto(s) del
            // pozo no llegan a bytes: `Const::Texto` baja a cero"*-- y ahi
            // llevaba desde F2.
            //
            // La seccion 10.2 del maestro los tenia juntos desde el principio:
            // *"CONGELADO: literales, constantes, un modulo cargado"*.
            //
            // ** Y baja a `Instr::Direccion` por el mismo motivo que una tabla,
            // que quedo escrito el 22-08: un `Const` que necesita una
            // REUBICACION obligaria a los veintitres sitios que cargan un valor
            // a saber apuntarla. Siendo una instruccion, el emisor la atiende en
            // UNO.
            //
            // [!] Los bytes van SIN cabecera. La forma de un objeto la declara
            // `bmo-abi`, y este crate no lo enlaza a proposito -- la cabecera de
            // 24 bytes con el bit de INMORTAL la pone el emisor, que si lo
            // conoce.
            Expr::Texto(t, _) => {
                let i = match self.textos.iter().position(|x| x == t) {
                    Some(i) => i,
                    None => {
                        self.textos.push(t.clone());
                        self.congelados.push(crate::ir::forma::Congelado {
                            nombre: format!("texto {}", self.textos.len() - 1),
                            bytes: t.clone().into_bytes(),
                            ancho: 1,
                            clase: crate::ir::forma::ClaseCongelada::Texto,
                        });
                        self.textos_congelados.push(self.congelados.len() as u32 - 1);
                        self.textos.len() - 1
                    }
                };
                let dst = self.temporal();
                self.pon(Instr::Direccion {
                    destino: dst,
                    congelado: self.textos_congelados[i],
                });
                Valor::Temporal(dst)
            }
            Expr::Logico(b, _) => Valor::Const(Const::Logico(*b)),
            Expr::Nada(_) => Valor::Const(Const::Nada),
            Expr::Nombre(n, _) => match self.busca_local(n) {
                Some(l) => Valor::Local(l),
                // ** UNA CONSTANTE DEL MODULO, y va antes que la del ABI porque
                // es de ESTE fichero: lo de dentro tapa a lo de fuera, que es lo
                // que espera cualquiera que lea el fuente de arriba abajo.
                //
                // *** Hasta el 2026-08-22 esto no existia: `Decl::Constante` se
                // tiraba en `bajar_con` con un `{}`, el nombre llegaba al emisor
                // suelto, y `carga` lo bajaba a un CERO. O sea que
                // `maximo = 100` compilaba limpio, pasaba el gate, salia firmado
                // **y valia cero** -- con su ejemplo en `GRAMATICA.md`.
                None if self.congeladas.contains_key(n) => {
                    Valor::Const(self.congeladas[n].clone())
                }
                // ** Una TABLA congelada no cabe en un inmediato: lo que se
                // carga es su DIRECCION, y eso es una instruccion.
                None if self.tablas.contains_key(n) => {
                    let t = self.temporal();
                    self.pon(Instr::Direccion {
                        destino: t,
                        congelado: self.tablas[n],
                    });
                    Valor::Temporal(t)
                }
                // ** Una constante del ABI se resuelve AQUI y no en el emisor.
                //
                // Es agnostica --`mi_tarea` vale lo mismo en toda maquina--, asi
                // que el sitio donde deja de ser un nombre es este. Bajarla al
                // emisor obligaria a cada emisor nuevo a acordarse de mirar la
                // misma tabla.
                None => match self.tabla.constante(n) {
                    Some(v) => Valor::Const(Const::Entero(v as i64)),
                    None => Valor::Nombre(n.clone()),
                },
            },
            Expr::Tipo(n, _) => Valor::Nombre(n.clone()),
            Expr::Binaria {
                op,
                izquierda,
                derecha,
                sitio,
            } => {
                // ** LA CLASE SALE DE LOS OPERANDOS, no de la expresion entera,
                // y la diferencia importa en un caso concreto: **comparar**.
                //
                //     0.0 / 0.0 = 1.0    la expresion vale un `logico`
                //                        la OPERACION es de coma flotante
                //
                // Preguntando por la expresion, una comparacion de flotantes se
                // bajaba a una comparacion de ENTEROS -- y comparar dos NaN como
                // enteros da que son iguales, que es justo lo contrario de lo
                // que manda IEEE-754.
                //
                // Lo cazaron las pruebas del NaN en cuanto `clase_de` aprendio a
                // contestar "esto no es un numero, es una pregunta". Son dos
                // preguntas distintas y ahora cada una se hace donde toca.
                //
                // Y se pregunta ANTES de bajar los operandos, sobre el arbol:
                // una vez bajados ya no son mas que valores, y un valor no dice
                // de que tipo era.
                let clase = if self.plano.es_flotante(izquierda, &self.tipos)
                    || self.plano.es_flotante(derecha, &self.tipos)
                {
                    Clase::Flotante
                } else {
                    Clase::Entero
                };
                // *** Y SI LLEVA SIGNO, que hasta el 2026-08-23 no se preguntaba
                // NUNCA: el emisor bajaba `setl`, `idiv` y `jo` para todo, asi
                // que `2 < 18446744073709551615` en `natural64` daba FALSO.
                //
                // ** No era comportamiento indefinido -- era peor de encontrar:
                // una respuesta equivocada, en silencio, sin que ninguna de las
                // doce reglas saltara. Las reglas vigilan lo que C deja sin
                // definir; esto estaba definido, y mal.
                //
                // Basta con que UNO de los lados sea `natural`: si `a` es una
                // direccion, `a < b` compara direcciones.
                let sin_signo = self.plano.sin_signo(izquierda, &self.tipos)
                    || self.plano.sin_signo(derecha, &self.tipos);

                // *** `texto + texto` NO ES UNA SUMA: ES UNA LLAMADA (2026-08-23)
                //
                // Bajarlo a un `add` sumaria las DOS DIRECCIONES y devolveria un
                // numero que no apunta a ningun sitio. Compilaria, correria, y
                // daria basura -- la misma familia que el signo de esta misma
                // manana: **una respuesta equivocada, en silencio**.
                //
                // Lo que hace falta es reservar y copiar, porque un `texto` es
                // INMUTABLE: si `a + b` no puede tocar ni `a` ni `b`, el
                // resultado es un TERCER objeto. Eso es `junta`, y esta escrita
                // en INTI en `runtime/objetos/texto.inti`.
                //
                // ** El monton NO viaja en la expresion: un operador no tiene
                // hueco donde llevarlo. Se coge con `Instr::MontonDeLaTarea`,
                // que es ambiente -- como en cualquier lenguaje con objetos.
                // *** `numero + numero` NO ES UNA SUMA DE REGISTROS (2026-08-23).
                //
                // Un `numero` mide 16 bytes --coeficiente `entero64` mas
                // escala-- asi que **no cabe en un registro**. Bajarlo a un
                // `add` sumaria los ocho bytes bajos de cada uno --los
                // coeficientes-- **ignorando las escalas**: `0.1 + 0.2` daria
                // `(3, ?)` con una escala inventada, y `1.5 + 0.25` daria
                // `(40, ?)` en vez de `1.75`.
                //
                // ** Compilaria, correria, y daria otro numero. La familia de
                // siempre.
                //
                // Lo que hace falta es `suma(destino, a, b)` de
                // `runtime/decimal`: iguala escalas SUBIENDO --nunca bajando,
                // que perderia digitos-- y deja el resultado donde se le diga.
                //
                // *** Y el destino es una LOCAL ANONIMA de 16 bytes. No puede
                // ser un temporal: un temporal es una palabra, y esa es toda su
                // definicion.
                if matches!(op, Op::Suma | Op::Por) && self.es_numero(izquierda) && self.es_numero(derecha) {
                    let i = self.expresion(izquierda);
                    let d = self.expresion(derecha);
                    let destino = self.local_anonima(16);
                    let dir = self.temporal();
                    self.pon(Instr::DireccionDeLocal {
                        destino: dir,
                        local: destino,
                    });
                    let t = self.temporal();
                    self.pon(Instr::Llama {
                        destino: Some(t),
                        que: Valor::Nombre(
                            if matches!(op, Op::Suma) { "suma" } else { "multiplica" }.to_string(),
                        ),
                        argumentos: vec![Valor::Temporal(dir), i, d],
                    });
                    // ** Lo que vale la expresion es la DIRECCION del resultado,
                    // no lo que `suma` devolvio --que es un si/no--. Un `numero`
                    // se pasa por direccion en todas partes, y esta es una mas.
                    let r = self.temporal();
                    self.pon(Instr::DireccionDeLocal {
                        destino: r,
                        local: destino,
                    });
                    return Valor::Temporal(r);
                }

                if matches!(op, Op::Suma) && self.es_texto(izquierda) && self.es_texto(derecha) {
                    // [!] IZQUIERDA PRIMERO. La Regla 8 fija el orden de
                    // evaluacion, y este camino no puede tener otro que el de
                    // al lado: si `a` y `b` fueran llamadas con efecto, `a + b`
                    // los haria en distinto orden segun el TIPO de a y b. Eso es
                    // una sorpresa que depende de algo invisible.
                    let i = self.expresion(izquierda);
                    let d = self.expresion(derecha);
                    let m = self.temporal();
                    self.pon(Instr::MontonDeLaTarea { destino: m });
                    let t = self.temporal();
                    self.pon(Instr::Llama {
                        destino: Some(t),
                        que: Valor::Nombre("junta".to_string()),
                        argumentos: vec![Valor::Temporal(m), i, d],
                    });
                    return Valor::Temporal(t);
                }
                let i = self.expresion(izquierda);
                let d = self.expresion(derecha);

                // ** LAS DOS FAMILIAS DE COMPROBACION, y por que van en sitios
                // distintos. Costo un dia entenderlo y explica por que tres de
                // las cuatro no llegaban a bytes:
                //
                //     DESBORDAR   se sabe DESPUES, mirando la bandera que la
                //                 propia operacion dejo puesta
                //     ENTRE CERO  se sabe ANTES, mirando el divisor -- porque
                //                 despues de dividir entre cero ya no hay nada
                //                 que mirar: la maquina se ha llevado el
                //                 programa por delante
                //
                // Ponerlas las dos detras --que es lo que se hacia-- deja la
                // segunda sin nada que comprobar, y entonces o se emite algo
                // que no comprueba o no se emite nada. Se hizo lo segundo, que
                // era lo honesto, y esto es el arreglo de verdad.
                if let Some(c) = comprobacion_antes(*op, clase) {
                    self.pon(Instr::Comprueba {
                        que: c,
                        sobre: d.clone(),
                        contra: None,
                        sin_signo,
                        sitio: *sitio,
                    });
                }

                // *** Y LA OTRA MITAD DE LA DIVISION, que hasta el 2026-08-22 no
                // pedia nadie: `-2^63 entre -1` NO CABE en 64 bits.
                //
                // Es la Regla 1 --un desborde-- pero se escribe con una barra, y
                // de la division solo se comprobaba el divisor. El resultado era
                // un programa que compilaba limpio, salia firmado, y en metal
                // moria con una autopsia del kernel: `idiv` levanta `#DE`, el
                // MISMO vector que dividir entre cero.
                //
                // ** Se pide AQUI, en la IR, y no se emite por su cuenta en el
                // emisor. La diferencia importa: hay una prueba que exige que lo
                // que la IR pide y lo que el binario lleva cuadren, y esa resta
                // es la que dira lo que quito el optimizador el dia que haya uno.
                // Un emisor que anade reglas por su cuenta rompe esa cuenta.
                //
                // Y lleva DOS valores porque el cociente solo se sale cuando el
                // dividendo es el minimo Y el divisor es -1. Es la unica de las
                // cinco que necesita mirar dos.
                if matches!(op, Op::Entre | Op::Divide | Op::Resto)
                    && !matches!(clase, Clase::Flotante)
                {
                    self.pon(Instr::Comprueba {
                        que: Comprobacion::Cociente,
                        sin_signo,
                        sobre: i.clone(),
                        contra: Some(d.clone()),
                        sitio: *sitio,
                    });
                }

                let t = self.temporal();
                self.pon(Instr::Binaria {
                    destino: t,
                    op: *op,
                    clase,
                    sin_signo,
                    izquierda: i,
                    derecha: d,
                });
                if let Some(c) = comprobacion_despues(*op, clase) {
                    self.pon(Instr::Comprueba {
                        que: c,
                        sobre: Valor::Temporal(t),
                        contra: None,
                        sin_signo,
                        sitio: *sitio,
                    });
                }
                Valor::Temporal(t)
            }
            Expr::Unaria { op, valor, .. } => {
                // ** Se pregunta ANTES de bajar, sobre el arbol, por lo mismo
                // que en la binaria: una vez bajado ya no es mas que un valor,
                // y un valor no dice de que tipo era.
                let clase = if self.plano.es_flotante(valor, &self.tipos) {
                    Clase::Flotante
                } else {
                    Clase::Entero
                };
                let v = self.expresion(valor);
                let t = self.temporal();
                self.pon(Instr::Unaria {
                    destino: t,
                    op: *op,
                    clase,
                    valor: v,
                });
                Valor::Temporal(t)
            }
            Expr::Llamada {
                que, argumentos, ..
            } => {
                // ** Es esto una llamada, o es tocar memoria?
                //
                // Lo decide `modulos.toml`, igual que la puerta. Y por el mismo
                // motivo: `lee_natural64` no puede ser una funcion de verdad
                // --seria una llamada por cada byte-- pero tampoco puede ser
                // una palabra del lenguaje, porque entonces un programa que no
                // toca memoria tendria que conocerla.
                // ** Y antes: es esto una CONVERSION?
                //
                // `flotante64(n)` se escribe como una llamada y no lo es. Se
                // mira aqui, antes que nada, porque el nombre de un tipo no
                // puede ser tambien el de una funcion -- lo impide que los
                // tipos vayan en mayuscula y estos no son tipos de usuario, son
                // filas de `medidas.toml`.
                if let Expr::Nombre(n, sitio) = &**que {
                    if self.plano.es_conversion(n) && argumentos.len() == 1 {
                        let hacia = if self.plano.convierte_a_flotante(n) {
                            Clase::Flotante
                        } else {
                            Clase::Entero
                        };
                        let desde = if self.plano.es_flotante(&argumentos[0].valor, &self.tipos) {
                            Clase::Flotante
                        } else {
                            Clase::Entero
                        };
                        let v = self.expresion(&argumentos[0].valor);

                        // ** LA REGLA 12, y va ANTES por lo mismo que la 3:
                        // despues de truncar ya no queda el numero original que
                        // mirar -- queda un entero cualquiera, y el que salio de
                        // 1e30 se parece a uno legitimo.
                        //
                        // Y lleva el ANCHO del destino porque sin el la pregunta
                        // no tiene respuesta: 1e10 cabe en un `entero64` y no
                        // cabe en un `entero32`. Sale del plano, que es quien
                        // mide.
                        if desde == Clase::Flotante && hacia == Clase::Entero {
                            let bytes = self
                                .plano
                                .medida_de(&crate::arbol::Tipo::Nombre(n.clone()))
                                .unwrap_or(8);
                            self.pon(Instr::Comprueba {
                                que: Comprobacion::Conversion(bytes),
                                sobre: v.clone(),
                                contra: None,
                                // La Regla 12 mira un FLOTANTE contra el rango
                                // del entero destino. El signo del destino lo
                                // lleva `bytes`, no esta bandera.
                                sin_signo: false,
                                sitio: *sitio,
                            });
                        }

                        let t = self.temporal();
                        self.pon(Instr::Convierte {
                            destino: t,
                            valor: v,
                            desde,
                            hacia,
                        });
                        // OJO: de entero a flotante NO se comprueba nada, y no
                        // es un olvido. Puede perder PRECISION --un `entero64`
                        // grande no cabe exacto en la mantisa-- pero el
                        // resultado sigue siendo un numero, y eso IEEE-754 lo
                        // define. Solo el otro sentido puede no tener respuesta.
                        return Valor::Temporal(t);
                    }
                }
                // ** ES ESTO UNA INSTRUCCION DE LA MAQUINA?
                //
                // `lee_reloj()` no es una llamada: son dos bytes. Bajarlo como
                // llamada --que es lo que se hacia-- produce un salto a un
                // simbolo que no existe, y **compila**. La tabla de la maquina
                // llevaba desde F2b entera y sin que nadie la leyera al emitir.
                //
                // Va ANTES que `accede` y que la llamada normal porque es lo
                // mas especifico: un nombre que la maquina trae no puede ser
                // ademas otra cosa.
                if let Expr::Nombre(n, _) = &**que {
                    if self.metal.iter().any(|m| m == n) {
                        let args: Vec<Valor> = argumentos
                            .iter()
                            .map(|a| self.expresion(&a.valor))
                            .collect();
                        let t = self.temporal();
                        self.pon(Instr::Metal {
                            destino: Some(t),
                            nombre: n.clone(),
                            argumentos: args,
                        });
                        return Valor::Temporal(t);
                    }
                }
                if let Expr::Nombre(n, _) = &**que {
                    if let Some((hace, ancho)) = self.tabla.accede(n) {
                        let hace = hace.to_string();
                        let mut vs: Vec<Valor> = argumentos
                            .iter()
                            .map(|a| self.expresion(&a.valor))
                            .collect();
                        if hace == "lee" && !vs.is_empty() {
                            let t = self.temporal();
                            self.pon(Instr::Lee {
                                destino: t,
                                direccion: vs.remove(0),
                                ancho,
                            });
                            return Valor::Temporal(t);
                        }
                        if hace == "escribe" && vs.len() >= 2 {
                            let valor = vs.remove(1);
                            self.pon(Instr::Escribe {
                                direccion: vs.remove(0),
                                valor,
                                ancho,
                            });
                            return Valor::Const(Const::Nada);
                        }
                    }
                }

                let q = self.expresion(que);
                let args: Vec<Valor> = argumentos
                    .iter()
                    .map(|a| self.expresion(&a.valor))
                    .collect();
                let t = self.temporal();
                self.pon(Instr::Llama {
                    destino: Some(t),
                    que: q,
                    argumentos: args,
                });
                Valor::Temporal(t)
            }
            Expr::Indice { .. } | Expr::Campo { .. } => {
                // ** Antes esto era el agujero: `p.x` se bajaba a `p` --el campo
                // se ignoraba sin una queja-- y `a[i]` bajaba a la DIRECCION del
                // elemento en vez de a su valor. Compilaba, corria, y hacia
                // otra cosa.
                //
                // Ahora se calcula la direccion con el plano y **se lee**. Un
                // acceso es siempre dos pasos, y antes solo se daba el primero.
                //
                // OJO: un `bufer` no lleva su longitud, asi que aqui no hay
                // `Comprueba::Indice` que valga -- no hay contra que comprobar.
                // Por eso indexarlo pide `crudo`, y por eso `lista de T` (que si
                // la lleva) sera otra cosa cuando llegue `pleno`.
                match self.direccion_de(e) {
                    Some((direccion, ancho)) => {
                        let t = self.temporal();
                        self.pon(Instr::Lee {
                            destino: t,
                            direccion,
                            ancho,
                        });
                        Valor::Temporal(t)
                    }
                    // ** Y si NO se supo la disposicion, un indice sigue
                    // trayendo su comprobacion.
                    //
                    // Casi se pierde aqui: al enchufar el plano, este camino se
                    // quedo sin emitir nada y con el se fue la Regla 2 --*un
                    // indice SIEMPRE se comprueba*-- sin que nadie la borrara.
                    // La cazo su propio test, que es para lo que estaba.
                    //
                    // Lo que se indexa sin disposicion conocida es una `lista de
                    // T` de `pleno`, y esa SI lleva su longitud dentro: tiene
                    // contra que comprobar, y por eso no pide `crudo`.
                    None => match e {
                        // *** INDEXAR UNA `lista de T`: LA REGLA 2, DE VERDAD
                        // (2026-08-23).
                        //
                        // Baja a `sitio_de(l, i, ancho)`, que compara el indice
                        // contra `cuantos` --que vive a un `mov` de distancia en
                        // la cabecera de la lista-- y devuelve **0 si se sale**.
                        // El `Comprueba` de detras convierte ese 0 en la trampa
                        // `E1002`.
                        //
                        // ** Hasta hoy este camino sumaba la direccion y el
                        // indice a pelo y pedia un `Comprueba::Indice` que **el
                        // emisor tiraba a la basura**: no emitia nada y ademas
                        // se descontaba del recuento. La regla estaba escrita,
                        // pedida, y no llegaba a un solo byte.
                        Expr::Indice { que, indice, sitio } if self.ancho_de_lista(que).is_some() => {
                            let ancho = self.ancho_de_lista(que).unwrap();
                            let l = self.expresion(que);
                            let i = self.expresion(indice);
                            let dir = self.temporal();
                            self.pon(Instr::Llama {
                                destino: Some(dir),
                                que: Valor::Nombre("sitio_de".to_string()),
                                argumentos: vec![
                                    l,
                                    i,
                                    Valor::Const(Const::Entero(ancho as i64)),
                                ],
                            });
                            self.pon(Instr::Comprueba {
                                que: Comprobacion::Indice,
                                sobre: Valor::Temporal(dir),
                                contra: None,
                                // Una direccion no lleva signo, y esta se mira
                                // contra cero: da igual, pero se dice.
                                sin_signo: true,
                                sitio: *sitio,
                            });
                            let t = self.temporal();
                            self.pon(Instr::Lee {
                                destino: t,
                                direccion: Valor::Temporal(dir),
                                ancho,
                            });
                            Valor::Temporal(t)
                        }
                        Expr::Indice { que, indice, sitio } => {
                            let q = self.expresion(que);
                            let i = self.expresion(indice);
                            let t = self.temporal();
                            self.pon(Instr::Binaria {
                                destino: t,
                                op: Op::Suma,
                                clase: Clase::Entero,
                                sin_signo: true,
                                izquierda: q,
                                derecha: i,
                            });
                            self.pon(Instr::Comprueba {
                                que: Comprobacion::Indice,
                                sobre: Valor::Temporal(t),
                                contra: None,
                                sin_signo: true,
                                sitio: *sitio,
                            });
                            Valor::Temporal(t)
                        }
                        // Un campo sin disposicion ya lo denuncio
                        // `disposicion`. No se inventa nada.
                        _ => Valor::Const(Const::Nada),
                    },
                }
            }
            // ** UN LITERAL DE LISTA **SIN TIPO ESCRITO** (2026-08-23).
            //
            // El runtime ya existe --`lista_nueva` y `agrega`-- y un literal se
            // construye de verdad cuando el destino dice de que es:
            // `notas es lista de entero64 = [1, 2, 3]`. Eso pasa en
            // `lista_literal`, en el descenso de la asignacion.
            //
            // *** Aqui llega el que NO lo dice, y sigue sin construirse. No es
            // pereza: **el ancho del elemento sale del TIPO**, y `[1, 2, 3]` a
            // secas no dice si sus elementos miden uno, cuatro u ocho. Deducirlo
            // de los literales tiene reglas propias que nadie ha escrito -- que
            // mide `[1, 2.5]`? -- y la deduccion de este compilador ya dejo esa
            // fila fuera con su motivo.
            //
            // [!] Los elementos SI se bajan, para que sus efectos ocurran. Lo
            // que no se hace es inventar una lista.
            Expr::Lista(v, _) => {
                for x in v {
                    self.expresion(x);
                }
                self.sin_ancho += 1;
                Valor::Const(Const::Nada)
            }
            // ** UN LITERAL DE TABLA **SIN TIPO ESCRITO**, igual que la lista.
            //
            // El que SI lo dice se construye en `tabla_literal`, en el descenso
            // de la asignacion. Aqui llega el que no, y sigue sin construirse:
            // sin el tipo del destino no se sabe que es la clave ni el valor.
            //
            // [!] Las parejas SI se bajan, para que sus efectos ocurran. Lo que
            // no se hace es inventar una tabla.
            Expr::Tabla(pares, _) => {
                for (k, val) in pares {
                    self.expresion(k);
                    self.expresion(val);
                }
                self.sin_ancho += 1;
                Valor::Const(Const::Nada)
            }
            Expr::OSiNo { intento, .. } => self.expresion(intento),
            // ** Sin `_ =>`. El `match` cubre todas las formas del arbol, y
            // dejarlo cerrado significa que **anadir una forma nueva no
            // compila** hasta que alguien decida como se baja. Con el comodin,
            // la forma nueva se habria bajado a `nada` en silencio -- que es
            // como se pierde una funcionalidad sin un solo test en rojo.
        }
    }
}
