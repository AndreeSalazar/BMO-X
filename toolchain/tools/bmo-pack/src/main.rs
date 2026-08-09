//! **`bmo-pack`** -- mete los datos de una app DENTRO de su `.bex`.
//!
//! ```text
//!   bmo-pack app.bex -r doom1.wad=C:\...\doom1.wad -o doom.bex
//!   bmo-pack doom.bex --listar
//!   bmo-pack doom.bex --sacar doom1.wad -o copia.wad
//! ```
//!
//! ## Que es un paquete aqui
//!
//! Un `.bex` con una seccion [`SectionKind::Resources`] dentro: **un solo
//! fichero** con el codigo y los datos. BMO-X lo lee como lo que es; para
//! Windows es un fichero opaco, que es exactamente lo que se quiere de un
//! binario en el disco de datos.
//!
//! Y **sigue siendo un `.bex` que arranca**: el cargador del kernel mapea
//! Code/RoData/Data/Bss y **salta el resto contandolo**, asi que los recursos
//! no le cuestan ni una pagina al proceso.
//!
//! ## Lo que este programa NO decide
//!
//! Nada del formato. La disposicion del indice vive en
//! `bmo_abi::bef::recursos` y la reemision en `bmo_abi::bef::paquete`, las dos
//! con sus filas en el anfitrion. Aqui solo hay lectura de argumentos y de
//! ficheros -- si esta herramienta y el kernel discreparan alguna vez sobre
//! donde empieza un recurso, seria porque alguien escribio el formato dos
//! veces, y por eso no esta escrito aqui.

use std::fs;
use std::path::PathBuf;
use std::process::ExitCode;

use bmo_abi::bef::paquete;

fn uso() -> ExitCode {
    eprintln!(
        "uso:
  bmo-pack <entrada.bex> -r <nombre>=<fichero> [-r ...] -o <salida.bex>
  bmo-pack <paquete.bex> --listar
  bmo-pack <paquete.bex> --sacar <nombre> -o <fichero>
  bmo-pack <paquete.bex> --vaciar -o <salida.bex>

  -r nombre=ruta   anade un recurso. El NOMBRE es como lo pedira el programa.
  --listar         ensena que lleva dentro.
  --sacar          escribe un recurso a un fichero.
  --vaciar         quita los recursos y deja la imagen como salio del compilador."
    );
    ExitCode::from(2)
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() {
        return uso();
    }

    let entrada = PathBuf::from(&args[0]);
    let mut salida: Option<PathBuf> = None;
    let mut recursos: Vec<(String, PathBuf)> = Vec::new();
    let mut listar = false;
    let mut vaciar = false;
    let mut sacar: Option<String> = None;

    let mut i = 1usize;
    while i < args.len() {
        match args[i].as_str() {
            "-r" | "--recurso" => {
                let Some(par) = args.get(i + 1) else { return uso() };
                let Some((nombre, ruta)) = par.split_once('=') else {
                    eprintln!("[X] un recurso se escribe nombre=ruta, y esto no lo es: {par}");
                    return ExitCode::from(2);
                };
                recursos.push((nombre.to_string(), PathBuf::from(ruta)));
                i += 2;
            }
            "-o" | "--salida" => {
                let Some(v) = args.get(i + 1) else { return uso() };
                salida = Some(PathBuf::from(v));
                i += 2;
            }
            "--listar" => {
                listar = true;
                i += 1;
            }
            "--vaciar" => {
                vaciar = true;
                i += 1;
            }
            "--sacar" => {
                let Some(v) = args.get(i + 1) else { return uso() };
                sacar = Some(v.clone());
                i += 2;
            }
            otro => {
                eprintln!("[X] no conozco la opcion {otro}");
                return uso();
            }
        }
    }

    let bex = match fs::read(&entrada) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("[X] no se pudo leer {}: {e}", entrada.display());
            return ExitCode::FAILURE;
        }
    };

    if listar {
        return listado(&bex, &entrada);
    }
    if let Some(nombre) = sacar {
        return extraer(&bex, &nombre, salida.as_deref());
    }

    let Some(destino) = salida else {
        eprintln!("[X] falta -o: no voy a escribir encima de la entrada sin que me lo pidas");
        return uso();
    };

    // Los ficheros se leen ANTES de empaquetar nada, para que un fichero que
    // falta no deje una salida a medias.
    let mut datos: Vec<(String, Vec<u8>)> = Vec::new();
    if !vaciar {
        for (nombre, ruta) in &recursos {
            match fs::read(ruta) {
                Ok(v) => datos.push((nombre.clone(), v)),
                Err(e) => {
                    eprintln!("[X] no se pudo leer el recurso {}: {e}", ruta.display());
                    return ExitCode::FAILURE;
                }
            }
        }
    }
    let lista: Vec<(&str, &[u8])> = datos
        .iter()
        .map(|(n, d)| (n.as_str(), d.as_slice()))
        .collect();

    let salida_bytes = match paquete::empaquetar(&bex, &lista) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("[X] no se pudo empaquetar: {e}");
            return ExitCode::FAILURE;
        }
    };

    // * SE COMPRUEBA ANTES DE ESCRIBIR, y no despues.
    //
    // Volver a abrir lo que acabamos de producir y encontrar los recursos por
    // nombre cuesta microsegundos y cierra el unico camino por el que esta
    // herramienta podria entregar un paquete que el programa no sabe leer.
    if !lista.is_empty() {
        match paquete::directorio(&salida_bytes) {
            Some(d) => {
                for (n, _) in &lista {
                    if d.buscar(n).is_none() {
                        eprintln!("[X] el paquete salio sin el recurso '{n}'");
                        return ExitCode::FAILURE;
                    }
                }
            }
            None => {
                eprintln!("[X] el paquete salio sin directorio legible");
                return ExitCode::FAILURE;
            }
        }
    }

    if let Err(e) = fs::write(&destino, &salida_bytes) {
        eprintln!("[X] no se pudo escribir {}: {e}", destino.display());
        return ExitCode::FAILURE;
    }

    println!(
        "  {} -> {}  ({} B, {} recurso(s), +{} B)",
        entrada.display(),
        destino.display(),
        salida_bytes.len(),
        lista.len(),
        salida_bytes.len().saturating_sub(bex.len()),
    );
    for (n, d) in &lista {
        println!("    {:<24} {} B", n, d.len());
    }
    ExitCode::SUCCESS
}

