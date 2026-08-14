//! **Preguntarle al silicio y contar lo que contesta.**
//!
//! Las once ordenes del shell de Ring 0 que hablan con el hardware: el CPU y lo
//! que gasta, la memoria, la red, el audio, el disco y los nucleos.
//!
//! # Por que estas once juntas y no otras
//!
//! Porque **son las que crecen**. En la sesion del 2026-08-12 aparecieron tres
//! sensores nuevos --frecuencia efectiva, vatios y el censo de audio-- y las tres
//! veces se toco este grupo y ninguno de los otros. Las ordenes de fichero
//! llevan semanas quietas.
//!
//! Separar lo que se mueve de lo que no es la mitad del valor de partir un
//! fichero; la otra mitad es que cada trozo diga a que pregunta contesta.
//!
//! # Lo unico que comparten con `phase.rs`
//!
//! `s_log`, que escribe una linea en el panel del arranque. Nada mas: cada
//! funcion trae sus propios ayudantes de formato dentro, que es como estaban
//! escritas y por lo que se pudieron sacar sin tocarles una linea.

use boot_context::BootContext;

// Los ayudantes de la REJILLA del shell se comparten con `phase.rs` en vez de
// duplicarse: son la misma rejilla --etiqueta a la izquierda, valor a la
// derecha-- y dos copias darian dos shells que se ven distintos sin que nadie
// lo hubiera decidido.
use super::super::dashboard::dashboard_log_color;
use super::super::phase::s_log;
use super::ui::{row, L, SH_TITLE, SH_VALUE};

/// `disk` -- que disco tiene BMO delante y que hay en el.
///
/// La tabla de particiones es como el kernel RECONOCE su disco: no se fia del
/// orden en que el PCI enumere ni de que el firmware repita el mismo orden dos
/// veces. El disco propio es el que lleva estas particiones y no otras.
/// **`net` -- la tarjeta de red, preguntada AHORA.**
///
/// No repite el barrido del PCI (65.000 lecturas de config): usa la direccion
/// que el arranque ya encontro y **vuelve al aparato** a por el enlace.
///
/// ** Y ahi esta la prueba, que se puede hacer con la mano: **desenchufa el
/// cable y escribe `net` otra vez**. Si el enlace se cae, la lectura llega al
/// silicio -- el BAR es el bueno, el mapeo esta vivo y `PHYstatus` es ese
/// registro y no otro. Si no cambia, se esta leyendo una copia o el sitio
/// equivocado, y eso hay que saberlo ANTES de montar un anillo de DMA encima.
///
/// La MAC sale con dos puntos, no como un numero: esta linea existe para
/// compararla a ojo con la que diga cualquier otro sistema.
/// **`audio`** -- le pregunta al aparato de audio como quiere las muestras.
///
/// Paso 0 de `docs/AUDIO_MAESTRO.md`. Es una ORDEN y no un paso del arranque
/// por lo mismo que `smp` y `net rx`: enumerar un puerto lo RESETEA, y aunque
/// aqui solo se tocan puertos que `bmo_uhid` no tomo, esa clase de operacion se
/// dispara a proposito y no por encender la maquina.
pub(crate) fn shell_audio() {
    // El envoltorio de CR3: tocar el xHC es MMIO que solo esta mapeado en el
    // PML4 del kernel, y esto puede venir desde un syscall. Misma razon que en
    // `dev::usb::pump_bus`.
    use crate::ring0::mm::vmm;
    let kpml4 = vmm::kernel_pml4();
    let previo = vmm::read_cr3();
    let cambiado = kpml4 != 0 && previo != kpml4;
    if cambiado { vmm::switch_to(kpml4); }
    let hubo = unsafe { crate::ring0::dev::usb::audio::censar() };
    if cambiado { vmm::switch_to(previo); }
    if hubo {
        s_log("[audio] aparato hallado -- los numeros estan en CABINA (F11)");
        s_log("[audio] compara con lo que dice Windows del mismo audifono");
    } else {
        s_log("[audio] ningun aparato de reproduccion en los puertos libres");
        s_log("[audio] si el audifono esta enchufado, F11 dice cuantos se miraron");
    }
}

