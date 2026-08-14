//! **Las ordenes que tocan el DISCO.** `ls`, `estratos`, `run` y `bex`.
//!
//! # Por que estas cuatro van juntas, y por que van las SEGUNDAS
//!
//! Porque son **el unico grupo donde un fallo se lleva datos**. Las de
//! [`super::hardware`] leen registros: equivocarse ahi da un numero raro. Aqui
//! se abren ficheros, se admiten programas y se monta un volumen -- y un fallo
//! no da un numero raro, da un archivo perdido.
//!
//! Ese es el criterio del orden entero de esta carpeta: **de lo que solo mira a
//! lo que no se deshace**.
//!
//! # Lo que comparten de verdad
//!
//! Las cuatro terminan en la misma pregunta --*"que hay en el disco y es
//! valido?"*-- y las cuatro hablan con `fsys`. `run` y `bex` ademas comparten
//! el gate: uno lo pasa para EJECUTAR y el otro solo para MIRAR, y tenerlas al
//! lado hace visible que son la misma comprobacion con dos finales.

use super::super::dashboard::dashboard_log_color;
use super::super::phase::s_log;
use super::ui::{similar_command, row, L, SH_TITLE, SH_VALUE};

/// `ls` -- recorre `EFI\BOOT\BOOTX64.EFI` de la particion de arranque y lo lee.
///
/// Lo que esto demuestra: que el camino entero --AHCI, GPT, FAT32, directorios,
/// cadena de clusteres-- funciona de punta a punta contra un archivo real.
///
/// Lo que NO demuestra: que ese archivo sea el nuestro. La version anterior
/// remataba con "es un ejecutable UEFI: SOY YO" a partir de la firma `MZ`, que
/// la lleva CUALQUIER ejecutable de Windows. En este disco la particion de
/// arranque es la ESP de 0,6 GB que comparte con el sistema del dueno, asi que
/// bien puede ser su cargador. Se dice lo que se sabe.
pub(crate) fn shell_ls() {
    use crate::ring0::fsys::fs;
    if !fs::is_mounted() {
        s_log("[fs] no hay volumen montado (mira la bitacora de CABINA)");
        return;
    }

    dashboard_log_color("== volumen de arranque ==", SH_TITLE);
    row("formato", |l| { l.txt(fs::fs_name()); l.txt("  LBA "); l.dec(fs::mounted_lba()); l.txt("  solo lectura"); });

    // Los nombres van en 8.3 crudo: 8 de nombre + 3 de extension, con
    // espacios de relleno. Feo, pero es como FAT los guarda en disco.
    let efi = match fs::find_dir(b"EFI        ") {
        Some(c) => c,
        None => { s_log("[fs] no encuentro el directorio EFI"); return; }
    };
    let boot = match fs::find_dir_in(b"BOOT       ", efi) {
        Some(c) => c,
        None => { s_log("[fs] no encuentro EFI\\BOOT"); return; }
    };
    // * Y buscar el archivo DENTRO de `boot`, no en la raiz. El primer
    // intento encontro los dos directorios y luego pregunto por el archivo en
    // la raiz de todas formas: tenia el cluster correcto en la mano y lo tiro.
    let (cluster, size) = match fs::find_in(b"BOOTX64 EFI", boot) {
        Some(v) => v,
        None => { s_log("[fs] BOOTX64.EFI no esta en EFI\\BOOT"); return; }
    };
    row("archivo", |l| { l.txt("EFI\\BOOT\\BOOTX64.EFI"); });
    row("tamano", |l| { l.size(size as u64); l.txt("   "); l.dec(size as u64); l.txt(" B   cluster "); l.dec(cluster as u64); });

    // Leer los primeros bytes. Un archivo que se encuentra pero no se lee no
    // demuestra nada.
    let mut head = [0u8; 64];
    let n = fs::read(cluster, 64.min(size), &mut head);
    if n >= 2 && head[0] == b'M' && head[1] == b'Z' {
        row("firma", |l| { l.txt("MZ  ejecutable PE   "); l.dec(n as u64); l.txt(" bytes leidos"); });
        row("cadena", |l| { l.txt("AHCI -> GPT -> FAT32 -> directorios -> clusteres  OK"); });
        crate::ring0::cabina::info("fs", "cadena de lectura verificada contra un PE real", size as u64);
    } else {
        row("firma", |l| { l.txt("?? no es un PE: revisar la cadena de clusteres"); });
        crate::ring0::cabina::warn("fs", "el archivo se encontro pero no se leyo bien", n as u64);
    }
}

