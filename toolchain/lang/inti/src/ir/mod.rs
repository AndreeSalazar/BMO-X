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

use crate::arbol::{self, Bloque, Decl, Expr, Modulo, Op, OpUno, Repeticion, Sent};
use crate::aviso::{Cosecha, Sitio};

/// Un valor con nombre y sin sitio. Donde vive lo decide el emisor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Temporal(pub u32);

/// Una ranura local. Es un INDICE, no una direccion: el marco lo reparte quien
/// sabe el ancho de un puntero, y este modulo no lo sabe.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Local(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Etiqueta(pub u32);

/// Una constante, **todavia sin convertir**.
///
/// El decimal sigue siendo texto por el mismo motivo que en el lexer: `numero`
/// es decimal exacto, y pasarlo por un binario intermedio perderia la exactitud
/// que el lenguaje promete en la portada.
#[derive(Debug, Clone, PartialEq)]
pub enum Const {
    Entero(i64),
    Decimal(String),
    /// Indice en el pozo de textos del modulo.
    Texto(u32),
    Logico(bool),
    Nada,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Valor {
    Temporal(Temporal),
    Local(Local),
    Const(Const),
    /// Un nombre que este modulo no sabe resolver: una funcion, o algo que trae
    /// un `usa`. El emisor lo resuelve contra sus tablas.
    Nombre(String),
}

/// Que se comprueba, y con que codigo se atrapa.
///
/// ** Cada variante es una fila de `REGLAS.md`. Tenerlas como instrucciones
/// **y no como codigo suelto dentro del emisor** es lo que deja contarlas: un
/// test puede exigir que una suma traiga la suya.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Comprobacion {
    /// Regla 1: la operacion se paso de la cuenta.
    Desborde,
    /// Regla 3: dividir entre cero.
    EntreCero,
    /// Regla 2: indice fuera de rango.
    Indice,
    /// Regla 12: convertir un flotante que no cabe.
    Conversion,
}

