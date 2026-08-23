//! `manifiesto` -- **lo que el binario declara sobre si mismo.**
//!
//! ## Que contesta, y por que es un modulo y no dos lineas en el emisor
//!
//! Contesta *"que es esto, sin abrir el fuente"*. Es una pregunta distinta de
//! las otras dos que se le parecen, y por eso no vive en ninguna de ellas:
//!
//! ```text
//!    perfil        cabe esto en el perfil que declaro el fichero?
//!    cabina        que numeros le mando al sistema para que los siga en el tiempo
//!    manifiesto    que DECLARA el binario, para quien solo tiene el binario
//! ```
//!
//! ** La tercera es la unica que sobrevive al compilador. Las otras dos hablan
//! con alguien que esta compilando; esta habla con quien encuentra un `.bex` en
//! un disco dentro de seis meses.
//!
//! ## ** Por que lo escribe el FRONTEND y no el emisor
//!
//! Porque es una afirmacion sobre el MODULO --su perfil, sus piezas, su
//! `crudo`--, y el emisor tiene prohibido saber que existe algo llamado
//! "perfil", por la misma regla que le prohibe al frontend saber que existe
//! algo llamado "registro de argumento".
//!
//! El emisor lo recibe hecho y lo mete en su seccion. No lo lee.
//!
//! ## El agujero que esto cierra
//!
//! Hasta el 2026-08-22, `empaquetar()` escribia **una** seccion --`Code`-- y el
//! perfil salia por la consola con `-i` y se moria ahi. Y `perfil/mod.rs`
//! llevaba escrito que *"va al informe del `.bex` para que `bmo-verify` pueda
//! exigirlo firmado"*, que **no era verdad**: `bmo-verify` no tenia ni la
//! palabra.
//!
//! El sitio existia desde que se diseno el formato y estaba vacio:
//! `SectionKind::Manifest = 0x09`, con su escritor y su validador. Es la misma
//! historia que la seccion `Resources = 0x0B` del paquete BEF.

use crate::arbol::Modulo;
use crate::perfil::Informe;

/// La ruta relativa donde vive la seccion, para quien la busque por nombre.
pub const CLASE: &str = "Manifest";

/// Lo que se puede recuperar de un `.bex` sin ver una sola linea de fuente.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Manifiesto {
    /// `inti`. Sin esto el manifiesto no se puede atribuir a nadie.
    pub lenguaje: String,
    /// **El perfil contra el que se JUZGO el modulo entero.**
    ///
    /// ** No dice "el mas estricto de los que lo componen": dice contra cual se
    /// comprobo, que hoy es el del fichero que llama. La diferencia importa el
    /// dia que se escriba la regla del mezclado, y decirlo mal ahora seria
    /// meter esa regla por la puerta de atras sin haberla decidido.
    pub perfil: String,
    /// El fichero que se compilo.
    pub fuente: String,
    /// Cuantos bloques `crudo` tiene. Es el medidor de *"cuanto de esto no lo
    /// comprueba nadie"*, y el numero que `bmo-verify` puede exigir firmado.
    pub crudo: usize,
    /// A que maquinas se ata. Vacio = este binario se porta.
    pub arquitecturas: Vec<String>,
    /// **De que esta hecho.** Lo que trajo cada `usa`, con el perfil que esa
    /// pieza declaro para si misma.
    pub piezas: Vec<PiezaDeclarada>,
}

/// Un trozo del binario que no lo escribio quien compilo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PiezaDeclarada {
    pub fichero: String,
    pub usa: String,
    /// El perfil que la pieza declaro. **Puede no ser el del binario**, y ese
    /// es justo el dato que hoy no se juzga y manana decidira la regla del
    /// mezclado.
    pub perfil: String,
}

/// El manifiesto de un modulo ya comprobado.
pub fn de(m: &Modulo, informe: &Informe, fuente: &str) -> Manifiesto {
    Manifiesto {
        lenguaje: "inti".to_string(),
        // *** EL RESULTANTE, no el declarado (P2, 2026-08-23).
        //
        // Es lo que el plan pedia con estas palabras: *"el manifiesto declara el
        // perfil RESULTANTE, no el declarado"*. Y no es cosmetico: quien lee
        // este campo es el cargador, para decidir si un `.bex` puede correr en
        // Ring 0. Poner ahi lo que el fichero DIJO en vez de lo que el binario
        // ES seria firmar la promesa equivocada.
        perfil: if informe.perfil_resultante.is_empty() {
            // [!] VACIO significa **que nadie corrio `perfil::comprobar`**, no
            // "sin perfil". Pasa en los bancos que arman un manifiesto sin
            // analizar, y lo unico que se sabe entonces es lo que el fichero
            // DECLARO -- que es peor respuesta que la resultante, y mucho mejor
            // que un campo vacio en un `.bex` que el cargador va a leer.
            m.perfil.nombre().to_string()
        } else {
            informe.perfil_resultante.clone()
        },
        fuente: fuente.to_string(),
        crudo: informe.bloques_crudo,
        arquitecturas: informe.arquitecturas.clone(),
        piezas: m
            .piezas
            .iter()
            .map(|p| PiezaDeclarada {
                fichero: p.fichero.clone(),
                usa: p.usa.clone(),
                perfil: p.perfil.nombre().to_string(),
            })
            .collect(),
    }
}