/// `estratos` -- el estado del volumen propio y su raiz.
///
/// Es la primera vez que BMO-X lee un sistema de ficheros **suyo**: FAT32 es
/// un formato prestado que habia que entender; ESTRATOS lo escribio el.
pub(crate) fn shell_estratos() {
    use crate::ring0::fsys::estratos as est;
    if !est::is_mounted() {
        s_log("[estratos] ninguna particion tiene un volumen ESTRATOS");
        s_log("[estratos] se formatea desde el anfitrion con estratos-fmt");
        return;
    }
    let sb = match est::superbloque() { Some(s) => s, None => return };

    dashboard_log_color("== ESTRATOS ==", SH_TITLE);
    row("particion", |l| { l.dec(est::particion() as u64); l.txt("   LBA "); l.dec(est::base_lba()); });
    row("generacion", |l| { l.dec(sb.generation); l.txt("   bloques "); l.dec(sb.total_blocks); });
    row("log", |l| { l.txt("cabeza en el bloque "); l.dec(sb.log_head); });

    // -- El espacio, que es lo que decide si se puede escribir --
    //
    // Un FS que no sobreescribe se llena AUNQUE nadie cree un archivo: cada
    // version se queda. Por eso esto no es un adorno del panel -- es la
    // condicion previa al paso 5 del diseno, y el aviso que impide que el
    // volumen se llene por sorpresa (section 9).
    if let Some(oc) = est::ocupacion() {
        row("espacio", |l| {
            l.size(oc.bytes_usados());
            l.txt(" usados de ");
            l.size(oc.bytes_usados() + oc.bytes_libres());
            l.txt("   (");
            l.dec(oc.por_ciento() as u64);
            l.txt("%)");
        });
        row("libre", |l| {
            l.size(oc.bytes_libres());
            l.txt("   ");
            l.txt(oc.nivel().name());
        });
        // Lo que de verdad contesta "cuando hara falta el recolector?": no un
        // porcentaje, sino cuantas VERSIONES mas caben. Con 414 GiB la
        // respuesta son millones, y por eso el GC es "algun dia".
        row("caben", |l| {
            l.dec(oc.caben_de(20 * 1024));
            l.txt(" objetos mas de 20 KiB (un .bex de C)");
        });
        if !oc.nivel().admite_escritura() {
            crate::ring0::cabina::fault(
                "estratos",
                "volumen al 95%: SOLO LECTURA hasta que se libere sitio",
                oc.por_ciento() as u64,
            );
        }
    }
    // El gate del diseno: si el volumen no nacio aqui, se dice EN ALTO. Hoy
    // solo se lee, pero el dia que se escriba esta linea es la que decide.
    row("identidad", |l| {
        l.txt(if est::identidad_ok() { "es de ESTE disco" } else { "NO nacio en este disco (clonado?)" });
    });

    if let Some(e) = est::estrato() {
        row("estrato", |l| { l.txt("\""); l.txt(e.motivo_str()); l.txt("\""); });
    }

    let (_, raiz) = match est::raiz() {
        Some(v) => v,
        None => { s_log("[estratos] el volumen no tiene raiz (recien formateado?)"); return; }
    };
    let (n, truncado) = match est::entries(&raiz) {
        Some(v) => v,
        None => { s_log("[estratos] no se pudo leer la raiz"); return; }
    };
    for i in 0..n {
        if let Some(e) = est::entrada(i) {
            let hijo = est::nodo(&e.nodo);
            let dir = matches!(hijo.map(|h| h.tipo), Some(bmo_estratos::Tipo::Directorio));
            let mut l = L::new();
            l.txt("  ");
            l.txt(e.nombre_str());
            if dir { l.txt("/"); }
            dashboard_log_color(l.as_str(), SH_VALUE);
            crate::ring0::dev::console::serial_write(l.as_str());
            crate::ring0::dev::console::serial_write("\n");
        }
    }
    if truncado {
        s_log("[estratos] ...la raiz tiene mas entries de las que caben en el listado");
    }
}