impl Comprobacion {
    /// El codigo con el que atrapa.
    pub fn codigo(self) -> &'static str {
        match self {
            Comprobacion::Desborde => "E1001",
            Comprobacion::Indice => "E1002",
            Comprobacion::EntreCero => "E1003",
            Comprobacion::Conversion => "E1012",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Instr {
    Mueve {
        destino: Temporal,
        origen: Valor,
    },
    Binaria {
        destino: Temporal,
        op: Op,
        izquierda: Valor,
        derecha: Valor,
    },
    Unaria {
        destino: Temporal,
        op: OpUno,
        valor: Valor,
    },
    /// La comprobacion que hace que no haya comportamiento indefinido.
    ///
    /// Va **detras** de la operacion que la necesita y mira su resultado. Si
    /// falla, atrapa con su codigo -- y atrapar en INTI es devolver un error,
    /// no abortar.
    Comprueba {
        que: Comprobacion,
        sobre: Valor,
        sitio: Sitio,
    },
    Llama {
        destino: Option<Temporal>,
        que: Valor,
        argumentos: Vec<Valor>,
    },
    /// Lee `ancho` bytes de una direccion.
    ///
    /// ** `ancho` va en BYTES, no en el nombre de un registro de la maquina:
    /// "8" es verdad en toda maquina y "qword" solo en una. Traducirlo a la
    /// instruccion es trabajo del emisor, y ese es el reparto entero.
    ///
    /// Esta instruccion **no comprueba nada**, y esa es su definicion. Por eso
    /// los nombres que la generan piden `crudo`: al otro lado de una direccion
    /// cruda no hay ningun kernel que valide. No es un descuido de la IR --
    /// es lo que se pidio.
    Lee {
        destino: Temporal,
        direccion: Valor,
        ancho: u32,
    },
    /// Escribe `ancho` bytes en una direccion.
    Escribe {
        direccion: Valor,
        valor: Valor,
        ancho: u32,
    },
    /// Un intrinseco de la maquina, por NOMBRE. El emisor lo busca en las
    /// tablas de la arquitectura; este modulo no sabe que hay detras.
    Metal {
        destino: Option<Temporal>,
        nombre: String,
        argumentos: Vec<Valor>,
    },
    Guarda {
        destino: Local,
        valor: Valor,
    },
    Etiqueta(Etiqueta),
    Salta(Etiqueta),
    SaltaSi {
        cond: Valor,
        cierto: Etiqueta,
        falso: Etiqueta,
    },
    Devuelve(Option<Valor>),
}

#[derive(Debug, Clone)]
pub struct FuncionIr {
    pub nombre: String,
    /// Cuantas de las locales son parametros.
    ///
    /// Son las PRIMERAS, y por eso basta un numero. El emisor lo necesita para
    /// guardarlos donde la maquina los deje al entrar -- pero **cuales son esos
    /// sitios es cosa suya**: aqui solo se dice cuantos.
    pub parametros: u32,
    /// Cuantas ranuras locales pide. El TAMANO de cada una lo decide el emisor
    /// con el perfil de la maquina: aqui solo se cuentan.
    pub locales: u32,
    pub temporales: u32,
    pub instrucciones: Vec<Instr>,
}

#[derive(Debug, Clone, Default)]
pub struct ModuloIr {
    pub funciones: Vec<FuncionIr>,
    /// El pozo de textos. Se comparte, y por eso puede prestarse congelado.
    pub textos: Vec<String>,
}

impl ModuloIr {
    /// Cuantas comprobaciones anti-UB emitio el modulo entero.
    ///
    /// ** Este numero es el precio del "sin comportamiento indefinido", y se
    /// puede leer. La seccion 6.3 del maestro dice que cuesta ~1%; aqui esta
    /// **cuantas son**, para que el dia de la medida se sepa contra que.
    pub fn comprobaciones(&self) -> usize {
        self.funciones
            .iter()
            .flat_map(|f| f.instrucciones.iter())
            .filter(|i| matches!(i, Instr::Comprueba { .. }))
            .count()
    }
}

/// Baja un modulo entero.
pub fn bajar(m: &Modulo) -> Cosecha<ModuloIr> {
    let plano = crate::disposicion::comprobar(m, crate::disposicion::Medidas::por_defecto()).valor;
    bajar_con(m, &crate::tablas::Modulos::por_defecto(), &plano)
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
) -> Cosecha<ModuloIr> {
    let mut salida = ModuloIr::default();

    for d in &m.declaraciones {
        match d {
            Decl::Funcion(f) => {
                let ir = Descenso::nueva(&mut salida.textos, tabla, plano).funcion(f);
                salida.funciones.push(ir);
            }
            Decl::Operacion { tipo, funcion } => {
                let mut ir = Descenso::nueva(&mut salida.textos, tabla, plano).funcion(funcion);
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
                    let mut ir = Descenso::nueva(&mut salida.textos, tabla, plano).funcion(f);
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
    /// Los tipos declarados de la funcion que se esta bajando.
    tipos: std::collections::HashMap<String, crate::arbol::Tipo>,
}

impl<'t> Descenso<'t> {
    fn nueva(
        textos: &'t mut Vec<String>,
        tabla: &'t crate::tablas::Modulos,
        plano: &'t crate::disposicion::Plano,
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
                    izquierda: i,
                    derecha: Valor::Const(Const::Entero(medida as i64)),
                });
                let t = self.temporal();
                self.pon(Instr::Binaria {
                    destino: t,
                    op: Op::Suma,
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
                    Valor::Const(Const::Decimal(n.texto.clone()))
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
                let i = self.expresion(izquierda);
                let d = self.expresion(derecha);
                let t = self.temporal();
                self.pon(Instr::Binaria {
                    destino: t,
                    op: *op,
                    izquierda: i,
                    derecha: d,
                });
                // ** Aqui es donde "sin comportamiento indefinido" deja de ser
                // una frase: la comprobacion se emite al lado de la operacion,
                // y se puede contar.
                if let Some(c) = comprobacion_de(*op) {
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
fn comprobacion_de(op: Op) -> Option<Comprobacion> {
    match op {
        // Regla 1: las tres que se pasan de la cuenta.
        Op::Suma | Op::Resta | Op::Por | Op::Elevado => Some(Comprobacion::Desborde),
        // Regla 3.
        Op::Divide | Op::Entre | Op::Resto => Some(Comprobacion::EntreCero),
        // Comparar, los bits y la logica no pueden salirse.
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
