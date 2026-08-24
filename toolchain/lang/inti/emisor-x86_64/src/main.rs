//! `inti` -- el compilador, con linea de ordenes.
//!
//! ## ** Por que esto faltaba, y lo que significaba que faltara
//!
//! Hasta hoy INTI compilaba **solo dentro de sus propias pruebas**. No habia
//! forma de coger un `.inti` de un disco y sacar un `.bex`, asi que:
//!
//! - la foto del Ryzen era imposible: no habia fichero que llevar a la maquina;
//! - nadie que no fuera el banco podia escribir un programa;
//! - y los numeros de CABINA se calculaban y no los veia nadie.
//!
//! Es la misma clase de agujero que F5d --*la pieza que se calcula bien y no la
//! lee nadie*-- vista desde mas arriba: el compilador entero estaba escrito y
//! **no tenia puerta de entrada**.
//!
//! ## Por que vive en el crate del EMISOR y no en el frontend
//!
//! Porque produce bytes de una maquina, y el frontend tiene prohibido nombrar
//! ninguna. El dia que exista `emisor-aarch64` tendra su propio `inti` al lado,
//! y el nombre del directorio dira cual es cual -- igual que `usa x86_64` lo
//! dice en el fichero del usuario.
//!
//! ## ** Y lo que NO hace, que es la pregunta de Eddi (21-08)
//!
//! > *"si INTI es inspirado en Python, no se espera que pueda tomar control en
//! > BEX, antes de BEF? Python es como ya sabes, todo. INTI no es posible?"*
//!
//! **Esto no ejecuta nada.** Compila, y quien ejecuta es el kernel cargando un
//! `.bex` firmado. Y la respuesta corta a la pregunta es que **Python tampoco
//! ejecuta un `.py`**: cuando escribes `./script.py`, el kernel lee la primera
//! linea, carga el BINARIO del interprete y le pasa tu fichero como un dato. El
//! `.py` nunca fue ejecutable; el interprete si.
//!
//! Asi que la comodidad que se quiere --`float.i` y ya-- se consigue igual aqui
//! y sin tocar el gate: este programa es ese binario. Lo unico que falta es que
//! la consola sepa que un `.i` se le entrega a el.

