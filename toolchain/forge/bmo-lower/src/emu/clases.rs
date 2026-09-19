//! **A donde se va cada instruccion** (2026-09-18).
//!
//! El metro del emisor (`toolchain/tools/metro`) cuenta instrucciones, y un
//! total no dice QUE arreglar: 451.306 instrucciones son un numero, no una
//! decision. Esto las reparte en clases, y la clase que mas pesa es la primera
//! optimizacion -- la elige el dato, no quien lo mira.
//!
//! Mira los BYTES en `rip` antes de ejecutar, no el resultado: clasificar por
//! codificacion es lo unico que no depende de que el emulador entienda la
//! instruccion, y por eso vale igual para lo que emite hoy cada emisor.
//!
//! ```text
//!    Pila        push / pop
//!    Marco       lee o escribe [rbp +- d] o [rsp +- d]: idas y vueltas al marco
//!    Memoria     lee o escribe cualquier otra direccion (globales, punteros)
//!    Registro    mov entre registros: barajar
//!    Inmediato   mov de una constante a un registro: cargar
//!    Aritmetica  ALU, desplazamientos, comparaciones, cmov/setcc: sin memoria
//!    Salto       saltos condicionales e incondicionales
//!    Llamada     call y ret
//!    Flotante    SSE escalar y VEX
//!    Otra        syscall, nop, cdq y lo que no cabe arriba
//! ```
//!
//! [!] Una instruccion ALU con operando en memoria (`add rax, [rbp-8]`) cuenta
//! como Marco o Memoria, no como Aritmetica: la pregunta de este censo es
//! DONDE va el trafico, y esa instruccion va al marco.

/// Las clases, en el orden en que se pintan.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Clase {
    Pila,
    Marco,
    Memoria,
    Registro,
    Inmediato,
    Aritmetica,
    Salto,
    Llamada,
    Flotante,
    Otra,
}

impl Clase {
    pub const TODAS: [Clase; 10] = [
        Clase::Pila, Clase::Marco, Clase::Memoria, Clase::Registro, Clase::Inmediato,
        Clase::Aritmetica, Clase::Salto, Clase::Llamada, Clase::Flotante, Clase::Otra,
    ];

    pub const fn nombre(self) -> &'static str {
        match self {
            Clase::Pila => "pila",
            Clase::Marco => "marco",
            Clase::Memoria => "memoria",
            Clase::Registro => "registro",
            Clase::Inmediato => "inmed.",
            Clase::Aritmetica => "aritmetica",
            Clase::Salto => "salto",
            Clase::Llamada => "llamada",
            Clase::Flotante => "flotante",
            Clase::Otra => "otra",
        }
    }
}

/// El censo de un programa: cuantas instrucciones de cada clase ejecuto.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Censo {
    cuentas: [u64; 10],
}

impl Censo {
    pub fn apuntar(&mut self, c: Clase) {
        self.cuentas[c as usize] += 1;
    }
    pub fn de(&self, c: Clase) -> u64 {
        self.cuentas[c as usize]
    }
    pub fn total(&self) -> u64 {
        self.cuentas.iter().sum()
    }
    /// Suma otro censo encima (el metro agrega por lenguaje y en total).
    pub fn sumar(&mut self, otro: &Censo) {
        for i in 0..10 {
            self.cuentas[i] += otro.cuentas[i];
        }
    }
}

/// Donde cae el operando de memoria de un ModRM. `None` = el operando es un
/// registro (mod = 3).
fn destino(b: &[u8], rex_b: bool) -> Option<Clase> {
    let modrm = *b.first()?;
    let modo = modrm >> 6;
    let rm = modrm & 7;
    if modo == 3 {
        return None;
    }
    if rm == 4 {
        // SIB: la base rsp es el marco; r12 (con REX.B) no lo es.
        let sib = *b.get(1)?;
        let base = sib & 7;
        return Some(if base == 4 && !rex_b { Clase::Marco } else { Clase::Memoria });
    }
    if rm == 5 {
        // mod 0 es RIP-relativo (un global) con o sin REX.B; mod 1/2 es rbp,
        // o r13 si lleva REX.B.
        return Some(if modo != 0 && !rex_b { Clase::Marco } else { Clase::Memoria });
    }
    Some(Clase::Memoria)
}

