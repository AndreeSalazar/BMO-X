//! La DISPOSICIÓN de un registro: qué byte ocupa cada campo dentro de su `01`.
//!
//! # Por qué NO se reutiliza `bmo_abi::types::disposicion`
//!
//! Aquella coloca miembros **alineados**, que es lo que manda el ABI de SysV
//! para un agregado de C. Aquí eso sería **veneno**: la disposición de un
//! registro COBOL *es el formato del fichero*. Un byte de relleno entre dos
//! campos no es una optimización de acceso — es un byte que aparece en el disco
//! y que el sistema de al lado va a leer como si fuera un dígito.
//!
//! Así que un registro va **byte a byte, sin huecos**, y por eso vive aquí. Es
//! la excepción que confirma la regla de la casa: se comparte la REGLA cuando
//! es la misma, y ésta no lo es.
//!
//! # El modelo, y qué decide (camino B de `PLAN_BANCA.md` §1.0)
//!
//! ```text
//!   01 REG-CUENTA.                          offset  bytes
//!       05 CTA-NUMERO PIC 9(10).                 0     10
//!       05 CTA-SALDO  PIC S9(7)V99 COMP-3.      10      5
//!       05 CTA-ESTADO PIC 9.                    15      1
//!                                              ─────────
//!                                REG-CUENTA:    0     16
//! ```
//!
//! Los 5 bytes del saldo salen de sus **nueve** dígitos (7 enteros + 2
//! decimales): `9/2 + 1`. La `V` no ocupa y el signo va en el último nibble.
//!
//! Cada `01` que es un GRUPO tiene un **área de registro** de ese tamaño, y
//! cada campo de dentro **conserva además su ranura de trabajo** de 64 bits. El
//! área es la representación EXTERNA —lo que va y viene del disco— y la ranura
//! es donde se calcula.
//!
//! Eso no es un rodeo para no tocar el almacenamiento: es lo que dice COBOL. El
//! área de registro sólo vale entre un `READ` y el siguiente, y por eso la
//! traducción entre las dos vive exactamente en esos dos puntos.
//!
//! # El tamaño de un campo lo decide su USAGE
//!
//! Y ahí ya no hay nada que inventar: `PicField::size()` lo sabe desde que
//! entró `COMP-3`. Un `PIC S9(7)V99 COMP-3` mide 5 bytes en el disco de un
//! mainframe y mide 5 aquí.

use std::collections::HashMap;

use crate::ast::{CobolError, DataItem};

/// Cómo está escrito un campo **en el área**, o sea en el disco.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Codificacion {
    /// No es un campo: es el nombre de lo que hay debajo.
    Grupo,
    /// `DISPLAY` — un byte por dígito, signo sobrepunzado en el último.
    Zonado,
    /// `COMP-3` — dos dígitos por byte, signo en el último nibble.
    Empaquetado,
    /// `PIC X` — bytes tal cual. Todavía no se guarda como texto (tarea 0.7).
    Texto,
    /// PIC de **EDICIÓN** (`$$$,$$9.99`, `ZZ9.99`, `120.00CR`).
    ///
    /// ★ No es una codificación de almacenamiento: es una MÁSCARA de
    /// presentación. Un campo así no va a un fichero de intercambio — va a un
    /// informe. Decir que es "zonado de 7 bytes" sería mentir en el sitio donde
    /// menos se puede: quien lea el copybook creería que ahí hay siete dígitos.
    Editado,
}

impl Codificacion {
    pub fn nombre(self) -> &'static str {
        match self {
            Codificacion::Grupo => "GRUPO",
            Codificacion::Zonado => "ZONED",
            Codificacion::Empaquetado => "PACKED",
            Codificacion::Texto => "TEXTO",
            Codificacion::Editado => "EDITADO",
        }
    }
}

