//! `disposicion::deduccion` -- de que tipo es esto, cuando nadie lo escribio.
//!
//! ## Por que sale de `mod.rs` (L6a, 2026-08-23)
//!
//! Porque son dos preguntas, y la segunda nacio el mismo dia:
//!
//! ```text
//!    disposicion   cuanto MIDE un tipo y donde cae cada campo
//!    deduccion     que tipo TIENE algo que no lo dice
//! ```
//!
//! ** La primera es aritmetica sobre tipos escritos. La segunda mira el arbol y
//! decide -- y crece con el lenguaje: cada forma nueva que se pueda deducir
//! anade un brazo aqui y ninguno alli.
//!
//! *** Y las reglas de aqui son POCAS a proposito, con su motivo escrito en
//! `tipos_de_con`: **deducir mal es peor que no deducir.** Un tipo que falta se
//! dice y se arregla; uno inventado no falla, lee ocho bytes donde habia cuatro.

use super::*;

/// Los tipos que se conocen dentro de una funcion, por nombre.
///
/// ## ** De donde salen, y de donde NO
///
/// Solo de lo que esta ESCRITO: los parametros (`p es Punto`) y las
/// declaraciones con tipo (`cambiante m es Punto = ...`). No hay inferencia.
///
/// Es una decision, no una carencia por rellenar despues. Con inferencia,
/// cambiar una linea de arriba cambia en silencio el ancho de un acceso a
/// memoria de veinte lineas mas abajo -- y el compilador no diria nada porque
/// no habria pasado nada raro. En un lenguaje que escribe sistema, **el ancho
/// de un acceso tiene que estar escrito en algun sitio que se pueda leer**.
///
/// Lo que no esta declarado no es un error por si mismo: solo lo es si alguien
/// intenta sacarle un campo o indexarlo.
pub fn tipos_de(f: &Funcion) -> HashMap<String, Tipo> {
    tipos_de_con(f, None)
}

/// Igual, pero **DEDUCIENDO** el tipo de lo que no lo dice (2026-08-23).
///
/// === Por que existe, y por que llego tarde ===
///
/// En `llano` los tipos son obligatorios (`E0020`), asi que recoger los
/// escritos bastaba y esta funcion no hacia falta. En `pleno` son **opcionales**
/// --seccion 10.11 del maestro-- y sin deducirlos, la mitad de este modulo
/// tenia que callarse alli: no se puede comprobar `a.nombre` sin saber que es
/// `a`.
///
/// Lo destapo `censo/f05_registro.inti`, que lleva declarando COMPILA desde F0:
///
/// ```text
///    a = Alumno("ana", 9)
///    escribe(a.nombre)
/// ```
///
/// *** **Y es la pieza que separa a INTI de servir para low-code**, que es donde
/// nadie escribe un tipo nunca.
///
/// === [!] LAS REGLAS SON POCAS A PROPOSITO ===
///
/// Este fichero ya tiene escrito el motivo, en el aviso de `para cada`:
/// *"un tipo supuesto aqui elegiria el ancho de un acceso a memoria"*. Deducir
/// **mal** es peor que no deducir: un tipo que falta se dice y se arregla; un
/// tipo inventado no falla, lee ocho bytes donde habia cuatro.
///
/// Asi que solo se deduce donde la respuesta es UNA:
///
/// ```text
///    x = Registro(...)     -> Registro    hay un `registro` con ese nombre
///    x = "hola"            -> texto       un literal de texto es un texto
///    x = y                 -> lo de `y`   copiar no cambia de tipo
/// ```
///
/// ** Y lo que NO se deduce, con su motivo, para que se vea que es una decision:
///
/// ```text
///    x = 1        el tipo de un literal numerico DEPENDE DEL PERFIL: en
///                 `pleno` es `numero` y en `llano` hay que escribirlo. Esta
///                 funcion no sabe en que perfil esta, y darselo para esto
///                 seria meterle el perfil a una deduccion que no lo necesita
///                 en ninguna otra regla.
///
///    x = [a, b]   pide que los elementos coincidan, y "coincidir" es una
///                 pregunta con reglas propias (que pasa con `[1, 2.5]`?).
///                 Nace con `lista de T` en ejecucion, no antes.
///
///    x = f()      pide el tipo de retorno de `f`, que hoy no se resuelve.
///                 Es el punto 2b de `ESTADO.md` y ya estaba en la lista.
/// ```
pub fn tipos_de_con(f: &Funcion, plano: Option<&Plano>) -> HashMap<String, Tipo> {
    let mut m = HashMap::new();
    for p in &f.parametros {
        if let Some(t) = &p.tipo {
            m.insert(p.nombre.clone(), t.clone());
        }
    }
    recoge_bloque_con(&f.cuerpo, &mut m, plano);
    m
}

