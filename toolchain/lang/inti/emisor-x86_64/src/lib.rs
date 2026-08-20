//! # INTI para x86-64 -- de la IR a bytes
//!
//! El unico crate de INTI que puede nombrar una maquina, y por eso vive fuera
//! del frontend: `bmo-inti-front/tests/agnostico.rs` prohibe alli lo que aqui
//! se hace en cada linea.
//!
//! ## La frontera, dicha entera
//!
//! ```text
//!    bmo-inti-front        texto -> piezas -> arbol -> analisis -> IR
//!                          NO nombra ninguna maquina. Hay un test.
//!
//!    bmo-inti-x86-64       IR -> bytes -> .bex
//!    (esto)                nombra x86-64 en cada linea, porque ES x86-64.
//! ```
//!
//! El dia de otra arquitectura, `emisor-aarch64/` al lado y **el frontend no se
//! toca**. Esa es la mitad B de la portabilidad de la seccion 7 del maestro,
//! convertida en dos carpetas.
//!
//! ## ** Y lo que se emite que ningun otro lenguaje emite
//!
//! Las comprobaciones. Una suma de INTI baja a **dos** cosas:
//!
//! ```text
//!    add   rax, rcx        la suma
//!    jo    <atrapa>        y la regla 1, que en C no existe
//! ```
//!
//! Ese `jo` es el "sin comportamiento indefinido" en bytes. La seccion 6.3 dice
//! que cuesta ~1%; aqui esta la instruccion que se va a medir.
//!
//! ## ** F3: los temporales viven en registros
//!
//! Desde el 2026-08-19. Y el cambio ocurrio **en `marco.rs` y en tres lineas de
//! este fichero**, que es lo que `LINAJE.md` habia prometido. Se pudo porque la
//! IR ya traia los temporales.
//!
//! ## ** Las llamadas, desde el 2026-08-19
//!
//! Una funcion de INTI llama a otra de INTI. Es la pieza que desbloquea todo lo
//! demas, porque **todo runtime son llamadas**.
//!
//! Y los destinos se resuelven **al final del modulo**: una funcion puede
//! llamar a otra declarada mas abajo, y resolver sobre la marcha obligaria a
//! ordenar las funciones por quien llama a quien -- imposible en cuanto dos se
//! llaman entre si.
//!
//! ## OJO: Lo que hoy NO hace, dicho por delante
//!
//! - **No llama fuera del modulo**: una funcion de biblioteca pide enlazado, y
//!   el hueco se deja marcado en vez de inventarle una direccion.
//! - **No emite `pleno`**: texto, listas y tablas piden monton.
//!
//! O sea: **INTI LLANO con aritmetica entera y control**. Que es justo lo que
//! hace falta para que el primer `.bex` exista y pase el gate.

pub mod marco;

use bmo_abi::bef::writer::{BefBuilder, BefSection};
use bmo_inti_front::arbol::Op;
use bmo_inti_front::ir::{Comprobacion, Const, FuncionIr, Instr, Local, ModuloIr, Valor};
use bmo_lower::x86;
use marco::{Marco, Sitio};

/// Los dos registros de trabajo.
///
/// Dos bastan mientras todo viva en la pila: uno para cada lado de una
/// operacion binaria. Cuando llegue el asignador de registros esto desaparece,
/// y ese es justo el cambio que la IR con temporales hace posible.
const IZQ: u8 = 0; // rax
const DER: u8 = 1; // rcx

/// Por donde llegan y se mandan los argumentos, en orden.
///
/// Es la convencion de llamada de esta maquina, y por eso esta linea solo puede
/// existir en este crate: el frontend tiene prohibido saber que existe algo
/// llamado "registro de argumento".
const ARGUMENTOS: [u8; 6] = [7, 6, 2, 1, 8, 9]; // rdi, rsi, rdx, rcx, r8, r9

