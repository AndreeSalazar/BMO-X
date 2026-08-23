//! `perfil` -- la frontera entre `llano` y `pleno`, comprobada.
//!
//! ## Que hace
//!
//! Recorre el arbol y contesta una sola pregunta: **esto cabe en el perfil que
//! declaro el fichero?** Nada mas. No sabe si los nombres existen, no sabe si
//! los tipos cuadran, y no emite un solo byte.
//!
//! ## Por que es un modulo y no un `if` dentro del parser
//!
//! Porque el parser **avanza** y esto **decide**. Un `crudo` es sintacticamente
//! igual de valido en los dos perfiles; lo que cambia es si esta permitido, y
//! esa es una pregunta sobre el modulo entero, no sobre la linea.
//!
//! Y porque es la ley que sostiene la promesa mas fuerte del lenguaje: *"en
//! `llano`, usar algo que asigna memoria es un error de compilacion con nombre
//! y sitio, no una sorpresa en ejecucion"*. Una promesa asi no puede vivir
//! repartida.
//!
//! ## La regla, dicha entera
//!
//! ```text
//!    llano                        pleno
//!    ------------------------     ------------------------
//!    sin texto/lista/tabla        todo
//!    medidas exactas              `numero` vale
//!    `crudo` SI                   `crudo` no (E0071)
//!    `en paralelo` no             `en paralelo` si
//! ```
//!
//! ** Y la que decide donde hace falta `crudo`: **no marca "bajo nivel", marca
//! "aqui nadie comprueba por ti"**. Por eso `invoca` no lo necesita --al otro
//! lado hay un kernel que valida una capability-- y `entrada_puerto` si.



use crate::arbol::*;
use crate::arquitectura::Maquina;
// El catalogo se mudo a `tablas` (gen 1) el 23-08: lo necesita tambien
// `disposicion`, que es de la 3 y no puede mirar aqui. Ver el LINAJE.
pub use crate::tablas::{Catalogo, RUTA_BIBLIOTECA as RUTA};
use crate::aviso::{codigos, Aviso, Cosecha, Sitio};

/// La ruta relativa a una raiz de tablas.

/// Lo que sale del analisis, aparte de los avisos.
#[derive(Debug, Clone, Default)]
pub struct Informe {
    /// Las arquitecturas que el fichero declaro con `usa`.
    ///
    /// ** Es la otra mitad del medidor: `crudo` dice **cuanto** no se comprueba
    /// y esto dice **a que maquina se ata**. Un modulo con esta lista vacia se
    /// recompila en cualquier sitio.
    pub arquitecturas: Vec<String>,
    /// **Cuantos bloques `crudo` tiene el modulo, PIEZAS INCLUIDAS.**
    ///
    /// ** Este numero es el que convierte *"cuanto de mi programa no lo
    /// comprueba nadie?"* en un dato. Desde el 2026-08-22 **va dentro del
    /// `.bex`**, en la seccion `Manifest` que escribe `crate::manifiesto`, y
    /// `bmo_verify::declaracion::exige_manifiesto` puede exigirla.
    ///
    /// *** **Hasta ese dia esta linea decia que ya iba, y no iba**: el perfil
    /// salia por la consola con `-i` y se moria ahi. `bmo-verify` no tenia ni
    /// la palabra. Se deja escrito porque un comentario que afirmaba algo que
    /// no existia es de la familia de fallos que este proyecto persigue -- y
    /// el que menos se nota, porque nadie compila un comentario.
    ///
    /// ** Y cuenta las de las PIEZAS. Un fuente con un solo `crudo` que hace
    /// `usa monton` declara **cuatro**: el medidor no dice cuantas ventanas
    /// abriste, dice **cuantas trae este binario**.
    pub bloques_crudo: usize,
}

