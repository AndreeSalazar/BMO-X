//! `ir` -- del arbol a instrucciones, sin nombrar ninguna maquina.
//!
//! ## Por que existe una IR, y por que ANTES del emisor
//!
//! Se podria emitir directamente del arbol. BMO C lo hace, y por eso evalua las
//! expresiones **empujandolas a la pila y sacandolas**, que es el techo del que
//! habla la seccion 13.6 del maestro: sin una forma intermedia con temporales
//! **no hay donde repartir los sitios rapidos de la maquina**, y sin eso
//! ninguna otra optimizacion se nota.
//!
//! (Esa frase se escribio primero nombrando un registro concreto, y
//! `tests/agnostico.rs` la tumbo. Tenia razon: si para explicar por que existe
//! la IR hace falta nombrar una maquina, la explicacion esta mal contada.)
//!
//! Asi que la IR no es una capa de mas: es el sitio donde cabe el 2-4x. Y va
//! antes del emisor porque hacerla despues significa reescribir el emisor
//! entero.
//!
//! ## Lo que este modulo se niega a saber
//!
//! **Todo lo de la maquina.** No hay registros, ni opcodes, ni anchos de
//! palabra. Un `Temporal` es un valor con nombre y sin sitio; donde acabe
//! viviendo lo decide otro. `tests/agnostico.rs` vigila este fichero como
//! vigila los demas.
//!
//! ## ** Y lo que la IR hace VISIBLE
//!
//! Las doce reglas de `REGLAS.md` dejan de ser un documento aqui: una suma de
//! enteros **emite su comprobacion de desbordamiento como una instruccion**, y
//! se puede contar. Un test comprueba que `a + b` genera esa comprobacion y que
//! `a + b` con `suma_circular` no.
//!
//! Eso es lo que separa *"INTI no tiene comportamiento indefinido"* de una
//! frase: aqui se ve.

//! ## El reparto de este directorio (L6b)
//!
//! ```text
//!    forma.rs   QUE es una instruccion       -> tipos, y cero decisiones
//!    mod.rs     COMO se llega a una          -> recorre el arbol y decide
//! ```
//!
//! ** El corte se eligio por la PREGUNTA y no por el tamano. La senal de que
//! esta bien puesto: `forma.rs` no importa nada de aqui, y quien solo quiera
//! saber que forma tiene una instruccion --el emisor, el marco-- no tiene que
//! leer ni una linea del descenso.

pub mod forma;

pub use forma::{
    Clase, Comprobacion, Const, Etiqueta, FuncionIr, Instr, Local, ModuloIr, Temporal, Valor,
};

use crate::arbol::{self, Bloque, Decl, Expr, Modulo, Op, Repeticion, Sent};
use crate::aviso::Cosecha;

/// Baja un modulo entero.
pub fn bajar(m: &Modulo) -> Cosecha<ModuloIr> {
    let plano = crate::disposicion::comprobar(m, crate::disposicion::Medidas::por_defecto()).valor;
    let tabla = crate::tablas::Modulos::por_defecto();
    let metal = metal_que_declara(m, &bmo_mods::Roots::find(), &tabla);
    bajar_con(m, &tabla, &plano, &metal)
}

/// Los nombres que son una instruccion, segun lo que el FUENTE declaro.
///
/// ## ** Las dos fuentes, y por que son dos
///
/// ```text
///    usa x86_64     nombres que SOLO existen ahi   -> el fichero no se porta
///    usa binarios   nombres que existen en todas   -> el fichero SI se porta
/// ```
///
/// Las dos acaban emitiendo una instruccion en esta maquina, y por eso salen
/// juntas de aqui. Lo que cambia es lo que el programa **declaro**, y eso ya lo
/// cuenta `perfil` -- que es donde tiene que contarse.
///
/// OJO: esto vive en `ir` y no nombra ninguna maquina. El nombre `"x86_64"` sale
/// de `m.usa`, o sea del fichero del usuario. Buscar una tabla por un nombre que
/// te dan no es conocerla.
pub fn metal_que_declara(
    m: &Modulo,
    raices: &bmo_mods::Roots,
    tabla: &crate::tablas::Modulos,
) -> Vec<String> {
    let mut v = Vec::new();
    for (n, _) in &m.usa {
        if let Some(maquina) = crate::arquitectura::Maquina::buscar(raices, n) {
            v.extend(maquina.nombres_que_trae());
        } else {
            // ** Un modulo de REX cuyos nombres son instrucciones aqui. Hoy solo
            // `binarios`, y por eso la pregunta se hace a la TABLA y no con un
            // `if n == "binarios"`: el dia que haya un segundo, es una fila.
            if tabla.son_instrucciones(n) {
                v.extend(tabla.trae(n).iter().cloned());
            }
        }
    }
    v
}

