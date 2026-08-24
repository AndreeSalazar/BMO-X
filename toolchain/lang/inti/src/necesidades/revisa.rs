//! **DE UNA LINEA `necesita` A UN REQUISITO QUE SE PUEDE ESCRIBIR.**
//!
//! Aqui se contesta lo que el parser no podia contestar: si la clase existe, si
//! la unidad existe, si el numero cabe, y si hay motivo.
//!
//! ## Por que esto no esta en el parser
//!
//! Porque para decir *"`mont0n` no existe"* hay que tener la tabla delante, y un
//! parser que cargue tablas para leer una linea es un parser que ya no se puede
//! probar solo. Es la misma frontera que separa `Tipo::Nombre("Alumno")` de
//! saber si `Alumno` existe.
//!
//! ## Y por que devuelve una lista y no un numero
//!
//! El monton es la unica clase que hoy cambia el codigo que se emite, y aun asi
//! esto devuelve **todo lo declarado**: lo demas va al `.bex` para que lo
//! conteste el cargador. Devolver solo el monton habria hecho que
//! `necesita pantalla 1` compilara, no dijera nada, y no llegara a ningun sitio.

use crate::arbol::Modulo;
use crate::aviso::{codigos, Aviso, Cosecha};

use super::Necesidades;

/// El numero del monton en el ABI (`CLASE_MONTON`).
///
/// ** Se escribe aqui en vez de importarse porque este crate **no enlaza
/// `bmo-abi` a proposito** -- lo dice su `Cargo.toml`: *"F1 no emite bytes"*. La
/// prueba que comprueba que los dos numeros siguen siendo el mismo vive en el
/// emisor, que si lo enlaza.
const MONTON: u16 = 8;

/// Un requisito ya resuelto, listo para `bef::requisitos::Declaracion`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pedido {
    /// El numero del ABI.
    pub clase: u16,
    /// La clase, por su nombre. Para los avisos y para el manifiesto.
    pub nombre: String,
    pub unidad: u16,
    /// Ya en la unidad base: bytes si la clase se mide en bytes.
    pub cantidad: u64,
    pub motivo: String,
}

