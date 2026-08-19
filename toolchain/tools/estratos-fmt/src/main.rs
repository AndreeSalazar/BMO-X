//! `estratos-fmt` -- formatea un volumen ESTRATOS desde el anfitrion.
//!
//! Paso 4c del orden de construccion (section 10 de `ESTRATOS.md`, que vive en
//! la raiz de la crate `bmo-estratos`). El diseno lo pide
//! asi a proposito: *"formatear desde el anfitrion con una herramienta del
//! toolchain, y que el kernel lo monte y lea. Sin riesgo: si el formato esta
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
//! aun asi imprime primero lo que va a destruir.
//!
//! # Se relee a si mismo
//!
//! Al terminar, vuelve a abrir lo que acaba de escribir y recorre el arbol
//! entero comprobando cada suma. Un formateador que no se relee no ha
//! demostrado nada: solo ha escrito bytes con confianza.

use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

use bmo_estratos as es;
use bmo_estratos::objects::{
    Attr, BlockPtr, Entrada, Nodo, Tipo, ATTR_DATOS, ATTR_ENTRADAS, ATTR_FIRMA, BLOQUE, ENTRADA_LEN,
    PTRS_POR_BLOQUE, PTR_LEN, RESIDENTE_MAX,
};

// -- El log: se escribe SIEMPRE hacia adelante -------------------------------
//
// section 5 del diseno. No hay "buscar un hueco": el log crece, y los objetos
// pequenos se empaquetan en el bloque en curso para no gastar 4096 bytes en un
// nodo de 560.

struct Log {
    f: File,
    /// Bloque que se esta llenando.
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

    /// Un objeto pequeno (nodo, estrato): comparte bloque con sus vecinos.
    fn objeto(&mut self, datos: &[u8]) -> std::io::Result<BlockPtr> {
        assert!(datos.len() <= BLOQUE);
        if self.usado + datos.len() > BLOQUE { self.volcar()?; }
        let off = self.usado;
        self.buf[off..off + datos.len()].copy_from_slice(datos);
        self.usado += datos.len();
        Ok(BlockPtr::nuevo(self.actual, off as u32, datos))
    }