use std::path::{Path, PathBuf};
use std::process::exit;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let mut fuente: Option<PathBuf> = None;
    let mut salida: Option<PathBuf> = None;
    let mut informe = false;
    let mut solo_mirar = false;

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "-o" | "--salida" => {
                i += 1;
                match args.get(i) {
                    Some(p) => salida = Some(PathBuf::from(p)),
                    None => fin("`-o` pide una ruta detras"),
                }
            }
            // ** El informe de CABINA. No es una curiosidad: son los numeros que
            // el compilador SABE --a que maquina se ata, cuanto paga por no
            // tener comportamiento indefinido-- y hasta hoy no los veia nadie
            // porque no habia por donde pedirlos.
            "-i" | "--informe" => informe = true,
            // Compila y no escribe. Para saber si un fuente esta bien sin
            // ensuciar el disco con un `.bex` que no se va a usar.
            "-c" | "--comprueba" => solo_mirar = true,
            // ** LO QUE SI SE HACER. Ver `puedo()`.
            "-p" | "--puedo" => {
                puedo();
                exit(0);
            }
            "-h" | "--ayuda" => {
                ayuda(&args[0]);
                exit(0);
            }
            otro if otro.starts_with('-') => fin(&format!("no conozco la opcion `{}`", otro)),
            otro => fuente = Some(PathBuf::from(otro)),
        }
        i += 1;
    }

    let Some(ruta) = fuente else {
        ayuda(&args[0]);
        exit(2);
    };
    let nombre = ruta.display().to_string();
    let texto = match std::fs::read_to_string(&ruta) {
        Ok(t) => t,
        Err(e) => fin(&format!("no puedo leer {}: {}", nombre, e)),
    };

    // -- El compilador entero, por el mismo camino que usan las pruebas -----
    //
    // ** Por `informar` y no montando los analisis a mano: si este programa
    // compilara por otro camino, estaria probando otro compilador. Es la misma
    // regla que el banco se puso en F2d.
    let (parte, eventos) = bmo_inti_front::informar(&texto, &nombre);

    // ** LOS AVISOS SALEN DE `comprobar`, QUE ES EL QUE LOS JUNTA TODOS.
    //
    // Aqui habia una lista escrita a mano con TRES analisis --sintaxis,
    // disposicion, tipos-- y el compilador corria los cinco. Los otros dos
    // --`perfil` y `nombres`-- se calculaban y **se tiraban**.
    //
    // *** Lo que eso significaba: `crudo` dentro de `perfil pleno` NO SE
    // DENUNCIABA. Un nombre desconocido tampoco. La sonda `p04_crudo_en_pleno`
    // del censo daba su E0071 en el banco y salia limpia por la linea de
    // ordenes, que es por donde la usa una persona.
    //
    // ** Y la causa no era olvidar dos lineas: era escribir a mano una lista que
    // ya existia en otro sitio. Es el mismo fallo que el censo tenia con sus
    // diez sondas, y por eso el arreglo no es anadir dos entradas -- es usar la
    // funcion que los junta, para que no se pueda volver a olvidar ninguno.
    let revisado = bmo_inti_front::comprobar(&texto);

    let arbol = bmo_inti_front::armar(&texto);
    let raices = bmo_mods::Roots::find();
    let modulos = bmo_inti_front::tablas::Modulos::cargar(&raices);
    let plano = bmo_inti_front::disposicion::comprobar(
        &arbol.valor,
        bmo_inti_front::disposicion::Medidas::cargar(&raices),
    );

    // -- Los avisos, con el formato de cuatro partes ------------------------
    //
    // Se pintan TODOS antes de decidir si se sigue: un compilador que para en
    // el primero obliga a compilar diez veces para ver diez errores.
    let mut hay_error = false;
    for a in &revisado.avisos {
        eprint!("{}", a.pintar(&nombre));
        if a.codigo.0.starts_with('E') {
            hay_error = true;
        }
    }
    if hay_error {
        eprintln!("no se ha escrito nada.");
        exit(1);
    }

    // -- El descenso y los bytes -------------------------------------------
    let metal = bmo_inti_front::ir::metal_que_declara(&arbol.valor, &raices, &modulos);
    let ir = bmo_inti_front::ir::bajar_con(&arbol.valor, &modulos, &plano.valor, &metal,
        &bmo_inti_front::necesidades::Necesidades::cargar(&raices)).valor;
    let emitido = bmo_inti_x86_64::emitir(&ir);

    // ** LO QUE NO LLEGO A UN BYTE, y sale ANTES de escribir nada.
    //
    // Un intrinseco mudo no rompe la compilacion --el resto del programa esta
    // bien-- asi que sin esto la unica senal seria el binario haciendo otra cosa
    // en metal. Y para un fichero que va a un Ryzen, esa senal llega tarde.
    // *** Y ESTO YA NO ES UN AVISO: ES UN NO (2026-08-23).
    //
    // ## Las manos desnudas del gate
    //
    // Aqui ponia `eprintln!("aviso: ...")` y se seguia: el `.ibex` se escribia
    // igual. Y eso deja pasar lo que ninguna tabla de tipos puede ver -- **una
    // llamada sin destino es un `call` a un simbolo que no existe**, o sea un
    // binario que carga, salta a la nada, y se lleva la maquina por delante.
    //
    // ** El gate de perfiles lo tapaba por accidente: como `pleno` no compilaba,
    // nadie llegaba hasta aqui. Al hacer el gate ATOMICO --que mira lo que usas
    // en vez de tu etiqueta-- ese agujero quedaba a la vista, y taparlo es la
    // otra mitad del mismo cambio.
    //
    // *** Y es la ley de esta casa aplicada sin excepcion: **nada que compile y
    // no haga lo que dice**. Cada linea de `sin_emitir` es literalmente "esto se
    // pidio y no llego a un byte". Un binario con una de esas no hace lo que
    // dice su fuente, y no hay grado intermedio.
    if !emitido.sin_emitir.is_empty() {
        eprintln!(
            "E0075 {} cosa(s) se pidieron y no llegaron a un byte.",
            emitido.sin_emitir.len()
        );
        for m in &emitido.sin_emitir {
            eprintln!("  - {}", m);
        }
        eprintln!(
            "   Un binario al que le falta algo no hace lo que dice su fuente, asi que"
        );
        eprintln!("   no se escribe. Antes esto era un aviso y el `.ibex` salia igual.");
        eprintln!("no se ha escrito nada.");
        exit(1);
    }

    if informe {
        pinta_informe(&parte, &emitido, eventos.len());
    }

    if solo_mirar {
        println!("ok: {} compila", nombre);
        return;
    }

    // -- LO QUE EL BINARIO VA A DECIR DE SI MISMO --------------------------
    //
    // ** Hasta el 2026-08-22 el `.bex` salia con UNA seccion, `Code`, y el
    // perfil moria en la consola. Un `.bex` de INTI llegaba al kernel
    // indistinguible de cualquier otra cosa: para saber si podia correr en Ring
    // 0 habia que tener el fuente delante.
    //
    // Sale de `arbol` y de `revisado`, que son los que YA se calcularon arriba.
    // Calcularlo por otro camino seria describir un modulo distinto del que se
    // acaba de emitir.
    let manifiesto = bmo_inti_front::manifiesto::de(&arbol.valor, &revisado.valor, &nombre);
    let manifiesto = manifiesto.a_toml();

    // -- EL GATE, y va antes de escribir -----------------------------------
    //
    // `empaquetar` llama a `bmo-verify`. Verificar despues dejaria un fichero
    // malo en el disco con un mensaje al lado, y el que lo encuentre manana vera
    // el `.bex` y no el mensaje. Un gate que avisa cuando el dano ya esta hecho
    // es un informe, no un gate.
    let bytes = match bmo_inti_x86_64::empaquetar(&emitido, Some(&manifiesto)) {
        Ok(b) => b,
        Err(e) => fin(&format!("el `.bex` no pasa el gate: {}", e)),
    };

    // -- ** `.ibex`, Y NO ES UNA ETIQUETA -------------------------------------
    //
    // Es el NOMBRE DE UN VEREDICTO. Este fichero llega al disco solo si paso las
    // dos exigencias de `empaquetar`: declara lo que es (`Manifest 0x09`) y su
    // mesa de katanas cuadra con sus bytes (`Katanas 0x16`). Si alguna falla, no
    // se escribe nada -- asi que un `.ibex` en un disco **ya paso el contrato**,
    // y eso se puede afirmar sin abrir ninguna herramienta.
    //
    // ** Por eso INTI escribe SIEMPRE `.ibex` y nunca `.bex`. Dos nombres para
    // lo mismo obligarian a preguntar cual es cual; uno solo no deja sitio a la
    // duda. `.bex` se queda para los demas lenguajes, que no firman este
    // contrato -- y no por ser peores, sino porque no emiten reglas y no tienen
    // nada que declarar aqui.
    //
    // Y se llama `.ibex` y no `.i` a proposito: **el linaje se ve en el
    // nombre**. Es un BEX, se carga con el mismo cargador, lo lee el mismo gate.
    // Lo unico que anade es a que se ha comprometido.
    let destino = salida.unwrap_or_else(|| Path::new(&ruta).with_extension("ibex"));
    match std::fs::write(&destino, &bytes) {
        Ok(_) => println!(
            "ok: {} bytes -> {}{}",
            bytes.len(),
            destino.display(),
            if emitido.arranca {
                ""
            } else {
                "  (biblioteca: no arranca sola)"
            }
        ),
        Err(e) => fin(&format!("no puedo escribir {}: {}", destino.display(), e)),
    }
}

