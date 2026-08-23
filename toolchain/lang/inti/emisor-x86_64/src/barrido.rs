//! **EL BARRIDO LINEAL: recorrer el codigo instruccion a instruccion.**
//!
//! ## Que cierra esto, y por que era lo que faltaba
//!
//! La mesa de katanas demuestra que las reglas **declaradas** estan donde dice.
//! No demuestra que no **falte** ninguna: una tabla vacia es la tabla honesta de
//! un binario sin reglas, y desde ahi no se distingue de uno al que se las
//! quitaron.
//!
//! *** Para eso hay que mirar el codigo y preguntar al reves: **por cada
//! operacion que pide regla, esta la suya?** Y eso pide recorrerlo.
//!
//! ## Por que se puede recorrer un `.ibex` y no un binario cualquiera
//!
//! Porque INTI emite **una sola seccion de codigo y ni un byte de datos
//! dentro**. Un binario de C mete tablas de saltos y constantes entre las
//! instrucciones, y entonces un recorrido en linea recta empieza a decodificar
//! datos como si fueran codigo -- el problema clasico del desensamblado.
//!
//! ** Ahi esta la exclusividad de INTI, y es tecnica y no un decreto: **acepta
//! una restriccion que hace sus binarios recorribles**. Cualquiera puede
//! aceptarla; casi nadie va a querer pagarla.
//!
//! ## Por que vive aqui y no en `bmo-verify`
//!
//! Porque esto ES x86-64 en cada linea. `bmo-verify` verifica el envase y su
//! cabecera lleva escrito desde el 2026-08-12 que no mira lo que hacen las
//! instrucciones. Meterle un decodificador seria romper esa frase, no cumplirla.
//!
//! ## Por que NO se reuso el decodificador del emulador
//!
//! `bmo-lower::emu` decodifica -- y **ejecuta en el mismo bucle**, y revienta con
//! un `panic` ante un opcode que ningun emisor de BMO produce. Eso esta bien
//! para un emulador y es inservible para un verificador: un verificador que
//! entra en panico ante un binario raro es un verificador que se puede tumbar
//! con un fichero. Separarlo habria sido destripar las 1.688 lineas de las que
//! dependen los cuatro lenguajes.
//!
//! ## *** LO QUE ESTE BARRIDO NO PUEDE, dicho por delante
//!
//! **No decodifica todo x86-64, y no lo pretende.** Conoce lo que emiten estas
//! carpetas, que es un conjunto acotado y contado: 47 ayudantes de `bmo-lower`
//! mas diecinueve secuencias que el emisor empuja a mano.
//!
//! Ante algo que no conoce **no adivina y no rechaza: se para y dice donde**.
//! Esas son tres respuestas distintas y confundirlas es lo que convierte un
//! verificador en un estorbo:
//!
//! ```text
//!    COMPLETO    recorri el codigo entero y puedo hablar de el
//!    ATASCADO    hay algo aqui que no se leer -- no digo que este mal
//!    (nunca)     "esto esta mal porque no lo entiendo"
//! ```
//!
//! ** Y atascarse tiene un significado util: un bloque `crudo` mete bytes de
//! `intrinsics.toml` que pueden no estar en esta lista. O sea que **el barrido
//! completa exactamente cuando el programa se queda dentro del lenguaje**, que
//! es justo lo que `crudo` marca al escribirlo.

use bmo_inti_front::ir::Comprobacion;

/// Que es la instruccion que hay en un sitio, de lo poco que hace falta saber.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Que {
    /// Una mas. No pide regla ni salta a ninguna.
    Corriente,
    /// **Pide una regla**: el resultado puede no caber, o no existir.
    Pide(Comprobacion),
    /// Un salto condicional, con el offset al que va.
    ///
    /// ** Hace falta el DESTINO y no solo saber que es un salto: una regla se
    /// reconoce porque su salto aterriza en un bloque de trampa declarado. Sin
    /// el destino, un `jo` cualquiera valdria por una comprobacion.
    Salta { destino: usize },
}

/// Una instruccion leida.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Paso {
    pub off: usize,
    pub len: usize,
    pub que: Que,
}

/// Como acabo el recorrido.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Barrido {
    /// Se leyo entero.
    Completo(Vec<Paso>),
    /// Hay algo que este lector no conoce. **No es un veredicto sobre el
    /// binario**: es un limite de quien lee, y por eso lleva el sitio y el byte.
    Atascado {
        off: usize,
        byte: u8,
        /// Lo que se habia leido hasta ahi. Sirve igual: lo de antes es cierto.
        leido: Vec<Paso>,
    },
}