/// El tipo de una expresion **para deducir**, con lo que se sabe hasta aqui.
///
/// [!] Devuelve `None` mucho mas de lo que parece, y eso es lo correcto: la
/// ausencia de respuesta se convierte en "este nombre no tiene tipo", que aguas
/// abajo **calla en `pleno`** en vez de acusar.
fn deduce(e: &Expr, sabidos: &HashMap<String, Tipo>, plano: Option<&Plano>) -> Option<Tipo> {
    match e {
        // `x = Registro(...)` -- el constructor.
        //
        // *** Y LA SENAL ES MAS FUERTE DE LO QUE PARECE: el arbol no trae un
        // `Nombre` aqui, trae un `Tipo`. La gramatica dice que **los tipos
        // empiezan por mayuscula**, asi que el parser ya separo las dos cosas
        // mucho antes de llegar aqui.
        //
        //     a = Alumno("ana")   ->  Llamada { que: Tipo("Alumno") }
        //     a = suma(1, 2)      ->  Llamada { que: Nombre("suma") }
        //
        // ** Una llamada a un `Tipo` no puede ser otra cosa que un constructor:
        // no hay nada mas en el lenguaje que se escriba asi. Por eso esta regla
        // no adivina -- lee.
        //
        // [!] Y aun asi se comprueba contra el plano. `Inventado("x")` tiene la
        // misma forma y no hay registro detras: deducir `Tipo::Nombre("Inventado")`
        // pondria en el mapa un tipo que no existe, y el siguiente en preguntar
        // se lo creeria.
        Expr::Llamada { que, .. } => {
            let p = plano?;
            match que.as_ref() {
                Expr::Tipo(n, _) => p.registro(n).map(|_| Tipo::Nombre(n.clone())),
                // `x = f()` -- lo que la funcion DIJO que devuelve (2026-08-23).
                //
                // *** Era el punto 2b de `ESTADO.md` y llevaba en la lista desde
                // que existe `tipos`: *"`si hay_algo()` no se comprueba porque el
                // tipo que devuelve una funcion no se resuelve todavia"*.
                //
                // ** Sale de lo que la funcion ESCRIBIO en su `devuelve T`, no de
                // mirarle el cuerpo. Una funcion que no lo dice sigue sin
                // contestar, y eso es lo correcto: deducirlo del primer
                // `devuelve` que aparezca elegiria un tipo por orden de lectura.
                Expr::Nombre(n, _) => p.retorno_de(n).cloned(),
                _ => None,
            }
        }
        Expr::Texto(_, _) => Some(Tipo::Nombre("texto".to_string())),
        // Copiar no cambia de tipo. Y solo vale para lo que YA se sabe, asi que
        // el orden de las lineas manda -- que es el orden en que se leen.
        Expr::Nombre(n, _) => sabidos.get(n).cloned(),
        _ => None,
    }
}

fn recoge_bloque_con(b: &Bloque, m: &mut HashMap<String, Tipo>, plano: Option<&Plano>) {
    for s in b {
        match s {
            Sent::Asigna {
                destino,
                tipo: Some(t),
                ..
            } => {
                if let Expr::Nombre(n, _) = destino {
                    m.insert(n.clone(), t.clone());
                }
            }
            // ** El tipo NO escrito: se deduce, y **el escrito siempre gana**.
            //
            // Este brazo va detras del de arriba a proposito. Si alguien
            // escribio `a es Alumno`, eso es lo que vale aunque la deduccion
            // opinara otra cosa -- porque entonces lo que hay es un error de
            // tipos, y decirlo es de `tipos`, no de aqui.
            Sent::Asigna {
                destino,
                tipo: None,
                valor,
                ..
            } => {
                if let Expr::Nombre(n, _) = destino {
                    // [!] Y no se pisa un tipo ya sabido. `a = otra_cosa` mas
                    // abajo no re-declara `a`: en INTI una variable no cambia
                    // de tipo, y si el valor no cuadra eso es un error, no una
                    // redefinicion.
                    if !m.contains_key(n) {
                        if let Some(t) = deduce(valor, m, plano) {
                            m.insert(n.clone(), t);
                        }
                    }
                }
            }
            Sent::Si { ramas, sino, .. } => {
                for (_, cuerpo) in ramas {
                    recoge_bloque_con(cuerpo, m, plano);
                }
                if let Some(c) = sino {
                    recoge_bloque_con(c, m, plano);
                }
            }
            Sent::Repite { cuerpo, .. } => recoge_bloque_con(cuerpo, m, plano),
            // OJO: `para cada x en xs` declara `x`, pero su tipo sale del tipo
            // de `xs`, y eso es una pregunta que este modulo todavia no
            // contesta. Se deja marcado en vez de meter una suposicion: un tipo
            // supuesto aqui elegiria el ancho de un acceso a memoria.
            Sent::ParaCada { cuerpo, .. } => recoge_bloque_con(cuerpo, m, plano),
            Sent::Crudo { cuerpo, .. } => recoge_bloque_con(cuerpo, m, plano),
            Sent::Paralelo { cuerpo, .. } => recoge_bloque_con(cuerpo, m, plano),
            _ => {}
        }
    }
}