pub(crate) fn shell_red(arg: &[u8]) {
    use crate::ring0::dev::net;
    const H: &[u8; 16] = b"0123456789ABCDEF";
    fn txt(b: &mut [u8; 80], o: &mut usize, t: &str) {
        for &c in t.as_bytes() { if *o < b.len() { b[*o] = c; *o += 1; } }
    }
    fn hex8(b: &mut [u8; 80], o: &mut usize, v: u8) {
        if *o < b.len() { b[*o] = H[(v >> 4) as usize]; *o += 1; }
        if *o < b.len() { b[*o] = H[(v & 0xF) as usize]; *o += 1; }
    }
    fn dec(b: &mut [u8; 80], o: &mut usize, mut v: u64) {
        let mut tmp = [0u8; 20];
        let mut i = 0;
        if v == 0 { tmp[0] = b'0'; i = 1; }
        while v > 0 { tmp[i] = b'0' + (v % 10) as u8; v /= 10; i += 1; }
        while i > 0 { i -= 1; if *o < b.len() { b[*o] = tmp[i]; *o += 1; } }
    }

    if !net::hay() {
        s_log("[red] no hay ninguna NIC Ethernet en el PCI");
        return;
    }
    let (ven, dev_id, bus, dev, func, bar) = net::donde();
    {
        let mut b = [0u8; 80];
        let mut o = 0usize;
        txt(&mut b, &mut o, "[red] ");
        hex8(&mut b, &mut o, (ven >> 8) as u8);
        hex8(&mut b, &mut o, ven as u8);
        txt(&mut b, &mut o, ":");
        hex8(&mut b, &mut o, (dev_id >> 8) as u8);
        hex8(&mut b, &mut o, dev_id as u8);
        txt(&mut b, &mut o, "  bus ");
        hex8(&mut b, &mut o, bus);
        txt(&mut b, &mut o, ":");
        hex8(&mut b, &mut o, dev);
        txt(&mut b, &mut o, ".");
        dec(&mut b, &mut o, func as u64);
        txt(&mut b, &mut o, "  BAR");
        dec(&mut b, &mut o, bar as u64);
        if let Ok(s) = core::str::from_utf8(&b[..o]) { s_log(s); }
    }
    // ** Al aparato, ahora. No la foto del arranque.
    let id = match net::releer() {
        Some(i) => i,
        None => {
            s_log("[red] la tarjeta esta, pero su vendor no se leer todavia");
            return;
        }
    };
    {
        let mut b = [0u8; 80];
        let mut o = 0usize;
        txt(&mut b, &mut o, "[red] MAC ");
        for i in 0..6 {
            if i > 0 { txt(&mut b, &mut o, ":"); }
            hex8(&mut b, &mut o, id.mac[i]);
        }
        if !id.creible() {
            // Ceros o unos no dicen "tarjeta rota": dicen que la lectura no
            // llego. Es el BAR, no la NIC.
            txt(&mut b, &mut o, "  <- NO es creible: el BAR no llega");
        }
        if let Ok(s) = core::str::from_utf8(&b[..o]) { s_log(s); }
    }
    {
        let mut b = [0u8; 80];
        let mut o = 0usize;
        txt(&mut b, &mut o, "[red] enlace ");
        if id.enlace_arriba() {
            txt(&mut b, &mut o, "ARRIBA  ");
            dec(&mut b, &mut o, id.megabits() as u64);
            txt(&mut b, &mut o, " Mbps  ");
            txt(&mut b, &mut o, if id.duplex_completo() { "full" } else { "half" });
        } else {
            txt(&mut b, &mut o, "ABAJO (sin cable, o el otro lado no contesta)");
        }
        txt(&mut b, &mut o, "  PHYstatus=0x");
        hex8(&mut b, &mut o, id.phy);
        if let Ok(s) = core::str::from_utf8(&b[..o]) { s_log(s); }
    }

    // ** `net rx` -- ARM THE RECEIVER. And `net` alone still touches nothing.
    //
    // Same shape as `smp`: the bare word CENSUSES and the argument ACTS. It is
    // deliberate. This is the first code that lets a device write into this
    // machine's memory on its own, so it must not be something you get by typing
    // the diagnostic command.
    if arg != b"rx" {
        s_log("[red] `net rx` arma el receptor y enseNa las tramas que lleguen");
        s_log("[red] no se transmite nada: ver docs/RED_MAESTRO.md, paso 1");
        return;
    }
    if !id.enlace_arriba() {
        // Not a failure of the ring, and worth separating: with no cable there
        // will be no frames no matter how correct everything below is.
        s_log("[red] el enlace esta ABAJO: enchufa el cable antes de armar nada");
        return;
    }
    if !net::rx_start() {
        s_log("[red] el receptor no se pudo armar -- CABINA dice por que");
        return;
    }
    let n = net::rx_poll();
    {
        let mut b = [0u8; 80];
        let mut o = 0usize;
        txt(&mut b, &mut o, "[red] receptor armado. tramas ahora ");
        dec(&mut b, &mut o, n as u64);
        txt(&mut b, &mut o, ", total ");
        dec(&mut b, &mut o, net::rx_tramas());
        if let Ok(s) = core::str::from_utf8(&b[..o]) { s_log(s); }
    }
    if n == 0 {
        // Zero on the first turn is the EXPECTED answer, not a failure: the ring
        // was armed a microsecond ago and broadcast traffic arrives every few
        // seconds. Saying so is what stops the next minute being spent looking
        // for a bug in a driver that is working.
        s_log("[red] cero de momento es normal: vuelve a escribir `net rx` en unos segundos");
    }
}

