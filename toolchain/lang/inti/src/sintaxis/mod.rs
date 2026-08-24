//! `sintaxis` -- de piezas a arbol.
//!
//! ## Que hace, y que se niega a saber
//!
//! Aplica la gramatica: dice que `si` lleva una condicion y un bloque, y que
//! una llamada lleva parentesis. **No sabe si los nombres existen**, ni si los
//! tipos cuadran, ni si el perfil admite lo que se escribio. Eso son tres
//! analisis distintos y ninguno vive aqui.
//!
//! El corte se nota en una frase: **aqui no se puede escribir `E0030`** (*"eso
//! no es cambiante"*), porque para saberlo hay que recordar lo que se declaro
//! antes, y este modulo no recuerda: avanza.
//!
//! ## Como se recupera de un fallo
//!
//! Parar en el primer error convertiria arreglar un fichero en adivinar cuantos
//! quedan. Asi que cuando una sentencia no se entiende, se **salta hasta el
//! final de la linea** y se sigue; cuando una declaracion no se entiende, se
//! salta hasta el margen cero. El arbol que sale esta incompleto **y se dice**:
//! la `Cosecha` trae los avisos.

mod expresion;
mod sentencia;

use crate::arbol::*;
use crate::aviso::{codigos, Aviso, Cosecha, Sitio};
use crate::lexico::{Clase, Pieza, Signo};
use crate::palabras::{Simbolo, Vocabulario};

/// El estado de la lectura. Es `pub(crate)` y no `pub`: fuera del modulo nadie
/// tiene por que poder mover el cursor a mano.
pub(crate) struct Cursor<'p> {
    /// Las piezas del lexer. `pub(crate)` porque los niveles de precedencia
    /// necesitan mirar una o dos piezas por delante -- `o si no` contra `o`,
    /// `no es` contra `no` -- y hacerlo con un metodo por cada caso seria mas
    /// codigo diciendo lo mismo.
    pub(crate) piezas: &'p [Pieza],
    pub(crate) i: usize,
    avisos: Vec<Aviso>,
    /// Como se escribe cada simbolo en el idioma de este fichero.
    ///
    /// Hace falta por las **palabras contextuales**: `y` es un operador en
    /// posicion de operador y un NOMBRE en posicion de valor, y para escribir
    /// ese nombre en el arbol hay que saber como se tecleo.
    pub(crate) vocab: &'p Vocabulario,
}

impl<'p> Cursor<'p> {
    fn nuevo(piezas: &'p [Pieza], vocab: &'p Vocabulario) -> Self {
        Self {
            piezas,
            i: 0,
            avisos: Vec::new(),
            vocab,
        }
    }

    pub(crate) fn mira(&self) -> &Pieza {
        // Nunca se sale: el lexer siempre termina en `Fin`.
        self.piezas.get(self.i).unwrap_or_else(|| self.ultima())
    }

    fn ultima(&self) -> &Pieza {
        &self.piezas[self.piezas.len() - 1]
    }

    pub(crate) fn sitio(&self) -> Sitio {
        self.mira().sitio
    }

    pub(crate) fn se_acabo(&self) -> bool {
        matches!(self.mira().clase, Clase::Fin)
    }

    pub(crate) fn avanza(&mut self) -> Pieza {
        let p = self.mira().clone();
        if self.i < self.piezas.len() - 1 {
            self.i += 1;
        }
        p
    }

    /// Si la siguiente es esta palabra, la come y dice que si.
    pub(crate) fn come(&mut self, s: Simbolo) -> bool {
        if self.mira().es(s) {
            self.avanza();
            true
        } else {
            false
        }
    }

    pub(crate) fn come_signo(&mut self, s: Signo) -> bool {
        if self.mira().es_signo(s) {
            self.avanza();
            true
        } else {
            false
        }
    }

    pub(crate) fn di(&mut self, a: Aviso) {
        self.avisos.push(a);
    }