/// Baja un modulo entero sabiendo que trae cada `usa`.
///
/// ** La tabla que entra aqui es AGNOSTICA: dice que `lee_natural64` lee ocho
/// bytes y que `mi_tarea` vale tal numero. Ninguna de las dos cosas depende de
/// una maquina, y por eso este modulo puede leerlas sin romper su promesa.
pub fn bajar_con(
    m: &Modulo,
    tabla: &crate::tablas::Modulos,
    plano: &crate::disposicion::Plano,
    metal: &[String],
) -> Cosecha<ModuloIr> {
    let mut salida = ModuloIr::default();

    for d in &m.declaraciones {
        match d {
            Decl::Funcion(f) => {
                let ir = Descenso::nueva(&mut salida.textos, tabla, plano, m.perfil, metal).funcion(f);
                salida.funciones.push(ir);
            }
            Decl::Operacion { tipo, funcion } => {
                let mut ir = Descenso::nueva(&mut salida.textos, tabla, plano, m.perfil, metal).funcion(funcion);
                // El nombre lleva el tipo delante para que dos operaciones con
                // el mismo nombre en tipos distintos no se pisen.
                ir.nombre = format!("{}.{}", tipo, funcion.nombre);
                salida.funciones.push(ir);
            }
            Decl::Registro {
                nombre,
                operaciones,
                ..
            } => {
                for f in operaciones {
                    let mut ir = Descenso::nueva(&mut salida.textos, tabla, plano, m.perfil, metal).funcion(f);
                    ir.nombre = format!("{}.{}", nombre, f.nombre);
                    salida.funciones.push(ir);
                }
            }
            Decl::Constante { .. } => {}
        }
    }

    Cosecha::nueva(salida)
}

struct Descenso<'t> {
    instrucciones: Vec<Instr>,
    siguiente_temporal: u32,
    siguiente_etiqueta: u32,
    locales: Vec<String>,
    textos: &'t mut Vec<String>,
    /// Donde salta un `corta` y donde un `continua`, de fuera a dentro.
    bucles: Vec<(Etiqueta, Etiqueta)>,
    tabla: &'t crate::tablas::Modulos,
    plano: &'t crate::disposicion::Plano,
    /// ** Que perfil, y hace falta por UNA sola pregunta: que es `3.5`.
    ///
    /// En `llano` es un `flotante64` --binario, ocho bytes-- porque `decimal`
    /// esta prohibido alli por no decir su medida. En `pleno` es un decimal
    /// EXACTO, y convertirlo a binario para "ya tenerlo hecho" perderia la
    /// exactitud que el lenguaje promete en la portada.
    ///
    /// El mismo caracter en el fuente, dos cosas distintas, y lo decide una
    /// palabra escrita en la primera linea del fichero. Es la unica vez que el
    /// perfil cambia lo que SIGNIFICA algo en vez de lo que se permite.
    perfil: crate::arbol::Perfil,
    /// Los nombres que son UNA INSTRUCCION de la maquina, no una funcion.
    ///
    /// ## ** Por que llegan de fuera y no se buscan aqui
    ///
    /// Porque este modulo no puede nombrar una maquina, y ese es todo el
    /// asunto. La lista se monta arriba, a partir de lo que el FUENTE declaro
    /// con `usa` -- asi que el nombre de la maquina lo escribio el usuario, no
    /// el compilador.
    ///
    /// ** Y sin esta lista pasaba lo peor que puede pasar: `lee_reloj()` se
    /// bajaba a una LLAMADA a un simbolo que no existe. Compilaba, pasaba el
    /// analisis de nombres --porque el nombre existe en la tabla--, pasaba el
    /// de perfiles, y el binario saltaba a la nada. La tabla de la maquina
    /// estaba entera y no la leia nadie a la hora de emitir.
    metal: &'t [String],
    /// Los tipos declarados de la funcion que se esta bajando.
    tipos: std::collections::HashMap<String, crate::arbol::Tipo>,
}

