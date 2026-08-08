//! `bex-link` -- el eslabon que faltaba entre Rust y BMO.
//!
//! Hasta ahora **no habia ninguna forma de convertir un crate de Rust en un
//! programa que BMO pudiera admitir**. Todos los `.bex` del sistema salian de
//! emisores de bytes x86 escritos a mano o de los compiladores propios de C y
//! COBOL. Por eso `Ultra_userspace/` llevaba meses lleno de stubs vacios: no
//! estaban sin escribir por descuido, es que no existia tuberia que los
//! convirtiera en nada ejecutable, y la API de Ring 3 acababa viviendo como
//! `Vec<u8>` dentro de herramientas de build.
//!
//! Esto lo cierra: ELF de `cargo` -> contenedor BEF.
//!
//! ```text
//!   cargo build --target x86_64-unknown-none   ->  ELF
//!   bex-link  elf  salida.bex                  ->  .bex
//! ```
//!
//! ## El contrato de direcciones, que es la parte delicada
//!
//! El kernel **decide donde va cada seccion**: las coloca secuencialmente
//! desde `USER_IMAGE_BASE`, respetando la alineacion que cada una declara y
//! avanzando por paginas enteras (ver `proc.rs`). No hay reubicacion: lo que
//! el enlazador escribio como direccion absoluta tiene que caer donde el
//! kernel va a mapearlo, o el programa salta al vacio.
//!
//! Por eso hay un guion de enlazado (`Ultra_userspace/userland/link.ld`) que
//! reproduce exactamente esa colocacion --cada seccion en su frontera de 4 KiB,
//! en este orden-- y por eso aqui **todas las secciones declaran alineacion
//! 4096**. Los dos lados calculan la misma direccion porque siguen la misma
//! regla, no porque coincidan por suerte.
//!
//! Si eso se rompe, se rompe en silencio: el programa carga, salta y muere en
//! Ring 3 con un `#UD` o un `#PF` a una direccion redonda. Al final del
//! proceso esta herramienta imprime el mapa que ha calculado, para que ese
//! numero se pueda comparar con el que diga el kernel.
//!
//! ## Lo que NO hace
//!
//! No reubica, no enlaza dinamicamente, no resuelve imports y no firma. El
//! `.bex` sale sin firma; quien la quiera la pone despues con las herramientas
//! del gate. Un enlazador que hace una cosa y la hace entera.

use std::path::PathBuf;

use bmo_abi::bef::writer::{BefBuilder, BefSection};

/// Lo que hace falta de un ELF64. Nada mas que esto.
struct Elf {
    entry: u64,
    secciones: Vec<SeccionElf>,
}

struct SeccionElf {
    nombre: String,
    tipo: u32,
    addr: u64,
    offset: u64,
    size: u64,
}

/// `SHT_NOBITS`: ocupa memoria pero no bytes en el archivo. Es `.bss`.
const SHT_NOBITS: u32 = 8;

fn u16le(b: &[u8], o: usize) -> u16 {
    u16::from_le_bytes([b[o], b[o + 1]])
}
fn u32le(b: &[u8], o: usize) -> u32 {
    u32::from_le_bytes([b[o], b[o + 1], b[o + 2], b[o + 3]])
}
fn u64le(b: &[u8], o: usize) -> u64 {
    let mut v = [0u8; 8];
    v.copy_from_slice(&b[o..o + 8]);
    u64::from_le_bytes(v)
}

fn leer_elf(b: &[u8]) -> Result<Elf, String> {
    if b.len() < 64 || &b[0..4] != b"\x7fELF" {
        return Err("no es un ELF".into());
    }
    if b[4] != 2 {
        return Err("no es ELF64".into());
    }
    if b[5] != 1 {
        return Err("no es little-endian".into());
    }
    let entry = u64le(b, 24);
    let shoff = u64le(b, 40) as usize;
    let shentsize = u16le(b, 58) as usize;
    let shnum = u16le(b, 60) as usize;
    let shstrndx = u16le(b, 62) as usize;
    if shoff == 0 || shnum == 0 {
        return Err("el ELF no trae tabla de secciones".into());
    }

    // La tabla de nombres, para poder buscar por `.text` en vez de por indice.
    let sh = |i: usize| shoff + i * shentsize;
    let strtab_off = u64le(b, sh(shstrndx) + 24) as usize;
    let strtab_size = u64le(b, sh(shstrndx) + 32) as usize;
    let strtab = &b[strtab_off..strtab_off + strtab_size];
    let nombre_en = |off: usize| -> String {
        let fin = strtab[off..].iter().position(|&c| c == 0).unwrap_or(0) + off;
        String::from_utf8_lossy(&strtab[off..fin]).into_owned()
    };

    let mut secciones = Vec::new();
    for i in 0..shnum {
        let base = sh(i);
        secciones.push(SeccionElf {
            nombre: nombre_en(u32le(b, base) as usize),
            tipo: u32le(b, base + 4),
            addr: u64le(b, base + 16),
            offset: u64le(b, base + 24),
            size: u64le(b, base + 32),
        });
    }
    Ok(Elf { entry, secciones })
}

