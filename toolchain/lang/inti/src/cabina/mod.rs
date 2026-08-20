//! `cabina` -- lo que INTI le cuenta al sistema.
//!
//! ## La idea, y es de Eddi
//!
//! > *"no olvides algo TAN ESENCIAL: CABINA. Ese mismo va a estar vigilando a
//! > INTI por completo, porque es el PRINCIPAL para decir y senalar: que fallo?
//! > Para asi mejorar todo eso en avances."*
//!
//! Y encaja sin forzar nada, porque **CABINA ya tenia el sitio hecho**:
//! `Layer::Lang` existe en `cabina-core` desde antes que INTI. La capa de los
//! lenguajes llevaba tiempo vacia esperando a alguien.
//!
//! ## ** Y lo que se manda NO son solo los fallos
//!
//! Esa es la parte que cambia el valor de esto. Si CABINA solo viera errores,
//! seria un registro de quejas. Lo que se le manda es **lo que el compilador
//! SABE**:
//!
//! ```text
//!    los avisos          que fallo, donde, y con que codigo
//!    los bloques `crudo` a que maquina se ata el programa
//!    las comprobaciones  lo que cuesta el "sin comportamiento indefinido"
//!    las arquitecturas   lo que declaro con `usa`
//! ```
//!
//! Los tres ultimos son numeros, y van en el campo `value` del evento **a
//! proposito**: un numero se puede seguir en el tiempo. Asi *"este programa se
//! esta atando mas a la maquina que el mes pasado"* deja de ser una impresion.
//!
//! *** Eso es *"mejorar en avances"* hecho instrumento: **CABINA no mira si
//! INTI fallo, mira como va cambiando lo que INTI produce.**
//!
//! ## Lo que este modulo se niega a saber
//!
//! Como se calcularon esos numeros. Recibe un [`Parte`] ya hecho y lo traduce.
//! No mira a `perfil`, ni a `nombres`, ni a `ir` -- si lo hiciera, seria un
//! quinto analisis disfrazado de informe, y el dia que uno de los tres se
//! reescribiera se lo llevaria por delante.

use cabina_core::{Entity, Event, Layer, Severity};

use crate::aviso::Aviso;

/// El nombre con el que INTI se presenta en CABINA.
pub const MODULO: &str = "inti";

/// El parte de una compilacion: lo que el compilador sabe cuando termina.
///
/// Es un tipo propio y no el `Informe` de `perfil` **para no atar este modulo a
/// aquel**. Quien compila rellena esto; aqui solo se traduce.
#[derive(Debug, Clone, Default)]
pub struct Parte {
    /// Como se llama lo que se compilo.
    pub fichero: String,
    /// `llano` o `pleno`.
    pub perfil: String,
    /// Las maquinas que el fichero declaro con `usa`.
    pub arquitecturas: Vec<String>,
    /// Cuantas ventanas sin comprobar tiene.
    pub bloques_crudo: usize,
    /// Cuantas comprobaciones anti-UB se emitieron.
    pub comprobaciones: usize,
    pub funciones: usize,
}

/// Que gravedad tiene un codigo de INTI para CABINA.
///
/// El mapa no es libre: sale de lo que cada familia significa.
///
/// ```text
///    E0xxx   no compila            -> Fault    algo hay que arreglar
///    E1xxx   atrapa en ejecucion   -> Fault    paso de verdad
///    A2xxx   aviso                 -> Warning  compila, y aun asi
/// ```
///
/// OJO: **Ninguno es `Panic`.** `Panic` en CABINA quiere decir *el sistema no
/// puede seguir*, y un programa que no compila no para el sistema. Usarlo
/// aqui gastaria la unica palabra que queda para lo de verdad grave.
pub fn gravedad(codigo: &str) -> Severity {
    match codigo.as_bytes().first() {
        Some(b'A') => Severity::Warning,
        _ => Severity::Fault,
    }
}

/// Traduce un aviso a un evento.
///
/// El `[DONDE]` del contrato de cuatro partes cae en los campos `fichero` y
/// `linea` que CABINA ya tenia: **no hubo que inventar ningun formato**, y eso
/// es la senal de que los dos lados habian entendido lo mismo por separado.
pub fn de_aviso(a: &Aviso, fichero: &str) -> Event {
    Event::new(
        gravedad(a.codigo.0),
        Layer::Lang,
        Entity::Module,
        MODULO,
        0,
        &a.que_paso,
        codigo_como_numero(a.codigo.0),
    )
    .en(fichero, a.sitio.linea as u32)
}

/// El codigo, sin la letra, como numero.
///
/// Va en `value` para que CABINA pueda contar **cuantas veces sale cada
/// codigo** sin leer el texto. Un mensaje se puede reescribir; un numero no
/// cambia nunca, y eso es lo que hace que la cuenta valga a lo largo de meses.
fn codigo_como_numero(codigo: &str) -> u64 {
    codigo[1..].parse::<u64>().unwrap_or(0)
}