impl Barrido {
    pub fn pasos(&self) -> &[Paso] {
        match self {
            Barrido::Completo(p) => p,
            Barrido::Atascado { leido, .. } => leido,
        }
    }
    pub fn completo(&self) -> bool {
        matches!(self, Barrido::Completo(_))
    }
}

/// **Recorre el codigo de principio a fin, leyendo la tabla de la maquina.**
///
/// ## ** Por que hace falta la tabla, y por que no se copio aqui
///
/// Un bloque `crudo` mete en el codigo los bytes EXACTOS de `intrinsics.toml`
/// --`0F A2` es `cpuid`, `0F 32` es `rdmsr`-- y el emisor los pega tal cual:
/// *"los bytes EXACTOS de la tabla. Ni uno escrito aqui."*
///
/// *** La primera version de esto se atasco en el `cpuid` de la sonda, y la
/// tentacion era anadir `0F A2` al decodificador. **Eso habrian sido dos
/// listas** -- la del emisor y la del lector -- y se habrian separado el dia que
/// alguien anadiera un intrinseco: el emisor emitiendolo y el lector atascandose
/// en un binario perfectamente honesto.
///
/// Lee la misma tabla. Una lista, no dos.
pub fn recorrer_con(codigo: &[u8], maquina: &[Vec<u8>]) -> Barrido {
    let mut pasos = Vec::new();
    let mut i = 0usize;
    while i < codigo.len() {
        // ** Los intrinsecos van PRIMERO y de mas largo a mas corto: son
        // secuencias exactas que el emisor pego, y una corta podria taparle el
        // sitio a una larga que empieza igual.
        if let Some(len) = intrinseco(codigo, i, maquina) {
            pasos.push(Paso { off: i, len, que: Que::Corriente });
            i += len;
            continue;
        }
        match instruccion(codigo, i) {
            Some(p) => {
                i += p.len;
                pasos.push(p);
            }
            None => {
                return Barrido::Atascado {
                    off: i,
                    byte: codigo[i],
                    leido: pasos,
                }
            }
        }
    }
    Barrido::Completo(pasos)
}

/// Recorre sin tabla de maquina. Vale para codigo sin `crudo`.
pub fn recorrer(codigo: &[u8]) -> Barrido {
    recorrer_con(codigo, &[])
}

/// Si en `off` empieza la secuencia exacta de un intrinseco, cuanto mide.
fn intrinseco(c: &[u8], off: usize, maquina: &[Vec<u8>]) -> Option<usize> {
    let mut mejor: Option<usize> = None;
    for b in maquina {
        if b.is_empty() || b.len() < 2 {
            // ** Los de UN byte no se buscan aqui, y es la regla de la roca 3:
            // `cli` es `FA` y ese byte aparece dentro de inmediatos. Un
            // intrinseco de un byte lo decodifica la tabla general o no se
            // decodifica -- adivinar seria condenar con un barrido de bytes.
            continue;
        }
        if c.len() >= off + b.len() && &c[off..off + b.len()] == b.as_slice() {
            mejor = Some(mejor.map_or(b.len(), |m: usize| m.max(b.len())));
        }
    }
    mejor
}