    /// Exige una palabra, y si no esta lo dice con las cuatro partes.
    pub(crate) fn exige(&mut self, s: Simbolo, para_que: &str) -> bool {
        if self.come(s) {
            return true;
        }
        let sitio = self.sitio();
        let hay = self.mira().como_se_llama();
        self.di(
            Aviso::nuevo(
                codigos::PAREJA_ROTA,
                format!("Aqui falta la palabra `{}`.", s.clave().to_lowercase()),
                sitio,
            )
            .con_habia(format!("En su sitio hay {}. {}", hay, para_que))
            .con_hacer(format!("escribe `{}`", s.clave().to_lowercase())),
        );
        false
    }

    pub(crate) fn exige_signo(&mut self, s: Signo, para_que: &str) -> bool {
        if self.come_signo(s) {
            return true;
        }
        let sitio = self.sitio();
        let hay = self.mira().como_se_llama();
        self.di(
            Aviso::nuevo(
                codigos::PAREJA_ROTA,
                format!("Aqui falta un `{}`.", s.texto()),
                sitio,
            )
            .con_habia(format!("En su sitio hay {}. {}", hay, para_que))
            .con_hacer(format!("escribe `{}`", s.texto())),
        );
        false
    }

    /// Come el final de la sentencia. Si sobra algo en la linea, lo dice una
    /// vez y tira el resto: es lo que evita que un parentesis de mas convierta
    /// el fichero en una cascada.
    pub(crate) fn fin_de_linea(&mut self) {
        // ** El `:` de Python al final de la linea se acepta y se ignora.
        //
        // No es una segunda sintaxis: es TOLERANCIA. Quien viene de Python
        // escribe `def f(x):` sin pensarlo, y hacerle tropezar en el primer
        // caracter para ganar una regla no compra nada. Dentro de una pareja el
        // `:` sigue significando lo suyo (`{a: 1}`, `f(x: 1)`), y por eso solo
        // se come **justo antes del final de la linea**.
        if self.mira().es_signo(Signo::DosPuntos)
            && matches!(
                self.piezas.get(self.i + 1).map(|p| &p.clase),
                Some(Clase::FinLinea)
            )
        {
            self.avanza();
        }
        if matches!(self.mira().clase, Clase::FinLinea) {
            self.avanza();
            return;
        }
        if self.se_acabo() || matches!(self.mira().clase, Clase::Desangra) {
            return;
        }
        let sitio = self.sitio();
        let hay = self.mira().como_se_llama();
        self.di(
            Aviso::nuevo(
                codigos::PAREJA_ROTA,
                "Sobra algo al final de esta linea.",
                sitio,
            )
            .con_habia(format!("Despues de la sentencia aparece {}.", hay))
            .con_hacer("parte la linea en dos, o borra lo que sobre"),
        );
        self.hasta_fin_de_linea();
    }

    pub(crate) fn hasta_fin_de_linea(&mut self) {
        while !self.se_acabo()
            && !matches!(self.mira().clase, Clase::FinLinea | Clase::Desangra)
        {
            self.avanza();
        }
        if matches!(self.mira().clase, Clase::FinLinea) {
            self.avanza();
        }
    }

    /// Lee un bloque sangrado: el `FinLinea` de la cabecera, la sangria, las
    /// sentencias, y la desangria.
    pub(crate) fn bloque(&mut self) -> Bloque {
        self.fin_de_linea();

        if !matches!(self.mira().clase, Clase::Sangra) {
            let sitio = self.sitio();
            self.di(
                Aviso::nuevo(
                    codigos::SANGRIA_RARA,
                    "Aqui hace falta un bloque, y la linea de abajo no entra.",
                    sitio,
                )
                .con_habia(
                    "Lo que va dentro de un bloque se escribe cuatro espacios mas a la derecha."
                        .to_string(),
                )
                .con_hacer("mete la linea de abajo cuatro espacios"),
            );
            return Vec::new();
        }
        self.avanza(); // Sangra

        let mut cuerpo = Vec::new();
        while !self.se_acabo() && !matches!(self.mira().clase, Clase::Desangra) {
            if matches!(self.mira().clase, Clase::FinLinea) {
                self.avanza();
                continue;
            }
            match sentencia::sentencia(self) {
                Some(s) => cuerpo.push(s),
                None => self.hasta_fin_de_linea(),
            }
        }
        if matches!(self.mira().clase, Clase::Desangra) {
            self.avanza();
        }
        cuerpo
    }
}