/// Dónde vive un campo dentro de su registro, y **cómo está escrito**.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Campo {
    /// El `01` al que pertenece. Un dato suelto es su propia raíz.
    pub raiz: String,
    /// Byte en el que empieza, contando desde el principio del `01`.
    pub offset: u32,
    /// Cuántos bytes ocupa. Para un grupo, la suma de los de dentro.
    pub bytes: u32,
    /// Un GRUPO no tiene PIC: es el nombre de lo que hay debajo.
    pub es_grupo: bool,
    /// Nivel con el que se declaró. Se guarda para poder decir el porqué en los
    /// errores sin volver a buscar el dato.
    pub nivel: u32,
    /// Cómo se escribe en el área. Va aquí y no en el codegen porque **es parte
    /// de la disposición**: dos campos en el mismo byte con distinta
    /// codificación son dos ficheros distintos.
    pub codificacion: Codificacion,
    /// Dígitos que declara la PIC. Un grupo no tiene.
    pub digitos: u32,
    /// Dígitos tras la coma implícita.
    pub escala: u32,
    /// Lleva `S`.
    pub con_signo: bool,
    /// El texto de la PIC tal cual se escribió, para el copybook.
    pub pic: Option<String>,
    /// `OCCURS n` — cuántas veces se repite.
    pub veces: u32,
}

/// La disposición de todos los datos de un programa.
#[derive(Debug, Default, Clone)]
pub struct Disposicion {
    campos: HashMap<String, Campo>,
    /// Las raíces en orden de declaración: el orden manda para reservar.
    raices: Vec<String>,
}

impl Disposicion {
    pub fn campo(&self, nombre: &str) -> Option<&Campo> {
        self.campos.get(&nombre.to_ascii_uppercase())
    }

    /// Los `01` en orden de declaración.
    pub fn raices(&self) -> &[String] {
        &self.raices
    }

    /// ¿Es un grupo, o sea algo que se mueve como un bloque de bytes?
    pub fn es_grupo(&self, nombre: &str) -> bool {
        self.campo(nombre).map(|c| c.es_grupo).unwrap_or(false)
    }

    /// Los campos ELEMENTALES de un grupo, en orden de byte.
    ///
    /// Es lo que recorren el empaquetado y el desempaquetado del área, y va en
    /// orden de offset y no de declaración a propósito: así el emisor escribe
    /// el área de izquierda a derecha y un `REDEFINES` futuro no puede colar un
    /// campo fuera de sitio sin que se note.
    pub fn hojas_de(&self, raiz: &str) -> Vec<(&String, &Campo)> {
        let raiz = raiz.to_ascii_uppercase();
        let mut v: Vec<_> = self
            .campos
            .iter()
            .filter(|(_, c)| !c.es_grupo && c.raiz == raiz)
            .collect();
        v.sort_by_key(|(_, c)| c.offset);
        v
    }
}