/// Memoria segun el ModRM, o `si_registro` cuando el operando es un registro.
fn por_operando(b: &[u8], rex_b: bool, si_registro: Clase) -> Clase {
    destino(b, rex_b).unwrap_or(si_registro)
}

/// Clasifica la instruccion que empieza en `code[0]`. Nunca falla: lo que no
/// reconoce es `Otra`, y el emulador dira despues si tampoco lo sabe ejecutar.
pub fn clasificar(code: &[u8]) -> Clase {
    let mut i = 0;
    let mut escalar = false;
    while let Some(&p) = code.get(i) {
        match p {
            0x66 | 0xF0 => {}
            0xF2 | 0xF3 => escalar = true,
            _ => break,
        }
        i += 1;
    }
    let Some(&op) = code.get(i) else { return Clase::Otra };
    if op == 0xC4 || op == 0xC5 {
        return Clase::Flotante;
    }
    let mut rex_b = false;
    let mut op = op;
    if (0x40..=0x4F).contains(&op) {
        rex_b = op & 1 != 0;
        i += 1;
        op = match code.get(i) { Some(&o) => o, None => return Clase::Otra };
    }
    let resto = &code[(i + 1).min(code.len())..];

    match op {
        0x50..=0x5F | 0x68 | 0x6A | 0x8F => Clase::Pila,
        // mov, lea, movsxd, mov imm a memoria: donde caiga el operando
        0x88..=0x8B | 0x8D | 0x63 | 0x86 | 0x87 => por_operando(resto, rex_b, Clase::Registro),
        // mov imm: a registro es cargar una constante; a memoria, escribirla
        0xC6 | 0xC7 => por_operando(resto, rex_b, Clase::Inmediato),
        0xB0..=0xBF => Clase::Inmediato,
        // ALU clasica: add/or/adc/sbb/and/sub/xor/cmp con reg o memoria
        0x00..=0x3F if op & 7 <= 3 => por_operando(resto, rex_b, Clase::Aritmetica),
        // las formas `al/eax, imm` no llevan ModRM
        0x00..=0x3F if op & 7 <= 5 => Clase::Aritmetica,
        0x80 | 0x81 | 0x83 | 0xC0 | 0xC1 | 0xD0..=0xD3 | 0xF6 | 0xF7 | 0x84 | 0x85 | 0x69 | 0x6B | 0xFE => {
            por_operando(resto, rex_b, Clase::Aritmetica)
        }
        0xFF => {
            // /0 inc, /1 dec, /2 call, /4 jmp, /6 push
            match resto.first().map(|m| (m >> 3) & 7) {
                Some(2) => Clase::Llamada,
                Some(4) => Clase::Salto,
                Some(6) => Clase::Pila,
                _ => por_operando(resto, rex_b, Clase::Aritmetica),
            }
        }
        0x70..=0x7F | 0xE3 | 0xE9 | 0xEB => Clase::Salto,
        0xE8 | 0xC2 | 0xC3 => Clase::Llamada,
        0xA4 | 0xA5 | 0xAA | 0xAB => Clase::Memoria,
        0x0F => {
            let Some(&op2) = resto.first() else { return Clase::Otra };
            let resto = &resto[1..];
            match op2 {
                0x80..=0x8F => Clase::Salto,
                // movzx / movsx: donde caiga el operando
                0xB6 | 0xB7 | 0xBE | 0xBF => por_operando(resto, rex_b, Clase::Registro),
                // imul, cmov, setcc, popcnt/tzcnt/lzcnt, bt
                0xAF | 0x40..=0x4F | 0x90..=0x9F | 0xB8 | 0xBC | 0xBD | 0xA3 | 0xAB | 0xB3 | 0xBA => {
                    por_operando(resto, rex_b, Clase::Aritmetica)
                }
                0xB1 | 0xC1 => por_operando(resto, rex_b, Clase::Registro),
                // SSE: mov/cvt/aritmetica escalar, y las formas empaquetadas
                0x10..=0x17 | 0x28..=0x2F | 0x51..=0x5F | 0xC2 | 0xC6 | 0x6E | 0x7E | 0xD6 | 0xEF => {
                    Clase::Flotante
                }
                _ if escalar => Clase::Flotante,
                _ => Clase::Otra,
            }
        }
        _ => Clase::Otra,
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn c(b: &[u8]) -> Clase {
        clasificar(b)
    }

    #[test]
    fn pila_y_marco() {
        assert_eq!(c(&[0x55]), Clase::Pila); // push rbp
        assert_eq!(c(&[0x41, 0x5C]), Clase::Pila); // pop r12
        assert_eq!(c(&[0x48, 0x89, 0x45, 0xF8]), Clase::Marco); // mov [rbp-8], rax
        assert_eq!(c(&[0x48, 0x8B, 0x85, 0x00, 0xFF, 0xFF, 0xFF]), Clase::Marco); // mov rax, [rbp-256]
        assert_eq!(c(&[0x48, 0x89, 0x44, 0x24, 0x08]), Clase::Marco); // mov [rsp+8], rax
        assert_eq!(c(&[0x48, 0x03, 0x45, 0xF8]), Clase::Marco); // add rax, [rbp-8]
    }

    #[test]
    fn memoria_que_no_es_marco() {
        assert_eq!(c(&[0x48, 0x8B, 0x05, 0x00, 0x00, 0x00, 0x00]), Clase::Memoria); // mov rax, [rip+0]
        assert_eq!(c(&[0x48, 0x8B, 0x00]), Clase::Memoria); // mov rax, [rax]
        assert_eq!(c(&[0x49, 0x8B, 0x45, 0x08]), Clase::Memoria); // mov rax, [r13+8]
        assert_eq!(c(&[0x49, 0x8B, 0x44, 0x24, 0x08]), Clase::Memoria); // mov rax, [r12+8]
        assert_eq!(c(&[0x66, 0x89, 0x08]), Clase::Memoria); // mov [rax], cx
    }

    #[test]
    fn registro_aritmetica_y_flujo() {
        assert_eq!(c(&[0x48, 0x89, 0xC3]), Clase::Registro); // mov rbx, rax
        assert_eq!(c(&[0xB8, 1, 0, 0, 0]), Clase::Inmediato); // mov eax, 1
        assert_eq!(c(&[0x48, 0xC7, 0xC0, 1, 0, 0, 0]), Clase::Inmediato); // mov rax, 1
        assert_eq!(c(&[0x48, 0xC7, 0x45, 0xF8, 1, 0, 0, 0]), Clase::Marco); // mov qword [rbp-8], 1
        assert_eq!(c(&[0x48, 0x01, 0xD8]), Clase::Aritmetica); // add rax, rbx
        assert_eq!(c(&[0x48, 0x83, 0xC0, 0x08]), Clase::Aritmetica); // add rax, 8
        assert_eq!(c(&[0x48, 0x0F, 0xAF, 0xC3]), Clase::Aritmetica); // imul rax, rbx
        assert_eq!(c(&[0xF3, 0x48, 0x0F, 0xB8, 0xC3]), Clase::Aritmetica); // popcnt
        assert_eq!(c(&[0x74, 0x05]), Clase::Salto);
        assert_eq!(c(&[0x0F, 0x84, 0, 0, 0, 0]), Clase::Salto);
        assert_eq!(c(&[0xE8, 0, 0, 0, 0]), Clase::Llamada);
        assert_eq!(c(&[0xC3]), Clase::Llamada);
        assert_eq!(c(&[0xFF, 0xD0]), Clase::Llamada); // call rax
        assert_eq!(c(&[0xFF, 0xE0]), Clase::Salto); // jmp rax
    }

    #[test]
    fn flotante_y_otra() {
        assert_eq!(c(&[0xF2, 0x0F, 0x10, 0x45, 0xF8]), Clase::Flotante); // movsd xmm0, [rbp-8]
        assert_eq!(c(&[0xF2, 0x0F, 0x58, 0xC1]), Clase::Flotante); // addsd
        assert_eq!(c(&[0xC5, 0xFD, 0x58, 0xC1]), Clase::Flotante); // vaddpd
        assert_eq!(c(&[0x0F, 0x05]), Clase::Otra); // syscall
        assert_eq!(c(&[0x90]), Clase::Otra);
        assert_eq!(c(&[]), Clase::Otra);
    }

    #[test]
    fn el_censo_suma() {
        let mut a = Censo::default();
        a.apuntar(Clase::Pila);
        a.apuntar(Clase::Marco);
        let mut b = Censo::default();
        b.apuntar(Clase::Marco);
        a.sumar(&b);
        assert_eq!(a.de(Clase::Marco), 2);
        assert_eq!(a.total(), 3);
    }
}