/// Lee un fichero entero.
pub fn leer(piezas: &[Pieza], vocab: &Vocabulario) -> Cosecha<Modulo> {
    let mut c = Cursor::nuevo(piezas, vocab);

    let (perfil, sitio_perfil) = lee_perfil(&mut c);
    let (usa, necesita) = lee_cabecera(&mut c);

    let mut declaraciones = Vec::new();
    while !c.se_acabo() {
        if matches!(c.mira().clase, Clase::FinLinea) {
            c.avanza();
            continue;
        }
        // Una sangria en el nivel superior es una linea suelta que se colo:
        // se dice una vez y se salta el bloque entero.
        if matches!(c.mira().clase, Clase::Sangra | Clase::Desangra) {
            c.avanza();
            continue;
        }
        match declaracion(&mut c) {
            Some(d) => declaraciones.push(d),
            None => c.hasta_fin_de_linea(),
        }
    }

    Cosecha::con(
        Modulo {
            perfil,
            sitio_perfil,
            usa,
            necesita,
            declaraciones,
            // Un fichero leido solo es de UNA pieza: la suya. Las costuras las
            // pone `armar`, que es quien fusiona -- aqui no hay nada cosido
            // todavia y un vector vacio lo dice sin mentir.
            piezas: Vec::new(),
        },
        c.avisos,
    )
}

/// El perfil. No hay valor por defecto y por eso su ausencia es un error del
/// fichero entero, no de una linea.
fn lee_perfil(c: &mut Cursor) -> (Perfil, Sitio) {
    let sitio = c.sitio();
    if !c.come(Simbolo::Perfil) {
        c.di(
            Aviso::nuevo(
                codigos::FALTA_PERFIL,
                "A este fichero le falta la primera linea: el perfil.",
                sitio,
            )
            .con_habia(
                "Un modulo de INTI dice si es `llano` (sistema, sin monton) o `pleno` \
                 (aplicacion). No hay un valor por defecto a proposito: elegirlo por ti \
                 seria elegirte el lenguaje."
                    .to_string(),
            )
            .con_hacer("empieza el fichero con `perfil pleno`"),
        );
        // Se sigue leyendo como `pleno` para poder dar el resto de avisos.
        return (Perfil::Pleno, sitio);
    }

    let p = c.mira().clone();
    if c.come(Simbolo::Llano) {
        c.fin_de_linea();
        return (Perfil::Llano, sitio);
    }
    if c.come(Simbolo::Pleno) {
        c.fin_de_linea();
        return (Perfil::Pleno, sitio);
    }

    c.di(
        Aviso::nuevo(
            codigos::PERFIL_RARO,
            "Ese perfil no existe.",
            p.sitio,
        )
        .con_habia(format!(
            "Hay dos: `llano` escribe sistema y `pleno` escribe aplicaciones. Aqui pone {}.",
            p.como_se_llama()
        ))
        .con_hacer("escribe `perfil llano` o `perfil pleno`"),
    );
    c.hasta_fin_de_linea();
    (Perfil::Pleno, sitio)
}

/// **LA CABECERA DEL MODULO: `usa` y `necesita`, en el orden que sea.**
///
/// ** Las dos en el mismo bucle a proposito. Con una funcion por palabra, un
/// fichero que escribiera `necesita` antes que `usa` habria dejado el `usa`
/// abajo, sin importar nada y sin un aviso -- porque el segundo lector no
/// habria llegado a mirarlo. El orden entre dos lineas de cabecera no significa
/// nada, y lo que no significa nada no debe cambiar el resultado.
fn lee_cabecera(c: &mut Cursor) -> (Vec<(String, Sitio)>, Vec<Necesidad>) {
    let mut v = Vec::new();
    let mut necesidades = Vec::new();
    loop {
        if matches!(c.mira().clase, Clase::FinLinea) {
            c.avanza();
            continue;
        }
        if c.mira().es(Simbolo::Necesita) {
            if let Some(x) = lee_necesita(c) {
                necesidades.push(x);
            }
            continue;
        }
        if !c.mira().es(Simbolo::Usa) {
            return (v, necesidades);
        }
        c.avanza();
        let sitio = c.sitio();
        match c.avanza().clase {
            Clase::Nombre(n) => v.push((n, sitio)),
            otra => {
                c.di(
                    Aviso::nuevo(
                        codigos::PAREJA_ROTA,
                        "Despues de `usa` va el nombre de lo que se importa.",
                        sitio,
                    )
                    .con_habia(format!(
                        "Aqui hay {}.",
                        Pieza::nueva(otra, sitio).como_se_llama()
                    ))
                    .con_hacer("por ejemplo `usa entrada`"),
                );
            }
        }
        c.fin_de_linea();
    }
}