/// El parte de CABINA, en la consola.
///
/// ** Los numeros primero y los fallos despues, en ese orden a proposito: quien
/// lea el informe ve **contra que** ocurrio todo antes de verlo. Es el mismo
/// orden que `cabina::eventos` usa para el sistema, y por el mismo motivo.
fn pinta_informe(
    parte: &bmo_inti_front::cabina::Parte,
    e: &bmo_inti_x86_64::Emitido,
    eventos: usize,
) {
    println!("-- informe de {} --", parte.fichero);
    println!("  perfil                  {}", parte.perfil);
    println!("  funciones               {}", parte.funciones);
    if parte.arquitecturas.is_empty() {
        println!("  se ata a               nada: este fuente se porta");
    } else {
        println!("  se ata a               {}", parte.arquitecturas.join(", "));
    }
    println!("  bloques crudo           {}", parte.bloques_crudo);
    println!("  instrucciones de maquina {}", parte.instrucciones);
    println!();
    // ** Los dos numeros de comprobaciones son DISTINTOS a proposito: uno es lo
    // que la IR pidio y otro lo que llego al binario. El dia que haya
    // eliminacion de comprobaciones, la resta es exactamente lo que el
    // optimizador quito -- y se podra leer sin creerselo.
    println!("  reglas pedidas          {}", parte.comprobaciones);
    println!("  reglas emitidas         {}", e.comprobaciones);
    println!();
    println!("  temporales en registro  {}", e.en_registros);
    println!("  temporales en pila      {}", e.en_pila);
    println!("  eventos a CABINA        {}", eventos);
    println!();
}

fn ayuda(programa: &str) {
    println!("INTI -- el lenguaje de BMO-X, para x86-64.");
    println!();
    println!("  {} <fichero.inti> [opciones]", programa);
    println!();
    println!("  -o, --salida <ruta>   donde dejar el `.bex` (por defecto, al lado)");
    println!("  -i, --informe         los numeros que el compilador sabe");
    println!("  -c, --comprueba       compila y no escribe nada");
    println!("  -p, --puedo           lo que se hacer hoy, y lo que no y por que");
    println!("  -h, --ayuda           esto");
    println!();
    println!("Este programa NO ejecuta nada: escribe un `.bex` firmado, y quien");
    println!("lo ejecuta es el kernel. La puerta del sistema entra por `usa bmo`.");
}