impl Disposicion {
    /// ★ EL COPYBOOK: el byte exacto de cada campo, escrito para una persona.
    ///
    /// # Por qué esto vale más de lo que parece
    ///
    /// En banca, el documento que dice *"el registro de cuentas mide 16 bytes y
    /// el saldo empieza en el 10, empaquetado, con dos decimales"* se llama
    /// **copybook**, y es lo que se intercambia para que dos sistemas lean el
    /// mismo fichero. Normalmente lo mantiene alguien a mano, y **siempre acaba
    /// mintiendo**: el código cambia y el documento no.
    ///
    /// Éste no puede mentir. Sale de **la misma tabla que usa el codegen** para
    /// emitir el `READ` y el `WRITE`. Si el layout cambia, el copybook cambia
    /// solo, porque no hay dos sitios donde pueda divergir.
    ///
    /// Es la regla de la casa —**tablas y no cerebros**— aplicada a la
    /// documentación: el documento no describe el formato, *es* el formato.
    /// `de_fichero` son los `01` que cuelgan de un `FD`. Se marcan porque **son
    /// los únicos que se intercambian de verdad**: un `01` de WORKING-STORAGE
    /// tiene disposición, pero nunca cruza a otro sistema. Mezclarlos sin decir
    /// cuál es cuál convertiría el documento en una lista de variables.
    pub fn copybook(&self, program_id: &str, de_fichero: &[String]) -> String {
        let es_de_fichero =
            |n: &str| de_fichero.iter().any(|r| r.eq_ignore_ascii_case(n));
        let mut s = String::new();
        s.push_str(&format!("* COPYBOOK de {program_id}\n"));
        s.push_str("* Generado por BMO COBOL desde la MISMA tabla que emite el READ y el\n");
        s.push_str("* WRITE. Si esto y el codigo no cuadran, es que este fichero es viejo.\n");
        s.push_str("*\n* Los offsets son BYTES desde el principio de su 01, sin relleno:\n");
        s.push_str("* un registro COBOL va pegado, porque esto es el formato del fichero.\n");
        s.push_str("*\n* Solo los marcados [FICHERO] cruzan a otro sistema. Los demas son\n");
        s.push_str("* de WORKING-STORAGE: tienen disposicion, pero no salen de aqui.\n");

        for raiz in &self.raices {
            let Some(cab) = self.campos.get(raiz) else { continue };
            let marca = if es_de_fichero(raiz) { "  [FICHERO]" } else { "" };
            let bytes = cab.bytes.max(1);
            let plural = if bytes == 1 { "byte" } else { "bytes" };
            s.push_str(&format!("\n{raiz}   ({bytes} {plural}){marca}\n"));

            // Todos los campos de esta raíz, en orden de byte y luego de nivel:
            // así un grupo sale ANTES que lo que contiene, que empieza en su
            // mismo offset.
            let mut campos: Vec<(&String, &Campo)> =
                self.campos.iter().filter(|(_, c)| c.raiz == *raiz).collect();
            campos.sort_by_key(|(_, c)| (c.offset, c.nivel));

            s.push_str("  desde  hasta  bytes  nivel  campo                 como     PICTURE\n");
            s.push_str("  -----  -----  -----  -----  --------------------  -------  -------------\n");
            for (nombre, c) in campos {
                if nombre == raiz && self.raices.contains(nombre) && c.es_grupo {
                    continue; // la cabecera ya lo dijo
                }
                let sangria = " ".repeat(((c.nivel.saturating_sub(1) / 4) as usize).min(4));
                let veces = if c.veces > 1 { format!(" x{}", c.veces) } else { String::new() };
                s.push_str(&format!(
                    "  {:>5}  {:>5}  {:>5}  {:>5}  {:<20}  {:<7}  {}{}\n",
                    c.offset,
                    c.offset + c.bytes,
                    c.bytes,
                    c.nivel,
                    format!("{sangria}{nombre}"),
                    c.codificacion.nombre(),
                    c.pic.as_deref().unwrap_or("-"),
                    veces,
                ));
            }

            // Y la leyenda de lo que un lector de fuera necesita saber para
            // interpretar los bytes sin adivinar.
            let hay_packed = self
                .campos
                .values()
                .any(|c| c.raiz == *raiz && c.codificacion == Codificacion::Empaquetado);
            let hay_zoned = self
                .campos
                .values()
                .any(|c| c.raiz == *raiz && c.codificacion == Codificacion::Zonado);
            if hay_zoned {
                s.push_str(
                    "\n  ZONED   un byte ASCII por digito. Con signo, el ULTIMO byte lleva\n\
                     \x20         la banda 0x70-0x79 ('p'..'y') si es negativo. La S no ocupa.\n",
                );
            }
            if hay_packed {
                s.push_str(
                    "\n  PACKED  dos digitos por byte. El ULTIMO nibble es el signo:\n\
                     \x20         C positivo, D negativo, F sin signo. Al leer, B tambien\n\
                     \x20         es negativo.\n",
                );
            }
            let con_escala: Vec<_> = {
                let mut v: Vec<_> = self
                    .campos
                    .iter()
                    .filter(|(_, c)| c.raiz == *raiz && c.escala > 0)
                    .collect();
                v.sort_by_key(|(_, c)| c.offset);
                v
            };
            if !con_escala.is_empty() {
                s.push_str("\n  La coma es IMPLICITA y no ocupa byte:\n");
                for (nombre, c) in con_escala {
                    s.push_str(&format!(
                        "\x20         {nombre}: {} decimales\n",
                        c.escala
                    ));
                }
            }
            if self
                .campos
                .values()
                .any(|c| c.raiz == *raiz && c.codificacion == Codificacion::Editado)
            {
                s.push_str(
                    "\n  EDITADO es una MASCARA de presentacion, no almacenamiento. Un campo\n\
                     \x20         asi va a un informe, NO a un fichero de intercambio: los\n\
                     \x20         bytes que salen son la mascara aplicada, no sus digitos.\n",
                );
            }
        }
        s
    }
}