pub(crate) fn shell_disk() {
    use crate::ring0::dev::disk;
    if !disk::is_ready() {
        s_log("[disk] sin disco SATA listo (mira la bitacora de CABINA)");
        return;
    }
    fn txt(b: &mut [u8; 80], o: &mut usize, t: &str) {
        for &c in t.as_bytes() { if *o < b.len() { b[*o] = c; *o += 1; } }
    }
    fn dec(b: &mut [u8; 80], o: &mut usize, mut v: u64, width: usize) {
        let mut tmp = [0u8; 20];
        let mut i = 0;
        if v == 0 { tmp[0] = b'0'; i = 1; }
        while v > 0 { tmp[i] = b'0' + (v % 10) as u8; v /= 10; i += 1; }
        for _ in i..width { if *o < b.len() { b[*o] = b' '; *o += 1; } }
        while i > 0 { i -= 1; if *o < b.len() { b[*o] = tmp[i]; *o += 1; } }
    }

    // Quien es el disco, segun el mismo. Con tres discos en la maquina y el
    // sistema del dueno en uno de ellos, esta linea es la que autoriza (o no)
    // a escribir algun dia.
    {
        let mut b = [0u8; 80];
        let mut o = 0usize;
        txt(&mut b, &mut o, "[disk] ");
        txt(&mut b, &mut o, disk::model());
        txt(&mut b, &mut o, "  ");
        dec(&mut b, &mut o, disk::total_sectors() >> 21, 1);
        txt(&mut b, &mut o, " GiB");
        if let Ok(s) = core::str::from_utf8(&b[..o]) { s_log(s); }
    }
    {
        let mut b = [0u8; 80];
        let mut o = 0usize;
        txt(&mut b, &mut o, "[disk] AHCI mmio=0x");
        {
            const H: &[u8; 16] = b"0123456789ABCDEF";
            for i in (0..8).rev() {
                if o < b.len() { b[o] = H[((disk::mmio() >> (i * 4)) & 0xF) as usize]; o += 1; }
            }
        }
        txt(&mut b, &mut o, " puerto=");
        dec(&mut b, &mut o, disk::port() as u64, 1);
        txt(&mut b, &mut o, "  sectores=");
        dec(&mut b, &mut o, disk::last_lba(), 1);
        if let Ok(s) = core::str::from_utf8(&b[..o]) { s_log(s); }
    }

    let parts = disk::partitions();
    if parts.is_empty() {
        s_log("[disk] sin tabla de particiones legible");
        return;
    }
    s_log(" #   primer LBA      GiB  tipo      nombre");
    for p in parts {
        let mut b = [0u8; 80];
        let mut o = 0usize;
        txt(&mut b, &mut o, " ");
        dec(&mut b, &mut o, p.index as u64, 2);
        dec(&mut b, &mut o, p.first_lba, 12);
        // Sectores de 512 B -> GiB: >>21 es dividir entre 2 Mi sectores.
        dec(&mut b, &mut o, p.sectors() >> 21, 9);
        txt(&mut b, &mut o, "  ");
        let tipo = if p.is_esp() { "ESP/boot " }
                   else if p.is_basic_data() { "datos    " }
                   else { "otro     " };
        txt(&mut b, &mut o, tipo);
        txt(&mut b, &mut o, p.name_str());
        if let Ok(s) = core::str::from_utf8(&b[..o]) { s_log(s); }
    }
    // El veredicto del gate, en palabras. Es la linea que decide si este disco
    // se puede escribir, asi que se pinta siempre -- diga que si o que no.
    {
        let mut b = [0u8; 80];
        let mut o = 0usize;
        txt(&mut b, &mut o, "[disk] ");
        txt(&mut b, &mut o, disk::gate_reason());
        if let Ok(s) = core::str::from_utf8(&b[..o]) { s_log(s); }
    }
    if disk::write_armed() {
        let mut b = [0u8; 80];
        let mut o = 0usize;
        txt(&mut b, &mut o, "[disk] serie=");
        txt(&mut b, &mut o, disk::serial());
        if let Some(p) = disk::data_partition() {
            txt(&mut b, &mut o, "  ventana=particion ");
            dec(&mut b, &mut o, p.index as u64, 1);
        }
        if let Ok(s) = core::str::from_utf8(&b[..o]) { s_log(s); }
    }
}