/// Una instruccion a partir de `off`, o `None` si no se sabe leer.
fn instruccion(c: &[u8], off: usize) -> Option<Paso> {
    let mut i = off;
    let mut rex = 0u8;
    let mut sse = false;

    // -- Prefijos. Los tres que emiten estas carpetas y ninguno mas.
    loop {
        match *c.get(i)? {
            0x66 | 0xF2 | 0xF3 => {
                sse = true;
                i += 1;
            }
            b @ 0x40..=0x4F => {
                rex = b;
                i += 1;
            }
            _ => break,
        }
    }
    let _ = rex;

    let op = *c.get(i)?;
    i += 1;

    // -- Dos bytes: `0F xx` ------------------------------------------------
    if op == 0x0F {
        let op2 = *c.get(i)?;
        i += 1;
        return match op2 {
            // `syscall`.
            0x05 => Some(Paso { off, len: i - off, que: Que::Corriente }),
            // Saltos condicionales cercanos, con rel32.
            0x80..=0x8F => {
                let rel = rel32(c, i)?;
                let fin = i + 4;
                Some(Paso {
                    off,
                    len: fin - off,
                    // ** El destino se cuenta desde el FINAL de la instruccion,
                    // que es como lo cuenta el procesador. Contarlo desde el
                    // principio da un numero que casi siempre esta cerca del
                    // bueno, que es la peor clase de error.
                    que: Que::Salta {
                        destino: (fin as i64 + rel as i64) as usize,
                    },
                })
            }
            // `setcc r/m8`.
            0x90..=0x9F => modrm(c, i).map(|fin| Paso { off, len: fin - off, que: Que::Corriente }),
            // `imul r64, r/m64` -- **PIDE LA REGLA 1**.
            0xAF => modrm(c, i).map(|fin| Paso {
                off,
                len: fin - off,
                que: Que::Pide(Comprobacion::Desborde),
            }),
            // `cvttsd2si` -- **PIDE LA REGLA 12**. El ancho no se sabe desde
            // aqui, y por eso se guarda uno cualquiera: lo que importa es que
            // la comprobacion exista, y su codigo (E1012) no depende del ancho.
            0x2C if sse => modrm(c, i).map(|fin| Paso {
                off,
                len: fin - off,
                que: Que::Pide(Comprobacion::Conversion(0)),
            }),
            // El resto de SSE y las extensiones de signo: corrientes.
            0x10 | 0x11 | 0x2A | 0x2F | 0x58 | 0x59 | 0x5C | 0x5E | 0x6E | 0x7E | 0xB6
            | 0xB7 | 0xBE | 0xBF | 0xB8 | 0xBC | 0xBD | 0xC8..=0xCF => {
                modrm(c, i).map(|fin| Paso { off, len: fin - off, que: Que::Corriente })
            }
            _ => None,
        };
    }

    // -- Un byte -----------------------------------------------------------
    match op {
        // push/pop de un registro, y `ret`.
        0x50..=0x5F | 0xC3 | 0x99 | 0xC9 => {
            Some(Paso { off, len: i - off, que: Que::Corriente })
        }
        // ALU r/m, r y sus parientes. Todos ModRM y nada detras.
        0x01 | 0x09 | 0x21 | 0x29 | 0x31 | 0x39 | 0x63 | 0x85 | 0x87 | 0x88 | 0x89 | 0x8A
        | 0x8B | 0x8D => modrm(c, i).map(|fin| Paso { off, len: fin - off, que: Que::Corriente }),
        // ALU r/m, imm8.
        0x83 | 0xC0 | 0xC1 | 0x6B => {
            let fin = modrm(c, i)?;
            Some(Paso { off, len: fin + 1 - off, que: Que::Corriente })
        }
        // ALU r/m, imm32.
        0x81 | 0xC7 | 0x69 => {
            let fin = modrm(c, i)?;
            c.get(fin + 3)?;
            Some(Paso { off, len: fin + 4 - off, que: Que::Corriente })
        }
        // Desplazamientos por `cl`.
        0xD1 | 0xD3 => modrm(c, i).map(|fin| Paso { off, len: fin - off, que: Que::Corriente }),
        // **GRUPO 3**: `F7 /7` es `idiv` y **PIDE LA REGLA 3**; `/3` es `neg` y
        // pide la 1. El resto (`mul`, `div`, `not`, `test`) no piden nada.
        //
        // ** El campo que lo decide son tres bits DENTRO del ModRM, y por eso
        // esta instruccion no se puede clasificar por su opcode: el mismo `F7`
        // es cuatro instrucciones distintas.
        0xF6 | 0xF7 => {
            let m = *c.get(i)?;
            let ext = (m >> 3) & 7;
            let mut fin = modrm(c, i)?;
            // `/0` lleva inmediato: es `test r/m, imm`.
            if ext == 0 {
                fin += if op == 0xF6 { 1 } else { 4 };
                c.get(fin - 1)?;
            }
            let que = match ext {
                7 => Que::Pide(Comprobacion::EntreCero),
                3 => Que::Pide(Comprobacion::Desborde),
                _ => Que::Corriente,
            };
            Some(Paso { off, len: fin - off, que })
        }
        // `mov r64, imm64` con REX.W; `mov r32, imm32` sin el.
        0xB8..=0xBF => {
            let ancho = if rex & 0x08 != 0 { 8 } else { 4 };
            c.get(i + ancho - 1)?;
            Some(Paso { off, len: i + ancho - off, que: Que::Corriente })
        }
        // `call rel32`, `jmp rel32`.
        0xE8 | 0xE9 => {
            c.get(i + 3)?;
            Some(Paso { off, len: i + 4 - off, que: Que::Corriente })
        }
        // `jmp rel8` y los saltos condicionales cortos.
        0xEB | 0x70..=0x7F => {
            let rel = *c.get(i)? as i8;
            let fin = i + 1;
            let que = if op == 0xEB {
                Que::Corriente
            } else {
                Que::Salta { destino: (fin as i64 + rel as i64) as usize }
            };
            Some(Paso { off, len: fin - off, que })
        }
        _ => None,
    }
}

