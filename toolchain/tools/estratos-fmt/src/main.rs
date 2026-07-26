//! `estratos-fmt` — formatea un volumen ESTRATOS desde el anfitrión.
//!
//! Paso 4c del orden de construcción (`ESTRATOS.md` §10). El diseño lo pide
//! así a propósito: *"formatear desde el anfitrión con una herramienta del
//! toolchain, y que el kernel lo monte y lea. Sin riesgo: si el formato está
//! mal, se reformatea"*.
//!
//! # Uso
//!
//! ```text
//! estratos-fmt disco.img --tam-mib 64 --desde .\contenido
//! estratos-fmt disco.img --tam-mib 64 --desde .\contenido --modelo "KINGSTON ..." --serie "5002..." --sectores 937703088
//! estratos-fmt \\.\F: --volumen --si-estoy-seguro --modelo ... --serie ... --sectores ...
//! ```
//!
//! Por defecto escribe una **imagen en un archivo**, que no puede romper nada.
//! Tocar un volumen de verdad exige `--volumen` **y** `--si-estoy-seguro`, y
//! aun así imprime primero lo que va a destruir.
//!
//! # Se relee a sí mismo
//!
//! Al terminar, vuelve a abrir lo que acaba de escribir y recorre el árbol
//! entero comprobando cada suma. Un formateador que no se relee no ha
//! demostrado nada: solo ha escrito bytes con confianza.

use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

use bmo_estratos as es;
use bmo_estratos::objects::{
    Attr, BlockPtr, Entrada, Nodo, Tipo, ATTR_DATOS, ATTR_ENTRADAS, BLOQUE, ENTRADA_LEN,
    PTRS_POR_BLOQUE, PTR_LEN, RESIDENTE_MAX,
};

// ── El log: se escribe SIEMPRE hacia adelante ───────────────────────────────
//
// §5 del diseño. No hay "buscar un hueco": el log crece, y los objetos
// pequeños se empaquetan en el bloque en curso para no gastar 4096 bytes en un
// nodo de 560.

struct Log {
    f: File,
    /// Bloque que se está llenando.
    actual: u64,
    buf: [u8; BLOQUE],
    usado: usize,
    bloques_escritos: u64,
}

impl Log {
    fn nuevo(f: File, primer_bloque: u64) -> Self {
        Self { f, actual: primer_bloque, buf: [0u8; BLOQUE], usado: 0, bloques_escritos: 0 }
    }

    fn volcar(&mut self) -> std::io::Result<()> {
        if self.usado == 0 { return Ok(()); }
        self.f.seek(SeekFrom::Start(self.actual * BLOQUE as u64))?;
        self.f.write_all(&self.buf)?;
        self.buf = [0u8; BLOQUE];
        self.usado = 0;
        self.actual += 1;
        self.bloques_escritos += 1;
        Ok(())
    }

    /// Un objeto pequeño (nodo, estrato): comparte bloque con sus vecinos.
    fn objeto(&mut self, datos: &[u8]) -> std::io::Result<BlockPtr> {
        assert!(datos.len() <= BLOQUE);
        if self.usado + datos.len() > BLOQUE { self.volcar()?; }
        let off = self.usado;
        self.buf[off..off + datos.len()].copy_from_slice(datos);
        self.usado += datos.len();
        Ok(BlockPtr::nuevo(self.actual, off as u32, datos))
    }

    /// Un bloque de datos entero. No comparte: los datos de un archivo se leen
    /// por bloques completos y partirlos costaría una lectura extra por trozo.
    fn bloque(&mut self, datos: &[u8]) -> std::io::Result<BlockPtr> {
        assert!(datos.len() <= BLOQUE);
        self.volcar()?;
        let lba = self.actual;
        let mut b = [0u8; BLOQUE];
        b[..datos.len()].copy_from_slice(datos);
        self.f.seek(SeekFrom::Start(lba * BLOQUE as u64))?;
        self.f.write_all(&b)?;
        self.actual += 1;
        self.bloques_escritos += 1;
        Ok(BlockPtr::nuevo(lba, 0, datos))
    }