pub(crate) fn shell_run(arg: &[u8]) {
    use crate::ring0::fsys::estratos as est;
    use crate::ring0::task::launch;

    let path = match core::str::from_utf8(arg) {
        Ok(s) => s.trim(),
        Err(_) => { s_log("[run] la ruta tiene bytes que no son texto"); return; }
    };

    // Buscar el archivo, comprobar la firma y admitirlo ya NO se hace aqui: lo
    // hace `launch::ruta`, que es EL MISMO camino que usa la caja de Ring 3.
    // Tener dos versiones del gate de firma era tener dos versiones que se
    // separan en cuanto alguien toque una. Al shell le queda lo suyo, que es
    // contarlo en filas.
    let inf = launch::ruta(path);

    if inf.res == Err(launch::Fallo::RutaVacia) {
        s_log("[run] uso: run c/holac.bex   (o A:/c/holac.bex)");
        return;
    }
    if let Err(launch::Fallo::NoSeEncuentra(_)) = inf.res {
        // ** ANTES DE DECIR "NO ESTA": ES UNA ORDEN?
        //
        // `run net` es lo que sale solo cuando uno se acostumbra a que todo se
        // lanza con `run`. Y hasta hoy contestaba *"el archivo no esta: revisa
        // la ruta"* -- que manda a mirar el disco cuando lo que hay que hacer es
        // quitar una palabra.
        //
        // No se ejecuta la orden por el: adivinar lo que alguien quiso decir es
        // como se acaba lanzando otra cosa. Se dice **como se escribe**, que es
        // lo que hace falta una sola vez.
        if let Some(orden) = similar_command(path) {
            let mut l = L::new();
            l.txt("[run] `");
            l.txt(orden);
            l.txt("` no es un programa: es una orden. Escribe `");
            l.txt(orden);
            l.txt("` a secas");
            dashboard_log_color(l.as_str(), SH_TITLE);
            crate::ring0::dev::console::serial_write(l.as_str());
            crate::ring0::dev::console::serial_write("\n");
            return;
        }
    }
    if let Err(launch::Fallo::NoSeEncuentra(e)) = inf.res {
        // El motivo exacto: "no esta" y "no cabe en 8.3" mandan a hacer cosas
        // distintas, y un "no se pudo" no manda a ninguna.
        let mut l = L::new();
        l.txt("[run] ");
        l.txt(e);
        l.txt(": ");
        l.txt(path);
        dashboard_log_color(l.as_str(), SH_TITLE);
        crate::ring0::dev::console::serial_write(l.as_str());
        crate::ring0::dev::console::serial_write("\n");
        return;
    }

    dashboard_log_color("== run ==", SH_TITLE);
    row("archivo", |l| { l.txt(path); });

    // Origen, tamano y firma solo si se llego a LEER el archivo. Con
    // `SinHueco` u `Ocupado` no se abrio nada, y pintar entonces "FAT32 no
    // puede llevar firma" seria contestar una pregunta que no se hizo -- el
    // informe hablaria de un archivo que nadie miro.
    if inf.bytes > 0 {
        row("origen", |l| { l.txt(inf.origen); });
        row("leido", |l| { l.size(inf.bytes as u64); });
        match inf.firma {
            Some(est::Firma::Cuadra) => row("firma", |l| l.txt("cuadra con el contenido")),
            Some(est::Firma::NoCuadra) => row("firma", |l| l.txt("NO CUADRA: el archivo no es el que se guardo")),
            Some(est::Firma::Ausente) => row("firma", |l| l.txt("el nodo no lleva :firma")),
            // Honestidad sobre la asimetria: FAT32 no tiene atributos con
            // nombre, asi que un binario de ahi no PUEDE traer su firma
            // pegada. No es que no se compruebe por pereza: es que no hay
            // donde guardarla.
            None => row("firma", |l| l.txt("FAT32 no puede llevar firma (sin atributos)")),
        }
    }

    match inf.res {
        Ok(tid) => {
            row("admitido", |l| { l.txt("tid "); l.dec(tid as u64); l.txt("   corre en el siguiente tick"); });
        }
        Err(f @ (launch::Fallo::FirmaMala | launch::Fallo::SinFirma)) => {
            row("gate", |l| { l.txt("RECHAZADO -- "); l.txt(f.motivo()); });
        }
        Err(f) => {
            row("rechazado", |l| { l.txt(f.motivo()); });
        }
    }
}