fn listado(bex: &[u8], ruta: &std::path::Path) -> ExitCode {
    match paquete::directorio(bex) {
        None => {
            println!("  {} no lleva recursos (es un .bex normal)", ruta.display());
            ExitCode::SUCCESS
        }
        Some(d) => {
            let (off, size) = paquete::localizar_recursos(bex).unwrap_or((0, 0));
            println!(
                "  {}  --  {} recurso(s), seccion en +{} ({} B)",
                ruta.display(),
                d.len(),
                off,
                size
            );
            for i in 0..d.len() {
                let e = d.entrada(i).expect("validado al abrir");
                println!(
                    "    {:<24} {:>10} B   en +{}",
                    d.nombre(i).unwrap_or("<nombre ilegible>"),
                    e.size,
                    off + e.offset
                );
            }
            ExitCode::SUCCESS
        }
    }
}

fn extraer(bex: &[u8], nombre: &str, destino: Option<&std::path::Path>) -> ExitCode {
    let Some(d) = paquete::directorio(bex) else {
        eprintln!("[X] esta imagen no lleva recursos");
        return ExitCode::FAILURE;
    };
    let Some(i) = d.buscar(nombre) else {
        eprintln!("[X] no hay ningun recurso llamado '{nombre}'");
        return ExitCode::FAILURE;
    };
    let datos = d.datos(i).expect("validado al abrir");
    let Some(destino) = destino else {
        eprintln!("[X] falta -o: a que fichero lo saco?");
        return ExitCode::from(2);
    };
    if let Err(e) = fs::write(destino, datos) {
        eprintln!("[X] no se pudo escribir {}: {e}", destino.display());
        return ExitCode::FAILURE;
    }
    println!("  {} -> {} ({} B)", nombre, destino.display(), datos.len());
    ExitCode::SUCCESS
}