/// Comprueba un modulo contra su perfil.
///
/// `maquinas` son las que el fichero declaro con `usa` y existen. Sin ellas,
/// un nombre como `entrada_puerto` es un nombre cualquiera -- que es lo
/// correcto: **solo existe si dijiste `usa x86_64`**.
pub fn comprobar(
    m: &Modulo,
    cat: &Catalogo,
    maquinas: &[Maquina],
    modulos: &crate::tablas::Modulos,
) -> Cosecha<Informe> {
    let mut informe = Informe::default();
    informe.arquitecturas = maquinas.iter().map(|x| x.nombre().to_string()).collect();
    let mut avisos_del_perfil: Vec<Aviso> = Vec::new();

    // ** LO PRIMERO: sabe este compilador bajar este perfil a bytes?
    //
    // Va antes de mirar nada porque no es una regla del programa, es una del
    // COMPILADOR -- y si la respuesta es que no, todo lo que se diga despues
    // habla de un binario que no se va a poder producir.
    //
    // Hasta el 22-08 no se preguntaba, y un fichero con `perfil pleno` salia
    // como un `.bex` FIRMADO de 768 bytes que devolvia ceros: `numero` se baja a
    // cero, `texto` y `lista` no existen, y las llamadas a REX no tienen
    // destino. El gate decia que si sobre algo que no hacia nada, y una firma
    // sobre eso es peor que ninguna firma.
    let nombre_del_perfil = m.perfil.nombre();
    if !cat.llega_a_bytes(nombre_del_perfil) {
        avisos_del_perfil.push(
            Aviso::nuevo(
                codigos::PERFIL_SIN_BYTES,
                format!(
                    "El compilador todavia no sabe bajar `perfil {}` a bytes.",
                    nombre_del_perfil
                ),
                Sitio::default(),
            )
            .con_habia(
                concat!(
                    "No es que el perfil este prohibido: esta especificado entero y es ",
                    "legitimo. Lo que falta es su runtime -- `texto`, `lista` y `tabla` piden ",
                    "monton, y `numero` es decimal exacto. Sin eso, lo que saldria es un ",
                    "`.bex` firmado que devuelve ceros."
                )
                .to_string(),
            )
            .con_hacer("escribe `perfil llano`, que si llega a bytes, o espera a que `pleno` este"),
        );
    }

    let mut v = Vigia {
        perfil: m.perfil,
        cat,
        maquinas,
        modulos,
        piezas: &m.piezas,
        en_declaracion: 0,
        en_constante: false,
        avisos: avisos_del_perfil,
        informe,
        dentro_de_crudo: false,
    };

    // ** El indice se lleva al dia porque es lo unico que dice DE DONDE sale la
    // declaracion que se esta mirando. El recorrido de dentro no lo sabe --y no
    // tiene por que--: aqui arriba se sabe, y desde aqui viaja.
    for (i, d) in m.declaraciones.iter().enumerate() {
        v.en_declaracion = i;
        v.declaracion(d);
    }

    Cosecha::con(v.informe, v.avisos)
}

struct Vigia<'c> {
    perfil: Perfil,
    cat: &'c Catalogo,
    maquinas: &'c [Maquina],
    /// Lo que traen los `usa` que no son maquinas.
    modulos: &'c crate::tablas::Modulos,
    /// **Las costuras del modulo**: que trozo vino de que fichero.
    ///
    /// ** Este analisis corre sobre el arbol YA FUSIONADO, asi que sin esto no
    /// puede distinguir una declaracion del usuario de una que trajo un `usa`
    /// -- y acusa al fichero equivocado con toda naturalidad.
    piezas: &'c [crate::arbol::Pieza],
    /// En que declaracion del modulo estamos. Lo pone el recorrido de arriba.
    en_declaracion: usize,
    /// **Estamos dentro del valor de una `constante`?**
    ///
    /// ** Porque ahi una lista literal NO CRECE: se congela cuando el modulo
    /// acaba de cargarse, y eso es lo que dice la seccion 10.2 del maestro --
    /// *"CONGELADO: inmortal. Nadie lo cambia, nadie cuenta sus referencias.
    /// Literales, constantes, un modulo cargado"*.
    ///
    /// *** La comprobacion de `llano` no distinguia *"esto crece"* de *"esto es
    /// un literal congelado"*, y por eso una tabla de senos o de CRC no se podia
    /// escribir. Es la misma regla, con la distincion que le faltaba.
    en_constante: bool,
    avisos: Vec<Aviso>,
    informe: Informe,
    dentro_de_crudo: bool,
}

impl<'c> Vigia<'c> {
    fn llano(&self) -> bool {
        self.perfil == Perfil::Llano
    }

    /// **Acusa, y dice de donde sale lo acusado.**
    ///
    /// ** Es un metodo y no seis `push` con la misma linea copiada al lado
    /// porque el dia que se anada una comprobacion, un `push` a pelo saldria
    /// sin marcar y nadie lo notaria: el aviso seria correcto, solo que
    /// senalando al fichero del que compila. Un fallo que no rompe nada es el
    /// que sobrevive.
    fn acusa(&mut self, a: Aviso) {
        let marcado = match self
            .piezas
            .iter()
            .find(|p| self.en_declaracion >= p.desde && self.en_declaracion < p.hasta)
        {
            Some(p) => a.con_pieza(p.fichero.clone(), p.usa.clone()),
            None => a,
        };
        self.avisos.push(marcado);
    }