/// **LO QUE SE HACER HOY -- y lo que no, con el motivo.**
///
/// ## Por que existe, y es una peticion de Eddi
///
/// > *"si algo no procesa, tiene que exponer lo que pueda hacer... INTI puede
/// > ayudar a traducir QUE FALLA y por que, para poder evitar problemas."*
///
/// ** INTI ya sabia decir lo que NO puede: `sin_emitir` cuenta lo que se pidio
/// emitir y no llego a un byte, y `E0073` distingue *"esta prohibido"* de
/// *"todavia no se hacerlo"*. Las dos son buenas y las dos son NEGATIVAS: solo
/// contestan cuando ya chocaste.
///
/// *** Lo que faltaba es la mitad positiva. Un compilador que solo sabe decir
/// que no se parece a una pared; uno que sabe decir lo que si sabe hacer es una
/// guia. Y el dato es exactamente el mismo, leido al reves.
///
/// ## Y sale ENTERO de las tablas
///
/// Ni una lista escrita aqui. Los perfiles salen de `biblioteca.toml`, los
/// nombres de la maquina de `arch/x86_64/inti.toml`, sus bytes de
/// `intrinsics.toml`, y las reglas de `Comprobacion::TODAS`. Una lista escrita
/// aqui seria una segunda lista, y dos listas que dicen lo mismo se separan.
fn puedo() {
    let raices = bmo_mods::Roots::find();
    println!("INTI -- lo que se hacer hoy. Nada de esto esta escrito aqui:");
    println!("todo sale de las mismas tablas con las que compilo.");
    println!();

    // -- LAS PIEZAS, que es lo que el gate mira de verdad ------------------
    //
    // ** Antes esto listaba PERFILES, y era la pregunta equivocada: un perfil es
    // una etiqueta. Lo que decide si un programa compila es que PIEZAS usa.
    let cat = bmo_inti_front::perfil::Catalogo::cargar(&raices);
    let bajan = cat.piezas_que_bajan();
    println!("PIEZAS (el gate mira lo que USAS, no tu perfil)");
    for nombre in ["texto", "lista", "tabla", "numero", "decimal"] {
        if bajan.iter().any(|x| x == nombre) {
            println!("  {:<8} SI -- un programa que la use compila", nombre);
        } else {
            println!("  {:<8} NO todavia (E0073), y solo te para si LA USAS", nombre);
        }
    }
    println!();

    // -- Las reglas ---------------------------------------------------------
    //
    // ** El "no" trae su motivo. Un no sin motivo manda a buscar al codigo.
    println!("LAS REGLAS ANTI-UB, en bytes");
    for c in bmo_inti_front::ir::Comprobacion::TODAS {
        if c.llega_a_bytes() {
            println!("  {} {:<22} SI, y atrapa devolviendo su codigo", c.codigo(), c.nombre());
        } else {
            println!("  {} {:<22} NO -- {}", c.codigo(), c.nombre(), c.por_que_no());
        }
    }
    println!();

    // -- La maquina ---------------------------------------------------------
    //
    // ** Se RECORRE la tabla, no se cuenta a mano. Un nombre que la tabla trae y
    // el emisor no sabe emitir sale aqui por su nombre -- que es justo lo que
    // `sin_emitir` cuenta cuando ya has escrito el programa.
    let taller = bmo_inti_x86_64::Taller::nuevo();
    match (taller.maquina.as_ref(), taller.intrinsecos.as_ref()) {
        (Some(maquina), Some(intrinsecos)) => {
            let nombres = maquina.nombres_que_trae();
            let mudos: Vec<String> = nombres
                .iter()
                .filter(|n| {
                    maquina
                        .instruccion(n)
                        .and_then(|i| intrinsecos.get(i))
                        .is_none()
                })
                .cloned()
                .collect();
            println!("LA MAQUINA x86_64  (`usa x86_64`)");
            println!(
                "  {} nombres en la tabla, {} con bytes detras",
                nombres.len(),
                nombres.len() - mudos.len()
            );
            if mudos.is_empty() {
                println!("  ninguno mudo");
            } else {
                println!("  MUDOS -- la tabla los nombra y no hay bytes:");
                for m in &mudos {
                    println!("    {}", m);
                }
            }
        }
        _ => println!("LA MAQUINA x86_64  -- no encuentro sus tablas"),
    }
    println!();
    println!("Y lo que NO sabe hacer un `.inti` cualquiera te lo dice al compilar:");
    println!("`sin_emitir` nombra lo que se pidio y no llego a un byte, con su motivo.");
}

fn fin(que: &str) -> ! {
    eprintln!("error: {}", que);
    exit(2)
}