/// `bex` -- la tabla de programas que este kernel ha ejecutado.
///
/// El log cuenta la historia segun pasa y se la lleva el desplazamiento; esto
/// es la FOTO, consultable en cualquier momento: que se admitio, de que
/// tamano, donde entra, con que pid, como acabo y cuanto llego a escribir.
pub(crate) fn shell_bex() {
    let progs = crate::ring0::task::proc::programs();
    if progs.is_empty() {
        s_log("[bex] ningun programa admitido todavia");
        return;
    }
    s_log("== programas BEX (BEF1 x86-64) ==");
    s_log(" tag     imagen  secc  entry       pid tid  estado     lineas");
    // Formateo con columnas de ancho fijo: quietas se leen de un vistazo.
    fn txt(b: &mut [u8; 80], o: &mut usize, t: &str) {
        for &c in t.as_bytes() { if *o < b.len() { b[*o] = c; *o += 1; } }
    }
    fn pad(b: &mut [u8; 80], o: &mut usize, t: &str, width: usize) {
        let n = t.len().min(width);
        txt(b, o, &t[..n]);
        for _ in n..width { if *o < b.len() { b[*o] = b' '; *o += 1; } }
    }
    fn dec(b: &mut [u8; 80], o: &mut usize, mut v: u64, width: usize) {
        let mut tmp = [0u8; 20];
        let mut i = 0;
        if v == 0 { tmp[0] = b'0'; i = 1; }
        while v > 0 { tmp[i] = b'0' + (v % 10) as u8; v /= 10; i += 1; }
        for _ in i..width { if *o < b.len() { b[*o] = b' '; *o += 1; } }
        while i > 0 { i -= 1; if *o < b.len() { b[*o] = tmp[i]; *o += 1; } }
    }
    fn hex(b: &mut [u8; 80], o: &mut usize, v: u64, digits: usize) {
        const H: &[u8; 16] = b"0123456789ABCDEF";
        for i in (0..digits).rev() {
            if *o < b.len() { b[*o] = H[((v >> (i * 4)) & 0xF) as usize]; *o += 1; }
        }
    }

    for p in progs {
        let mut b = [0u8; 80];
        let mut o = 0usize;
        txt(&mut b, &mut o, " ");
        pad(&mut b, &mut o, p.tag, 7);
        dec(&mut b, &mut o, p.image_bytes as u64, 6);
        txt(&mut b, &mut o, "B ");
        dec(&mut b, &mut o, p.sections as u64, 4);
        txt(&mut b, &mut o, "  0x");
        hex(&mut b, &mut o, p.entry_va, 8);
        dec(&mut b, &mut o, p.pid as u64, 4);
        dec(&mut b, &mut o, p.tid as u64, 4);
        txt(&mut b, &mut o, "  ");
        // El estado sale del scheduler AHORA, no de lo que anotamos al
        // admitirlo: la tabla dice la verdad del momento en que se mira.
        let estado = if !p.admitted { "RECHAZADO" } else {
            match crate::ring0::task::scheduler::tid_state(p.tid) {
                0x01 => "listo    ",
                0x02 => "corriendo",
                0x03 => "bloqueado",
                0x04 => "saliendo ",
                _    => "terminado",
            }
        };
        txt(&mut b, &mut o, estado);
        dec(&mut b, &mut o, crate::ring0::uconsole::lines_of(p.pid) as u64, 7);
        if let Ok(s) = core::str::from_utf8(&b[..o]) { s_log(s); }
    }
}