    fn declaracion(&mut self, d: &Decl) {
        match d {
            Decl::Constante { valor, .. } => {
                let antes = self.en_constante;
                self.en_constante = true;
                self.expresion(valor);
                self.en_constante = antes;
            }
            Decl::Registro {
                campos, operaciones, ..
            } => {
                for f in operaciones {
                    self.funcion(f);
                }
                for c in campos {
                    if let Some(t) = &c.tipo {
                        self.tipo(t, c.sitio);
                    }
                    if let Some(e) = &c.defecto {
                        self.expresion(e);
                    }
                }
            }
            Decl::Funcion(f) => self.funcion(f),
            Decl::Operacion { funcion, .. } => self.funcion(funcion),
        }
    }

    fn funcion(&mut self, f: &Funcion) {
        for p in &f.parametros {
            match &p.tipo {
                Some(t) => self.tipo(t, p.sitio),
                None if self.llano() => self.falta_tipo(&p.nombre, p.sitio),
                None => {}
            }
        }
        if let Some(r) = &f.retorno {
            self.tipo(&r.tipo, f.sitio);
        }
        self.bloque(&f.cuerpo);
    }

    fn bloque(&mut self, b: &Bloque) {
        for s in b {
            self.sentencia(s);
        }
    }

    fn sentencia(&mut self, s: &Sent) {
        match s {
            Sent::Asigna { tipo, valor, .. } => {
                if let Some(t) = tipo {
                    self.tipo(t, s.sitio());
                }
                // *** ATAR UN TEXTO A UN NOMBRE ES TENER UNA VARIABLE `texto`,
                // y eso sigue sin caber en `llano` (2026-08-23).
                //
                // ** La linea que este arreglo traza, y conviene verla entera:
                //
                //     lee_natural8("hola" + i)   dentro de `crudo`  ->  VALE
                //     saludo = "hola"                               ->  E0070
                //
                // Los BYTES de un literal estan congelados y se llega a ellos
                // igual que a los de `PRIMOS`: son una direccion en `RoData` y
                // no cuestan nada. Lo que no cabe es la VARIABLE, porque una
                // variable de tipo `texto` es del tipo que crece -- y que HOY
                // solo se le pueda meter un literal no es una propiedad del
                // tipo, es una carencia del perfil que manana no lo sera.
                //
                // *** Deducirlo del literal y no del tipo escrito es a proposito:
                // en `llano` los tipos son obligatorios, asi que la unica forma
                // de que aparezca un `texto` sin escribirlo es esta.
                if self.llano() && tipo.is_none() && matches!(valor, Expr::Texto(_, _)) {
                    self.crece("un texto", s.sitio());
                }
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
            Sent::Repite { forma, cuerpo, .. } => {
                match forma {
                    Repeticion::Veces(e) | Repeticion::Mientras(e) => self.expresion(e),
                    Repeticion::Siempre => {}
                }
                self.bloque(cuerpo);
            }
            Sent::Devuelve { valor, .. } => {
                if let Some(e) = valor {
                    self.expresion(e);
                }
            }
            Sent::Falla { motivo, .. } => self.expresion(motivo),
            Sent::Corta(_) | Sent::Continua(_) => {}
            Sent::Crudo { cuerpo, sitio } => self.crudo(cuerpo, *sitio),
            Sent::Paralelo { cuerpo, sitio } => self.paralelo(cuerpo, *sitio),
            Sent::Expresion(e) => self.expresion(e),
        }
    }

    fn crudo(&mut self, cuerpo: &Bloque, sitio: Sitio) {
        if !self.llano() {
            self.acusa(
                Aviso::nuevo(
                    codigos::CRUDO_EN_PLENO,
                    "`crudo` no existe en el perfil `pleno`.",
                    sitio,
                )
                .con_habia(
                    "La ventana sin comprobar es del perfil de sistema, y alli se cuenta \
                     y se puede exigir firmada. En `pleno` no hay nada que abrir."
                        .to_string(),
                )
                .con_hacer("si de verdad tocas el metal, empieza el fichero con `perfil llano`"),
            );
        }
        self.informe.bloques_crudo += 1;
        let antes = self.dentro_de_crudo;
        self.dentro_de_crudo = true;
        self.bloque(cuerpo);
        self.dentro_de_crudo = antes;
    }

    fn paralelo(&mut self, cuerpo: &Bloque, sitio: Sitio) {
        if self.llano() {
            self.acusa(
                Aviso::nuevo(
                    codigos::LLANO_NO_ADMITE,
                    "`en paralelo` no existe en el perfil `llano`.",
                    sitio,
                )
                .con_habia(
                    "Una tarea necesita su propio monton, y en `llano` no hay monton."
                        .to_string(),
                )
                .con_hacer("cambia el fichero a `perfil pleno`"),
            );
        }
        self.bloque(cuerpo);
    }

    fn expresion(&mut self, e: &Expr) {
        match e {
            // *** UN LITERAL DE TEXTO NO CRECE, y por eso ya no se denuncia
            // (2026-08-23).
            //
            // Esto decia `self.crece("un texto")` en `llano`, con el motivo
            // *"lo que crece pide memoria"*. Y **un literal no crece**: es
            // CONGELADO --seccion 10.2 del maestro-- asi que sus bytes viven en
            // `RoData` con el bit de INMORTAL puesto, nadie le cuenta las
            // referencias y **no se reserva nada**.
            //
            // ** Es EXACTAMENTE el fallo que se cerro el 22-08 con
            // `PRIMOS = [2, 3, 5]`, un tipo mas alla. La comprobacion no
            // distinguia *"esto crece"* de *"esto es un literal congelado"*, y
            // por eso la lista de al lado ya tiene su excepcion.
            //
            // *** Y la de aqui es MAS FUERTE que la de la lista. Una lista
            // literal solo esta congelada dentro de una `constante` --fuera se
            // le puede anadir-- y por eso su brazo mira `en_constante`. Un texto
            // es INMUTABLE por definicion del tipo: `"hola"` no puede crecer en
            // ningun sitio, asi que no hace falta preguntar donde esta.
            //
            // [!] Lo que SIGUE denunciado es el TIPO `texto` en una declaracion,
            // y no es incoherencia: una variable de ese tipo puede acabar
            // guardando un texto CONSTRUIDO, y eso si pide monton. Se puede
            // llegar a los bytes congelados --con `crudo`, como se llega a
            // `PRIMOS`-- y no se puede tener la variable. Es la misma linea.
            Expr::Texto(_, _) => {}
            Expr::Lista(v, sitio) => {
                // ** Una lista dentro de una CONSTANTE esta congelada: no crece,
                // no pide monton, y cabe en `llano`. Va a `RoData`.
                if self.llano() && !self.en_constante {
                    self.crece("una lista", *sitio);
                }
                for x in v {
                    self.expresion(x);
                }
            }
            Expr::Tabla(v, sitio) => {
                if self.llano() {
                    self.crece("una tabla", *sitio);
                }
                for (k, val) in v {
                    self.expresion(k);
                    self.expresion(val);
                }
            }
            Expr::Binaria {
                izquierda, derecha, ..
            } => {
                self.expresion(izquierda);
                self.expresion(derecha);
            }
            Expr::Unaria { valor, .. } => self.expresion(valor),
            Expr::Llamada {
                que, argumentos, ..
            } => {
                // El nombre se comprueba al visitarlo, no aqui: hacerlo en los
                // dos sitios daba el mismo aviso dos veces, y un aviso repetido
                // es peor que uno que falta -- el lector deja de contar.
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
            Expr::OSiNo {
                intento, respaldo, ..
            } => {
                self.expresion(intento);
                match respaldo {
                    Respaldo::Valor(v) => self.expresion(v),
                    Respaldo::Bloque(b) => self.bloque(b),
                }
            }
            Expr::Nombre(n, sitio) => self.quiza_pide_crudo(n, *sitio),
            _ => {}
        }
    }

    /// El nombre toca el metal y no esta dentro de un `crudo`.
    fn quiza_pide_crudo(&mut self, nombre: &str, sitio: Sitio) {
        if self.dentro_de_crudo {
            return;
        }
        // ** Dos fuentes, una regla: el `crudo` viaja con quien trae el
        // nombre. La maquina trae `entrada_puerto` y su prohibicion; el modulo
        // `memoria` trae `escribe_natural64` y la suya.
        let de_la_maquina = self.maquinas.iter().any(|m| m.pide_crudo(nombre));
        if !de_la_maquina && !self.modulos.pide_crudo(nombre) {
            return;
        }
        self.acusa(
            Aviso::nuevo(
                codigos::METAL_SIN_CRUDO,
                format!("`{}` tiene que ir dentro de un bloque `crudo`.", nombre),
                sitio,
            )
            .con_habia(
                "`crudo` no marca \"esto es de bajo nivel\": marca \"aqui nadie comprueba \
                 por ti\". Al otro lado de un puerto --o de una direccion cruda-- no hay \
                 ningun kernel que valide nada."
                    .to_string(),
            )
            .con_hacer("mete la linea dentro de un bloque `crudo`"),
        );
    }

    fn crece(&mut self, que: &str, sitio: Sitio) {
        self.acusa(
            Aviso::nuevo(
                codigos::LLANO_NO_ADMITE,
                format!("En el perfil `llano` no se puede usar {}.", que),
                sitio,
            )
            .con_habia(
                "Lo que crece pide memoria, y `llano` no tiene monton: por eso puede \
                 escribir un manejador de interrupciones."
                    .to_string(),
            )
            .con_hacer("cambia el fichero a `perfil pleno`, o usa una medida fija"),
        );
    }

    fn falta_tipo(&mut self, nombre: &str, sitio: Sitio) {
        self.acusa(
            Aviso::nuevo(
                codigos::FALTA_TAMANO,
                format!("En `llano`, `{}` tiene que decir su tipo.", nombre),
                sitio,
            )
            .con_habia(
                "Sin tipo no hay medida, y sin medida no se puede reservar en la pila. \
                 La obligacion sale del perfil, no del gusto."
                    .to_string(),
            )
            .con_hacer(format!("escribe `{} es entero32`", nombre)),
        );
    }

    fn tipo(&mut self, t: &Tipo, sitio: Sitio) {
        match t {
            Tipo::Nombre(n) => {
                if !self.llano() {
                    return;
                }
                if self.cat.crece(n) {
                    self.crece(&format!("`{}`", n), sitio);
                } else if self.cat.cuesta(n) {
                    self.acusa(
                        Aviso::nuevo(
                            codigos::CUESTA_DEMASIADO,
                            format!("En el perfil `llano` no existe `{}`.", n),
                            sitio,
                        )
                        .con_habia(
                            concat!(
                                "`numero` es decimal exacto, y una suma suya cuesta entre 5 y ",
                                "20 veces una entera de 64 bits. No es que falte decir su ",
                                "medida: es que `llano` escribe drivers y manejadores de ",
                                "interrupciones, y ahi ese precio no se paga sin decirlo.",
                            )
                                .to_string(),
                        )
                        .con_hacer(
                            concat!(
                                "escribe `entero64` --o el ancho que necesites-- o cambia el ",
                                "fichero a `perfil pleno`, donde `numero` es el tipo por defecto",
                            ),
                        ),
                    );
                } else if self.cat.sin_medida(n) {
                    self.acusa(
                        Aviso::nuevo(
                            codigos::FALTA_TAMANO,
                            format!("En el perfil `llano` no existe `{}`.", n),
                            sitio,
                        )
                        .con_habia(
                            "Hay que decir la medida exacta. Sin medida no se puede elegir \
                             la instruccion ni reservar en la pila."
                                .to_string(),
                        )
                        .con_hacer("usa `entero32`, `natural8`, `flotante64`..."),
                    );
                }
            }
            // ** Un `bufer` SI vale en `llano`, y esa es su razon de existir.
            //
            // Es una direccion: no crece, no pide monton, y mide lo que un
            // puntero. Lo que no lleva es su longitud -- por eso indexarlo pide
            // `crudo` y `lista de T` no. Dos tipos porque son dos promesas
            // distintas, y aqui se ve cual es cual.
            Tipo::Bufer(dentro) => self.tipo(dentro, sitio),
            Tipo::Lista(t) => {
                if self.llano() {
                    self.crece("una lista", sitio);
                }
                self.tipo(t, sitio);
            }
            Tipo::Tabla(k, v) => {
                if self.llano() {
                    self.crece("una tabla", sitio);
                }
                self.tipo(k, sitio);
                self.tipo(v, sitio);
            }
            Tipo::Quiza(t) => self.tipo(t, sitio),
        }
    }
}

#[cfg(test)]
mod pruebas;
