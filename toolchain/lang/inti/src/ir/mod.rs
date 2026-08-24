//! `ir` -- del arbol a instrucciones, sin nombrar ninguna maquina.
//!
//! ## Por que existe una IR, y por que ANTES del emisor
//!
//! Se podria emitir directamente del arbol. BMO C lo hace, y por eso evalua las
//! expresiones **empujandolas a la pila y sacandolas**, que es el techo del que
//! habla la seccion 13.6 del maestro: sin una forma intermedia con temporales
//! **no hay donde repartir los sitios rapidos de la maquina**, y sin eso
//! ninguna otra optimizacion se nota.
//!
//! (Esa frase se escribio primero nombrando un registro concreto, y
//! `tests/agnostico.rs` la tumbo. Tenia razon: si para explicar por que existe
//! la IR hace falta nombrar una maquina, la explicacion esta mal contada.)
//!
//! Asi que la IR no es una capa de mas: es el sitio donde cabe el 2-4x. Y va
//! antes del emisor porque hacerla despues significa reescribir el emisor
//! entero.
//!
//! ## Lo que este modulo se niega a saber
//!
//! **Todo lo de la maquina.** No hay registros, ni opcodes, ni anchos de
//! palabra. Un `Temporal` es un valor con nombre y sin sitio; donde acabe
//! viviendo lo decide otro. `tests/agnostico.rs` vigila este fichero como
//! vigila los demas.
//!
//! ## ** Y lo que la IR hace VISIBLE
//!
//! Las doce reglas de `REGLAS.md` dejan de ser un documento aqui: una suma de
//! enteros **emite su comprobacion de desbordamiento como una instruccion**, y
//! se puede contar. Un test comprueba que `a + b` genera esa comprobacion y que
//! `a + b` con `suma_circular` no.
//!
//! Eso es lo que separa *"INTI no tiene comportamiento indefinido"* de una
//! frase: aqui se ve.

//! ## El reparto de este directorio (L6b)
//!
//! ```text
//!    forma.rs   QUE es una instruccion       -> tipos, y cero decisiones
//!    mod.rs     COMO se llega a una          -> recorre el arbol y decide
//! ```
//!
//! ** El corte se eligio por la PREGUNTA y no por el tamano. La senal de que
//! esta bien puesto: `forma.rs` no importa nada de aqui, y quien solo quiera
//! saber que forma tiene una instruccion --el emisor, el marco-- no tiene que
//! leer ni una linea del descenso.

mod descenso;
mod expresion;
use descenso::Descenso;
pub mod forma;

pub use forma::{ClaseCongelada, Congelado,
    Clase, Comprobacion, Const, Etiqueta, FuncionIr, Instr, Local, ModuloIr, Temporal, Valor,
};

use crate::arbol::{self, Bloque, Decl, Expr, Modulo, Op, Repeticion, Sent};
use crate::aviso::Cosecha;

/// Baja un modulo entero.
pub fn bajar(m: &Modulo) -> Cosecha<ModuloIr> {
    let plano = crate::disposicion::comprobar(m, crate::disposicion::Medidas::por_defecto()).valor;
    let tabla = crate::tablas::Modulos::por_defecto();
    let metal = metal_que_declara(m, &bmo_mods::Roots::find(), &tabla);
    bajar_con(m, &tabla, &plano, &metal)
}

/// Los nombres que son una instruccion, segun lo que el FUENTE declaro.
///
/// ## ** Las dos fuentes, y por que son dos
///
/// ```text
///    usa x86_64     nombres que SOLO existen ahi   -> el fichero no se porta
///    usa binarios   nombres que existen en todas   -> el fichero SI se porta
/// ```
///
/// Las dos acaban emitiendo una instruccion en esta maquina, y por eso salen
/// juntas de aqui. Lo que cambia es lo que el programa **declaro**, y eso ya lo
/// cuenta `perfil` -- que es donde tiene que contarse.
///
/// OJO: esto vive en `ir` y no nombra ninguna maquina. El nombre `"x86_64"` sale
/// de `m.usa`, o sea del fichero del usuario. Buscar una tabla por un nombre que
/// te dan no es conocerla.
pub fn metal_que_declara(
    m: &Modulo,
    raices: &bmo_mods::Roots,
    tabla: &crate::tablas::Modulos,
) -> Vec<String> {
    let mut v = Vec::new();
    for (n, _) in &m.usa {
        if let Some(maquina) = crate::arquitectura::Maquina::buscar(raices, n) {
            v.extend(maquina.nombres_que_trae());
        } else {
            // ** Un modulo de REX cuyos nombres son instrucciones aqui. Hoy solo
            // `binarios`, y por eso la pregunta se hace a la TABLA y no con un
            // `if n == "binarios"`: el dia que haya un segundo, es una fila.
            if tabla.son_instrucciones(n) {
                v.extend(tabla.trae(n).iter().cloned());
            }
        }
    }
    v
}