/// **UNA LINEA `necesita`.**
///
/// ```text
///     necesita monton 64 megas "los pesos del modelo viven en RAM"
/// ```
///
/// ** La unidad y el motivo se leen si estan y NO se exigen aqui. Que falte el
/// motivo es un error --lo dice `E0132`-- pero es un error de SIGNIFICADO, y
/// quien lo dice es quien tiene la tabla delante. Un parser que exigiera el
/// motivo tendria que explicar por que, y para explicarlo tendria que saber que
/// el ABI se niega a escribir un requisito sin el. No lo sabe ni tiene que
/// saberlo.
fn lee_necesita(c: &mut Cursor) -> Option<Necesidad> {
    let sitio = c.sitio();
    c.avanza(); // `necesita`

    let clase = match c.avanza().clase {
        Clase::Nombre(n) => n,
        otra => {
            c.di(
                Aviso::nuevo(
                    codigos::PAREJA_ROTA,
                    "Despues de `necesita` va el nombre de lo que se pide.",
                    sitio,
                )
                .con_habia(format!(
                    "Aqui hay {}.",
                    Pieza::nueva(otra, sitio).como_se_llama()
                ))
                .con_hacer("por ejemplo `necesita monton 64 megas \"y el motivo\"`"),
            );
            c.hasta_fin_de_linea();
            return None;
        }
    };

    let sitio_n = c.sitio();
    let cantidad = match c.avanza().clase {
        // ** El numero sigue siendo TEXTO aqui, como en todas partes: el lexer
        // no lo convierte a proposito. Lo pasa una vez quien sabe en que forma
        // va a vivir, y para un requisito esa forma es un `u64` de bytes.
        Clase::Numero(x) if !x.con_punto => x.texto,
        otra => {
            c.di(
                Aviso::nuevo(
                    codigos::PAREJA_ROTA,
                    "Despues de la clase va CUANTO se necesita, en un numero entero.",
                    sitio_n,
                )
                .con_habia(format!(
                    "Aqui hay {}.",
                    Pieza::nueva(otra, sitio_n).como_se_llama()
                ))
                .con_hacer("por ejemplo `necesita monton 64 megas \"y el motivo\"`"),
            );
            c.hasta_fin_de_linea();
            return None;
        }
    };

    let unidad = match &c.mira().clase {
        Clase::Nombre(u) => {
            let u = u.clone();
            c.avanza();
            Some(u)
        }
        _ => None,
    };

    let motivo = match &c.mira().clase {
        Clase::Texto(t) => {
            let t = t.clone();
            c.avanza();
            Some(t)
        }
        _ => None,
    };

    c.fin_de_linea();
    Some(Necesidad {
        clase,
        cantidad,
        unidad,
        motivo,
        sitio,
    })
}

fn declaracion(c: &mut Cursor) -> Option<Decl> {
    if c.mira().es(Simbolo::Registro) {
        return registro(c);
    }
    if c.mira().es(Simbolo::Funcion) {
        return funcion(c).map(Decl::Funcion);
    }
    if c.mira().es(Simbolo::Operacion) {
        return operacion(c);
    }
    if let Clase::Nombre(_) = c.mira().clase {
        return constante(c);
    }
    // `cambiante` arriba del todo: se detecta a proposito para poder explicar
    // POR QUE no vale, en vez de soltar "esto no puede ir aqui".
    if c.mira().es(Simbolo::Cambiante) {
        let sitio = c.sitio();
        c.di(
            Aviso::nuevo(
                codigos::CAMBIANTE_ARRIBA,
                "Lo de nivel superior no puede ser `cambiante`.",
                sitio,
            )
            .con_habia(
                "Todo lo de arriba se CONGELA cuando el modulo termina de cargarse, y por eso se puede prestar a otra tarea sin un solo cerrojo. Eso es lo que hace que INTI no necesite un GIL."
                    .to_string(),
            )
            .con_hacer("quita el `cambiante`, o metelo dentro de una funcion"),
        );
        c.hasta_fin_de_linea();
        return None;
    }

    let sitio = c.sitio();
    let hay = c.mira().como_se_llama();
    c.di(
        Aviso::nuevo(
            codigos::PAREJA_ROTA,
            "Esto no puede ir aqui, en el margen del fichero.",
            sitio,
        )
        .con_habia(format!(
            "En el nivel de arriba solo van `funcion`, `registro`, `operacion` y constantes. \
             Aqui hay {}.",
            hay
        ))
        .con_hacer("mete esta linea dentro de una `funcion`"),
    );
    None
}