/// Las cuatro secciones que BMO mapea, EN EL ORDEN en que el kernel las va a
/// colocar. El orden es parte del contrato con `link.ld`, no una preferencia.
const ORDEN: [&str; 4] = [".text", ".rodata", ".data", ".bss"];

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() != 2 {
        eprintln!("uso: bex-link <entrada.elf> <salida.bex>");
        std::process::exit(2);
    }
    let entrada = PathBuf::from(&args[0]);
    let salida = PathBuf::from(&args[1]);

    let bytes = std::fs::read(&entrada).unwrap_or_else(|e| {
        eprintln!("bex-link: no puedo leer {}: {e}", entrada.display());
        std::process::exit(1);
    });
    let elf = leer_elf(&bytes).unwrap_or_else(|e| {
        eprintln!("bex-link: {e}");
        std::process::exit(1);
    });

    println!("== bex-link ==");
    println!("  entrada  {}", entrada.display());

    let mut b = BefBuilder::new();
    let mut base_text = None;
    // La direccion que el kernel ira calculando, para poder comprobar que
    // coincide con la que el enlazador escribio. Empieza en USER_IMAGE_BASE.
    let mut va_kernel: u64 = 0x0000_0000_4000_0000;
    let mut desajuste = false;

    for nombre in ORDEN {
        let Some(s) = elf.secciones.iter().find(|s| s.nombre == nombre) else {
            continue;
        };
        if s.size == 0 {
            continue;
        }
        // El kernel alinea el cursor y luego avanza por paginas enteras.
        va_kernel = (va_kernel + 4095) & !4095;
        if s.addr != va_kernel {
            eprintln!(
                "  !! {nombre} enlazada en 0x{:X} pero el kernel la mapeara en 0x{:X}",
                s.addr, va_kernel
            );
            desajuste = true;
        }
        println!(
            "  {:<8} 0x{:08X}  {:>6} B",
            nombre, s.addr, s.size
        );

        let mut seccion = if s.tipo == SHT_NOBITS {
            // `.bss` no aporta bytes: el kernel pone la pagina a cero.
            BefSection::bss(s.size)
        } else {
            let datos = bytes[s.offset as usize..(s.offset + s.size) as usize].to_vec();
            match nombre {
                ".text" => {
                    base_text = Some(s.addr);
                    BefSection::code(datos)
                }
                ".rodata" => BefSection::rodata(datos),
                _ => BefSection::data(datos),
            }
        };
        // TODAS a 4096: es lo que hace predecible la colocacion del kernel.
        seccion.alignment = 4096;
        b.add_section(seccion);

        va_kernel += (s.size + 4095) & !4095;
    }

    let Some(base_text) = base_text else {
        eprintln!("bex-link: el ELF no tiene `.text` con contenido");
        std::process::exit(1);
    };
    if elf.entry < base_text {
        eprintln!(
            "bex-link: el punto de entrada (0x{:X}) cae fuera de `.text` (0x{:X})",
            elf.entry, base_text
        );
        std::process::exit(1);
    }
    // El kernel lo lee como desplazamiento DENTRO de la seccion de codigo.
    b.entry_offset = elf.entry - base_text;
    println!("  entrada en +0x{:X} de .text", b.entry_offset);

    if desajuste {
        eprintln!(
            "bex-link: el guion de enlazado no reproduce la colocacion del kernel.\n\
             Ese programa cargaria y saltaria al vacio. Revisa link.ld."
        );
        std::process::exit(1);
    }

    let salida_bytes = b.build().unwrap_or_else(|e| {
        eprintln!("bex-link: construyendo el BEF: {e}");
        std::process::exit(1);
    });
    std::fs::write(&salida, &salida_bytes).unwrap_or_else(|e| {
        eprintln!("bex-link: no puedo escribir {}: {e}", salida.display());
        std::process::exit(1);
    });
    println!("  ->  {} ({} B)", salida.display(), salida_bytes.len());
}