/// `cpu` -- que estado extendido tiene este procesador y si el perfil acierta.
///
/// La pregunta que responde: **hay registros que el cambio de contexto esta
/// perdiendo hoy?** Todos los numeros salen de `CPUID` hoja 0xD; el perfil solo
/// sirve para avisar si el silicio no es el que esperaba.
pub(crate) fn shell_cpu() {
    use crate::ring0::cpu_vendor::xsave;
    let inf = xsave::informe();
    let p = crate::ring0::cpu_vendor::profile::active();

    dashboard_log_color("== CPU : estado extendido ==", SH_TITLE);
    row("perfil", |l| { l.txt(p.name); l.txt("  "); l.txt(p.microarch); });
    if !inf.xsave {
        row("xsave", |l| l.txt("el procesador no lo implementa"));
        return;
    }
    row("xsave", |l| {
        l.txt("si   guardado: ");
        if inf.xsavec { l.txt("XSAVEC "); }
        if inf.xsaveopt { l.txt("XSAVEOPT "); }
        if inf.xsaves { l.txt("XSAVES"); }
    });
    row("soporta", |l| { l.txt("0x"); l.hex(inf.soportado, 4); l.txt("   todo: "); l.dec(inf.area_maxima as u64); l.txt(" B"); });
    row("xcr0", |l| {
        if inf.osxsave {
            l.txt("0x"); l.hex(inf.xcr0, 4);
            // El area que IMPORTA: la de los componentes habilitados ahora, no
            // la maxima teorica del CPU.
            l.txt("   habilitado: "); l.dec(inf.area_actual as u64); l.txt(" B");
        } else {
            l.txt("CR4.OSXSAVE apagado -- el estado extendido no esta habilitado");
        }
    });

    // Cada componente con su tamano y su sitio, tal como los declara el CPU.
    for c in inf.comps() {
        let mut l = L::new();
        l.txt("   bit ");
        l.dec(c.bit as u64);
        l.col(12);
        l.txt(c.name());
        l.col(32);
        l.dec(c.tam as u64); l.txt(" B en +"); l.dec(c.offset as u64);
        dashboard_log_color(l.as_str(), SH_VALUE);
        crate::ring0::dev::console::serial_write(l.as_str());
        crate::ring0::dev::console::serial_write("\n");
    }

    // El veredicto contra el perfil, y el aviso que justifica todo esto.
    row("perfil dice", |l| { l.txt("0x"); l.hex(p.xsave_componentes, 4); l.txt("   area "); l.dec(p.xsave_area as u64); l.txt(" B"); });
    // XCR0 aparte: lo habilitado no es lo soportado, y este lo pone el firmware,
    // no el kernel -- es el unico de los tres que puede moverse por debajo.
    row("perfil xcr0", |l| { l.txt("0x"); l.hex(p.xsave_xcr0, 4); l.txt("   habilitado que se espera"); });
    // El MISMO veredicto que dio el arranque. Antes esta linea tenia su propia
    // comparacion y contestaba distinto a la misma pregunta.
    let coincide = xsave::coincide(&inf);
    row("veredicto", |l| l.txt(if coincide { "el silicio coincide con el perfil" }
                                else { "DIFIERE -- manda el silicio, el perfil esta desfasado" }));
    // Lo que hace el cambio de contexto HOY. Esta linea decia "AVX aun no es
    // seguro" mucho despues de que dejara de ser cierto: un informe que se
    // queda contando una etapa anterior es peor que no tener informe, porque
    // se le cree.
    row("contexto", |l| {
        l.txt("XSAVE  reserva "); l.dec(crate::ring0::plat::trap::XSAVE_AREA as u64);
        l.txt(" B  usa "); l.dec(inf.area_actual as u64); l.txt(" B");
    });
    if inf.hay_estado_sin_guardar() {
        row("preserva", |l| {
            l.txt("mas alla de x87/SSE: 0x");
            l.hex(inf.soportado & !0b11, 4);
            l.txt("  (AVX seguro en Ring 3)");
        });
    }
}