/// Lo que sale de emitir un modulo.
pub struct Emitido {
    pub codigo: Vec<u8>,
    /// Donde empieza cada funcion dentro del codigo.
    pub inicios: Vec<(String, usize)>,
    /// Cuantas comprobaciones anti-UB se emitieron **en bytes**.
    ///
    /// ** No es el mismo numero que `ModuloIr::comprobaciones()`: aquel cuenta
    /// las que la IR pidio y este las que llegaron al binario. El dia que haya
    /// eliminacion de comprobaciones, **la diferencia entre los dos numeros es
    /// exactamente lo que el optimizador quito**, y se podra leer sin creerselo.
    pub comprobaciones: usize,
    /// Cuantos temporales viven en un registro, y cuantos en la pila.
    ///
    /// ** Es el numero de F3: si el segundo baja y el primero sube, el
    /// asignador esta haciendo su trabajo. Y como sale del emisor y no de una
    /// estimacion, se puede seguir en el tiempo -- igual que los `crudo`.
    pub en_registros: usize,
    pub en_pila: usize,
    /// Llamadas cuyo destino todavia no se sabia al emitirlas.
    ///
    /// ** Se resuelven al final del modulo y no sobre la marcha, porque una
    /// funcion puede llamar a otra **declarada mas abajo**. Resolver segun se
    /// emite obligaria a ordenar las funciones por quien llama a quien -- y eso
    /// es imposible en cuanto dos se llaman entre si.
    huecos_de_llamada: Vec<(usize, String)>,
}

/// Emite un modulo entero.
pub fn emitir(m: &ModuloIr) -> Emitido {
    let mut salida = Emitido {
        codigo: Vec::new(),
        inicios: Vec::new(),
        comprobaciones: 0,
        en_registros: 0,
        en_pila: 0,
        huecos_de_llamada: Vec::new(),
    };

    for f in &m.funciones {
        salida.inicios.push((f.nombre.clone(), salida.codigo.len()));
        let cuenta = emitir_funcion(f, &mut salida.codigo);
        salida.comprobaciones += cuenta.comprobaciones;
        salida.en_registros += cuenta.en_registros;
        salida.en_pila += cuenta.en_pila;
        salida.huecos_de_llamada.extend(cuenta.huecos_de_llamada);
    }

    // Ahora si: todas las funciones tienen sitio, asi que todas las llamadas
    // tienen destino.
    let huecos = std::mem::take(&mut salida.huecos_de_llamada);
    for (hueco, nombre) in huecos {
        let destino = salida
            .inicios
            .iter()
            .find(|(n, _)| *n == nombre)
            .map(|(_, off)| *off);
        // Una llamada a algo que no esta en este modulo se deja en cero: seria
        // una funcion de la biblioteca, y para eso hace falta enlazado. Se deja
        // marcada en vez de inventarle una direccion.
        if let Some(d) = destino {
            let rel = (d as i64 - (hueco as i64 + 4)) as i32;
            salida.codigo[hueco..hueco + 4].copy_from_slice(&rel.to_le_bytes());
        }
    }

    salida
}

/// Lo que una funcion aprende mientras se emite.
///
/// Se DEVUELVE en vez de escribirse sobre la marcha porque el codigo esta
/// prestado mientras se emite. No es una pelea con el prestamo: es la senal de
/// que emitir y contabilizar son dos cosas, y mezclarlas fue lo primero que
/// probe.
#[derive(Default)]
struct Cuenta {
    comprobaciones: usize,
    en_registros: usize,
    en_pila: usize,
    huecos_de_llamada: Vec<(usize, String)>,
}