/// Un entero escalado, escrito con su coma.
///
/// `-123456` con escala 2 es `-1234.56`. Es la misma regla que emite
/// `bmo_lower::fmt::formatear_decimal_scaled`, pero resuelta aquí: el visor
/// mira un fichero **sin ejecutar nada**, así que no puede pedirle el número a
/// un emisor de instrucciones.
fn con_coma(v: i64, escala: u32) -> String {
    if escala == 0 {
        return v.to_string();
    }
    let signo = if v < 0 { "-" } else { "" };
    let m = v.unsigned_abs();
    let div = 10u64.pow(escala);
    format!("{signo}{}.{:0ancho$}", m / div, m % div, ancho = escala as usize)
}

impl Disposicion {
    /// ★ EL VISOR: un fichero de registros binarios, DECODIFICADO.
    ///
    /// # Por qué esto hace falta desde hoy
    ///
    /// En cuanto un `COMP-3` sale al disco, el fichero **deja de poderse
    /// mirar**: los nibbles no son texto y un `cat` enseña basura. El copybook
    /// dice qué hay dentro, pero no lo *enseña*.
    ///
    /// Y hay una cosa que este visor sí puede prometer y una herramienta de
    /// fuera no: **lee con la misma regla que escribió el programa**. Los
    /// decodificadores son `packed::desempaquetar_en_rust` y
    /// `zoned::leer_en_rust`, y hay tests que los comparan contra los EMITIDOS
    /// sobre todos los patrones de dos bytes. Si divergieran, el visor
    /// enseñaría un importe y el programa leería otro — que es peor que no
    /// tener visor.
    ///
    /// # El tamaño que no cuadra
    ///
    /// Si el fichero no es múltiplo del registro, se dice y se enseña lo que
    /// sobra. Ése es **el síntoma clásico de un copybook equivocado**, y callarlo
    /// dejaría al que mira creyendo que el último registro es raro.
    pub fn ver(&self, raiz: &str, datos: &[u8], max: usize) -> String {
        let raiz = raiz.to_ascii_uppercase();
        let Some(cab) = self.campos.get(&raiz) else {
            return format!("no hay ningun registro llamado {raiz}\n");
        };
        let n = cab.bytes as usize;
        if n == 0 {
            return format!("{raiz} no mide nada: no es un registro\n");
        }
        let enteros = datos.len() / n;
        let sobra = datos.len() % n;

        let mut s = String::new();
        s.push_str(&format!(
            "* {} bytes = {enteros} registro(s) de {n}, segun {raiz}\n",
            datos.len()
        ));
        if sobra != 0 {
            s.push_str(&format!(
                "* ⚠ SOBRAN {sobra} BYTES. O el fichero esta truncado, o este no es\n\
                 *   su copybook. Un registro de largo fijo divide exacto.\n"
            ));
        }

        let hojas = self.hojas_de(&raiz);
        let ancho = hojas.iter().map(|(n, _)| n.len()).max().unwrap_or(8);

        for i in 0..enteros.min(max) {
            let reg = &datos[i * n..(i + 1) * n];
            s.push_str(&format!("\n#{:<4} byte {}\n", i + 1, i * n));
            for (nombre, c) in &hojas {
                let trozo = &reg[c.offset as usize..(c.offset + c.bytes) as usize];
                let valor = match c.codificacion {
                    Codificacion::Empaquetado => {
                        con_coma(bmo_lower::packed::desempaquetar_en_rust(trozo), c.escala)
                    }
                    Codificacion::Zonado => {
                        con_coma(bmo_lower::zoned::leer_en_rust(trozo), c.escala)
                    }
                    // Lo que no se sabe decodificar se enseña TAL CUAL en vez de
                    // inventarle un número. Un visor que adivina es peor que uno
                    // que dice "no sé".
                    _ => trozo.iter().map(|b| format!("{b:02X}")).collect::<Vec<_>>().join(" "),
                };
                let crudo: String =
                    trozo.iter().map(|b| format!("{b:02X}")).collect::<Vec<_>>().join(" ");
                s.push_str(&format!(
                    "  {nombre:<ancho$}  {valor:>16}   {crudo}\n",
                    ancho = ancho
                ));
            }
        }
        if enteros > max {
            s.push_str(&format!("\n… y {} registro(s) mas\n", enteros - max));
        }
        if sobra != 0 {
            let cola: String = datos[enteros * n..]
                .iter()
                .map(|b| format!("{b:02X}"))
                .collect::<Vec<_>>()
                .join(" ");
            s.push_str(&format!("\nLO QUE SOBRA ({sobra} bytes):  {cola}\n"));
        }
        s
    }
}