/// `info` -- el informe completo de la maquina.
///
/// Antes escribia TODO al puerto serie y al panel no llegaba nada: en una
/// maquina sin cable serie el comando parecia no hacer nada. Ahora cada linea
/// va a los dos sitios.
pub(crate) fn shell_info(ctx: &BootContext) {
    use crate::ring0::mm::phys;
    const PAGE: u64 = 4096;

    dashboard_log_color("== BMO-X : informe del sistema ==", SH_TITLE);

    // -- CPU --
    let p = crate::ring0::cpu_vendor::profile::active();
    row("cpu", |l| { l.txt(p.vendor); l.txt(" "); l.txt(p.name); });
    row("uarch", |l| { l.txt(p.microarch); l.txt("  familia "); l.txt(p.family_model); });
    row("tsc", |l| {
        // Hz -> GHz con dos decimales, sin flotantes.
        l.dec(ctx.tsc_freq / 1_000_000_000);
        l.txt(".");
        let frac = (ctx.tsc_freq % 1_000_000_000) / 10_000_000;
        if frac < 10 { l.txt("0"); }
        l.dec(frac);
        l.txt(" GHz");
    });

    // -- MEMORIA: lo que hay, lo que se esta comiendo y en que --
    //
    // `used` es lo que el asignador de marcos NO tiene disponible: la imagen
    // del kernel, su bitmap, las pilas, las tablas de paginas, los buffers de
    // DMA y las regiones que el firmware declaro inutilizables. No se desglosa
    // mas porque el asignador no lo sabe, y un desglose inventado seria peor
    // que ninguno.
    let (total_frames, free_frames) = phys::stats();
    let total_b = total_frames * PAGE;
    let free_b = free_frames * PAGE;
    let used_b = total_b.saturating_sub(free_b);

    dashboard_log_color("== memoria ==", SH_TITLE);
    row("total", |l| { l.size(total_b); l.txt("   "); l.dec(total_frames); l.txt(" marcos de 4 KiB"); });
    row("usada", |l| { l.size(used_b); l.txt("   "); l.pct(used_b, total_b); l.txt("   "); l.dec(total_frames - free_frames); l.txt(" marcos"); });
    row("libre", |l| { l.size(free_b); l.txt("   "); l.pct(free_b, total_b); l.txt("   "); l.dec(free_frames); l.txt(" marcos"); });

    // El tamano REAL del kernel en RAM: desde donde lo linkea el script hasta
    // el final de su .bss (que incluye la pila de 64 KiB). Es un dato medido,
    // no el tamano del archivo.
    extern "C" { static __bss_end: u8; }
    let kernel_end = unsafe { &__bss_end as *const u8 as u64 };
    row("kernel", |l| { l.size(kernel_end.saturating_sub(0x400000)); l.txt("   en 0x400000"); });

    if crate::info::has_fb() {
        let (fw, fh, fs) = unsafe { (crate::info::FB_WIDTH as u64, crate::info::FB_HEIGHT as u64, crate::info::FB_STRIDE as u64) };
        row("video", |l| { l.size(fs * fh * 4); l.txt("   "); l.dec(fw); l.txt("x"); l.dec(fh); l.txt("x32  fb 0x"); l.hex(unsafe { crate::info::FB_ADDR }, 8); });
    }

    // -- ALMACENAMIENTO --
    {
        use crate::ring0::dev::disk;
        dashboard_log_color("== almacenamiento ==", SH_TITLE);
        if disk::is_ready() {
            row("disco", |l| { l.txt(disk::model()); l.txt("  puerto "); l.dec(disk::port() as u64); });
            row("serie", |l| { l.txt(disk::serial()); });
            row("tamano", |l| { l.size(disk::total_sectors() * 512); l.txt("   "); l.dec(disk::total_sectors()); l.txt(" sectores"); });
            row("escrit.", |l| { l.txt(if disk::write_armed() { "ARMADA" } else { "cerrada" }); });
        } else {
            row("disco", |l| { l.txt("sin disco listo"); });
        }
        let fs = crate::ring0::fsys::fs::fs_name();
        row("arranque", |l| { l.txt(fs); l.txt("  LBA "); l.dec(crate::ring0::fsys::fs::mounted_lba()); l.txt("  solo lectura"); });
        if crate::ring0::fsys::fs::data_mounted() {
            row("datos", |l| { l.txt("LBA "); l.dec(crate::ring0::fsys::fs::data_lba()); l.txt("  LECTURA/ESCRITURA"); });
        } else {
            row("datos", |l| { l.txt("sin montar"); });
        }
    }

    // -- PROCESOS Y ARRANQUE --
    dashboard_log_color("== sistema ==", SH_TITLE);
    let (tasks, runnable) = crate::ring0::task::scheduler::counts();
    row("tareas", |l| { l.dec(tasks as u64); l.txt(" totales   "); l.dec(runnable as u64); l.txt(" ejecutables"); });
    row("ticks", |l| { l.txt("0x"); l.hex(crate::ring0::plat::timer::ticks(), 8); });
    row("boot", |l| { l.txt("BootContext v"); l.dec(ctx.version as u64); l.txt("   "); l.dec(ctx.memory_map_count as u64); l.txt(" entries de mapa"); });
    row("pml4", |l| { l.txt("0x"); l.hex(ctx.pml4, 8); l.txt("   rsdp 0x"); l.hex(ctx.rsdp, 8); });
}

pub(crate) fn shell_tasks() {
    let (total, runnable) = crate::ring0::task::scheduler::counts();
    crate::ring0::dev::console::serial_write("[tasks] total=");
    crate::ring0::dev::console::serial_write_u64(total as u64, 10);
    crate::ring0::dev::console::serial_write(" runnable=");
    crate::ring0::dev::console::serial_write_u64(runnable as u64, 10);
    crate::ring0::dev::console::serial_write(" current_tid=");
    crate::ring0::dev::console::serial_write_u64(
        crate::ring0::task::scheduler::current_tid() as u64,
        10,
    );
    crate::ring0::dev::console::serial_write(" ticks=");
    crate::ring0::dev::console::serial_write_u64(crate::ring0::plat::timer::ticks(), 10);
    crate::ring0::dev::console::serial_write("\n");
}