    /// Punta del log: el primer bloque libre.
    fn cabeza(&mut self) -> std::io::Result<u64> {
        self.volcar()?;
        Ok(self.actual)
    }
}

/// Escribe el contenido de un archivo y devuelve `(raiz, niveles)`.
///
/// Es la decisión 2 del modelo de objetos puesta a trabajar: los datos se
/// parten en bloques, y si no caben en un puntero se construye un nivel de
/// indirección encima. Se repite hasta que queda una sola raíz.
fn escribir_arbol(log: &mut Log, datos: &[u8]) -> std::io::Result<(BlockPtr, u8)> {
    if datos.len() <= BLOQUE {
        return Ok((log.bloque(datos)?, 0));
    }
    let mut nivel: Vec<BlockPtr> = Vec::new();
    for trozo in datos.chunks(BLOQUE) {
        nivel.push(log.bloque(trozo)?);
    }
    let mut niveles = 0u8;
    while nivel.len() > 1 {
        let mut arriba = Vec::new();
        for grupo in nivel.chunks(PTRS_POR_BLOQUE) {
            let mut b = vec![0u8; grupo.len() * PTR_LEN];
            for (i, p) in grupo.iter().enumerate() {
                b[i * PTR_LEN..(i + 1) * PTR_LEN].copy_from_slice(&p.encode());
            }
            arriba.push(log.bloque(&b)?);
        }
        nivel = arriba;
        niveles += 1;
    }
    Ok((nivel[0], niveles))
}

/// Escribe un archivo como nodo con su `:datos`.
fn escribir_archivo(log: &mut Log, datos: &[u8]) -> std::io::Result<BlockPtr> {
    let attr = if datos.len() <= RESIDENTE_MAX {
        // Lo pequeño no gasta bloque (decisión 3).
        Attr::residente(ATTR_DATOS, datos).expect("residente cabe")
    } else {
        let (raiz, niveles) = escribir_arbol(log, datos)?;
        Attr::en_bloques(ATTR_DATOS, datos.len() as u64, niveles, raiz).expect("niveles validos")
    };
    let nodo = Nodo::nuevo(Tipo::Archivo).con(attr).expect("un solo atributo");
    log.objeto(&nodo.encode())
}

/// Escribe un directorio con sus entradas ya resueltas.
fn escribir_directorio(log: &mut Log, entradas: &[(String, BlockPtr)]) -> std::io::Result<BlockPtr> {
    let mut cuerpo = Vec::with_capacity(entradas.len() * ENTRADA_LEN);
    for (nombre, ptr) in entradas {
        let e = Entrada::nueva(nombre, *ptr).expect("nombre valido");
        cuerpo.extend_from_slice(&e.encode());
    }
    let attr = if cuerpo.len() <= RESIDENTE_MAX {
        Attr::residente(ATTR_ENTRADAS, &cuerpo).expect("residente cabe")
    } else {
        let (raiz, niveles) = escribir_arbol(log, &cuerpo)?;
        Attr::en_bloques(ATTR_ENTRADAS, cuerpo.len() as u64, niveles, raiz).expect("niveles validos")
    };
    let nodo = Nodo::nuevo(Tipo::Directorio).con(attr).expect("un solo atributo");
    log.objeto(&nodo.encode())
}

/// Mete una carpeta del anfitrión en el volumen, recursivamente.
fn meter_carpeta(log: &mut Log, dir: &Path, sangria: usize) -> std::io::Result<BlockPtr> {
    let mut entradas: Vec<(String, BlockPtr)> = Vec::new();
    let mut hijos: Vec<_> = std::fs::read_dir(dir)?.filter_map(|e| e.ok()).collect();
    // Orden estable: dos formateos de la misma carpeta dan el mismo volumen, y
    // eso hace que los hashes se puedan comparar entre ejecuciones.
    hijos.sort_by_key(|e| e.file_name());
    for h in hijos {
        let nombre = h.file_name().to_string_lossy().to_string();
        let ruta = h.path();
        let ptr = if ruta.is_dir() {
            println!("{:sangria$}{}/", "", nombre, sangria = sangria);
            meter_carpeta(log, &ruta, sangria + 2)?
        } else {
            let mut datos = Vec::new();
            File::open(&ruta)?.read_to_end(&mut datos)?;
            println!("{:sangria$}{}  ({} B)", "", nombre, datos.len(), sangria = sangria);
            escribir_archivo(log, &datos)?
        };
        entradas.push((nombre, ptr));
    }
    escribir_directorio(log, &entradas)
}

