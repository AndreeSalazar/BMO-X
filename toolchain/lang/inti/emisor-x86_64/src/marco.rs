//! `marco` -- donde cae cada valor dentro de una funcion.
//!
//! ## Lo que decide, y lo que NO
//!
//! La IR habla de `Local(3)` y `Temporal(7)`: **indices sin sitio**. Aqui se
//! convierten en un registro o en un desplazamiento, y para eso hace falta
//! saber el ancho de una palabra y cuantos registros hay -- que es justo lo que
//! el frontend tiene prohibido saber.
//!
//! Por eso este reparto vive en el crate de la maquina y no al otro lado de la
//! frontera.
//!
//! ## ** F3: los temporales viven en registros
//!
//! Hasta el 2026-08-19 todo iba a la pila, como hace BMO C, y eso era el techo
//! del que habla la seccion 13.6 del maestro. Ahora un temporal vive en un
//! registro **si le toca uno**, y en la pila si no.
//!
//! Y el cambio ocurrio **en este fichero y en nada mas**, que es exactamente lo
//! que `LINAJE.md` prometio: *"cuando llegue F3, `marco.rs` es lo unico que
//! cambia"*. Se pudo porque la IR ya traia los temporales -- que es lo unico que
//! un asignador necesita.
//!
//! ## El metodo: recorrido lineal
//!
//! Se calcula el TRAMO DE VIDA de cada temporal --de donde nace a donde se usa
//! por ultima vez-- y se recorren en orden repartiendo los registros libres.
//! Cuando un tramo acaba, su registro vuelve al bote.
//!
//! No es coloreado de grafo. El coloreado da mejores resultados en funciones
//! grandes y **pide un grafo de interferencia entero**; el recorrido lineal saca
//! la mayor parte del beneficio con una pasada. Es lo que usan los JIT por el
//! mismo motivo, y es lo que cabe en un fichero que se puede leer de una vez.
//!
//! ## OJO: El freno, y por que existe
//!
//! **Si la funcion llama a alguien, no se asigna ningun registro.** Los tres que
//! se reparten aqui los puede pisar la funcion llamada --son de los que la
//! convencion deja tocar-- y guardarlos alrededor de cada llamada costaria mas
//! de lo que ahorran.
//!
//! Hoy el emisor no emite llamadas, asi que el freno no quita nada. Esta puesto
//! **antes** de que haga falta a proposito: el dia que se emitan, esto no se
//! rompe en silencio -- deja de optimizar, que es lo correcto.

use bmo_inti_front::ir::{FuncionIr, Instr, Local, Temporal, Valor};

/// El ancho de una palabra en esta maquina.
///
/// Sale de `arch/x86_64/inti.toml` cuando el compilador corre de verdad; aqui
/// hay una constante porque este crate **ES** el de esa maquina.
pub const PALABRA: i32 = 8;

/// Los registros que se reparten entre los temporales, **cuando la tabla de la
/// maquina no esta a mano**.
///
/// ** Es un respaldo, no la fuente. La lista de verdad vive en
/// `arch/x86_64/inti.toml`, seccion `[reparto]`, y llega por
/// [`Marco::con_registros`].
///
/// Existe por lo mismo que el vocabulario tiene respaldo: un emisor que no
/// arranca porque falta un fichero de datos es peor que uno que arranca con lo
/// que traia. Y aqui **si** se puede nombrar la maquina: este crate ES el de
/// esa maquina.
pub const RESPALDO: [u8; 3] = [2, 6, 7]; // rdx, rsi, rdi

/// Donde vive un valor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sitio {
    /// En un registro. Lo rapido.
    Registro(u8),
    /// En el marco, a este desplazamiento desde `rbp`.
    Pila(i32),
}

#[derive(Debug, Clone)]
pub struct Marco {
    locales: u32,
    temporales: u32,
    /// Donde vive cada temporal, por indice.
    sitios: Vec<Sitio>,
}

impl Marco {
    /// El reparto con los registros de respaldo.
    pub fn de(f: &FuncionIr) -> Self {
        Self::con_registros(f, &RESPALDO)
    }

    /// ** El reparto con los registros que diga la maquina.
    ///
    /// Es la forma buena: el emisor no decide cuales son, los recibe de
    /// `arch/<maquina>/inti.toml`. El dia que la tabla anada `r10` y `r11`,
    /// este fichero no cambia.
    pub fn con_registros(f: &FuncionIr, disponibles: &[u8]) -> Self {
        let mut m = Self {
            locales: f.locales,
            temporales: f.temporales,
            sitios: Vec::new(),
        };
        m.sitios = m.reparte(f, disponibles);
        m
    }