fn emitir_funcion(f: &FuncionIr, out: &mut Vec<u8>) -> Cuenta {
    let marco = Marco::de(f);
    let mut cuenta = Cuenta {
        en_registros: marco.en_registros(),
        en_pila: f.temporales as usize - marco.en_registros(),
        ..Default::default()
    };
    let mut comprobaciones = 0usize;
    let mut salida_huecos: Vec<(usize, String)> = Vec::new();

    // Prologo.
    out.push(0x55); // push rbp
    x86::mov_r64_r64(out, 5, 4); // mov rbp, rsp
    let tam = marco.size();
    if tam > 0 {
        if tam <= 127 {
            x86::sub_r64_imm8(out, 4, tam as i8);
        } else {
            // Marcos grandes: se emite el inmediato de 32 bits a mano porque
            // `bmo_lower` no trae ese helper todavia.
            out.extend_from_slice(&[0x48, 0x81, 0xEC]);
            out.extend_from_slice(&(tam as u32).to_le_bytes());
        }
    }

    // ** Los parametros llegan en registros y las locales viven en el marco,
    // asi que lo primero que hace toda funcion es bajarlos.
    //
    // El orden de esos registros es la convencion de llamada de esta maquina, y
    // por eso esta linea solo puede existir en este crate: el frontend tiene
    // prohibido saber que existe algo llamado "registro de argumento".
    for i in 0..f.parametros.min(6) as usize {
        mov_a_marco(out, marco.local(Local(i as u32)), ARGUMENTOS[i]);
    }

    // Los saltos se rellenan al final, cuando se sabe donde cayo cada etiqueta.
    let mut sitios_de_etiqueta: Vec<(u32, usize)> = Vec::new();
    let mut huecos: Vec<(usize, u32)> = Vec::new();
    // A donde saltan las comprobaciones que fallan.
    let mut huecos_de_atrapa: Vec<usize> = Vec::new();

    for i in &f.instrucciones {
        match i {
            Instr::Etiqueta(e) => sitios_de_etiqueta.push((e.0, out.len())),

            Instr::Mueve { destino, origen } => {
                carga(out, IZQ, origen, &marco);
                guarda_temporal(out, IZQ, *destino, &marco);
            }

            Instr::Guarda { destino, valor } => {
                carga(out, IZQ, valor, &marco);
                mov_a_marco(out, marco.local(*destino), IZQ);
            }

            Instr::Binaria {
                destino,
                op,
                izquierda,
                derecha,
            } => {
                carga(out, IZQ, izquierda, &marco);
                carga(out, DER, derecha, &marco);
                binaria(out, *op);
                guarda_temporal(out, IZQ, *destino, &marco);
            }

            Instr::Unaria { destino, op, valor } => {
                carga(out, IZQ, valor, &marco);
                match op {
                    bmo_inti_front::arbol::OpUno::Menos => x86::neg_r64(out, IZQ),
                    bmo_inti_front::arbol::OpUno::No => {
                        // `no x` sobre un logico: comparar con cero y quedarse
                        // con el bit de igualdad.
                        x86::test_r64_r64(out, IZQ, IZQ);
                        out.extend_from_slice(&[0x0F, 0x94, 0xC0]); // sete al
                        out.extend_from_slice(&[0x48, 0x0F, 0xB6, 0xC0]); // movzx
                    }
                }
                guarda_temporal(out, IZQ, *destino, &marco);
            }

            // ** La regla, en bytes.
            Instr::Comprueba { que, .. } => {
                comprobaciones += 1;
                match que {
                    Comprobacion::Desborde => {
                        // `jo` mira la bandera que la propia suma dejo puesta:
                        // la comprobacion no vuelve a calcular nada, solo
                        // pregunta. Por eso cuesta lo que cuesta.
                        out.extend_from_slice(&[0x0F, 0x80]);
                        huecos_de_atrapa.push(out.len());
                        out.extend_from_slice(&[0, 0, 0, 0]);
                    }
                    Comprobacion::EntreCero
                    | Comprobacion::Indice
                    | Comprobacion::Conversion => {
                        // Estas tres piden mirar un operando ANTES de la
                        // operacion, no la bandera de despues. Se dejan sin
                        // emitir en vez de emitir algo que no comprueba: una
                        // comprobacion que no comprueba es peor que ninguna,
                        // porque el numero de arriba diria que si esta.
                        comprobaciones -= 1;
                    }
                }
            }

            Instr::Devuelve(v) => {
                if let Some(v) = v {
                    carga(out, IZQ, v, &marco);
                }
                epilogo(out);
            }

            Instr::Salta(e) => {
                out.push(0xE9);
                huecos.push((out.len(), e.0));
                out.extend_from_slice(&[0, 0, 0, 0]);
            }

            Instr::SaltaSi { cond, falso, .. } => {
                carga(out, IZQ, cond, &marco);
                x86::test_r64_r64(out, IZQ, IZQ);
                // Si es cero, al camino falso; si no, sigue.
                out.extend_from_slice(&[0x0F, 0x84]);
                huecos.push((out.len(), falso.0));
                out.extend_from_slice(&[0, 0, 0, 0]);
            }

            Instr::Llama {
                destino,
                que,
                argumentos,
            } => {
                // Los argumentos van a los registros que dice la convencion.
                //
                // ** Y aqui se ve para que sirve el freno del asignador: como
                // una funcion con llamadas no reparte registros, ningun
                // argumento puede estar viviendo en `rdi` cuando toca cargar el
                // siguiente. Cargarlos en orden es seguro **porque el reparto
                // se apago**, no por suerte.
                for (i, a) in argumentos.iter().enumerate().take(6) {
                    carga(out, ARGUMENTOS[i], a, &marco);
                }

                match que {
                    Valor::Nombre(n) => {
                        out.push(0xE8); // call rel32
                        salida_huecos.push((out.len(), n.clone()));
                        out.extend_from_slice(&[0, 0, 0, 0]);
                    }
                    otro => {
                        // Una llamada a un valor --una funcion guardada en una
                        // variable-- pide `call reg`. Se deja sin emitir en vez
                        // de emitir algo que salta a donde no debe.
                        let _ = otro;
                    }
                }

                // Lo que devuelve viene en el registro de retorno.
                if let Some(d) = destino {
                    guarda_temporal(out, IZQ, *d, &marco);
                }
            }
            // El metal se emite cuando el emisor lea `intrinsics.toml`. Se deja
            // marcado, no escondido.
            Instr::Metal { .. } => {}
        }
    }

    // Toda funcion acaba volviendo, aunque el fuente no lo diga.
    epilogo(out);

    // El sitio al que van las comprobaciones que fallan.
    if !huecos_de_atrapa.is_empty() {
        let atrapa = out.len();
        // Por ahora el codigo de error se pone en rax y se vuelve. Cuando haya
        // errores como datos de verdad, esto construye el valor de error.
        x86::mov_r64_imm64(out, IZQ, 1001);
        epilogo(out);
        for h in huecos_de_atrapa {
            let rel = (atrapa as i64 - (h as i64 + 4)) as i32;
            out[h..h + 4].copy_from_slice(&rel.to_le_bytes());
        }
    }

    // Y ahora los saltos.
    for (hueco, etiqueta) in huecos {
        let destino = sitios_de_etiqueta
            .iter()
            .find(|(e, _)| *e == etiqueta)
            .map(|(_, off)| *off)
            .unwrap_or(out.len());
        let rel = (destino as i64 - (hueco as i64 + 4)) as i32;
        out[hueco..hueco + 4].copy_from_slice(&rel.to_le_bytes());
    }

    cuenta.comprobaciones = comprobaciones;
    cuenta.huecos_de_llamada = salida_huecos;
    cuenta
}

