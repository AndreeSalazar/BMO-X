//! El lexico de Ada. Propio, como manda la regla: cada lenguaje entero.
//!
//! ## Lo que Ada tiene y COBOL no
//!
//! - **Identificadores insensibles a mayusculas**: `Saldo`, `SALDO` y `saldo`
//!   son el MISMO nombre. Se guardan en mayuscula para comparar.
//! - **Comentarios con `--`** hasta el final de linea.
//! - **`:=` para asignar y `=` para comparar**, que es al reves de C y es una
//!   de las razones por las que Ada se eligio para lo critico: un `=` donde
//!   iba un `:=` no compila en vez de asignar en silencio.
//! - **Guiones bajos dentro de los numeros**: `1_000_000`. Se ignoran; son
//!   para el ojo.
//! - **Las comillas se duplican** dentro de un literal: `"di ""hola"""`.

/// Un componente lexico ya clasificado.
#[derive(Debug, Clone, PartialEq)]
pub enum Tok {
    /// Un nombre, siempre en MAYUSCULA (Ada no distingue).
    Ident(String),
    /// Un numero tal cual se escribio, sin los guiones bajos: `19.99`.
    Numero(String),
    /// El texto entre comillas, ya con las comillas dobles resueltas.
    Texto(String),
    /// Un simbolo: `:=`, `=>`, `..`, `(`, `)`, `;`, `,`, `:`, `+`, `-`, `*`,
    /// `/`, `=`, `/=`, `<`, `>`, `<=`, `>=`, `.`, `'`.
    Simbolo(String),
    /// Fin de la entrada.
    Fin,
}

/// Un componente con la linea en la que aparecio, para poder senalarla.
#[derive(Debug, Clone, PartialEq)]
pub struct Componente {
    pub tok: Tok,
    pub linea: usize,
}

/// Parte el fuente en componentes.
///
/// No falla: lo que no reconoce sale como `Simbolo` de un caracter y lo
/// rechaza el analisis, que es quien sabe que esperaba. Un lexer que opina
/// sobre gramatica es un lexer que hay que tocar dos veces.
pub fn lexar(fuente: &str) -> Vec<Componente> {
    let b: Vec<char> = fuente.chars().collect();
    let mut out = Vec::new();
    let mut i = 0usize;
    let mut linea = 1usize;

    while i < b.len() {
        let c = b[i];

        if c == '\n' {
            linea += 1;
            i += 1;
            continue;
        }
        if c.is_whitespace() {
            i += 1;
            continue;
        }
        // Comentario: `--` hasta el final de la linea.
        if c == '-' && i + 1 < b.len() && b[i + 1] == '-' {
            while i < b.len() && b[i] != '\n' {
                i += 1;
            }
            continue;
        }
        // Literal de texto. Dos comillas seguidas DENTRO son una comilla.
        if c == '"' {
            i += 1;
            let mut s = String::new();
            while i < b.len() {
                if b[i] == '"' {
                    if i + 1 < b.len() && b[i + 1] == '"' {
                        s.push('"');
                        i += 2;
                        continue;
                    }
                    i += 1;
                    break;
                }
                if b[i] == '\n' {
                    linea += 1;
                }
                s.push(b[i]);
                i += 1;
            }
            out.push(Componente { tok: Tok::Texto(s), linea });
            continue;
        }
        // Nombre: letra seguida de letras, digitos y guiones bajos.
        if c.is_alphabetic() {
            let mut s = String::new();
            while i < b.len() && (b[i].is_alphanumeric() || b[i] == '_') {
                s.push(b[i]);
                i += 1;
            }
            out.push(Componente { tok: Tok::Ident(s.to_ascii_uppercase()), linea });
            continue;
        }
        // Numero. El punto solo entra si le sigue un digito: en `1..5` los dos
        // puntos son el rango, no la coma decimal del 1.
        if c.is_ascii_digit() {
            let mut s = String::new();
            while i < b.len() {
                if b[i].is_ascii_digit() {
                    s.push(b[i]);
                    i += 1;
                } else if b[i] == '_' {
                    // Separador de millares: es para el ojo, no para el valor.
                    i += 1;
                } else if b[i] == '.' && i + 1 < b.len() && b[i + 1].is_ascii_digit() {
                    s.push('.');
                    i += 1;
                } else {
                    break;
                }
            }
            out.push(Componente { tok: Tok::Numero(s), linea });
            continue;
        }
        // Simbolos de dos caracteres primero: `:=` contiene `:`.
        let dos: String = b[i..(i + 2).min(b.len())].iter().collect();
        if matches!(dos.as_str(), ":=" | "=>" | ".." | "/=" | "<=" | ">=" | "**" | "<>") {
            out.push(Componente { tok: Tok::Simbolo(dos), linea });
            i += 2;
            continue;
        }
        out.push(Componente { tok: Tok::Simbolo(c.to_string()), linea });
        i += 1;
    }

    out.push(Componente { tok: Tok::Fin, linea });
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn toks(s: &str) -> Vec<Tok> {
        lexar(s).into_iter().map(|c| c.tok).collect()
    }

    #[test]
    fn los_nombres_no_distinguen_mayusculas() {
        assert_eq!(toks("Saldo saldo SALDO")[..3], [
            Tok::Ident("SALDO".into()),
            Tok::Ident("SALDO".into()),
            Tok::Ident("SALDO".into()),
        ]);
    }

    #[test]
    fn el_comentario_se_come_la_linea() {
        assert_eq!(toks("A -- esto no cuenta\nB"), [
            Tok::Ident("A".into()),
            Tok::Ident("B".into()),
            Tok::Fin,
        ]);
    }

    /// Asignar y comparar son simbolos DISTINTOS. Es la diferencia que hace
    /// que un `=` donde iba un `:=` no compile en vez de asignar callando.
    #[test]
    fn asignar_y_comparar_son_distintos() {
        assert_eq!(toks("A := B = C")[1], Tok::Simbolo(":=".into()));
        assert_eq!(toks("A := B = C")[3], Tok::Simbolo("=".into()));
    }

    #[test]
    fn los_guiones_bajos_de_un_numero_no_cuentan() {
        assert_eq!(toks("1_000_000")[0], Tok::Numero("1000000".into()));
    }

    /// El punto de `19.99` es coma decimal; el de `1..5` es un rango. Se
    /// distinguen mirando si detras hay un digito.
    #[test]
    fn el_punto_decimal_no_se_confunde_con_el_rango() {
        assert_eq!(toks("19.99")[0], Tok::Numero("19.99".into()));
        assert_eq!(toks("1..5")[..3], [
            Tok::Numero("1".into()),
            Tok::Simbolo("..".into()),
            Tok::Numero("5".into()),
        ]);
    }

    #[test]
    fn las_comillas_dobles_dentro_son_una() {
        assert_eq!(toks("\"di \"\"hola\"\"\"")[0], Tok::Texto("di \"hola\"".into()));
    }
}