/// **Lo que este modulo necesita, comprobado contra la tabla.**
pub fn revisa(m: &Modulo, t: &Necesidades) -> Cosecha<Vec<Pedido>> {
    let mut avisos = Vec::new();
    let mut out: Vec<Pedido> = Vec::new();

    for n in &m.necesita {
        let Some(clase) = t.clase(&n.clase) else {
            avisos.push(
                Aviso::nuevo(
                    codigos::NECESITA_DESCONOCIDA,
                    format!("`{}` no es algo que se pueda necesitar.", n.clase),
                    n.sitio,
                )
                .con_habia(format!(
                    "Lo que este sistema sabe conceder es: {}.",
                    t.clases_conocidas().join(", ")
                ))
                .con_hacer(
                    "si el nombre esta bien, la tabla que manda es \
                     `lang/inti/necesidades.toml`",
                ),
            );
            continue;
        };

        // ** DOS VECES LA MISMA CLASE NO SE SUMA. Sumarlas seria decidir por el
        // programa; quedarse con una seria decidir por el ORDEN, que es peor
        // porque no se ve.
        if let Some(ya) = out.iter().find(|p| p.clase == clase.numero) {
            avisos.push(
                Aviso::nuevo(
                    codigos::NECESITA_REPETIDA,
                    format!("`{}` se necesita dos veces.", n.clase),
                    n.sitio,
                )
                .con_habia(format!(
                    "Ya habia una linea que pedia {} de `{}`.",
                    ya.cantidad, ya.nombre
                ))
                .con_hacer("junta las dos en una sola linea con el total"),
            );
            continue;
        }

        // La unidad: la escrita, o la base si no se escribio ninguna.
        let escala = match &n.unidad {
            Some(u) => match t.unidad(u) {
                Some(e) => e,
                None => {
                    avisos.push(
                        Aviso::nuevo(
                            codigos::UNIDAD_DESCONOCIDA,
                            format!("`{u}` no es una unidad."),
                            n.sitio,
                        )
                        .con_habia(format!(
                            "Las que hay son: {}.",
                            t.unidades_conocidas().join(", ")
                        ))
                        .con_hacer("se anaden en `lang/inti/necesidades.toml`"),
                    );
                    continue;
                }
            },
            None => 1,
        };

        // ** El numero se convierte AQUI y no en el lexer, y por eso el
        // desbordamiento se caza aqui: una cantidad que no cabe en 64 bits es
        // un numero que no existe, y decirlo es mejor que envolverlo.
        let base: u64 = match n.cantidad.parse() {
            Ok(v) => v,
            Err(_) => {
                avisos.push(
                    Aviso::nuevo(
                        codigos::NECESITA_DE_MAS,
                        format!("`{}` no cabe en un numero de esta maquina.", n.cantidad),
                        n.sitio,
                    )
                    .con_habia("Una cantidad se cuenta en un natural de 64 bits.")
                    .con_hacer("pide menos"),
                );
                continue;
            }
        };
        let Some(cantidad) = base.checked_mul(escala) else {
            avisos.push(
                Aviso::nuevo(
                    codigos::NECESITA_DE_MAS,
                    "Esta cantidad por su unidad se sale de un numero de 64 bits.",
                    n.sitio,
                )
                .con_habia(format!("Se escribio `{}`.", n.cantidad))
                .con_hacer("pide menos"),
            );
            continue;
        };

        // El techo, y solo para el monton: es la unica clase que ESTE
        // compilador concede. Las demas las contesta el cargador, que sabe lo
        // que hay en la maquina -- y adivinarlo aqui seria decir que no a algo
        // que quiza si.
        if clase.numero == MONTON && t.monton_maximo() > 0 && cantidad > t.monton_maximo() {
            avisos.push(
                Aviso::nuevo(
                    codigos::NECESITA_DE_MAS,
                    "Este monton es mas grande de lo que se puede pedir.",
                    n.sitio,
                )
                .con_habia(format!(
                    "Se piden {cantidad} bytes y el techo esta en {}.",
                    t.monton_maximo()
                ))
                .con_hacer(
                    "el techo lo pone `[monton] maximo` en \
                     `lang/inti/necesidades.toml`, y subirlo es una linea",
                ),
            );
            continue;
        }

        // *** EL MOTIVO. No es rigor de estilo: `bef::requisitos::construir` se
        // niega a escribir un requisito obligatorio sin el, y lo dice con estas
        // palabras -- *"no se puede contestar"*. Sin esta comprobacion el fallo
        // saldria al empaquetar, hablando de bytes, y lejos de esta linea.
        let Some(motivo) = n.motivo.clone().filter(|m| !m.trim().is_empty()) else {
            avisos.push(
                Aviso::nuevo(
                    codigos::NECESITA_SIN_MOTIVO,
                    format!("`necesita {}` no dice para que.", n.clase),
                    n.sitio,
                )
                .con_habia(
                    "Un requisito puede tumbar el arranque, y un rechazo que no \
                     dice por que no se puede contestar.",
                )
                .con_hacer(
                    "escribe el motivo detras, entre comillas, por ejemplo: \
                     los pesos del modelo viven en RAM",
                ),
            );
            continue;
        };

        if !motivo.is_ascii() {
            avisos.push(
                Aviso::nuevo(
                    codigos::NOMBRE_NO_ASCII,
                    "El motivo de un requisito tiene que ser ASCII.",
                    n.sitio,
                )
                .con_habia("Va dentro del `.bex`, y los ficheros del sistema son ASCII.")
                .con_hacer("quita las tildes del motivo"),
            );
            continue;
        }

        out.push(Pedido {
            clase: clase.numero,
            nombre: n.clase.clone(),
            unidad: clase.unidad,
            cantidad,
            motivo,
        });
    }

    Cosecha::con(out, avisos)
}

/// **Cuanto monton pide este modulo**: lo declarado, o lo que dice la tabla.
pub fn monton_de(pedidos: &[Pedido], t: &Necesidades) -> u64 {
    pedidos
        .iter()
        .find(|p| p.clase == MONTON)
        .map(|p| p.cantidad)
        .unwrap_or_else(|| t.monton_por_defecto())
}