/// **Despierta los otros nucleos y cuenta cuantos contestan.**
///
/// [!] Es una orden a mano y no un paso del arranque **a proposito**. El
/// trampolin corre en modo real, antes de que exista nada, y no lo ha ejecutado
/// ningun CPU todavia. Si esta mal, lo que se cuelga es este comando y no la
/// maquina al encenderla; la salida es un reinicio a boton. Ver
/// `plat/smp.rs` y `docs/SMP_MAESTRO.md`.
/// **La tabla de nucleos: en que esta cada uno y POR QUE.**
///
/// Es la mitad de AXION que se puede tener hoy sin tocar el hardware -- ver
/// `docs/AXION_MAESTRO.md`, apartado 4. No manda: **mira**, y eso ya cambia
/// algo, porque hasta ahora la unica pregunta que el sistema sabia contestar
/// era *cuantos* estan en pie.
///
/// ** Y la ultima fila es la que el dueno olio: cuantos nucleos estan al 100%
/// **sin hacer nada**. Hoy son todos los obreros, porque el que espera gira en
/// vez de dormir. Ese numero es el que tiene que bajar a cero el dia que entre
/// `MWAIT`, y por eso se pinta ANTES de que exista: una mejora que no se puede
/// comparar con el numero de antes no se puede demostrar.
pub(crate) fn shell_smp_tabla() {
    use crate::ring0::plat::smp;
    let hilos = match (crate::ring0::cpu_vendor::profile::active().nucleos)() {
        Some(t) => t.hilos as u32,
        // Sin perfil se mira lo que contesto, que es lo unico que se sabe de
        // verdad. Inventar "seguro que son 8" seria pintar filas de nucleos que
        // a lo mejor no existen.
        None => smp::alive().0 + 1,
    };
    let tope = if hilos > 32 { 32 } else { hilos };
    // ** CORE o THREAD, en ingles y pegado al numero.
    //
    // `12 hilos` no dice si son doce nucleos o seis con SMT, y esa diferencia
    // **decide el reparto**: una faena de calculo denso quiere seis obreros, no
    // doce. Tenerlo en la misma fila que el estado evita cruzar dos pantallas
    // para contestar "cuantos de estos son de verdad".
    let mut cores = 0u32;
    let mut threads = 0u32;
    for id in 0..tope {
        let e = smp::estado_de(id);
        let t = smp::tipo_de(id);
        match t {
            "CORE" => cores += 1,
            "THREAD" => threads += 1,
            _ => {}
        }
        row("cpu", |l| {
            l.dec(id as u64);
            l.txt("  ");
            l.txt(t);
            // `THREAD` es dos letras mas largo que `CORE`: se rellena para que
            // la columna del estado quede recta y se lea en vertical.
            if t == "CORE" { l.txt("    "); } else if t == "?" { l.txt("       "); } else { l.txt("  "); }
            l.txt(e.nombre());
            l.txt("   ");
            l.txt(e.motivo());
        });
    }
    row("reparto", |l| {
        l.dec(cores as u64);
        l.txt(" CORE + ");
        l.dec(threads as u64);
        l.txt(" THREAD. Calculo denso: pide ");
        l.dec(cores as u64);
        l.txt(". Si ESPERA memoria: los ");
        l.dec((cores + threads) as u64);
    });
    let girando = smp::girando();
    row("coste", |l| {
        l.dec(girando as u64);
        l.txt(" nucleos GIRANDO en vacio (al 100%). Con MWAIT esto seria 0");
    });
}

