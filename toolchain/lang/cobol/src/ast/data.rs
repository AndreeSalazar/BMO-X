use crate::ast::error::CobolError;
use crate::edicion::Plantilla;
use crate::pic::{parse_pic, PicField, Usage};

/// Uno de los valores de un nivel 88.
///
/// Un nombre de condicion no compara con UN valor: compara con un conjunto.
/// `88 LABORABLE VALUE 1 THRU 5.` y `88 FESTIVO VALUE 6, 7.` son las dos formas
/// que escribe todo el mundo, y las dos estaban rechazadas porque expandirlas
/// pide un `OR` que el analizador de condiciones no tenia.
#[derive(Debug, Clone, PartialEq)]
pub enum Valor88 {
    Uno(String),
    /// `VALUE 1 THRU 5` -- los dos extremos INCLUIDOS, como manda el estandar.
    Rango(String, String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct DataItem {
    pub level: u32,
    pub name: String,
    pub pic: Option<String>,
    pub pic_field: Option<PicField>,
    /// La plantilla de edicion, si la PIC es de PRESENTACION
    /// (`$$$,$$9.99`) en vez de de calculo (`S9(7)V99`). Lo que la lleva no
    /// se guarda distinto --sigue siendo un entero escalado-- pero al
    /// ensenarlo no se escribe el numero: se escribe la mascara.
    pub edicion: Option<Plantilla>,
    pub value: Option<String>,
    pub usage: Usage,
    /// De quien es este `88`. Un nombre de condicion no es un dato: es un
    /// APODO de una comparacion sobre el dato que lo precede. `None` en todo
    /// lo que no sea nivel 88.
    pub padre: Option<String>,
    /// Los valores con los que compara un nivel 88. Vacio en todo lo demas.
    ///
    /// Va aparte de `value` porque un 88 puede tener varios y `value` es uno
    /// solo: dejarlo ahi obligaria a que cada consumidor volviera a partir el
    /// texto, y el que se olvidara compararia solo con el primero **en
    /// silencio** -- que es como estaba antes de rechazarlo.
    pub valores: Vec<Valor88>,
    /// `OCCURS <n> TIMES` -- cuantas veces se repite el dato.
    ///
    /// `None` = un dato suelto. `Some(n)` = una TABLA de `n` elementos, y
    /// entonces el nombre **exige subindice**: `TOTAL(I)`. Un `MOVE 0 TO
    /// TOTAL` sobre una tabla no es "el primero", es una pregunta sin
    /// respuesta, y se rechaza.
    pub occurs: Option<u32>,
}

/// Analiza una PIC decidiendo primero de cual de las dos familias es.
///
/// Son dos gramaticas distintas y por eso hay dos analizadores: `parse_pic`
/// sabe de `9`, `S` y `V` --cuantos digitos y donde cae la coma-- y se atraganta
/// con un `$`. Mandarle una PIC editada devolvia error, y como el error se
/// tragaba con `.ok()`, el dato acababa con escala 0: `MOVE 19.99` guardaba
/// 19 y los centavos desaparecian sin que nadie dijera nada.
fn analizar_pic(pic: &str, usage: Usage) -> Result<(PicField, Option<Plantilla>), String> {
    if !Plantilla::es_editada(pic) {
        return Ok((parse_pic(pic, usage)?, None));
    }
    let plantilla = Plantilla::parse(pic)?;
    let escala = plantilla.escala;
    let campo = PicField {
        integer_digits: plantilla.digitos() as u32 - escala,
        scale: escala,
        // Una PIC editada puede ensenar signo (`-`, `CR`, `DB`), asi que el
        // dato que la alimenta se guarda con signo. Al reves --guardarlo sin
        // signo-- un saldo en rojo saldria en verde.
        signed: true,
        numeric: true,
        char_count: 0,
        usage,
    };
    Ok((campo, Some(plantilla)))
}

impl DataItem {
    pub fn new(level: u32, name: String, pic: Option<String>, value: Option<String>) -> Self {
        Self::new_with_usage(level, name, pic, value, Usage::Display)
    }

    pub fn new_with_usage(
        level: u32,
        name: String,
        pic: Option<String>,
        value: Option<String>,
        usage: Usage,
    ) -> Self {
        let (pic_field, edicion) = match pic.as_deref().map(|p| analizar_pic(p, usage)) {
            Some(Ok((campo, plantilla))) => (Some(campo), plantilla),
            _ => (None, None),
        };
        DataItem {
            level, name, pic, pic_field, edicion, value, usage,
            padre: None, valores: Vec::new(), occurs: None,
        }
    }

    /// Bytes de almacenamiento del item (minimo 8, alineado por el codegen).
    pub fn storage_size(&self) -> usize {
        self.pic_field.as_ref().map(|p| p.size()).unwrap_or(8)
    }

    /// Cuantos elementos tiene. Un dato suelto es una tabla de uno.
    pub fn elementos(&self) -> u32 {
        self.occurs.unwrap_or(1)
    }

    /// Escala decimal (digitos tras la V). 0 = entero. Es la llave del
    /// decimal exacto: el codegen escala los operandos a esta escala.
    pub fn scale(&self) -> u32 {
        self.pic_field.as_ref().map(|p| p.scale).unwrap_or(0)
    }
}

impl DataItem {
    pub fn from_parsed(
        level: u32,
        name: String,
        pic_str: Option<&str>,
        value: Option<&str>,
    ) -> Result<Self, CobolError> {
        let pic = pic_str.map(|s| s.to_string());
        let val = value.map(|s| s.to_string());
        let mut item = DataItem::new(level, name, pic, val);

        if let Some(p) = pic_str {
            match analizar_pic(p, Usage::Display) {
                Ok((field, plantilla)) => {
                    item.pic_field = Some(field);
                    item.edicion = plantilla;
                }
                Err(e) => return Err(CobolError::new(0, format!("invalid PIC '{p}': {e}"))),
            }
        }
        Ok(item)
    }
}