/// Calcula la disposición de todos los datos, en orden de declaración.
///
/// ## Cómo se cierra un grupo
///
/// Un grupo no dice cuánto mide: se sabe cuando aparece algo de nivel **igual o
/// menor**, que es lo que lo cierra. Por eso hay una pila — y por eso el
/// recorrido tiene que ser en orden de declaración, que es el único sitio donde
/// esa información existe.
pub fn calcular(items: &[DataItem]) -> Result<Disposicion, CobolError> {
    let mut d = Disposicion::default();
    // (nivel, nombre, offset donde empezó)
    let mut pila: Vec<(u32, String, u32)> = Vec::new();
    let mut cursor = 0u32;
    let mut raiz = String::new();

    for item in items {
        // Un 88 es un apodo de una comparación: no ocupa ni un byte y no entra
        // en la disposición de nadie.
        if item.level == 88 {
            continue;
        }
        let nombre = item.name.to_ascii_uppercase();

        // Cerrar los grupos que este nivel deja atrás.
        while pila.last().map(|(n, _, _)| *n >= item.level).unwrap_or(false) {
            let (_, cerrado, ini) = pila.pop().unwrap();
            if let Some(c) = d.campos.get_mut(&cerrado) {
                c.bytes = cursor - ini;
            }
        }

        // Sin nada abierto, esto empieza un registro nuevo.
        if pila.is_empty() {
            cursor = 0;
            raiz = nombre.clone();
            d.raices.push(nombre.clone());
        }

        if d.campos.contains_key(&nombre) {
            return Err(CobolError::new(
                0,
                format!(
                    "'{nombre}' esta declarado dos veces: con dos campos del mismo nombre \
                     no hay forma de saber a cual va un MOVE"
                ),
            ));
        }

        let offset = cursor;
        let es_grupo = item.pic.is_none();
        // La codificación sale del USAGE y de la PIC, que es donde vive. Un
        // campo no numérico es texto aunque su USAGE diga otra cosa: el parser
        // ya rechaza un `COMP-3` sobre una `PIC X`.
        let codificacion = match (&item.pic_field, es_grupo) {
            (_, true) => Codificacion::Grupo,
            // La edición va PRIMERO: un `$$$,$$9.99` es numérico y no es
            // almacenamiento, y confundirlo con uno es el error que este
            // documento existe para no cometer.
            _ if item.edicion.is_some() => Codificacion::Editado,
            (Some(f), _) if !f.numeric => Codificacion::Texto,
            (Some(_), _) if item.usage == crate::pic::Usage::Comp3 => Codificacion::Empaquetado,
            (Some(_), _) => Codificacion::Zonado,
            (None, _) => Codificacion::Texto,
        };
        let (digitos, escala, con_signo) = item
            .pic_field
            .as_ref()
            .map(|f| (f.total_digits(), f.scale, f.signed))
            .unwrap_or((0, 0, false));
        d.campos.insert(
            nombre.clone(),
            Campo {
                raiz: raiz.clone(),
                offset,
                bytes: 0,
                es_grupo,
                nivel: item.level,
                codificacion,
                digitos,
                escala,
                con_signo,
                pic: item.pic.clone(),
                veces: item.elementos(),
            },
        );

        if es_grupo {
            // Su tamaño se sabrá al cerrarlo.
            pila.push((item.level, nombre, offset));
        } else {
            // ★ Sin alineado: los bytes van pegados, porque esto es el formato
            // del fichero y un hueco aquí es un byte de más en el disco.
            let bytes = item.storage_size() as u32 * item.elementos();
            d.campos.get_mut(&nombre).unwrap().bytes = bytes;
            cursor += bytes;
        }
    }

    // Y cerrar lo que quede abierto al final del programa.
    while let Some((_, cerrado, ini)) = pila.pop() {
        if let Some(c) = d.campos.get_mut(&cerrado) {
            c.bytes = cursor - ini;
        }
    }

    Ok(d)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pic::Usage;

    fn dato(nivel: u32, nombre: &str, pic: Option<&str>) -> DataItem {
        DataItem::new(nivel, nombre.into(), pic.map(|p| p.into()), None)
    }

    fn comp3(nivel: u32, nombre: &str, pic: &str) -> DataItem {
        DataItem::new_with_usage(nivel, nombre.into(), Some(pic.into()), None, Usage::Comp3)
    }

    /// ★ El registro de una cuenta, con los tres tipos mezclados. Es el caso
    /// que decide: los offsets tienen que caer **pegados**, sin relleno, porque
    /// esto es lo que va al disco.
    #[test]
    fn los_campos_van_pegados_sin_relleno() {
        let items = vec![
            dato(1, "REG-CUENTA", None),
            dato(5, "CTA-NUMERO", Some("9(10)")),
            comp3(5, "CTA-SALDO", "S9(7)V99"),
            dato(5, "CTA-ESTADO", Some("9")),
        ];
        let d = calcular(&items).unwrap();

        assert_eq!(d.campo("CTA-NUMERO").unwrap().offset, 0);
        assert_eq!(d.campo("CTA-NUMERO").unwrap().bytes, 10);

        // COMP-3 de 9 dígitos = 9/2+1 = 5 bytes. Empieza justo detrás.
        assert_eq!(d.campo("CTA-SALDO").unwrap().offset, 10);
        assert_eq!(d.campo("CTA-SALDO").unwrap().bytes, 5);

        assert_eq!(d.campo("CTA-ESTADO").unwrap().offset, 15);
        assert_eq!(d.campo("CTA-ESTADO").unwrap().bytes, 1);

        // Y el grupo mide la suma, ni un byte más.
        let reg = d.campo("REG-CUENTA").unwrap();
        assert!(reg.es_grupo);
        assert_eq!(reg.offset, 0);
        assert_eq!(reg.bytes, 16, "un registro con relleno no es el del fichero");
    }

    /// Grupos DENTRO de grupos: el nivel 10 cierra cuando llega otro 05.
    #[test]
    fn los_grupos_se_anidan_y_se_cierran_solos() {
        let items = vec![
            dato(1, "REG", None),
            dato(5, "NOMBRE-COMPLETO", None),
            dato(10, "NOMBRE", Some("9(4)")),
            dato(10, "APELLIDO", Some("9(6)")),
            dato(5, "EDAD", Some("9(3)")),
        ];
        let d = calcular(&items).unwrap();

        assert_eq!(d.campo("NOMBRE").unwrap().offset, 0);
        assert_eq!(d.campo("APELLIDO").unwrap().offset, 4);
        // El grupo de en medio mide lo suyo y se cerró al llegar el 05 EDAD.
        let nc = d.campo("NOMBRE-COMPLETO").unwrap();
        assert!(nc.es_grupo);
        assert_eq!((nc.offset, nc.bytes), (0, 10));
        // Y EDAD sigue detrás, no encima.
        assert_eq!(d.campo("EDAD").unwrap().offset, 10);
        assert_eq!(d.campo("REG").unwrap().bytes, 13);
    }

    /// Cada `01` empieza en su propio cero: son registros distintos, no un
    /// bloque continuo.
    #[test]
    fn cada_registro_empieza_en_su_propio_cero() {
        let items = vec![
            dato(1, "UNO", None),
            dato(5, "A", Some("9(4)")),
            dato(1, "DOS", None),
            dato(5, "B", Some("9(4)")),
        ];
        let d = calcular(&items).unwrap();
        assert_eq!(d.campo("A").unwrap().offset, 0);
        assert_eq!(d.campo("B").unwrap().offset, 0, "el segundo 01 arranco donde acabo el primero");
        assert_eq!(d.campo("A").unwrap().raiz, "UNO");
        assert_eq!(d.campo("B").unwrap().raiz, "DOS");
        assert_eq!(d.raices(), &["UNO".to_string(), "DOS".to_string()]);
    }

    /// Un `01` con PIC es un dato suelto: es su propia raíz y mide lo suyo.
    #[test]
    fn un_01_con_pic_es_un_dato_suelto() {
        let items = vec![dato(1, "SALDO", Some("S9(7)V99"))];
        let d = calcular(&items).unwrap();
        let c = d.campo("SALDO").unwrap();
        assert!(!c.es_grupo);
        assert_eq!((c.raiz.as_str(), c.offset), ("SALDO", 0));
    }

    /// Un `OCCURS` ocupa sus `n` veces dentro del registro, y lo que viene
    /// detrás empieza pasada la tabla entera.
    #[test]
    fn un_occurs_ocupa_todas_sus_veces() {
        let mut tabla = dato(5, "MES", Some("9(4)"));
        tabla.occurs = Some(12);
        let items = vec![dato(1, "REG", None), tabla, dato(5, "TOTAL", Some("9(6)"))];
        let d = calcular(&items).unwrap();
        assert_eq!(d.campo("MES").unwrap().bytes, 4 * 12);
        assert_eq!(d.campo("TOTAL").unwrap().offset, 48);
    }

    /// Los 88 no ocupan nada, y por eso no pueden mover el campo de al lado.
    #[test]
    fn un_88_no_mueve_a_nadie() {
        let mut ochenta = dato(88, "ES-BUENO", None);
        ochenta.value = Some("1".into());
        let items = vec![
            dato(1, "REG", None),
            dato(5, "VALE", Some("9")),
            ochenta,
            dato(5, "OTRO", Some("9(4)")),
        ];
        let d = calcular(&items).unwrap();
        assert_eq!(d.campo("OTRO").unwrap().offset, 1);
        assert_eq!(d.campo("REG").unwrap().bytes, 5);
        assert!(d.campo("ES-BUENO").is_none());
    }

    /// Las hojas salen en orden de BYTE, que es el orden en que se escribe el
    /// área — no en el de declaración ni en el que quiera un `HashMap`.
    #[test]
    fn las_hojas_salen_en_orden_de_byte() {
        let items = vec![
            dato(1, "REG", None),
            dato(5, "A", Some("9")),
            dato(5, "B", Some("9(3)")),
            dato(5, "C", Some("9(2)")),
        ];
        let d = calcular(&items).unwrap();
        let nombres: Vec<&str> = d.hojas_de("REG").iter().map(|(n, _)| n.as_str()).collect();
        assert_eq!(nombres, vec!["A", "B", "C"]);
    }

    /// ★ El copybook dice los bytes EXACTOS, y sale de la misma tabla que emite
    /// el `READ`. Este test es lo que impide que se separen.
    #[test]
    fn el_copybook_dice_donde_esta_cada_byte() {
        let items = vec![
            dato(1, "REG-CUENTA", None),
            dato(5, "CTA-NUMERO", Some("9(10)")),
            comp3(5, "CTA-SALDO", "S9(7)V99"),
            dato(5, "CTA-ESTADO", Some("9")),
        ];
        let d = calcular(&items).unwrap();
        let cb = d.copybook("CUENTAS", &["REG-CUENTA".to_string()]);

        assert!(cb.contains("REG-CUENTA   (16 bytes)"), "{cb}");
        // Los tres campos con su tramo de bytes.
        assert!(cb.contains("0     10     10"), "falta CTA-NUMERO:\n{cb}");
        assert!(cb.contains("10     15      5"), "falta CTA-SALDO:\n{cb}");
        assert!(cb.contains("15     16      1"), "falta CTA-ESTADO:\n{cb}");
        // Y la codificación de cada uno, que es lo que un lector de fuera
        // necesita para no adivinar.
        assert!(cb.contains("PACKED"), "{cb}");
        assert!(cb.contains("ZONED"), "{cb}");
        // La leyenda del signo, sin la cual los bytes no se pueden interpretar.
        assert!(cb.contains("0x70-0x79"), "falta como se lee el signo zonado");
        assert!(cb.contains("C positivo, D negativo"), "falta el nibble de signo");
        // Y la coma implícita, que no ocupa byte y se pierde si no se dice.
        assert!(cb.contains("CTA-SALDO: 2 decimales"), "{cb}");
    }

    /// La codificación sale del `USAGE`, y es parte de la disposición: dos
    /// campos en el mismo byte con distinta codificación son dos ficheros.
    #[test]
    fn la_codificacion_va_en_la_disposicion() {
        let items = vec![
            dato(1, "R", None),
            dato(5, "A", Some("9(4)")),
            comp3(5, "B", "S9(5)"),
            dato(5, "C", Some("X(4)")),
        ];
        let d = calcular(&items).unwrap();
        assert_eq!(d.campo("A").unwrap().codificacion, Codificacion::Zonado);
        assert_eq!(d.campo("B").unwrap().codificacion, Codificacion::Empaquetado);
        assert_eq!(d.campo("C").unwrap().codificacion, Codificacion::Texto);
        assert_eq!(d.campo("R").unwrap().codificacion, Codificacion::Grupo);
        assert!(d.campo("B").unwrap().con_signo);
        assert!(!d.campo("A").unwrap().con_signo);
    }

    /// Dos campos con el mismo nombre no se pueden distinguir en un `MOVE`. En
    /// COBOL de verdad se resuelve con `A OF REG`, que todavía no existe — así
    /// que se dice en vez de quedarse con uno de los dos.
    #[test]
    fn dos_campos_con_el_mismo_nombre_se_rechazan() {
        let items = vec![
            dato(1, "UNO", None),
            dato(5, "IMPORTE", Some("9(4)")),
            dato(1, "DOS", None),
            dato(5, "IMPORTE", Some("9(4)")),
        ];
        let err = calcular(&items).unwrap_err();
        assert!(err.message.contains("dos veces"), "{}", err.message);
    }
}