// ── Lectura de vuelta ───────────────────────────────────────────────────────

struct Lector { f: File }

impl Lector {
    fn bloque(&mut self, lba: u64) -> std::io::Result<[u8; BLOQUE]> {
        let mut b = [0u8; BLOQUE];
        self.f.seek(SeekFrom::Start(lba * BLOQUE as u64))?;
        self.f.read_exact(&mut b)?;
        Ok(b)
    }

    /// Lee lo que un puntero promete, comprobándolo.
    fn seguir(&mut self, p: &BlockPtr) -> Result<Vec<u8>, String> {
        let b = self.bloque(p.lba).map_err(|e| format!("leyendo bloque {}: {}", p.lba, e))?;
        let ini = p.off as usize;
        let fin = ini + p.len as usize;
        let datos = b[ini..fin].to_vec();
        if !p.verifica(&datos) {
            return Err(format!("el bloque {} no cuadra con su suma", p.lba));
        }
        Ok(datos)
    }

    /// Reconstruye un flujo entero bajando por sus niveles.
    fn flujo(&mut self, a: &Attr) -> Result<Vec<u8>, String> {
        if let Some(d) = a.datos_residentes() { return Ok(d.to_vec()); }
        let raiz = a.raiz().ok_or("atributo sin raiz")?;
        let mut out = Vec::new();
        self.bajar(&raiz, a.levels, &mut out)?;
        out.truncate(a.size as usize);
        Ok(out)
    }

    fn bajar(&mut self, p: &BlockPtr, niveles: u8, out: &mut Vec<u8>) -> Result<(), String> {
        let datos = self.seguir(p)?;
        if niveles == 0 { out.extend_from_slice(&datos); return Ok(()); }
        for trozo in datos.chunks(PTR_LEN) {
            if trozo.len() < PTR_LEN { break; }
            let hijo = BlockPtr::decode(trozo).map_err(|e| e.name().to_string())?;
            self.bajar(&hijo, niveles - 1, out)?;
        }
        Ok(())
    }
}

/// Vuelve a abrir el volumen y lo recorre entero. Devuelve archivos y bytes.
fn verificar(ruta: &Path) -> Result<(usize, u64), String> {
    let f = File::open(ruta).map_err(|e| e.to_string())?;
    let mut l = Lector { f };

    let a = l.bloque(es::SUPER_A_BLOCK).map_err(|e| e.to_string())?;
    let b = l.bloque(es::SUPER_B_BLOCK).map_err(|e| e.to_string())?;
    let (sb, cual) = es::pick_superblock(&a, &b).map_err(|e| e.name().to_string())?;
    println!("  superbloque      copia {} generacion {}", cual, sb.generation);

    let e = es::Estrato::decode(&l.seguir(&sb.estrato)?).map_err(|e| e.name().to_string())?;
    println!("  estrato          \"{}\"  autor {:?}", e.motivo_str(), e.autor);

    let mut archivos = 0usize;
    let mut bytes = 0u64;
    recorrer(&mut l, &e.raiz, 2, &mut archivos, &mut bytes)?;
    Ok((archivos, bytes))
}