impl<'t> Descenso<'t> {
    fn nueva(
        textos: &'t mut Vec<String>,
        tabla: &'t crate::tablas::Modulos,
        plano: &'t crate::disposicion::Plano,
        perfil: crate::arbol::Perfil,
        metal: &'t [String],
    ) -> Self {
        Self {
            instrucciones: Vec::new(),
            siguiente_temporal: 0,
            siguiente_etiqueta: 0,
            locales: Vec::new(),
            textos,
            bucles: Vec::new(),
            tabla,
            plano,
            perfil,
            metal,
            tipos: std::collections::HashMap::new(),
        }
    }

    /// La direccion de un sitio de memoria escrito con `.` o con `[]`.
    ///
    /// ** Devolver la DIRECCION y no el valor es lo que deja usar la misma
    /// cuenta para leer y para escribir. `p.x` y `p.x = 3` calculan
    /// exactamente lo mismo; lo unico que cambia es la instruccion de despues.
    ///
    /// `None` si no se sabe la disposicion -- y entonces no se emite nada,
    /// porque `disposicion` ya lo denuncio. Emitir "algo" para un programa que
    /// esta mal es como un compilador acaba produciendo binarios plausibles.
    fn direccion_de(&mut self, e: &Expr) -> Option<(Valor, u32)> {
        match e {
            Expr::Campo { que, nombre, .. } => {
                let t = self.plano.tipo_de(que, &self.tipos)?;
                let crate::arbol::Tipo::Nombre(r) = t else {
                    return None;
                };
                let hueco = self.plano.registro(&r)?.campo(nombre)?;
                let (desplazamiento, medida) = (hueco.desplazamiento, hueco.medida);
                let base = self.expresion(que);
                let t = self.temporal();
                self.pon(Instr::Binaria {
                    destino: t,
                    op: Op::Suma,
                    // Una direccion es un entero. Siempre. Aunque lo que haya al
                    // final sea un flotante: `p.x` suma bytes, no numeros.
                    clase: Clase::Entero,
                    izquierda: base,
                    derecha: Valor::Const(Const::Entero(desplazamiento as i64)),
                });
                Some((Valor::Temporal(t), medida))
            }
            Expr::Indice { que, indice, .. } => {
                let t = self.plano.tipo_de(que, &self.tipos)?;
                let (_, medida) = self.plano.elemento(&t)?;
                let base = self.expresion(que);
                let i = self.expresion(indice);
                // indice * medida
                let paso = self.temporal();
                self.pon(Instr::Binaria {
                    destino: paso,
                    op: Op::Por,
                    clase: Clase::Entero,
                    izquierda: i,
                    derecha: Valor::Const(Const::Entero(medida as i64)),
                });
                let t = self.temporal();
                self.pon(Instr::Binaria {
                    destino: t,
                    op: Op::Suma,
                    clase: Clase::Entero,
                    izquierda: base,
                    derecha: Valor::Temporal(paso),
                });
                Some((Valor::Temporal(t), medida))
            }
            _ => None,
        }
    }

    fn temporal(&mut self) -> Temporal {
        let t = Temporal(self.siguiente_temporal);
        self.siguiente_temporal += 1;
        t
    }

    fn etiqueta(&mut self) -> Etiqueta {
        let e = Etiqueta(self.siguiente_etiqueta);
        self.siguiente_etiqueta += 1;
        e
    }

    fn local(&mut self, nombre: &str) -> Local {
        match self.locales.iter().position(|n| n == nombre) {
            Some(i) => Local(i as u32),
            None => {
                self.locales.push(nombre.to_string());
                Local((self.locales.len() - 1) as u32)
            }
        }
    }

    fn busca_local(&self, nombre: &str) -> Option<Local> {
        self.locales
            .iter()
            .position(|n| n == nombre)
            .map(|i| Local(i as u32))
    }

    fn pon(&mut self, i: Instr) {
        self.instrucciones.push(i);
    }

    fn funcion(mut self, f: &arbol::Funcion) -> FuncionIr {
        // Los tipos escritos de esta funcion. Sin esto, `p.x` no sabe de que
        // registro es `p` -- y esa es toda la informacion que hace falta.
        self.tipos = crate::disposicion::tipos_de(f);
        for p in &f.parametros {
            self.local(&p.nombre);
        }
        self.bloque(&f.cuerpo);
        FuncionIr {
            nombre: f.nombre.clone(),
            parametros: f.parametros.len() as u32,
            locales: self.locales.len() as u32,
            temporales: self.siguiente_temporal,
            instrucciones: self.instrucciones,
        }
    }