impl Manifiesto {
    /// El TOML que va dentro de la seccion.
    ///
    /// ** Se escribe a mano y no con un serializador porque el orden de las
    /// claves tiene que ser SIEMPRE el mismo: dos compilaciones de la misma
    /// fuente tienen que dar el mismo fichero byte a byte, o *"este `.bex` es
    /// el que audite"* deja de poder decirse. Es la misma razon por la que
    /// `Runtime::traer` ordena las piezas por nombre.
    pub fn a_toml(&self) -> String {
        let mut t = String::new();
        t.push_str("# Lo que este binario declara sobre si mismo.\n");
        t.push_str("# Lo escribio el compilador de INTI; no se edita a mano.\n\n");
        t.push_str("[modulo]\n");
        t.push_str(&format!("lenguaje = {}\n", cadena(&self.lenguaje)));
        t.push_str(&format!("perfil = {}\n", cadena(&self.perfil)));
        t.push_str(&format!("fuente = {}\n", cadena(&self.fuente)));
        t.push_str("\n[metal]\n");
        t.push_str(&format!("crudo = {}\n", self.crudo));
        let arcos: Vec<String> = self.arquitecturas.iter().map(|a| cadena(a)).collect();
        t.push_str(&format!("arquitecturas = [{}]\n", arcos.join(", ")));
        for p in &self.piezas {
            t.push_str("\n[[pieza]]\n");
            t.push_str(&format!("fichero = {}\n", cadena(&p.fichero)));
            t.push_str(&format!("usa = {}\n", cadena(&p.usa)));
            t.push_str(&format!("perfil = {}\n", cadena(&p.perfil)));
        }
        t
    }

    /// Recuperarlo de vuelta. **Es la mitad que demuestra que la otra sirve.**
    ///
    /// Un manifiesto que se escribe y no se puede volver a leer no es un
    /// contrato: es un comentario largo dentro de un fichero binario.
    pub fn de_toml(t: &str) -> Option<Self> {
        let raiz: toml::Value = t.parse().ok()?;
        let cad = |v: Option<&toml::Value>| -> String {
            v.and_then(|x| x.as_str()).unwrap_or_default().to_string()
        };
        let modulo = raiz.get("modulo");
        let metal = raiz.get("metal");
        let piezas = raiz
            .get("pieza")
            .and_then(|v| v.as_array())
            .map(|a| {
                a.iter()
                    .map(|p| PiezaDeclarada {
                        fichero: cad(p.get("fichero")),
                        usa: cad(p.get("usa")),
                        perfil: cad(p.get("perfil")),
                    })
                    .collect()
            })
            .unwrap_or_default();
        Some(Self {
            lenguaje: cad(modulo.and_then(|m| m.get("lenguaje"))),
            perfil: cad(modulo.and_then(|m| m.get("perfil"))),
            fuente: cad(modulo.and_then(|m| m.get("fuente"))),
            crudo: metal
                .and_then(|m| m.get("crudo"))
                .and_then(|v| v.as_integer())
                .unwrap_or(0) as usize,
            arquitecturas: metal
                .and_then(|m| m.get("arquitecturas"))
                .and_then(|v| v.as_array())
                .map(|a| {
                    a.iter()
                        .filter_map(|x| x.as_str().map(|s| s.to_string()))
                        .collect()
                })
                .unwrap_or_default(),
            piezas,
        })
    }
}

/// Una cadena TOML con lo que hay que escapar, escapado.
///
/// ** No es un detalle: `fuente` es una RUTA, y en esta maquina las rutas
/// llevan `\`. Sin esto, `C:\proyecto\cpu.inti` sale al TOML como una secuencia
/// de escape invalida y el manifiesto que el compilador acaba de escribir no se
/// puede volver a leer -- con el agravante de que el `.bex` seguiria pasando el
/// gate, porque el validador solo mira que sea UTF-8.
fn cadena(s: &str) -> String {
    let mut r = String::with_capacity(s.len() + 2);
    r.push('"');
    for c in s.chars() {
        match c {
            '"' => r.push_str("\\\""),
            '\\' => r.push_str("\\\\"),
            '\n' => r.push_str("\\n"),
            '\r' => r.push_str("\\r"),
            '\t' => r.push_str("\\t"),
            c => r.push(c),
        }
    }
    r.push('"');
    r
}

#[cfg(test)]
mod pruebas;
