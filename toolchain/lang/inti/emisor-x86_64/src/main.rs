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

    let arbol = bmo_inti_front::armar(&texto);
    let raices = bmo_mods::Roots::find();
    let modulos = bmo_inti_front::tablas::Modulos::cargar(&raices);
    let plano = bmo_inti_front::disposicion::comprobar(
        &arbol.valor,
        bmo_inti_front::disposicion::Medidas::cargar(&raices),
    );
    let tipos = bmo_inti_front::tipos::comprobar(&arbol.valor, &plano.valor);

    // -- Los avisos, con el formato de cuatro partes ------------------------
    //
    // Se pintan TODOS antes de decidir si se sigue: un compilador que para en
    // el primero obliga a compilar diez veces para ver diez errores.
    let mut hay_error = false;
    for (c, quien) in [
        (&arbol.avisos, "sintaxis"),
        (&plano.avisos, "disposicion"),
        (&tipos.avisos, "tipos"),
    ] {
        for a in c {
            let _ = quien;
            eprint!("{}", a.pintar(&nombre));
            if a.codigo.0.starts_with('E') {
                hay_error = true;
            }
        }
    }
    if hay_error {
        eprintln!("no se ha escrito nada.");
        exit(1);
    }

    // -- El descenso y los bytes -------------------------------------------
    let metal = bmo_inti_front::ir::metal_que_declara(&arbol.valor, &raices, &modulos);
    let ir = bmo_inti_front::ir::bajar_con(&arbol.valor, &modulos, &plano.valor, &metal).valor;
    let emitido = bmo_inti_x86_64::emitir(&ir);

    // ** LO QUE NO LLEGO A UN BYTE, y sale ANTES de escribir nada.
    //
    // Un intrinseco mudo no rompe la compilacion --el resto del programa esta
    // bien-- asi que sin esto la unica senal seria el binario haciendo otra cosa
    // en metal. Y para un fichero que va a un Ryzen, esa senal llega tarde.
    if !emitido.sin_emitir.is_empty() {
        eprintln!("aviso: {} cosa(s) no llegaron a un byte:", emitido.sin_emitir.len());
        for m in &emitido.sin_emitir {
            eprintln!("  - {}", m);
        }
    }

    if informe {
        pinta_informe(&parte, &emitido, eventos.len());
    }

    if solo_mirar {
        println!("ok: {} compila", nombre);
        return;
    }

    // -- EL GATE, y va antes de escribir -----------------------------------
    //
    // `empaquetar` llama a `bmo-verify`. Verificar despues dejaria un fichero
    // malo en el disco con un mensaje al lado, y el que lo encuentre manana vera
    // el `.bex` y no el mensaje. Un gate que avisa cuando el dano ya esta hecho
    // es un informe, no un gate.
    let bytes = match bmo_inti_x86_64::empaquetar(&emitido) {
        Ok(b) => b,
        Err(e) => fin(&format!("el `.bex` no pasa el gate: {}", e)),
    };

    let destino = salida.unwrap_or_else(|| Path::new(&ruta).with_extension("bex"));
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
    println!("  -h, --ayuda           esto");
    println!();
    println!("Este programa NO ejecuta nada: escribe un `.bex` firmado, y quien");
    println!("lo ejecuta es el kernel. La puerta del sistema entra por `usa bmo`.");
}

fn fin(que: &str) -> ! {
    eprintln!("error: {}", que);
    exit(2)
}