fn recorrer(l: &mut Lector, ptr: &BlockPtr, sangria: usize, archivos: &mut usize, bytes: &mut u64)
    -> Result<(), String>
{
    let nodo = Nodo::decode(&l.seguir(ptr)?).map_err(|e| e.name().to_string())?;
    if nodo.tipo == Tipo::Archivo {
        let a = nodo.attr(ATTR_DATOS).ok_or("archivo sin :datos")?;
        let d = l.flujo(a)?;
        if d.len() as u64 != a.size { return Err("el flujo no mide lo que dice".into()); }
        *archivos += 1;
        *bytes += a.size;
        return Ok(());
    }
    let a = nodo.attr(ATTR_ENTRADAS).ok_or("directorio sin :entradas")?;
    let cuerpo = l.flujo(a)?;
    for trozo in cuerpo.chunks(ENTRADA_LEN) {
        if trozo.len() < ENTRADA_LEN { break; }
        let ent = Entrada::decode(trozo).map_err(|e| e.name().to_string())?;
        let hijo = Nodo::decode(&l.seguir(&ent.nodo)?).map_err(|e| e.name().to_string())?;
        let marca = if hijo.tipo == Tipo::Directorio { "/" } else { "" };
        println!("{:sangria$}{}{}", "", ent.nombre_str(), marca, sangria = sangria);
        recorrer(l, &ent.nodo, sangria + 2, archivos, bytes)?;
    }
    Ok(())
}

// ── CLI ─────────────────────────────────────────────────────────────────────

struct Opciones {
    destino: PathBuf,
    tam_mib: u64,
    desde: Option<PathBuf>,
    volumen: bool,
    seguro: bool,
    solo_verificar: bool,
    modelo: String,
    serie: String,
    sectores: u64,
    motivo: String,
}