fn rel32(c: &[u8], i: usize) -> Option<i32> {
    Some(i32::from_le_bytes([
        *c.get(i)?,
        *c.get(i + 1)?,
        *c.get(i + 2)?,
        *c.get(i + 3)?,
    ]))
}

/// Lee el ModRM (y su SIB y su desplazamiento) y devuelve donde acaba.
fn modrm(c: &[u8], i: usize) -> Option<usize> {
    let m = *c.get(i)?;
    let modo = m >> 6;
    let rm = m & 7;
    let mut fin = i + 1;

    // `rm == 100` con modo != 11 trae SIB.
    if modo != 3 && rm == 4 {
        c.get(fin)?;
        fin += 1;
    }
    match modo {
        // `rm == 101` en modo 00 es direccionamiento relativo a `rip`: disp32.
        0 if rm == 5 => {
            c.get(fin + 3)?;
            fin += 4;
        }
        0 => {}
        1 => {
            c.get(fin)?;
            fin += 1;
        }
        2 => {
            c.get(fin + 3)?;
            fin += 4;
        }
        _ => {}
    }
    Some(fin)
}

// ===================================================================
//  ** LA AUDITORIA: por cada operacion que pide regla, esta la suya?
// ===================================================================

/// Una operacion que pide regla y no la tiene al lado.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Descubierta {
    pub off: usize,
    pub regla: &'static str,
}

/// **Que ninguna operacion se quede sin su regla.**
///
/// `trampas` son los offsets de los bloques de trampa declarados en la mesa de
/// katanas, con su codigo. Una operacion esta cubierta si **en la misma funcion
/// hay un salto que aterriza en un bloque con SU codigo**.
///
/// ## *** Por que "en la misma funcion" y no "justo al lado"
///
/// Porque las tres reglas se colocan distinto y a proposito:
///
/// ```text
///    la 1    el salto va DETRAS de la operacion: mira la bandera que dejo
///    la 3    la comprobacion va DELANTE de la division, porque despues de
///            dividir entre cero no queda programa que mire nada
///    la 12   detras, y con un bloque de dos preguntas por medio
/// ```
///
/// Exigir adyacencia habria obligado a escribir tres reglas de vecindad
/// distintas, y la primera vez que el emisor moviera una instruccion, el
/// verificador diria que falta una regla que si esta. **Un verificador que se
/// equivoca hacia el "no" se apaga en una semana.**
///
/// Lo que se exige es mas debil y se sostiene: **la regla existe y va a su
/// sitio**. Lo que esto caza es lo que importa -- una operacion cuya regla
/// **no se emitio**.
pub fn descubiertas(
    barrido: &Barrido,
    inicios: &[(String, usize)],
    trampas: &[(u64, usize)],
) -> Vec<Descubierta> {
    let pasos = barrido.pasos();
    let mut fallos = Vec::new();

    // A que codigo pertenece cada bloque de trampa.
    let codigo_de = |destino: usize| -> Option<u64> {
        trampas.iter().find(|(_, o)| *o == destino).map(|(c, _)| *c)
    };

    for p in pasos {
        let Que::Pide(regla) = p.que else { continue };
        let esperado: u64 = regla.codigo()[1..].parse().unwrap_or(0);
        let (desde, hasta) = funcion_de(inicios, p.off, pasos);
        let cubierta = pasos
            .iter()
            .filter(|q| q.off >= desde && q.off < hasta)
            .any(|q| matches!(q.que, Que::Salta { destino } if codigo_de(destino) == Some(esperado)));
        if !cubierta {
            fallos.push(Descubierta {
                off: p.off,
                regla: regla.codigo(),
            });
        }
    }
    fallos
}

/// El rango `[desde, hasta)` de la funcion que contiene `off`.
fn funcion_de(inicios: &[(String, usize)], off: usize, pasos: &[Paso]) -> (usize, usize) {
    let fin_total = pasos.last().map(|p| p.off + p.len).unwrap_or(0);
    let mut desde = 0usize;
    let mut hasta = fin_total;
    let mut ordenados: Vec<usize> = inicios.iter().map(|(_, o)| *o).collect();
    ordenados.sort_unstable();
    for (i, o) in ordenados.iter().enumerate() {
        if *o <= off {
            desde = *o;
            hasta = ordenados.get(i + 1).copied().unwrap_or(fin_total);
        }
    }
    (desde, hasta)
}

#[cfg(test)]
mod pruebas;