fn constante(c: &mut Cursor) -> Option<Decl> {
    let sitio = c.sitio();
    let nombre = match c.avanza().clase {
        Clase::Nombre(n) => n,
        _ => return None,
    };
    if !c.exige_signo(Signo::Igual, "Una constante del modulo se escribe `NOMBRE = valor`.") {
        c.hasta_fin_de_linea();
        return None;
    }
    let valor = expresion::expresion(c)?;
    c.fin_de_linea();
    Some(Decl::Constante {
        nombre,
        valor,
        sitio,
    })
}

fn registro(c: &mut Cursor) -> Option<Decl> {
    let sitio = c.sitio();
    c.avanza(); // registro

    let nombre = match c.mira().clase.clone() {
        Clase::Tipo(t) => {
            c.avanza();
            t
        }
        Clase::Nombre(n) => {
            let s = c.sitio();
            c.avanza();
            c.di(
                Aviso::nuevo(
                    codigos::PAREJA_ROTA,
                    "El nombre de un registro empieza por mayuscula.",
                    s,
                )
                .con_habia(format!("Aqui pone `{}`.", n))
                .con_hacer(format!("escribelo `{}`", en_mayuscula(&n))),
            );
            en_mayuscula(&n)
        }
        _ => {
            let s = c.sitio();
            c.di(
                Aviso::nuevo(codigos::PAREJA_ROTA, "A este registro le falta el nombre.", s)
                    .con_hacer("por ejemplo `registro Alumno`"),
            );
            c.hasta_fin_de_linea();
            return None;
        }
    };

    // Herencia: se detecta a proposito para poder decir que NO existe, en vez
    // de dar un error de sintaxis que no explica nada.
    if c.mira().es(Simbolo::De) {
        let s = c.sitio();
        c.di(
            Aviso::nuevo(codigos::SIN_HERENCIA, "En INTI no hay herencia.", s)
                .con_habia(
                    "Un registro son datos. El comportamiento son funciones, y lo que se \
                     comparte se compone escribiendolo."
                        .to_string(),
                )
                .con_hacer("quita el `de` y pon el campo que necesites"),
        );
        c.hasta_fin_de_linea();
    } else {
        c.fin_de_linea();
    }

    let mut campos = Vec::new();
    let mut operaciones = Vec::new();
    if matches!(c.mira().clase, Clase::Sangra) {
        c.avanza();
        while !c.se_acabo() && !matches!(c.mira().clase, Clase::Desangra) {
            if matches!(c.mira().clase, Clase::FinLinea) {
                c.avanza();
                continue;
            }
            // ** El BLOQUE PROPIO: una `operacion` escrita dentro del registro
            // no repite el nombre del tipo, porque ya se dijo arriba.
            if c.mira().es(Simbolo::Operacion) {
                let sitio_op = c.sitio();
                c.avanza();
                match cabecera_de_funcion(c, sitio_op) {
                    Some(f) => operaciones.push(f),
                    None => c.hasta_fin_de_linea(),
                }
                continue;
            }
            match campo(c) {
                Some(k) => campos.push(k),
                None => c.hasta_fin_de_linea(),
            }
        }
        if matches!(c.mira().clase, Clase::Desangra) {
            c.avanza();
        }
    }

    Some(Decl::Registro {
        nombre,
        campos,
        operaciones,
        sitio,
    })
}

