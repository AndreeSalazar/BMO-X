//! **EL REENVIO** -- una funcion que solo reexpide sus parametros no necesita
//! marco: mueve registros y salta.
//!
//! [fase]     EMISION
//!
//! [aparece]  BANCO -- si esto reordena mal los registros, la funcion de
//!            destino recibe el segundo argumento como primero, y el banco lo
//!            ve en la primera fila que reenvie dos parametros cruzados
//!
//! [carril]   VERDE -- y NO se elige: sale de su `[aparece]` (BANCO).
//!            Ver la tabla en toolchain/tools/fases/fases.py
//!
//! # Lo que dijo el metro (2026-09-19)
//!
//! Con los parametros en registro, la funcion mas llamada del banco quedo PEOR:
//!
//! ```c
//!    unsigned long long bmo_valor(cap, op, a0, a1, a2) {
//!        return __syscall_valor(BMO_INVOKE, cap, op, a0, a1, a2);
//!    }
//! ```
//!
//! volcaba cinco parametros al marco y los volvia a cargar para la puerta,
//! cuando ya estaban en `rdi, rsi, rdx, rcx, r8` -- y la puerta los quiere en
//! `rdi, rsi, rdx, r10, r8`. La diferencia entera es UN `mov r10, rcx`.
//!
//! # Que es un reenvio
//!
//! Una funcion NO variadica, con parametros escalares (los que llegan en
//! registro), cuyo cuerpo es exactamente `return destino(args...)` donde el
//! destino es un intrinseco o una funcion no variadica con firma escalar, y
//! cada argumento es un parametro o una constante. Entonces:
//!
//! ```text
//!    intrinseco   mueve los registros, suelta los bytes, normaliza, `ret`
//!    funcion      mueve los registros y `jmp destino` -- la llamada de cola:
//!                 el `ret` del destino vuelve a quien nos llamo a nosotros
//! ```
//!
//! Sin `push rbp`, sin marco, sin volcar nada. `roja.h` esta lleno de estas:
//! `bmo_pid`, `bmo_codigo`, `bmo_valor`, `bmo_esperar`...
//!
//! # El baile de los registros
//!
//! Los parametros llegan en un orden y el destino los quiere en otro; puede
//! haber CICLOS (`f(a, b)` reenviado como `g(b, a)`: rdi <-> rsi). `bailar`
//! resuelve el reparto con la regla clasica: emitir un movimiento cuyo destino
//! nadie necesite ya como origen, y si no queda ninguno --un ciclo--, sacar un
//! origen a `r11` y seguir. Las constantes van al final, cuando ya no pisan a
//! nadie. Es puro y se prueba con listas.

use crate::ast::{Expr, Function, Stmt};

use super::llamada::REGISTROS;

/// De donde sale un argumento del reenvio.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::codegen) enum Fuente {
    /// El registro en el que llego un parametro (por su numero).
    Registro(u8),
    /// Una constante.
    Constante(i64),
}

/// A donde va el reenvio.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::codegen) enum Destino {
    Intrinseco(String),
    Funcion(String),
}

/// Un reenvio reconocido: el destino y, por posicion del argumento, su fuente.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::codegen) struct Reenvio {
    pub destino: Destino,
    pub fuentes: Vec<Fuente>,
}

/// Reconoce un reenvio. `es_escalar(i)` dice si el parametro `i` llega en
/// registro; `destino_valido(nombre, cuantos)` si el destino acepta esos
/// argumentos por registro (o es un intrinseco), y `constante` pliega un
/// argumento constante.
pub(in crate::codegen) fn detectar(
    func: &Function,
    es_escalar: impl Fn(usize) -> bool,
    destino_valido: impl Fn(&str, usize) -> Option<Destino>,
    constante: impl Fn(&Expr) -> Option<i64>,
) -> Option<Reenvio> {
    if func.variadica || func.name == "main" || func.params.len() > REGISTROS.len() {
        return None;
    }
    if !(0..func.params.len()).all(&es_escalar) {
        return None;
    }
    let [Stmt::Return(Some(llamada))] = func.body.as_slice() else { return None };
    let (nombre, args) = match llamada {
        Expr::Intrinsic(n, a) | Expr::Call(n, a) => (n, a),
        _ => return None,
    };
    let destino = destino_valido(nombre, args.len())?;
    let mut fuentes = Vec::with_capacity(args.len());
    for a in args {
        if let Some(v) = constante(a) {
            fuentes.push(Fuente::Constante(v));
            continue;
        }
        let Expr::Var(n) = a else { return None };
        let i = func.params.iter().position(|p| &p.name == n)?;
        fuentes.push(Fuente::Registro(REGISTROS[i]));
    }
    Some(Reenvio { destino, fuentes })
}

