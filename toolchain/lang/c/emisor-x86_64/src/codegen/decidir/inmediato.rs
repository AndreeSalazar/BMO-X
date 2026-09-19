//! **EL INMEDIATO** -- si el operando derecho es una constante que cabe en la
//! instruccion, la instruccion lo lleva dentro.
//!
//! [fase]     EMISION
//!
//! [aparece]  BANCO -- decide, no emite. Si esto contesta `Corto` para un
//!            numero que no cabe en ocho bits, el programa suma otro numero y
//!            el banco lo ve en la primera fila que reste 200
//!
//! [carril]   VERDE -- y NO se elige: sale de su `[aparece]` (BANCO).
//!            Ver la tabla en toolchain/tools/fases/fases.py
//!
//! # Lo que dijo el metro (2026-09-18)
//!
//! `x & 0xFF` se emitia asi:
//!
//! ```text
//!    movsxd rax, [x] ; mov rdx, rax ; mov rax, 0xFF ; and rax, rdx    4 instrucciones, 17 bytes
//! ```
//!
//! y x86-64 tiene la forma con inmediato desde 1978:
//!
//! ```text
//!    movsxd rax, [x] ; and rax, 0xFF                                  2 instrucciones,  8 bytes
//! ```
//!
//! En los 14 programas de C del banco el 28 % de lo ejecutado era barajar
//! registros y el 10 % cargar constantes; buena parte era ESTO, en cada `i + 1`,
//! cada `i < n` y cada mascara.
//!
//! # Las dos anchuras, y por que importa cual
//!
//! ```text
//!    Corto   imm8   se EXTIENDE CON SIGNO a 64 bits    83 /op ib
//!    Largo   imm32  se EXTIENDE CON SIGNO a 64 bits    81 /op id
//! ```
//!
//! ** El signo es lo que hace esto correcto y no una casualidad: `mov rax, imm32`
//! (`C7 C0`) tambien extiende con signo, asi que el valor que llega al operador
//! es EL MISMO por los dos caminos. Un numero que no cabe en 32 bits con signo
//! --`0xFFFFFFFF`, por ejemplo-- contesta `None` y sigue por la pila, que es lo
//! unico correcto: no existe `and rax, imm64`.
//!
//! [!] Y la CUENTA de un desplazamiento es otra pregunta: es un byte SIN signo
//! y el CPU la enmascara a seis bits. Aqui solo se contesta `0..=63`; fuera de
//! eso es comportamiento indefinido en C y se deja al camino largo, que hace lo
//! que haga el CPU.

use crate::ast::Expr;

use super::plegado::constante_para_emitir;

/// Un inmediato que cabe en la instruccion.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::codegen) enum Inmediato {
    Corto(i8),
    Largo(i32),
}

/// Lo que el emisor carga con UN `mov rax, imm`: un literal, o lo que
/// `constante_para_emitir` pliega. Es el mismo conjunto que `solo_toca_rax`,
/// y tiene que seguir siendolo: un literal suelto NO pasa por `recortar_a_32`
/// en la ruta larga, y por eso `constante_para_emitir` se niega a plegarlo --
/// aqui se acepta porque el inmediato tampoco recorta. Mismo valor en `rax`
/// por los dos caminos.
fn valor_de(b: &Expr) -> Option<i64> {
    match b {
        Expr::Int(n) => Some(*n),
        Expr::CharLit(c) => Some(*c as i64),
        _ => constante_para_emitir(b),
    }
}

/// Si `b` es una constante que cabe en 32 bits con signo, su forma.
pub(in crate::codegen) fn inmediato_de(b: &Expr) -> Option<Inmediato> {
    let v = valor_de(b)?;
    if let Ok(c) = i8::try_from(v) {
        return Some(Inmediato::Corto(c));
    }
    i32::try_from(v).ok().map(Inmediato::Largo)
}

/// Si `b` es una cuenta de desplazamiento valida, su byte.
pub(in crate::codegen) fn cuenta_de_desplazamiento(b: &Expr) -> Option<u8> {
    match valor_de(b)? {
        v @ 0..=63 => Some(v as u8),
        _ => None,
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    /// Un negativo se escribe como `0 - v`: `Neg` no es raiz plegable (ver
    /// `constante_para_emitir`) y un `-5` suelto sigue por el camino largo.
    fn n(v: i64) -> Expr {
        if v < 0 { Expr::Sub(Box::new(Expr::Int(0)), Box::new(Expr::Int(-v))) } else { Expr::Int(v) }
    }

    #[test]
    fn corto_largo_y_ninguno() {
        assert_eq!(inmediato_de(&n(1)), Some(Inmediato::Corto(1)));
        assert_eq!(inmediato_de(&n(-128)), Some(Inmediato::Corto(-128)));
        assert_eq!(inmediato_de(&n(127)), Some(Inmediato::Corto(127)));
        assert_eq!(inmediato_de(&n(128)), Some(Inmediato::Largo(128)));
        assert_eq!(inmediato_de(&n(0xFF)), Some(Inmediato::Largo(255)));
        assert_eq!(inmediato_de(&n(0x1900)), Some(Inmediato::Largo(0x1900)));
        assert_eq!(inmediato_de(&n(i32::MAX as i64)), Some(Inmediato::Largo(i32::MAX)));
        assert_eq!(inmediato_de(&n(i32::MIN as i64)), Some(Inmediato::Largo(i32::MIN)));
        // 0xFFFFFFFF NO cabe con signo: por la pila
        assert_eq!(inmediato_de(&n(0xFFFF_FFFF)), None);
        assert_eq!(inmediato_de(&n(1 << 40)), None);
        // una variable no es un inmediato
        assert_eq!(inmediato_de(&Expr::Var("x".into())), None);
    }

    /// Lo que pliega `constante_para_emitir` es inmediato; lo que no --una
    /// raiz `<<`, por la regla del recorte-- sigue por el camino largo. Esta
    /// fila existe para que ampliar aquella lista se note tambien aqui.
    #[test]
    fn lo_plegable_tambien_es_inmediato_y_lo_otro_no() {
        let e = Expr::Mul(Box::new(Expr::Int(1)), Box::new(Expr::Int(8)));
        assert_eq!(inmediato_de(&e), Some(Inmediato::Corto(8)));
        let e = Expr::Shl(Box::new(Expr::Int(1)), Box::new(Expr::Int(4)));
        assert_eq!(inmediato_de(&e), None);
    }

    #[test]
    fn la_cuenta_del_desplazamiento_va_de_0_a_63() {
        assert_eq!(cuenta_de_desplazamiento(&n(0)), Some(0));
        assert_eq!(cuenta_de_desplazamiento(&n(63)), Some(63));
        assert_eq!(cuenta_de_desplazamiento(&n(64)), None);
        assert_eq!(cuenta_de_desplazamiento(&n(-1)), None);
        assert_eq!(cuenta_de_desplazamiento(&Expr::Var("x".into())), None);
    }
}