fn ayuda() -> ! {
    eprintln!("estratos-fmt — formatea un volumen ESTRATOS

  estratos-fmt <destino> [opciones]

  --tam-mib N        tamano del volumen en MiB (imagen). Por defecto 64
  --desde CARPETA    mete el contenido de esa carpeta en el volumen
  --motivo TEXTO     motivo del primer estrato. Por defecto \"formato inicial\"
  --verificar        NO escribe: solo lee el volumen y comprueba sus sumas

  Identidad del disco (la graba DENTRO del volumen; el kernel la compara al
  montar y si no cuadra monta en solo lectura):
  --modelo TEXTO     modelo del disco (IDENTIFY)
  --serie TEXTO      numero de serie
  --sectores N       sectores totales del DISPOSITIVO

  Peligroso:
  --volumen          el destino es un volumen real, no un archivo
  --si-estoy-seguro  confirma que se puede DESTRUIR lo que haya alli
");
    std::process::exit(2);
}

fn parsear() -> Opciones {
    let mut a = std::env::args().skip(1);
    let mut o = Opciones {
        destino: PathBuf::new(), tam_mib: 64, desde: None,
        volumen: false, seguro: false, solo_verificar: false,
        modelo: String::new(), serie: String::new(), sectores: 0,
        motivo: "formato inicial".into(),
    };
    while let Some(x) = a.next() {
        match x.as_str() {
            "--tam-mib" => o.tam_mib = a.next().unwrap_or_default().parse().unwrap_or(64),
            "--desde" => o.desde = a.next().map(PathBuf::from),
            "--motivo" => o.motivo = a.next().unwrap_or_default(),
            "--modelo" => o.modelo = a.next().unwrap_or_default(),
            "--serie" => o.serie = a.next().unwrap_or_default(),
            "--sectores" => o.sectores = a.next().unwrap_or_default().parse().unwrap_or(0),
            "--volumen" => o.volumen = true,
            "--si-estoy-seguro" => o.seguro = true,
            "--verificar" => o.solo_verificar = true,
            "-h" | "--help" => ayuda(),
            otro => {
                if otro.starts_with("--") { ayuda(); }
                o.destino = PathBuf::from(otro);
            }
        }
    }
    if o.destino.as_os_str().is_empty() { ayuda(); }
    o
}

fn main() {
    let o = parsear();

    // Solo mirar. Existe porque su ausencia estuvo a punto de costar caro: al
    // querer comprobar que un volumen corrupto se detectaba, la herramienta lo
    // REFORMATEO —no tenia otra forma de mirarlo— y borro justo la prueba. Una
    // herramienta que solo sabe escribir acaba escribiendo donde no debe.
    if o.solo_verificar {
        println!("== estratos-fmt --verificar ==");
        println!("  volumen          {}", o.destino.display());
        match verificar(&o.destino) {
            Ok((n, b)) => println!("  OK               {} archivos, {} bytes, todas las sumas cuadran", n, b),
            Err(e) => { eprintln!("  FALLO            {}", e); std::process::exit(1); }
        }
        return;
    }

    // La barrera. Escribir en un volumen real borra lo que hubiera, y esta
    // herramienta no lo hace por accidente ni por descuido de quien la llama.
    if o.volumen && !o.seguro {
        eprintln!("estratos-fmt: `--volumen` DESTRUYE todo lo que haya en {}.", o.destino.display());
        eprintln!("              Si es lo que quieres, anade --si-estoy-seguro.");
        std::process::exit(1);
    }

    let total_bloques = o.tam_mib * 1024 * 1024 / BLOQUE as u64;
    let disk_id = es::disk_id(o.modelo.as_bytes(), o.serie.as_bytes(), o.sectores);

    println!("== estratos-fmt ==");
    println!("  destino          {}", o.destino.display());
    println!("  modo             {}", if o.volumen { "VOLUMEN REAL" } else { "imagen en archivo" });
    if !o.volumen { println!("  tamano           {} MiB ({} bloques)", o.tam_mib, total_bloques); }
    if o.modelo.is_empty() || o.serie.is_empty() || o.sectores == 0 {
        println!("  identidad        SIN identidad de disco — el kernel lo montara en SOLO LECTURA");
    } else {
        println!("  identidad        {} / {} / {} sectores", o.modelo, o.serie, o.sectores);
    }

    let f = match OpenOptions::new().read(true).write(true).create(!o.volumen).open(&o.destino) {
        Ok(f) => f,
        Err(e) => { eprintln!("estratos-fmt: no se pudo abrir {}: {}", o.destino.display(), e); std::process::exit(1); }
    };
    if !o.volumen {
        if let Err(e) = f.set_len(total_bloques * BLOQUE as u64) {
            eprintln!("estratos-fmt: no se pudo dimensionar la imagen: {}", e);
            std::process::exit(1);
        }
    }

    // El log empieza en el bloque 2: los dos primeros son las copias del
    // superbloque y no se pisan jamas.
    let mut log = Log::nuevo(f, 2);

    println!("  contenido:");
    let raiz = match &o.desde {
        Some(dir) => meter_carpeta(&mut log, dir, 4).expect("escribiendo el arbol"),
        None => escribir_directorio(&mut log, &[]).expect("raiz vacia"),
    };

    let estrato = es::Estrato::new(raiz, BlockPtr::NULO, 0, es::Autor::Herramienta, &o.motivo);
    let ptr_estrato = log.objeto(&estrato.encode()).expect("escribiendo el estrato");
    let cabeza = log.cabeza().expect("cerrando el log");

    let mut sb = es::Superblock::new(disk_id, total_bloques);
    sb.estrato = ptr_estrato;
    sb.log_head = cabeza;

    // Las dos copias. En un volumen recien formateado ambas valen; el valor de
    // tener dos aparece en la PRIMERA escritura posterior, cuando una se queda
    // como el estado anterior mientras la otra se reemplaza.
    let bytes = sb.encode();
    let mut f = log.f;
    for lba in [es::SUPER_A_BLOCK, es::SUPER_B_BLOCK] {
        let mut b = [0u8; BLOQUE];
        b[..es::SUPER_LEN].copy_from_slice(&bytes);
        f.seek(SeekFrom::Start(lba * BLOQUE as u64)).expect("seek superbloque");
        f.write_all(&b).expect("escribiendo superbloque");
    }
    f.sync_all().expect("vaciando al disco");
    drop(f);

    println!("  escrito          {} bloques de log, cabeza en {}", log.bloques_escritos, cabeza);

    // Y ahora se relee. Un formateador que no se relee a si mismo no ha
    // demostrado nada: solo ha escrito bytes con confianza.
    println!("  verificando de vuelta:");
    match verificar(&o.destino) {
        Ok((n, b)) => println!("  OK               {} archivos, {} bytes, todas las sumas cuadran", n, b),
        Err(e) => { eprintln!("  FALLO            {}", e); std::process::exit(1); }
    }
}