/// Baja un modulo entero sabiendo que trae cada `usa`.
///
/// ** La tabla que entra aqui es AGNOSTICA: dice que `lee_natural64` lee ocho
/// bytes y que `mi_tarea` vale tal numero. Ninguna de las dos cosas depende de
/// una maquina, y por eso este modulo puede leerlas sin romper su promesa.
pub fn bajar_con(
    m: &Modulo,
    tabla: &crate::tablas::Modulos,
    plano: &crate::disposicion::Plano,
    metal: &[String],
) -> Cosecha<ModuloIr> {
    let mut salida = ModuloIr::default();

    // *** LAS CONSTANTES CONGELADAS, ANTES QUE NADA.
    //
    // Se recogen en una pasada propia y no dentro del bucle de abajo porque una
    // funcion declarada ARRIBA puede usar una constante declarada abajo: en el
    // nivel superior no hay orden, todo se congela cuando el modulo acaba de
    // cargarse. Resolverlas sobre la marcha obligaria a ordenar el fichero.
    //
    // ** Hasta el 2026-08-22 esta pasada no existia y `Decl::Constante` se
    // tiraba con un `{}`. El nombre llegaba suelto al emisor y `carga` lo bajaba
    // a un CERO: `maximo = 100` compilaba, pasaba el gate, salia firmado y valia
    // cero. Con su ejemplo escrito en `GRAMATICA.md`.
    let mut congeladas: std::collections::HashMap<String, Const> =
        std::collections::HashMap::new();
    let mut tablas: std::collections::HashMap<String, u32> = std::collections::HashMap::new();
    // El mapa "indice del pozo -> indice en congelados", compartido por todas
    // las funciones: el pozo no se repite, asi que su congelado tampoco.
    let mut textos_congelados: Vec<u32> = Vec::new();

    for d in &m.declaraciones {
        if let Decl::Constante { nombre, valor, .. } = d {
            if let Some(c) = congelar(valor) {
                congeladas.insert(nombre.clone(), c);
            } else if let Some((bytes, ancho)) = congelar_tabla(valor) {
                // ** Una TABLA congelada: no cabe en un inmediato, asi que va a
                // `RoData` y lo que se carga es su direccion.
                tablas.insert(nombre.clone(), salida.congelados.len() as u32);
                salida.congelados.push(crate::ir::forma::Congelado {
                    nombre: nombre.clone(),
                    bytes,
                    ancho,
                    clase: crate::ir::forma::ClaseCongelada::Tabla,
                });
            }
            // ** Lo que NO se deja congelar no se inventa: se queda fuera, el
            // nombre llega suelto al emisor y **el emisor lo dice** por
            // `sin_emitir`. Meter aqui un cero seria repetir el fallo que esta
            // pasada viene a cerrar.
        }
    }

    for d in &m.declaraciones {
        match d {
            Decl::Funcion(f) => {
                let ir = Descenso::nueva(&mut salida.textos, &mut salida.congelados, &mut textos_congelados, &congeladas, &tablas, tabla, plano, m.perfil, metal).funcion(f);
                salida.funciones.push(ir);
            }
            Decl::Operacion { tipo, funcion } => {
                let mut ir = Descenso::nueva(&mut salida.textos, &mut salida.congelados, &mut textos_congelados, &congeladas, &tablas, tabla, plano, m.perfil, metal).funcion(funcion);
                // El nombre lleva el tipo delante para que dos operaciones con
                // el mismo nombre en tipos distintos no se pisen.
                ir.nombre = format!("{}.{}", tipo, funcion.nombre);
                salida.funciones.push(ir);
            }
            Decl::Registro {
                nombre,
                operaciones,
                ..
            } => {
                for f in operaciones {
                    let mut ir = Descenso::nueva(&mut salida.textos, &mut salida.congelados, &mut textos_congelados, &congeladas, &tablas, tabla, plano, m.perfil, metal).funcion(f);
                    ir.nombre = format!("{}.{}", nombre, f.nombre);
                    salida.funciones.push(ir);
                }
            }
            Decl::Constante { .. } => {}
        }
    }

    Cosecha::nueva(salida)
}


/// Que comprobacion pide cada operacion. Sale de `REGLAS.md` y de ningun otro
/// sitio.
/// La que se sabe ANTES de operar, mirando un operando.
///
/// ** Solo hay una familia aqui, y es la de dividir: el cero del divisor es la
/// unica cosa que hay que ver **antes**, porque despues de la division no queda
/// programa que mire nada.
fn comprobacion_antes(op: Op, clase: Clase) -> Option<Comprobacion> {
    if matches!(clase, Clase::Flotante) {
        return None;
    }
    match op {
        Op::Divide | Op::Entre | Op::Resto => Some(Comprobacion::EntreCero),
        _ => None,
    }
}