/// Todo lo que INTI tiene que contar de una compilacion.
///
/// Primero lo que sabe, luego lo que fallo. En ese orden a proposito: quien lea
/// el registro ve **contra que** ocurrieron los fallos antes de verlos.
pub fn eventos(parte: &Parte, avisos: &[Aviso]) -> Vec<Event> {
    let mut v = Vec::new();

    let info = |msg: &str, valor: u64| {
        Event::new(
            Severity::Info,
            Layer::Lang,
            Entity::Module,
            MODULO,
            0,
            msg,
            valor,
        )
        .en(&parte.fichero, 0)
    };

    v.push(info(
        if parte.perfil == "llano" {
            "compilado en perfil llano"
        } else {
            "compilado en perfil pleno"
        },
        parte.funciones as u64,
    ));

    // ** Los dos numeros que miden la salud de un programa, y que ningun otro
    // lenguaje puede dar: a que se ata, y lo que paga por no tener UB.
    v.push(info("bloques crudo", parte.bloques_crudo as u64));
    v.push(info("comprobaciones emitidas", parte.comprobaciones as u64));

    for a in &parte.arquitecturas {
        v.push(
            Event::new(
                Severity::Info,
                Layer::Lang,
                Entity::Module,
                MODULO,
                0,
                &format!("se ata a la maquina {}", a),
                1,
            )
            .en(&parte.fichero, 0),
        );
    }

    for a in avisos {
        v.push(de_aviso(a, &parte.fichero));
    }

    v
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::aviso::{codigos, Sitio};

    fn parte() -> Parte {
        Parte {
            fichero: "notas.inti".into(),
            perfil: "llano".into(),
            arquitecturas: vec!["x86_64".into()],
            bloques_crudo: 2,
            comprobaciones: 7,
            funciones: 3,
        }
    }

    #[test]
    fn un_error_es_un_fallo_y_un_aviso_es_una_advertencia() {
        assert_eq!(gravedad("E0030"), Severity::Fault);
        assert_eq!(gravedad("E1001"), Severity::Fault);
        assert_eq!(gravedad("A2010"), Severity::Warning);
    }

    /// `Panic` quiere decir *el sistema no puede seguir*, y un programa que no
    /// compila no para el sistema. Gastarlo aqui dejaria sin palabra a lo de
    /// verdad grave.
    #[test]
    fn nada_de_inti_llega_a_panic() {
        for c in codigos::TODOS {
            assert_ne!(gravedad(c.0), Severity::Panic, "{}", c.0);
        }
    }

    /// El `[DONDE]` del aviso cae en los campos que CABINA ya tenia.
    #[test]
    fn el_donde_del_aviso_llega_entero() {
        let a = Aviso::nuevo(
            codigos::NO_ES_CAMBIANTE,
            "`x` se fijo y no se puede cambiar.",
            Sitio::nuevo(12, 5),
        );
        let e = de_aviso(&a, "notas.inti");
        assert_eq!(e.fichero_str(), "notas.inti");
        assert_eq!(e.linea, 12);
        assert_eq!(e.value, 30, "el codigo, sin la letra");
        assert_eq!(e.severity, Severity::Fault);
    }

    /// ** Los numeros van en `value` para que se puedan seguir en el tiempo sin
    /// leer el texto.
    #[test]
    fn los_numeros_del_parte_van_donde_se_pueden_contar() {
        let evs = eventos(&parte(), &[]);
        let crudo = evs
            .iter()
            .find(|e| e.msg_str().contains("crudo"))
            .expect("falta el parte de crudo");
        assert_eq!(crudo.value, 2);
        assert_eq!(crudo.severity, Severity::Info);

        let comp = evs
            .iter()
            .find(|e| e.msg_str().contains("comprobaciones"))
            .expect("falta el parte de comprobaciones");
        assert_eq!(comp.value, 7);
    }

    /// Primero lo que sabe, luego lo que fallo: quien lea el registro ve contra
    /// que ocurrieron los fallos antes de verlos.
    #[test]
    fn lo_que_sabe_va_antes_que_lo_que_fallo() {
        let a = Aviso::nuevo(codigos::TABULADOR, "Hay un tabulador.", Sitio::nuevo(3, 1));
        let evs = eventos(&parte(), &[a]);
        let primer_fallo = evs
            .iter()
            .position(|e| e.severity == Severity::Fault)
            .expect("deberia haber un fallo");
        let ultimo_info = evs
            .iter()
            .rposition(|e| e.severity == Severity::Info)
            .expect("deberia haber informacion");
        assert!(ultimo_info < primer_fallo);
    }

    #[test]
    fn todo_lo_de_inti_va_en_la_capa_de_los_lenguajes() {
        let evs = eventos(&parte(), &[]);
        assert!(!evs.is_empty());
        for e in &evs {
            assert_eq!(e.layer, Layer::Lang);
            assert_eq!(e.module_str(), "inti");
        }
    }

    #[test]
    fn la_maquina_declarada_se_cuenta() {
        let evs = eventos(&parte(), &[]);
        assert!(evs.iter().any(|e| e.msg_str().contains("x86_64")));
    }
}
