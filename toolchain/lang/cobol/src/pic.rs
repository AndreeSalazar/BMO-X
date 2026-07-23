//! Parser de cláusulas PICTURE (PIC) — 100% BMO, sin dependencias ajenas.
//!
//! Reemplaza a `gnucobol-rs` (GPL): la PIC es el corazón del DATA DIVISION y
//! la esencia de COBOL debe ser TUYA. Extrae lo que importa para la
//! aritmética decimal exacta que Grace Hopper diseñó para la banca:
//!
//! ```text
//!   PIC 9(5)V99   → 5 dígitos enteros · 2 decimales (scale=2) · sin signo
//!   PIC S9(7)V99  → signed · 7 enteros · 2 decimales
//!   PIC X(10)     → 10 caracteres alfanuméricos (no numérico)
//! ```
//!
//! La `V` es el punto decimal IMPLÍCITO (no ocupa byte): marca dónde queda la
//! coma. `scale` = dígitos tras la V = cuántos decimales. Ese número es la
//! llave del decimal exacto: un `PIC 9(3)V99` guarda $10.05 como el entero
//! 1005 (centavos), y sumar centavos con `add` es suma decimal exacta.

/// Representación en memoria del dato (por ahora DISPLAY; COMP/COMP-3 =
/// packed decimal vendrán después, sin cambiar esta interfaz).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Usage {
    Display,
    Comp,
    Comp3,
}

/// Un campo PIC ya analizado.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PicField {
    /// Dígitos enteros (antes de la V).
    pub integer_digits: u32,
    /// Dígitos decimales (tras la V). **La escala** — cero = entero.
    pub scale: u32,
    /// Lleva `S` (con signo).
    pub signed: bool,
    /// Numérico (9/S/V/Z) vs texto (X/A).
    pub numeric: bool,
    /// Caracteres para PIC X(n)/A(n).
    pub char_count: u32,
    pub usage: Usage,
}

impl PicField {
    /// Total de dígitos numéricos (enteros + decimales).
    pub fn total_digits(&self) -> u32 {
        self.integer_digits + self.scale
    }

    /// Tamaño en bytes del almacenamiento.
    pub fn size(&self) -> usize {
        if !self.numeric {
            return self.char_count.max(1) as usize;
        }
        match self.usage {
            // DISPLAY: 1 byte por dígito (zoned). El signo va sobrepunzado.
            Usage::Display => self.total_digits().max(1) as usize,
            // COMP-3 (packed): 2 dígitos por byte + nibble de signo.
            Usage::Comp3 => (self.total_digits() as usize / 2) + 1,
            // COMP (binario): según los dígitos.
            Usage::Comp => {
                let d = self.total_digits();
                if d <= 4 { 2 } else if d <= 9 { 4 } else { 8 }
            }
        }
    }
}

/// Analiza una cláusula PIC. `usage` viene del `USAGE`/`COMP` del data item.
pub fn parse_pic(pic: &str, usage: Usage) -> Result<PicField, String> {
    let s = pic.trim().to_uppercase();
    let bytes = s.as_bytes();

    let mut integer_digits = 0u32;
    let mut scale = 0u32;
    let mut char_count = 0u32;
    let mut signed = false;
    let mut numeric = false;
    let mut seen_v = false;

    let mut i = 0usize;
    while i < bytes.len() {
        let sym = bytes[i] as char;
        i += 1;

        // Cuenta de repetición opcional: SÍMBOLO(n).
        let mut count = 1u32;
        if i < bytes.len() && bytes[i] == b'(' {
            let start = i + 1;
            let mut j = start;
            while j < bytes.len() && bytes[j] != b')' {
                j += 1;
            }
            if j >= bytes.len() {
                return Err(format!("PIC '{pic}': falta ')'"));
            }
            count = s[start..j]
                .trim()
                .parse()
                .map_err(|_| format!("PIC '{pic}': repetición inválida"))?;
            i = j + 1;
        }

        match sym {
            // Dígitos (9) y dígitos con supresión de ceros (Z, *): posiciones.
            '9' | 'Z' | '*' => {
                numeric = true;
                if seen_v {
                    scale += count;
                } else {
                    integer_digits += count;
                }
            }
            // Signo: no ocupa posición de dígito (DISPLAY lo sobrepunza).
            'S' => {
                signed = true;
                numeric = true;
            }
            // Punto decimal implícito.
            'V' => {
                seen_v = true;
                numeric = true;
            }
            // Alfanumérico / alfabético.
            'X' | 'A' => {
                char_count += count;
            }
            ' ' => {}
            other => {
                return Err(format!("PIC '{pic}': símbolo no soportado '{other}'"));
            }
        }
    }

    if !numeric && char_count == 0 {
        return Err(format!("PIC '{pic}': vacía o sin símbolos válidos"));
    }

    Ok(PicField {
        integer_digits,
        scale,
        signed,
        numeric,
        char_count,
        usage,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn money_pic_scale_two() {
        let f = parse_pic("9(5)V99", Usage::Display).unwrap();
        assert_eq!(f.integer_digits, 5);
        assert_eq!(f.scale, 2); // ← 2 decimales: centavos exactos
        assert!(!f.signed);
        assert!(f.numeric);
        assert_eq!(f.total_digits(), 7);
    }

    #[test]
    fn signed_money() {
        let f = parse_pic("S9(7)V99", Usage::Comp3).unwrap();
        assert!(f.signed);
        assert_eq!(f.integer_digits, 7);
        assert_eq!(f.scale, 2);
    }

    #[test]
    fn expanded_and_text() {
        assert_eq!(parse_pic("9999", Usage::Display).unwrap().integer_digits, 4);
        let x = parse_pic("X(10)", Usage::Display).unwrap();
        assert!(!x.numeric);
        assert_eq!(x.char_count, 10);
        assert_eq!(x.size(), 10);
    }

    #[test]
    fn integer_has_scale_zero() {
        assert_eq!(parse_pic("9(4)", Usage::Display).unwrap().scale, 0);
    }
}