/// Un paso del baile.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::codegen) enum Paso {
    /// `mov dst, src`
    Mover(u8, u8),
    /// `mov dst, imm`
    Poner(u8, i64),
}

/// El registro de paso para romper un ciclo: r11, que ninguna convencion
/// de aqui usa para argumentos y que no hay que devolver.
pub(in crate::codegen) const TEMPORAL: u8 = 11;

/// Los movimientos, en un orden que no pisa a nadie. `destinos[i]` es el
/// registro que el destino quiere para el argumento `i`.
pub(in crate::codegen) fn bailar(fuentes: &[Fuente], destinos: &[u8]) -> Vec<Paso> {
    let mut pasos = Vec::new();
    // (destino, origen) pendientes de los que vienen de registro
    let mut pendientes: Vec<(u8, u8)> = fuentes
        .iter()
        .zip(destinos)
        .filter_map(|(f, &d)| match f {
            Fuente::Registro(r) if *r != d => Some((d, *r)),
            _ => None,
        })
        .collect();
    while !pendientes.is_empty() {
        // uno cuyo destino no sea el origen de otro
        match pendientes.iter().position(|&(d, _)| !pendientes.iter().any(|&(_, o)| o == d)) {
            Some(i) => {
                let (d, o) = pendientes.remove(i);
                pasos.push(Paso::Mover(d, o));
            }
            None => {
                // un ciclo: el primer origen se aparta a r11 y quien lo
                // necesitaba lo toma de ahi
                let (_, o) = pendientes[0];
                pasos.push(Paso::Mover(TEMPORAL, o));
                for p in pendientes.iter_mut() {
                    if p.1 == o {
                        p.1 = TEMPORAL;
                    }
                }
            }
        }
    }
    for (f, &d) in fuentes.iter().zip(destinos) {
        if let Fuente::Constante(v) = f {
            pasos.push(Paso::Poner(d, *v));
        }
    }
    pasos
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn el_mismo_orden_no_mueve_nada() {
        assert!(bailar(&[Fuente::Registro(7), Fuente::Registro(6)], &[7, 6]).is_empty());
    }

    #[test]
    fn la_puerta_solo_necesita_r10_y_el_cero() {
        // bmo_valor(cap, op, a0, a1, a2) -> __syscall(0, cap, op, a0, a1, a2)
        let f = [Fuente::Constante(0), Fuente::Registro(7), Fuente::Registro(6), Fuente::Registro(2), Fuente::Registro(1), Fuente::Registro(8)];
        assert_eq!(bailar(&f, &[0, 7, 6, 2, 10, 8]), [Paso::Mover(10, 1), Paso::Poner(0, 0)]);
    }

    #[test]
    fn un_ciclo_pasa_por_r11() {
        // g(b, a) con a en rdi y b en rsi
        let p = bailar(&[Fuente::Registro(6), Fuente::Registro(7)], &[7, 6]);
        assert_eq!(p, [Paso::Mover(TEMPORAL, 6), Paso::Mover(6, 7), Paso::Mover(7, TEMPORAL)]);
    }

    #[test]
    fn una_cadena_se_ordena_desde_el_final() {
        // h(x, y) reenviado como h2(c, x, y): x rdi->rsi, y rsi->rdx, c const->rdi
        let p = bailar(&[Fuente::Constante(9), Fuente::Registro(7), Fuente::Registro(6)], &[7, 6, 2]);
        assert_eq!(p, [Paso::Mover(2, 6), Paso::Mover(6, 7), Paso::Poner(7, 9)]);
    }
}