    /// Cuantos bytes hay que reservar, redondeado a 16.
    ///
    /// La alineacion de 16 no es adorno: la ABI la exige antes de una llamada,
    /// y saltarsela da un fallo que aparece **dentro de la funcion llamada**,
    /// que es el peor sitio donde puede aparecer un fallo.
    ///
    /// Se reserva sitio para **todos** los temporales aunque vivan en un
    /// registro. Son unos bytes de pila que no se tocan, y a cambio el
    /// desplazamiento de cada uno no depende de a quien le tocara registro --
    /// que es la clase de dependencia que convierte un fallo del asignador en
    /// un fallo del marco.
    pub fn size(&self) -> i32 {
        let bruto = (self.locales + self.temporales) as i32 * PALABRA;
        (bruto + 15) & !15
    }

    /// El desplazamiento de una local desde `rbp`. Negativo: el marco crece
    /// hacia abajo, que es lo que dice `la_pila_crece` en la tabla.
    pub fn local(&self, l: Local) -> i32 {
        -((l.0 as i32 + 1) * PALABRA)
    }

    /// Donde vive un temporal.
    pub fn sitio(&self, t: Temporal) -> Sitio {
        self.sitios
            .get(t.0 as usize)
            .copied()
            .unwrap_or_else(|| Sitio::Pila(self.en_pila(t)))
    }

    /// Su sitio en el marco, viva donde viva. Sirve para el reparto y para los
    /// que no consiguieron registro.
    pub fn en_pila(&self, t: Temporal) -> i32 {
        -((self.locales as i32 + t.0 as i32 + 1) * PALABRA)
    }

    /// Cuantos temporales viven en un registro. Es el numero que dice si el
    /// asignador esta haciendo algo.
    pub fn en_registros(&self) -> usize {
        self.sitios
            .iter()
            .filter(|s| matches!(s, Sitio::Registro(_)))
            .count()
    }

    // -----------------------------------------------------------------
    //  El reparto
    // -----------------------------------------------------------------

    fn reparte(&self, f: &FuncionIr, disponibles: &[u8]) -> Vec<Sitio> {
        let mut sitios: Vec<Sitio> = (0..f.temporales)
            .map(|i| Sitio::Pila(self.en_pila(Temporal(i))))
            .collect();

        // El freno: una llamada puede pisar estos tres registros.
        if f.instrucciones
            .iter()
            .any(|i| matches!(i, Instr::Llama { .. } | Instr::Metal { .. }))
        {
            return sitios;
        }

        let tramos = tramos_de_vida(f);

        // Recorrido lineal: los tramos ya salen ordenados por nacimiento,
        // porque un temporal nace donde se le asigna por primera vez.
        let mut libres: Vec<u8> = disponibles.to_vec();
        // (fin del tramo, registro) de lo que esta vivo ahora.
        let mut vivos: Vec<(usize, u8, u32)> = Vec::new();

        for (temporal, (nace, muere)) in tramos.iter().enumerate() {
            if *nace == usize::MAX {
                continue; // nunca se uso
            }

            // Lo que ya murio devuelve su registro.
            vivos.retain(|(fin, reg, _)| {
                if *fin < *nace {
                    libres.push(*reg);
                    false
                } else {
                    true
                }
            });

            if let Some(reg) = libres.pop() {
                sitios[temporal] = Sitio::Registro(reg);
                vivos.push((*muere, reg, temporal as u32));
            }
            // Si no queda registro, se queda en la pila. Sin drama y sin
            // desalojar a nadie: desalojar pide emitir movimientos, y eso ya no
            // es un recorrido lineal simple.
        }

        sitios
    }
}