    fn bloque(&mut self, b: &Bloque) {
        for s in b {
            self.sentencia(s);
        }
    }

    fn sentencia(&mut self, s: &Sent) {
        match s {
            Sent::Asigna { destino, valor, .. } => {
                let v = self.expresion(valor);
                match destino {
                    Expr::Nombre(n, _) => {
                        let l = self.local(n);
                        self.pon(Instr::Guarda {
                            destino: l,
                            valor: v,
                        });
                    }
                    // ** `p.x = 3` y `a[i] = 3`: la MISMA cuenta que al leer, y
                    // por eso comparten `direccion_de`. Lo unico que cambia es
                    // la instruccion del final.
                    Expr::Campo { .. } | Expr::Indice { .. } => {
                        if let Some((direccion, ancho)) = self.direccion_de(destino) {
                            self.pon(Instr::Escribe {
                                direccion,
                                valor: v,
                                ancho,
                            });
                        }
                        // Si no se supo la direccion, no se emite nada:
                        // `disposicion` ya lo denuncio y aqui no hay nada que
                        // inventar.
                    }
                    // Asignar a otra cosa no es asignar a nada: es una forma
                    // que la gramatica no deberia haber dejado pasar.
                    _ => {}
                }
            }
            Sent::Si { ramas, sino, .. } => self.si(ramas, sino.as_ref()),
            Sent::Repite { forma, cuerpo, .. } => self.repite(forma, cuerpo),
            Sent::ParaCada { .. } => {
                // Recorrer pide saber como esta hecha una lista, y eso es el
                // runtime. Cuando exista, esto se convierte en un bucle con
                // llamadas a `siguiente`.
            }
            Sent::Devuelve { valor, .. } => {
                let v = valor.as_ref().map(|e| self.expresion(e));
                self.pon(Instr::Devuelve(v));
            }
            Sent::Falla { motivo, .. } => {
                let v = self.expresion(motivo);
                self.pon(Instr::Devuelve(Some(v)));
            }
            Sent::Corta(_) => {
                if let Some((salida, _)) = self.bucles.last().copied() {
                    self.pon(Instr::Salta(salida));
                }
            }
            Sent::Continua(_) => {
                if let Some((_, vuelta)) = self.bucles.last().copied() {
                    self.pon(Instr::Salta(vuelta));
                }
            }
            Sent::Crudo { cuerpo, .. } | Sent::Paralelo { cuerpo, .. } => self.bloque(cuerpo),
            Sent::Expresion(e) => {
                self.expresion(e);
            }
        }
    }

    fn si(&mut self, ramas: &[(Expr, Bloque)], sino: Option<&Bloque>) {
        let fin = self.etiqueta();
        for (cond, cuerpo) in ramas {
            let cierto = self.etiqueta();
            let siguiente = self.etiqueta();
            let v = self.expresion(cond);
            self.pon(Instr::SaltaSi {
                cond: v,
                cierto,
                falso: siguiente,
            });
            self.pon(Instr::Etiqueta(cierto));
            self.bloque(cuerpo);
            self.pon(Instr::Salta(fin));
            self.pon(Instr::Etiqueta(siguiente));
        }
        if let Some(c) = sino {
            self.bloque(c);
        }
        self.pon(Instr::Etiqueta(fin));
    }

    fn repite(&mut self, forma: &Repeticion, cuerpo: &Bloque) {
        let vuelta = self.etiqueta();
        let dentro = self.etiqueta();
        let salida = self.etiqueta();

        self.pon(Instr::Etiqueta(vuelta));
        match forma {
            Repeticion::Siempre => {}
            Repeticion::Mientras(cond) => {
                let v = self.expresion(cond);
                self.pon(Instr::SaltaSi {
                    cond: v,
                    cierto: dentro,
                    falso: salida,
                });
                self.pon(Instr::Etiqueta(dentro));
            }
            Repeticion::Veces(_) => {
                // Un contador pide una local anonima y un tipo entero. Se hara
                // con el emisor, que es quien sabe los anchos.
            }
        }

        self.bucles.push((salida, vuelta));
        self.bloque(cuerpo);
        self.bucles.pop();

        self.pon(Instr::Salta(vuelta));
        self.pon(Instr::Etiqueta(salida));
    }