fn campo(c: &mut Cursor) -> Option<Campo> {
    let sitio = c.sitio();
    // `registro Punto` con campos `x` e `y`: el segundo es una palabra clave y
    // aun asi tiene que poder ser un campo.
    if let Clase::Palabra(s) = c.mira().clase {
        if es_nombrable(s) {
            let nombre = c.vocab.texto(s).to_string();
            c.avanza();
            return campo_con_nombre(c, nombre, sitio);
        }
    }
    let nombre = match c.mira().clase.clone() {
        Clase::Nombre(n) => {
            c.avanza();
            n
        }
        _ => {
            let hay = c.mira().como_se_llama();
            c.di(
                Aviso::nuevo(codigos::PAREJA_ROTA, "Aqui va el nombre de un campo.", sitio)
                    .con_habia(format!("Hay {}.", hay))
                    .con_hacer("por ejemplo `nota es numero`"),
            );
            return None;
        }
    };

    campo_con_nombre(c, nombre, sitio)
}

fn campo_con_nombre(c: &mut Cursor, nombre: String, sitio: Sitio) -> Option<Campo> {
    let tipo = if c.come(Simbolo::Es) { lee_tipo(c) } else { None };
    let defecto = if c.come_signo(Signo::Igual) {
        expresion::expresion(c)
    } else {
        None
    };
    c.fin_de_linea();

    Some(Campo {
        nombre,
        tipo,
        defecto,
        sitio,
    })
}

/// La misma lista que usa el parser de expresiones, preguntada desde aqui.
pub(crate) fn es_nombrable(s: Simbolo) -> bool {
    matches!(
        s,
        Simbolo::Y | Simbolo::O | Simbolo::A | Simbolo::Un | Simbolo::En | Simbolo::De
    )
}

fn funcion(c: &mut Cursor) -> Option<Funcion> {
    let sitio = c.sitio();
    c.avanza(); // funcion
    cabecera_de_funcion(c, sitio)
}

fn operacion(c: &mut Cursor) -> Option<Decl> {
    let sitio = c.sitio();
    c.avanza(); // operacion

    let tipo = match c.mira().clase.clone() {
        Clase::Tipo(t) => {
            c.avanza();
            t
        }
        _ => {
            let s = c.sitio();
            c.di(
                Aviso::nuevo(
                    codigos::PAREJA_ROTA,
                    "Una operacion dice de que tipo es.",
                    s,
                )
                .con_hacer("por ejemplo `operacion Punto suma(a, b)`"),
            );
            c.hasta_fin_de_linea();
            return None;
        }
    };

    let f = cabecera_de_funcion(c, sitio)?;
    Some(Decl::Operacion { tipo, funcion: f })
}

fn cabecera_de_funcion(c: &mut Cursor, sitio: Sitio) -> Option<Funcion> {
    let nombre = match c.mira().clase.clone() {
        // `funcion a(...)`: una funcion llamada `a` o `y` es legitima, igual
        // que una variable.
        Clase::Palabra(s) if es_nombrable(s) => {
            let n = c.vocab.texto(s).to_string();
            c.avanza();
            n
        }
        Clase::Nombre(n) => {
            c.avanza();
            n
        }
        _ => {
            let hay = c.mira().como_se_llama();
            c.di(
                Aviso::nuevo(codigos::PAREJA_ROTA, "A esta funcion le falta el nombre.", sitio)
                    .con_habia(format!("Despues de `funcion` hay {}.", hay))
                    .con_hacer("por ejemplo `funcion media(notas)`"),
            );
            c.hasta_fin_de_linea();
            return None;
        }
    };

    // Los parentesis son opcionales solo si no hay parametros: `funcion
    // principal`. Con parametros hacen falta, porque `f` y `f()` tienen que
    // verse distintos.
    let mut parametros = Vec::new();
    if c.come_signo(Signo::ParenAbre) {
        if !c.mira().es_signo(Signo::ParenCierra) {
            loop {
                match parametro(c) {
                    Some(p) => parametros.push(p),
                    None => break,
                }
                if !c.come_signo(Signo::Coma) {
                    break;
                }
            }
        }
        c.exige_signo(Signo::ParenCierra, "Los parametros van entre parentesis.");
    }

    let retorno = if c.come(Simbolo::Devuelve) {
        lee_tipo(c).map(|tipo| {
            // `devuelve numero o error`
            let puede_fallar = if c.mira().es(Simbolo::O) {
                let guardado = c.i;
                c.avanza();
                if c.come(Simbolo::Error) {
                    true
                } else {
                    c.i = guardado;
                    false
                }
            } else {
                false
            };
            TipoRetorno { tipo, puede_fallar }
        })
    } else {
        None
    };

    let cuerpo = c.bloque();

    Some(Funcion {
        nombre,
        parametros,
        retorno,
        cuerpo,
        sitio,
    })
}