/// **`smp prueba`** -- reparte una cuenta pura y mide la aceleracion real.
///
/// === Por que hacia falta cablearlo aqui ===
///
/// La prueba existe desde el 08-08 y **solo se podia pedir desde Ring 3**, o sea
/// desde la caja del escritorio. Y al shell de Ring 0 se llega justo cuando el
/// escritorio NO arranca: la unica orden capaz de decir si los doce nucleos
/// sirven de algo estaba detras de la cosa que a veces no enciende.
///
/// El 08-08 contesto `0.00x` --`repartir` se rindio esperando-- y de ahi
/// salieron los tres testigos. **Se pintan siempre, salga bien o mal**, porque
/// son ellos y no la aceleracion los que dicen DONDE se rompio:
///
/// | | |
/// |---|---|
/// | `ENTRARON` corto | el fallo esta ANTES: trampolin o pila |
/// | `VIERON` corto | la publicacion de las atomicas |
/// | `HECHOS` corto | la faena murio a medias |
///
/// ** Y el techo, dicho antes de que decepcione: 6 nucleos con 2 hilos cada uno
/// no dan 12x en calculo puro. Dos hermanos SMT comparten las unidades de
/// ejecucion, asi que **~6x ES el maximo** de esta maquina, no un fallo.
pub(crate) fn shell_smp_prueba() {
    use crate::ring0::plat::smp::{self, crew};
    let (alive, _) = smp::alive();
    if alive == 0 {
        row("   ", |l| l.txt("no hay obreros en pie: usa `smp` primero"));
        return;
    }
    if crew::parados() {
        row("   ", |l| l.txt("los obreros estan PARADOS y sin IPI no vuelven: reinicia"));
        return;
    }
    row("   ", |l| l.txt("repartiendo... (decimas de segundo)"));
    let (uno, todos, partes) = crew::prueba(alive);
    row("un nucleo", |l| { l.dec(uno); l.txt(" ticks"); });
    row("todos", |l| { l.dec(todos); l.txt(" ticks"); });
    if partes == 0 {
        // `prueba` devuelve 0 partes cuando alguien no llego, y entonces el
        // tiempo de "todos" mide una carrera INCOMPLETA -- seria el mas bonito
        // de los dos y no vale nada. No se pinta como aceleracion.
        row("resultado", |l| l.txt("INCOMPLETA: falto un obrero, el tiempo no vale"));
    } else if todos > 0 {
        let x100 = uno.saturating_mul(100) / todos;
        row("aceleracion", |l| {
            l.dec(x100 / 100);
            l.txt(",");
            let d = x100 % 100;
            if d < 10 { l.txt("0"); }
            l.dec(d);
            l.txt("x  con ");
            l.dec(partes as u64);
            l.txt(" partes");
        });
    }
    // ** LA MEDIDA SE DENUNCIA A SI MISMA.
    //
    // La faena es una cadena de dependencias: ni un CPU perfecto haria una
    // vuelta por ciclo. Menos ticks que vueltas no es "muy rapido", es
    // imposible -- y entonces el roto es el cronometro, no el reparto. Paso el
    // 08-11 con `37` ticks para 400 millones de vueltas.
    if !crew::medida_creible(uno) {
        row("[!]", |l| l.txt("esa medida es IMPOSIBLE: el cronometro miente, no el reparto"));
    }
    let (entraron, vieron, hechos) = crew::testigos();
    row("testigos", |l| {
        l.txt("ENTRARON ");
        l.dec(entraron as u64);
        l.txt("   VIERON ");
        l.dec(vieron as u64);
        l.txt("   TERMINARON ");
        l.dec(hechos as u64);
    });
    if entraron < alive {
        row("   ", |l| l.txt("faltan por ENTRAR: mira el trampolin o la pila del AP"));
    } else if vieron < entraron {
        row("   ", |l| l.txt("entraron y no VIERON: es la publicacion de las atomicas"));
    } else if hechos < vieron {
        row("   ", |l| l.txt("vieron y no TERMINARON: la faena murio a medias"));
    }
    // ** EL TECHO DEPENDE DEL TRABAJO, y esta faena es el caso mas favorable.
    //
    // Aqui decia *"~6x es el maximo aqui"* por lo de siempre --dos hilos SMT
    // comparten las unidades de ejecucion-- y **el metal contesto 11,44x el
    // 2026-08-11**. La razon no rompe la regla, la precisa: esta faena es una
    // CADENA DE DEPENDENCIAS, o sea que cada hilo pasa la mayor parte del
    // tiempo esperando su propia multiplicacion. Dos hilos asi se turnan sin
    // pisarse, que es exactamente para lo que sirve SMT.
    //
    // > **SMT no da el doble de calculo: da el doble de ESPERAS solapadas.**
    //
    // Por eso el numero de esta prueba es el techo y no la promesa: un trabajo
    // que sature las unidades --vectorizado, con buen IPC-- se quedara cerca de
    // los 6 nucleos fisicos. Decirlo aqui evita que un 11x en la pantalla se
    // lea como "el sistema va 11 veces mas rapido".
    row("techo", |l| {
        l.txt("esta faena ESPERA memoria: el mejor caso para SMT. Con calculo denso, ~6x")
    });
}

/// Las cuatro ordenes, con lo que hace cada una. Cuatro filas se leen; un
/// parrafo no.
pub(crate) fn shell_smp_ayuda() {
    row("smp", |l| l.txt("esto: el estado de cada nucleo, sin tocar nada"));
    row("smp despertar", |l| l.txt("llama a los demas nucleos y los pone a esperar faena"));
    row("smp test", |l| l.txt("reparte una cuenta entre todos y mide la aceleracion"));
    row("smp stop", |l| l.txt("los duerme. [!] sin IPI NO vuelven: hay que reiniciar"));
}