    fn expresion(&mut self, e: &Expr) -> Valor {
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
            Expr::Texto(t, _) => {
                let i = match self.textos.iter().position(|x| x == t) {
                    Some(i) => i,
                    None => {
                        self.textos.push(t.clone());
                        self.textos.len() - 1
                    }
                };
                Valor::Const(Const::Texto(i as u32))
            }
            Expr::Logico(b, _) => Valor::Const(Const::Logico(*b)),
            Expr::Nada(_) => Valor::Const(Const::Nada),
            Expr::Nombre(n, _) => match self.busca_local(n) {
                Some(l) => Valor::Local(l),
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
                        sitio: *sitio,
                    });
                }

                let t = self.temporal();
                self.pon(Instr::Binaria {
                    destino: t,
                    op: *op,
                    clase,
                    izquierda: i,
                    derecha: d,
                });
                if let Some(c) = comprobacion_despues(*op, clase) {
                    self.pon(Instr::Comprueba {
                        que: c,
                        sobre: Valor::Temporal(t),
                        sitio: *sitio,
                    });
                }
                Valor::Temporal(t)
            }
            Expr::Unaria { op, valor, .. } => {
                let v = self.expresion(valor);
                let t = self.temporal();
                self.pon(Instr::Unaria {
                    destino: t,
                    op: *op,
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
                        Expr::Indice { que, indice, sitio } => {
                            let q = self.expresion(que);
                            let i = self.expresion(indice);
                            let t = self.temporal();
                            self.pon(Instr::Binaria {
                                destino: t,
                                op: Op::Suma,
                                clase: Clase::Entero,
                                izquierda: q,
                                derecha: i,
                            });
                            self.pon(Instr::Comprueba {
                                que: Comprobacion::Indice,
                                sobre: Valor::Temporal(t),
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
            Expr::Lista(v, _) => {
                // Los elementos se bajan para que sus efectos ocurran; la lista
                // en si necesita el runtime, que todavia no existe.
                for x in v {
                    self.expresion(x);
                }
                Valor::Const(Const::Nada)
            }
            Expr::Tabla(pares, _) => {
                for (k, val) in pares {
                    self.expresion(k);
                    self.expresion(val);
                }
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

/// Que comprobacion pide cada operacion. Sale de `REGLAS.md` y de ningun otro
/// sitio.
/// La que se sabe ANTES de operar, mirando un operando.
///
/// ** Solo hay una familia aqui, y es la de dividir: el cero del divisor es la
/// unica cosa que hay que ver **antes**, porque despues de la division no queda
/// programa que mire nada.
fn comprobacion_antes(op: Op, clase: Clase) -> Option<Comprobacion> {
    if matches!(clase, Clase::Flotante) {
        return None;
    }
    match op {
        Op::Divide | Op::Entre | Op::Resto => Some(Comprobacion::EntreCero),
        _ => None,
    }
}

/// La que se sabe DESPUES, mirando lo que la operacion dejo dicho.
fn comprobacion_despues(op: Op, clase: Clase) -> Option<Comprobacion> {
    // ** LA COMA FLOTANTE NO LLEVA COMPROBACION, y no es una excepcion comoda
    // a "INTI no tiene comportamiento indefinido". Es que ya esta definido.
    //
    // La Regla 1 y la Regla 3 existen porque en los ENTEROS desbordar y dividir
    // entre cero **no tienen respuesta**: cualquier bit que salga es una
    // invencion del compilador. En IEEE-754 si la tienen --infinito y NaN, que
    // son valores con los que se puede seguir operando-- y esta escrita en una
    // norma de 1985.
    //
    // Atrapar aqui no anadiria ni una pizca de seguridad. Quitaria la
    // aritmetica: un calculo que desborda a infinito y luego vuelve al rango es
    // corriente, y con una trampa en medio no se puede escribir.
    if matches!(clase, Clase::Flotante) {
        return None;
    }
    match op {
        // Regla 1: las tres que se pasan de la cuenta.
        Op::Suma | Op::Resta | Op::Por | Op::Elevado => Some(Comprobacion::Desborde),
        // Comparar, los bits y la logica no pueden salirse. Y la Regla 3 ya no
        // esta aqui: se mudo a `comprobacion_antes`, que es donde servia.
        _ => None,
    }
}

/// El valor de un literal. **La cuenta vive en `lexico`**, que es de quien es el
/// numero -- aqui solo se pide.
fn parse_entero(texto: &str, base: crate::lexico::Base) -> Option<i64> {
    crate::lexico::valor_entero(texto, base)
}

#[cfg(test)]
mod pruebas;