/// **Un valor que se puede CONGELAR al cargar el modulo.**
///
/// ## Que entra y que no
///
/// Entran los literales: un numero, un `si/no`, `nada`. Y el menos de un numero,
/// porque `-1` se escribe con dos piezas y nadie lo lee como dos cosas.
///
/// ** NO entra una llamada, ni una operacion entre dos nombres, ni nada que pida
/// ejecutar algo. No es una limitacion tecnica: es lo que significa **congelado**
/// en la seccion 10.2 del maestro -- *"inmortal, nadie lo cambia"*. Un valor que
/// hay que calcular no esta congelado, esta pendiente.
///
/// *** Y lo que no entra **no se inventa**. Devolver un cero aqui seria
/// exactamente el fallo que esta funcion viene a cerrar.
fn congelar(e: &Expr) -> Option<Const> {
    match e {
        Expr::Numero(n, _) if !n.con_punto => {
            parse_entero(&n.texto, n.base).map(Const::Entero)
        }
        // ** El flotante se congela como BITS y no como texto: es `llano` quien
        // usa constantes hoy, y alli `3.5` es IEEE-754. En `pleno` un `numero`
        // es decimal exacto y esa conversion lo estropearia -- por eso no se
        // hace aqui, se deja fuera y el emisor lo dice.
        Expr::Numero(n, _) => n.texto.parse::<f64>().ok().map(|f| Const::Flotante(f.to_bits())),
        Expr::Logico(b, _) => Some(Const::Logico(*b)),
        Expr::Nada(_) => Some(Const::Nada),
        Expr::Unaria { op: crate::arbol::OpUno::Menos, valor, .. } => match congelar(valor)? {
            Const::Entero(v) => Some(Const::Entero(-v)),
            Const::Flotante(bits) => Some(Const::Flotante((-f64::from_bits(bits)).to_bits())),
            _ => None,
        },
        _ => None,
    }
}

/// **Una lista de literales, congelada a bytes.**
///
/// ## El ancho, que es la decision de esta funcion
///
/// Los elementos salen como **`entero64`**, ocho bytes cada uno. Es una eleccion
/// y se dice: la gramatica de una constante --`NOMBRE = expr`-- no tiene sitio
/// para escribir el tipo, asi que hay que elegir uno.
///
/// ** Ocho porque **nunca trunca un literal**: una tabla de CRC-32 tiene valores
/// que no caben en cuatro bytes con signo, y una tabla que pierde bits en
/// silencio es peor que una tabla que ocupa el doble.
///
/// *** Lo que cuesta esta escrito para que se pueda cambiar cuando haga falta:
/// una tabla de 256 bytes ocupa 2 KiB en vez de 256. El dia que la gramatica
/// deje decir `senos es bufer de entero32 = [...]`, **lo que cambia es este
/// numero**.
///
/// Y no entra nada que haya que calcular: una lista con una llamada dentro no
/// esta congelada, esta pendiente.
fn congelar_tabla(e: &Expr) -> Option<(Vec<u8>, u32)> {
    let Expr::Lista(elementos, _) = e else {
        return None;
    };
    let mut bytes = Vec::with_capacity(elementos.len() * 8);
    for x in elementos {
        match congelar(x)? {
            Const::Entero(v) => bytes.extend_from_slice(&v.to_le_bytes()),
            Const::Flotante(b) => bytes.extend_from_slice(&b.to_le_bytes()),
            Const::Logico(b) => bytes.extend_from_slice(&(b as u64).to_le_bytes()),
            // ** Un decimal exacto no cabe en ocho bytes y no se le inventa una
            // conversion: se queda fuera y el emisor lo dice.
            _ => return None,
        }
    }
    Some((bytes, 8))
}

/// La que se sabe DESPUES, mirando lo que la operacion dejo dicho.
fn comprobacion_despues(op: Op, clase: Clase) -> Option<Comprobacion> {
    // ** LA COMA FLOTANTE NO LLEVA COMPROBACION, y no es una excepcion comoda
    // a "INTI no tiene comportamiento indefinido". Es que ya esta definido.
    //
    // La Regla 1 y la Regla 3 existen porque en los ENTEROS desbordar y dividir
    // entre cero **no tienen respuesta**: cualquier bit que salga es una
    // invencion del compilador. En IEEE-754 si la tienen --infinito y NaN, que
    // son valores con los que se puede seguir operando-- y esta escrita en una
    // norma de 1985.
    //
    // Atrapar aqui no anadiria ni una pizca de seguridad. Quitaria la
    // aritmetica: un calculo que desborda a infinito y luego vuelve al rango es
    // corriente, y con una trampa en medio no se puede escribir.
    if matches!(clase, Clase::Flotante) {
        return None;
    }
    match op {
        // Regla 1: las tres que se pasan de la cuenta.
        Op::Suma | Op::Resta | Op::Por | Op::Elevado => Some(Comprobacion::Desborde),
        // Comparar, los bits y la logica no pueden salirse. Y la Regla 3 ya no
        // esta aqui: se mudo a `comprobacion_antes`, que es donde servia.
        _ => None,
    }
}

/// El valor de un literal. **La cuenta vive en `lexico`**, que es de quien es el
/// numero -- aqui solo se pide.
fn parse_entero(texto: &str, base: crate::lexico::Base) -> Option<i64> {
    crate::lexico::valor_entero(texto, base)
}

#[cfg(test)]
mod pruebas;