fn epilogo(out: &mut Vec<u8>) {
    x86::mov_r64_r64(out, 4, 5); // mov rsp, rbp
    out.push(0x5D); // pop rbp
    out.push(0xC3); // ret
}

fn carga(out: &mut Vec<u8>, reg: u8, v: &Valor, marco: &Marco) {
    match v {
        Valor::Const(Const::Entero(n)) => x86::mov_r64_imm64(out, reg, *n as u64),
        Valor::Const(Const::Logico(b)) => x86::mov_r64_imm64(out, reg, u64::from(*b)),
        Valor::Const(Const::Nada) => x86::zero_r32(out, reg),
        // Un decimal exacto no cabe en un inmediato: lo construye el runtime.
        Valor::Const(Const::Decimal(_)) | Valor::Const(Const::Texto(_)) => {
            x86::zero_r32(out, reg)
        }
        Valor::Local(l) => mov_de_marco(out, reg, marco.local(*l)),
        // ** F3: si el temporal vive en un registro, esto es un `mov` entre
        // registros en vez de una lectura de memoria. Ese es el 2-4x, y cabe en
        // estas tres lineas porque la IR ya traia los temporales.
        Valor::Temporal(t) => match marco.sitio(*t) {
            Sitio::Registro(r) => {
                if r != reg {
                    x86::mov_r64_r64(out, reg, r);
                }
            }
            Sitio::Pila(disp) => mov_de_marco(out, reg, disp),
        },
        // Una funcion o algo de un `usa`: lo resuelve el enlazado, que todavia
        // no existe.
        Valor::Nombre(_) => x86::zero_r32(out, reg),
    }
}