fn parametro(c: &mut Cursor) -> Option<Parametro> {
    let sitio = c.sitio();
    let cambiante = c.come(Simbolo::Cambiante);
    if let Clase::Palabra(s) = c.mira().clase {
        if es_nombrable(s) {
            let nombre = c.vocab.texto(s).to_string();
            c.avanza();
            let tipo = if c.come(Simbolo::Es) { lee_tipo(c) } else { None };
            let defecto = if c.come_signo(Signo::Igual) {
                expresion::expresion(c)
            } else {
                None
            };
            return Some(Parametro { nombre, cambiante, tipo, defecto, sitio });
        }
    }
    let nombre = match c.mira().clase.clone() {
        Clase::Nombre(n) => {
            c.avanza();
            n
        }
        _ => {
            let hay = c.mira().como_se_llama();
            c.di(
                Aviso::nuevo(codigos::PAREJA_ROTA, "Aqui va el nombre de un parametro.", sitio)
                    .con_habia(format!("Hay {}.", hay))
                    .con_hacer("por ejemplo `funcion f(notas es lista de numero)`"),
            );
            return None;
        }
    };
    let tipo = if c.come(Simbolo::Es) { lee_tipo(c) } else { None };
    let defecto = if c.come_signo(Signo::Igual) {
        expresion::expresion(c)
    } else {
        None
    };
    Some(Parametro {
        nombre,
        cambiante,
        tipo,
        defecto,
        sitio,
    })
}

/// Lee un tipo. **No comprueba que exista**: eso es de otro analisis.
pub(crate) fn lee_tipo(c: &mut Cursor) -> Option<Tipo> {
    if c.come(Simbolo::Quiza) {
        return lee_tipo(c).map(|t| Tipo::Quiza(Box::new(t)));
    }
    if c.come(Simbolo::Lista) {
        c.exige(Simbolo::De, "Se escribe `lista de <tipo>`.");
        return lee_tipo(c).map(|t| Tipo::Lista(Box::new(t)));
    }
    if c.come(Simbolo::Bufer) {
        c.exige(Simbolo::De, "Se escribe `bufer de <tipo>`.");
        return lee_tipo(c).map(|t| Tipo::Bufer(Box::new(t)));
    }
    if c.come(Simbolo::Tabla) {
        c.exige(Simbolo::De, "Se escribe `tabla de <clave> a <valor>`.");
        let clave = lee_tipo(c)?;
        c.exige(Simbolo::A, "Se escribe `tabla de <clave> a <valor>`.");
        let valor = lee_tipo(c)?;
        return Some(Tipo::Tabla(Box::new(clave), Box::new(valor)));
    }
    match c.mira().clase.clone() {
        Clase::Nombre(n) => {
            c.avanza();
            Some(Tipo::Nombre(n))
        }
        Clase::Tipo(t) => {
            c.avanza();
            Some(Tipo::Nombre(t))
        }
        _ => {
            let sitio = c.sitio();
            let hay = c.mira().como_se_llama();
            c.di(
                Aviso::nuevo(codigos::PAREJA_ROTA, "Aqui falta un tipo.", sitio)
                    .con_habia(format!("Hay {}.", hay))
                    .con_hacer("por ejemplo `numero`, `texto` o `lista de numero`"),
            );
            None
        }
    }
}

fn en_mayuscula(s: &str) -> String {
    let mut cs = s.chars();
    match cs.next() {
        Some(p) => p.to_uppercase().collect::<String>() + cs.as_str(),
        None => String::new(),
    }
}

#[cfg(test)]
mod pruebas;