pub(crate) fn shell_smp(arg: &[u8]) {
    dashboard_log_color("== smp ==", SH_TITLE);

    // ** `smp` A SECAS YA NO DESPIERTA A NADIE (2026-08-11).
    //
    // Lo hacia, y era una contradiccion con lo que este mismo fichero dice dos
    // funciones mas arriba: *preguntar por el estado no puede tener como efecto
    // secundario cambiarlo*. Y aqui el efecto no es teorico -- despertar deja
    // **once nucleos girando al 100%**, que es justo el coste que la tabla de
    // abajo confiesa.
    //
    // > Escribir el nombre de algo para ver que es no puede encenderlo.
    //
    // Ahora la orden que enciende **se llama** `smp despertar`, que ademas es lo
    // que hay que teclear para acordarse de que enciende algo.
    if arg == b"prueba" || arg == b"test" {
        shell_smp_prueba();
        return;
    }
    if arg == b"tabla" || arg == b"estado" {
        shell_smp_tabla();
        return;
    }
    if arg == b"parar" || arg == b"stop" {
        crate::ring0::plat::smp::crew::parar();
        row("parar", |l| l.txt("obreros PARADOS. Sin IPI no vuelven: hace falta reiniciar"));
        return;
    }
    if arg.is_empty() {
        // El estado primero --que es lo que se venia a ver-- y las opciones
        // debajo, que es lo que hace falta para lo siguiente.
        shell_smp_tabla();
        shell_smp_ayuda();
        return;
    }
    if arg != b"despertar" {
        row("   ", |l| l.txt("no lo conozco:"));
        shell_smp_ayuda();
        return;
    }

    // Por el PERFIL, no por el nombre del fabricante: ver `profile.rs`.
    if let Some(t) = (crate::ring0::cpu_vendor::profile::active().nucleos)() {
        row("silicio", |l| {
            l.dec(t.nucleos as u64);
            l.txt(" nucleos   ");
            l.dec(t.hilos as u64);
            l.txt(" hilos   ");
            l.dec(t.ccx as u64);
            l.txt(" CCX");
        });
    }

    row("   ", |l| l.txt("despertando... (si se queda aqui, reinicia a boton)"));

    // * Una linea POR NUCLEO y antes de mandarle nada. Es lo unico de este
    // comando que puede colgarse, y asi el cuelgue deja dicho en cual fue.
    // El censo se pinta ANTES de llamar a nadie: si algo cuelga, ya se sabe a
    // quien se iba a llamar y con que lista.
    if let Some(c) = crate::ring0::plat::madt::censo() {
        row("firmware", |l| {
            l.dec(c.ids().len() as u64);
            l.txt(" nucleos en la MADT");
            if c.apagados() > 0 {
                l.txt("   (+");
                l.dec(c.apagados() as u64);
                l.txt(" apagados)");
            }
        });
    } else {
        row("firmware", |l| l.txt("sin MADT: se supondran los APIC IDs"));
    }

    // El shell de Ring 0 despierta a TODOS: aqui no hay linea que escribir un
    // argumento, y quien llega a este shell es porque el escritorio no arranco.
    let (alive, esperados) = crate::ring0::plat::smp::despertar(u32::MAX, |id| {
        row("  ->", |l| {
            l.txt("APIC ");
            l.dec(id as u64);
        });
    });
    let (_, mascara) = crate::ring0::plat::smp::alive();

    // El BSP cuenta: es un nucleo que esta corriendo esto mismo.
    row("en pie", |l| {
        l.dec(alive as u64 + 1);
        l.txt(" / ");
        l.dec(esperados as u64 + 1);
        l.txt(" hilos");
    });
    // * Cuales, y no solo cuantos: "faltan dos" no dice a cual mirar.
    row("mascara", |l| {
        l.txt("APIC IDs que contestaron: ");
        l.hex(mascara as u64, 8);
    });
    if alive < esperados {
        row("   ", |l| l.txt("los que faltan no arrancaron o el trampolin no llego"));
    }

    // Y ahora que hay a quien mirar, la tabla. Va DESPUES de despertar por lo
    // obvio: antes todas las filas dirian "nadie lo ha llamado todavia".
    shell_smp_tabla();
    shell_smp_ayuda();
}

pub(crate) fn shell_mem() {
    let (total, free) = crate::ring0::mm::phys::stats();
    const PAGE: u64 = 4096;
    let total_b = total * PAGE;
    let free_b = free * PAGE;
    let used_b = total_b.saturating_sub(free_b);

    // Antes esto pintaba en el panel la linea "[mem] stats printed on serial",
    // que es la definicion de un comando inutil: te dice que la informacion
    // existe en un sitio donde no estas mirando.
    dashboard_log_color("== memoria ==", SH_TITLE);
    row("total", |l| { l.size(total_b); l.txt("   "); l.dec(total); l.txt(" marcos"); });
    row("usada", |l| { l.size(used_b); l.txt("   "); l.pct(used_b, total_b); });
    row("libre", |l| { l.size(free_b); l.txt("   "); l.pct(free_b, total_b); });

    if crate::ring0::mm::vmm::self_test() {
        s_log("[mem] vmm selftest OK (alloc/map/translate/unmap/destroy)");
    } else {
        s_log("[mem] vmm selftest FAILED");
    }
}