fn guarda_temporal(
    out: &mut Vec<u8>,
    reg: u8,
    t: bmo_inti_front::ir::Temporal,
    marco: &Marco,
) {
    match marco.sitio(t) {
        Sitio::Registro(r) => {
            if r != reg {
                x86::mov_r64_r64(out, r, reg);
            }
        }
        Sitio::Pila(disp) => mov_a_marco(out, disp, reg),
    }
}

/// `mov reg, [rbp+disp]`
fn mov_de_marco(out: &mut Vec<u8>, reg: u8, disp: i32) {
    out.push(0x48);
    out.push(0x8B);
    out.push(0x85 | (reg << 3));
    out.extend_from_slice(&disp.to_le_bytes());
}

/// `mov [rbp+disp], reg`
fn mov_a_marco(out: &mut Vec<u8>, disp: i32, reg: u8) {
    out.push(0x48);
    out.push(0x89);
    out.push(0x85 | (reg << 3));
    out.extend_from_slice(&disp.to_le_bytes());
}

fn binaria(out: &mut Vec<u8>, op: Op) {
    match op {
        Op::Suma => x86::add_r64_r64(out, IZQ, DER),
        Op::Resta => x86::sub_r64_r64(out, IZQ, DER),
        Op::Por => x86::imul_r64_r64(out, IZQ, DER),
        Op::Entre | Op::Divide => {
            x86::cqo(out);
            x86::idiv_r64(out, DER);
        }
        Op::Resto => {
            x86::cqo(out);
            x86::idiv_r64(out, DER);
            x86::mov_r64_r64(out, IZQ, 2); // el resto vive en rdx
        }
        Op::BitsY => {
            out.extend_from_slice(&[0x48, 0x21, 0xC8]); // and rax, rcx
        }
        Op::BitsO => x86::or_r64_r64(out, IZQ, DER),
        Op::BitsXor => x86::xor_r64_r64(out, IZQ, DER),
        // Las comparaciones dejan el resultado en 0/1.
        Op::Igual | Op::NoEs | Op::Menor | Op::Mayor | Op::MenorIgual | Op::MayorIgual => {
            x86::cmp_r64_r64(out, IZQ, DER);
            // ** El orden importa y costo un test: `setcc` PRIMERO y despues
            // extender. Poner el registro a cero antes con un `xor` --que es lo
            // que hace `zero_r32`-- **destruye las banderas que el `cmp` acaba
            // de dejar**, y entonces la comparacion contesta siempre lo mismo.
            let cc = match op {
                Op::Igual => 0x94,
                Op::NoEs => 0x95,
                Op::Menor => 0x9C,
                Op::Mayor => 0x9F,
                Op::MenorIgual => 0x9E,
                _ => 0x9D,
            };
            out.extend_from_slice(&[0x0F, cc, 0xC0]); // setcc al
            out.extend_from_slice(&[0x48, 0x0F, 0xB6, 0xC0]); // movzx rax, al
        }
        // Lo que pide runtime o no cabe en una instruccion.
        _ => {}
    }
}

/// Envuelve el codigo en un `.bex` y **lo pasa por el gate**.
///
/// Ningun `.bex` del sistema se escribe sin pasar por `bmo-verify`: es el unico
/// checkpoint comun, y aqui no se abre un quinto camino que lo esquive.
pub fn empaquetar(e: &Emitido) -> Result<Vec<u8>, String> {
    let mut b = BefBuilder::new();
    b.add_section(BefSection::code(e.codigo.clone()));
    let bytes = b.build().map_err(|x| x.to_string())?;

    match bmo_verify::verify(&bytes) {
        bmo_verify::Verdict::Ok => Ok(bytes),
        bmo_verify::Verdict::Rejected(motivos) => Err(motivos.join("; ")),
    }
}

#[cfg(test)]
mod pruebas;