    /// Un bloque de datos entero. No comparte: los datos de un archivo se leen
    /// por bloques completos y partirlos costaria una lectura extra por trozo.
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
/// Es la decision 2 del modelo de objetos puesta a trabajar: los datos se
/// parten en bloques, y si no caben en un puntero se construye un nivel de
/// indireccion encima. Se repite hasta que queda una sola raiz.
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

/// Escribe un archivo como nodo con su `:datos` y su `:firma`.
///
/// ## Que prueba la firma y que NO
///
/// `:firma` es el BLAKE3 del contenido. Con eso, quien abre el archivo puede
/// comprobar que **los bytes son los que se guardaron**: detecta corrupcion
/// del disco, una escritura a medias o un bloque que se leyo mal.
///
/// Lo que NO prueba es autenticidad. Quien pueda escribir en el volumen puede
/// cambiar el archivo *y* recalcular su hash: no hay clave por medio, asi que
/// no hay nada que un atacante no pueda rehacer. Para eso hace falta firmar el
/// hash con una clave que el kernel conozca y el atacante no -- el esqueleto
/// esta en `bmo-abi/src/bef/signing.rs` y es trabajo aparte.
///
/// Se dice aqui porque la diferencia importa: hoy el gate protege de un disco
/// que miente, no de alguien que quiere colar un binario.
fn escribir_archivo(log: &mut Log, datos: &[u8]) -> std::io::Result<BlockPtr> {
    let attr = if datos.len() <= RESIDENTE_MAX {
        // Lo pequeno no gasta bloque (decision 3).
        Attr::residente(ATTR_DATOS, datos).expect("residente cabe")
    } else {
        let (raiz, niveles) = escribir_arbol(log, datos)?;
        Attr::en_bloques(ATTR_DATOS, datos.len() as u64, niveles, raiz).expect("niveles validos")
    };
    // 32 bytes: residente, no gasta bloque. Va en el MISMO nodo que los datos,
    // que es la idea entera de los atributos con nombre -- la firma no puede
    // separarse del binario al copiarlo, como pasaria con un `.sig` suelto.
    let firma = bmo_hash::hash(datos);
    let nodo = Nodo::nuevo(Tipo::Archivo)
        .con(attr).expect("primer atributo")
        .con(Attr::residente(ATTR_FIRMA, &firma).expect("32 bytes caben")).expect("segundo atributo");
    log.objeto(&nodo.encode())
}

/// **El tope de una carpeta, dicho antes de escribir nada.**
///
/// Vive suelta porque se pregunta DOS veces y las dos hacen falta:
/// [`meter_carpeta`] la llama con los hijos contados --antes de meter un solo
/// fichero, para no dejar media imagen escrita-- y [`escribir_directorio`] la
/// vuelve a llamar por si algun dia hay otro camino hasta el. La primera es la
/// educada; la segunda es la que no se puede saltar.
fn cabe_la_carpeta(de_donde: &str, cuantas: usize) {
    if cuantas <= es::escritura::ENTRADAS_POR_BLOQUE {
        return;
    }
    eprintln!();
    eprintln!(
        "estratos-fmt: {} tiene {} entradas y el tope de hoy son {}.",
        de_donde, cuantas, es::escritura::ENTRADAS_POR_BLOQUE
    );
    eprintln!("              El volumen se escribiria bien y el kernel NO podria crear,");
    eprintln!("              borrar ni renombrar nada dentro de esa carpeta: republicarla");
    eprintln!("              pasa por leer sus entradas a un bloque de 4096 bytes.");
    eprintln!("              Reparte esa carpeta en subcarpetas y vuelve a formatear.");
    std::process::exit(1);
}

/// Escribe un directorio con sus entradas ya resueltas.
///
/// === ** POR QUE ESTO SE NIEGA A ESCRIBIR UNA CARPETA GRANDE ===
///
/// Aqui hay `Vec`, asi que una carpeta de mil entradas se escribiria sin
/// pestanear, con su arbol de indireccion y todo. **El kernel no la puede
/// modificar**: republicar una carpeta pasa por leer sus entradas a UN bloque
/// --36 entradas-- y buscar un nombre pasa por un buffer de 64.
///
/// O sea que sin este tope, el anfitrion puede parir un volumen que arranca,
/// que se lee y que **rechaza cualquier escritura dentro de esa carpeta** con un
/// motivo que no lleva a ninguna parte. El sitio de decir que no es este: el
/// unico momento en el que esa carpeta todavia no existe.
///
/// ** El tope es de la version del KERNEL, no del formato. `Attr::en_bloques`
/// admite cuatro niveles y este mismo fichero los escribe; el dia que
/// `:entradas` tenga indireccion al republicar, este numero sube con el.
fn escribir_directorio(
    log: &mut Log,
    de_donde: &str,
    entradas: &[(String, BlockPtr)],
) -> std::io::Result<BlockPtr> {
    cabe_la_carpeta(de_donde, entradas.len());
    let mut cuerpo = Vec::with_capacity(entradas.len() * ENTRADA_LEN);
    for (name, ptr) in entradas {
        let e = Entrada::nueva(name, *ptr).expect("nombre valido");
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

/// Mete una carpeta del anfitrion en el volumen, recursivamente.
fn meter_carpeta(log: &mut Log, dir: &Path, sangria: usize) -> std::io::Result<BlockPtr> {
    let mut entradas: Vec<(String, BlockPtr)> = Vec::new();
    let mut hijos: Vec<_> = std::fs::read_dir(dir)?.filter_map(|e| e.ok()).collect();
    // Orden estable: dos formateos de la misma carpeta dan el mismo volumen, y
    // eso hace que los hashes se puedan comparar entre ejecuciones.
    hijos.sort_by_key(|e| e.file_name());
    // ** ANTES de meter un solo fichero. Si la carpeta no va a caber, decirlo
    // despues de haber escrito sus cuarenta hijos deja media imagen hecha.
    cabe_la_carpeta(&dir.display().to_string(), hijos.len());
    for h in hijos {
        let name = h.file_name().to_string_lossy().to_string();
        let ruta = h.path();
        let ptr = if ruta.is_dir() {
            println!("{:sangria$}{}/", "", name, sangria = sangria);
            meter_carpeta(log, &ruta, sangria + 2)?
        } else {
            let mut datos = Vec::new();
            File::open(&ruta)?.read_to_end(&mut datos)?;
            println!("{:sangria$}{}  ({} B)", "", name, datos.len(), sangria = sangria);
            escribir_archivo(log, &datos)?
        };
        entradas.push((name, ptr));
    }
    escribir_directorio(log, &dir.display().to_string(), &entradas)
}

// -- Lectura de vuelta -------------------------------------------------------

struct Lector { f: File }

impl Lector {
    fn bloque(&mut self, lba: u64) -> std::io::Result<[u8; BLOQUE]> {
        let mut b = [0u8; BLOQUE];
        self.f.seek(SeekFrom::Start(lba * BLOQUE as u64))?;
        self.f.read_exact(&mut b)?;
        Ok(b)
    }

    /// Lee lo que un puntero promete, comprobandolo.
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

// -- Tomar el volumen (Windows) ----------------------------------------------
//
// Windows no deja escribir sectores crudos de un volumen que tiene montado:
// el driver de NTFS los considera suyos y la escritura se ignora o falla. Hay
// que pedirle el volumen formalmente -- bloquearlo y desmontarlo -- antes de
// tocarlo. Sin esto, el formateador parece funcionar y no escribe nada.

#[cfg(windows)]
mod win {
    use std::fs::File;
    use std::os::windows::io::AsRawHandle;

    type Handle = *mut core::ffi::c_void;

    extern "system" {
        fn DeviceIoControl(
            h: Handle, code: u32,
            entrada: *mut core::ffi::c_void, n_entrada: u32,
            salida: *mut core::ffi::c_void, n_salida: u32,
            devueltos: *mut u32, solapado: *mut core::ffi::c_void,
        ) -> i32;
    }

    const FSCTL_LOCK_VOLUME: u32 = 0x0009_0018;
    const FSCTL_UNLOCK_VOLUME: u32 = 0x0009_001C;
    const FSCTL_DISMOUNT_VOLUME: u32 = 0x0009_0020;

    fn ctl(f: &File, code: u32) -> bool {
        let mut devueltos = 0u32;
        unsafe {
            DeviceIoControl(
                f.as_raw_handle() as Handle, code,
                core::ptr::null_mut(), 0, core::ptr::null_mut(), 0,
                &mut devueltos, core::ptr::null_mut(),
            ) != 0
        }
    }

    /// Bloquea y desmonta el volumen. El bloqueo falla si alguien tiene
    /// archivos abiertos ahi, asi que se reintenta: normalmente es el
    /// indexador o el antivirus soltando el volumen.
    pub fn take(f: &File) -> Result<(), String> {
        for intento in 0..10 {
            if ctl(f, FSCTL_LOCK_VOLUME) {
                if ctl(f, FSCTL_DISMOUNT_VOLUME) { return Ok(()); }
                return Err("se bloqueo el volumen pero no se pudo desmontar".into());
            }
            std::thread::sleep(std::time::Duration::from_millis(300));
            let _ = intento;
        }
        Err("no se pudo bloquear el volumen: alguien lo tiene abierto \
             (cierra exploradores y terminales apuntando a esa unidad)".into())
    }

    pub fn release(f: &File) { let _ = ctl(f, FSCTL_UNLOCK_VOLUME); }
}

// -- CLI ---------------------------------------------------------------------

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
    // REFORMATEO --no tenia otra forma de mirarlo-- y borro justo la prueba. Una
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

    // * El seguro que de verdad importa. Esta herramienta escribe SIEMPRE
    // desde el offset 0 de lo que le den, y el offset 0 de un disco fisico es
    // su TABLA DE PARTICIONES. Apuntarla a `\\.\PhysicalDriveN` no formatearia
    // una particion: se llevaria por delante el mapa del disco entero y con el
    // todas las demas particiones -- incluida la de arranque. Un volumen
    // (`\\.\F:`) no tiene ese problema: su offset 0 ES el principio de su
    // particion, y no puede alcanzar a ninguna otra.
    let destino_txt = o.destino.to_string_lossy().to_ascii_lowercase();
    if destino_txt.contains("physicaldrive") {
        eprintln!("estratos-fmt: me niego a escribir sobre un disco FISICO.");
        eprintln!("              Escribo desde el offset 0, y el offset 0 de un disco es su");
        eprintln!("              tabla de particiones: se perderian TODAS, no solo esta.");
        eprintln!("              Apuntame a un volumen concreto, por ejemplo \\\\.\\F:");
        std::process::exit(1);
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
    } else {
        println!("  rango            bytes 0 .. {} del volumen", total_bloques * BLOQUE as u64);
        #[cfg(windows)]
        {
            print!("  desmontando...   ");
            use std::io::Write as _;
            let _ = std::io::stdout().flush();
            match win::take(&f) {
                Ok(()) => println!("volumen tomado"),
                Err(e) => { println!("FALLO"); eprintln!("estratos-fmt: {}", e); std::process::exit(1); }
            }
        }
    }

    // El log empieza en el bloque 2: los dos primeros son las copias del
    // superbloque y no se pisan jamas.
    let mut log = Log::nuevo(f, 2);

    println!("  contenido:");
    let raiz = match &o.desde {
        Some(dir) => meter_carpeta(&mut log, dir, 4).expect("escribiendo el arbol"),
        None => escribir_directorio(&mut log, "la raiz", &[]).expect("raiz vacia"),
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
    #[cfg(windows)]
    if o.volumen { win::release(&f); }
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