/// De donde a donde vive cada temporal.
///
/// `usize::MAX` en el nacimiento quiere decir *nunca se uso*, que pasa con los
/// temporales de una expresion cuyo resultado se tira.
fn tramos_de_vida(f: &FuncionIr) -> Vec<(usize, usize)> {
    let mut tramos = vec![(usize::MAX, 0usize); f.temporales as usize];

    let mut toca = |t: Temporal, i: usize, tramos: &mut Vec<(usize, usize)>| {
        let e = &mut tramos[t.0 as usize];
        if e.0 == usize::MAX {
            e.0 = i;
        }
        if i > e.1 {
            e.1 = i;
        }
    };

    let mut mira = |v: &Valor, i: usize, tramos: &mut Vec<(usize, usize)>| {
        if let Valor::Temporal(t) = v {
            toca(*t, i, tramos);
        }
    };

    for (i, instr) in f.instrucciones.iter().enumerate() {
        match instr {
            Instr::Mueve { destino, origen } => {
                mira(origen, i, &mut tramos);
                toca(*destino, i, &mut tramos);
            }
            Instr::Binaria {
                destino,
                izquierda,
                derecha,
                ..
            } => {
                mira(izquierda, i, &mut tramos);
                mira(derecha, i, &mut tramos);
                toca(*destino, i, &mut tramos);
            }
            Instr::Unaria { destino, valor, .. } => {
                mira(valor, i, &mut tramos);
                toca(*destino, i, &mut tramos);
            }
            Instr::Comprueba { sobre, .. } => mira(sobre, i, &mut tramos),
            Instr::Guarda { valor, .. } => mira(valor, i, &mut tramos),
            Instr::Devuelve(Some(v)) => mira(v, i, &mut tramos),
            Instr::SaltaSi { cond, .. } => mira(cond, i, &mut tramos),
            Instr::Llama {
                destino,
                que,
                argumentos,
            } => {
                mira(que, i, &mut tramos);
                for a in argumentos {
                    mira(a, i, &mut tramos);
                }
                if let Some(d) = destino {
                    toca(*d, i, &mut tramos);
                }
            }
            Instr::Metal {
                destino,
                argumentos,
                ..
            } => {
                for a in argumentos {
                    mira(a, i, &mut tramos);
                }
                if let Some(d) = destino {
                    toca(*d, i, &mut tramos);
                }
            }
            _ => {}
        }
    }

    // OJO: Un temporal que cruza una etiqueta vive hasta el final.
    //
    // El recorrido lineal cuenta posiciones, no caminos, y un salto hacia atras
    // hace que la posicion 3 se ejecute despues de la 9. Sin esto, un temporal
    // de dentro de un bucle podria compartir registro con otro de fuera y
    // pisarlo en la segunda vuelta -- un fallo que solo aparece cuando el bucle
    // da mas de una.
    let hay_saltos = f
        .instrucciones
        .iter()
        .any(|i| matches!(i, Instr::Salta(_) | Instr::SaltaSi { .. }));
    if hay_saltos {
        let fin = f.instrucciones.len();
        for t in tramos.iter_mut() {
            if t.0 != usize::MAX {
                t.1 = fin;
            }
        }
    }

    tramos
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use bmo_inti_front::arbol::Op;
    use bmo_inti_front::ir::Const;

    fn funcion(locales: u32, temporales: u32, instrucciones: Vec<Instr>) -> FuncionIr {
        FuncionIr {
            nombre: "f".into(),
            parametros: 0,
            locales,
            temporales,
            instrucciones,
        }
    }

    #[test]
    fn cada_local_tiene_su_sitio_y_no_se_pisan() {
        let m = Marco::de(&funcion(3, 0, vec![]));
        assert_eq!(m.local(Local(0)), -8);
        assert_eq!(m.local(Local(1)), -16);
        assert_eq!(m.local(Local(2)), -24);
    }

    /// Los temporales van DETRAS de las locales. Si empezaran en el mismo
    /// sitio, un temporal pisaria un parametro.
    #[test]
    fn los_temporales_no_pisan_a_las_locales() {
        let m = Marco::de(&funcion(2, 2, vec![]));
        assert_eq!(m.local(Local(1)), -16);
        assert_eq!(m.en_pila(Temporal(0)), -24);
        assert_eq!(m.en_pila(Temporal(1)), -32);
    }

    #[test]
    fn el_marco_se_alinea_a_dieciseis() {
        assert_eq!(Marco::de(&funcion(1, 0, vec![])).size(), 16);
        assert_eq!(Marco::de(&funcion(3, 0, vec![])).size(), 32);
        assert_eq!(Marco::de(&funcion(0, 0, vec![])).size(), 0);
    }

    // ---------------------------------------------------------------
    //  ** F3: el reparto
    // ---------------------------------------------------------------

    fn suma(destino: u32, a: u32, b: u32) -> Instr {
        Instr::Binaria {
            destino: Temporal(destino),
            op: Op::Suma,
            izquierda: Valor::Temporal(Temporal(a)),
            derecha: Valor::Temporal(Temporal(b)),
        }
    }

    #[test]
    fn un_temporal_solo_se_lleva_un_registro() {
        let f = funcion(
            0,
            1,
            vec![
                Instr::Mueve {
                    destino: Temporal(0),
                    origen: Valor::Const(Const::Entero(1)),
                },
                Instr::Devuelve(Some(Valor::Temporal(Temporal(0)))),
            ],
        );
        let m = Marco::de(&f);
        assert!(matches!(m.sitio(Temporal(0)), Sitio::Registro(_)));
        assert_eq!(m.en_registros(), 1);
    }

    /// Hay tres registros. Cuando cuatro temporales estan vivos a la vez, el
    /// cuarto se queda en la pila **sin desalojar a nadie**: desalojar pide
    /// emitir movimientos, y eso ya no es un recorrido lineal.
    ///
    /// ** Este test se escribio esperando 3 y salieron 4, y el equivocado era
    /// el test: dos de los tramos habian MUERTO para cuando nacio el ultimo, y
    /// el asignador reutilizo su registro. Queda asi porque ensena lo que de
    /// verdad importa -- tres registros no son un tope de tres temporales, son
    /// un tope de tres A LA VEZ.
    #[test]
    fn cuando_se_acaban_los_registros_se_usa_la_pila() {
        let mut instrs = Vec::new();
        for i in 0..5u32 {
            instrs.push(Instr::Mueve {
                destino: Temporal(i),
                origen: Valor::Const(Const::Entero(i as i64)),
            });
        }
        // Todos vivos hasta el final.
        instrs.push(suma(5, 0, 1));
        instrs.push(suma(6, 2, 3));
        instrs.push(Instr::Devuelve(Some(Valor::Temporal(Temporal(4)))));

        let m = Marco::de(&funcion(0, 7, instrs));
        assert!(
            m.en_registros() < 7,
            "con siete temporales y tres registros, alguno tiene que ir a la pila"
        );
        assert_eq!(
            m.en_registros(),
            4,
            "tres a la vez, mas uno que reutiliza el registro de otro ya muerto"
        );
    }

    /// ** Un registro que queda libre se vuelve a usar. Es lo que hace que tres
    /// registros valgan para funciones con muchos mas temporales.
    #[test]
    fn un_registro_se_reutiliza_cuando_su_tramo_acaba() {
        // Cuatro temporales que NO se solapan: cada uno nace y muere seguido.
        let mut instrs = Vec::new();
        for i in 0..4u32 {
            instrs.push(Instr::Mueve {
                destino: Temporal(i),
                origen: Valor::Const(Const::Entero(1)),
            });
            instrs.push(Instr::Guarda {
                destino: Local(0),
                valor: Valor::Temporal(Temporal(i)),
            });
        }
        let m = Marco::de(&funcion(1, 4, instrs));
        assert_eq!(m.en_registros(), 4, "los cuatro, reutilizando registros");
    }

    /// OJO: El freno: si hay una llamada, no se reparte nada. Los tres registros
    /// los puede pisar la funcion llamada.
    #[test]
    fn con_una_llamada_no_se_reparte_ningun_registro() {
        let f = funcion(
            0,
            1,
            vec![
                Instr::Llama {
                    destino: Some(Temporal(0)),
                    que: Valor::Nombre("f".into()),
                    argumentos: vec![],
                },
                Instr::Devuelve(Some(Valor::Temporal(Temporal(0)))),
            ],
        );
        assert_eq!(Marco::de(&f).en_registros(), 0);
    }

    /// OJO: Y con saltos, todo tramo llega al final: el recorrido lineal cuenta
    /// posiciones y un salto hacia atras hace que la 3 se ejecute despues de la
    /// 9. Sin esto, un temporal de dentro de un bucle podria pisar a otro en la
    /// segunda vuelta -- un fallo que solo aparece cuando el bucle da mas de
    /// una.
    #[test]
    fn con_saltos_nadie_reutiliza_un_registro() {
        let mut instrs = Vec::new();
        for i in 0..4u32 {
            instrs.push(Instr::Mueve {
                destino: Temporal(i),
                origen: Valor::Const(Const::Entero(1)),
            });
            instrs.push(Instr::Guarda {
                destino: Local(0),
                valor: Valor::Temporal(Temporal(i)),
            });
        }
        instrs.push(Instr::Salta(bmo_inti_front::ir::Etiqueta(0)));

        let m = Marco::de(&funcion(1, 4, instrs));
        assert_eq!(m.en_registros(), 3, "sin reutilizar: solo caben tres");
    }

    /// El sitio en el marco de un temporal **no depende** de si le toco
    /// registro. Esa dependencia convertiria un fallo del asignador en un fallo
    /// del marco, que es mucho mas dificil de encontrar.
    #[test]
    fn el_sitio_en_la_pila_no_depende_del_reparto() {
        let con = Marco::de(&funcion(1, 3, vec![]));
        assert_eq!(con.en_pila(Temporal(0)), -16);
        assert_eq!(con.en_pila(Temporal(2)), -32);
    }
}
