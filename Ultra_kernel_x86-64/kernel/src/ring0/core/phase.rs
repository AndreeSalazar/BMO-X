//! Ring 0 boot phases - orchestrator for the kernel entry path.
//!
//! In Ultra_kernel_x86-64's minimal Ring 0 base we keep only what's necessary:
//! the splash animation, the framebuffer init, and a serial shell.
//! All GDT/IDT/CPU/MM/IRQ/SMP subsystems live in the faggin stages
//! (s2_gdt, s3_idt, s4_cpuid, s5_control, s9_paging) and are already
//! configured by the time the kernel runs.
//!
//! Phases:
//!   0. fb    - framebuffer init from BootContext
//!   1. ui    - splash animation (if FB available)
//!
//! After phases: serial shell takes over so the user has a way to
//! interact even without a display.

use boot_context::BootContext;
use super::splash;

use super::shell;
use super::dashboard::{dash_log, dashboard_log};
use super::desktop::{start_desktop, wait_for_demo_tasks, COMPOSITOR_PATH};
use super::shell::ui::row;

pub(super) fn s_log(msg: &str) {
    crate::ring0::dev::console::serial_write(msg);
    crate::ring0::dev::console::serial_write("\n");
    // Mirror to the on-screen log panel (if framebuffer present).
    dashboard_log(msg);
}

fn phase0_fb(ctx: &BootContext) {
    s_log("[phase0] === Framebuffer Init ===");
    let fmt = match ctx.fb_pixel_format {
        0 => crate::ring0::dev::framebuffer::PixelFormat::Bgr,
        1 => crate::ring0::dev::framebuffer::PixelFormat::Rgb,
        _ => crate::ring0::dev::framebuffer::PixelFormat::Unknown,
    };
    crate::ring0::dev::framebuffer::init_gop(
        ctx.fb_addr,
        ctx.fb_width,
        ctx.fb_height,
        ctx.fb_stride,
        fmt,
    );
    s_log("[phase0] done");
}

fn phase1_ui(_ctx: &BootContext) {
    s_log("[phase1] === UI (dashboard) ===");
    if crate::info::has_fb() {
        // Land on the persistent dashboard after the cinematic intro.
        splash::splash_dashboard_init();
    } else {
        s_log("[splash] no framebuffer, skipping");
    }
    s_log("[phase1] done");
}

// ---------------------------------------------------------------------------
// Serial shell (with optional framebuffer echo)
// ---------------------------------------------------------------------------

/// Public entry: called from `entry::kernel_main_real` after the
/// naked `_start` BSS zero.
pub fn main(ctx: &mut BootContext) {
    // Boot bisected cleanly on real hardware; the visual progress markers
    // are retired. `kbar!` is now a no-op so the call sites can stay as
    // documentation of the init order without painting over the UI. (Their
    // spirit lives on in the planned per-module status/version registry.)
    macro_rules! kbar { ($y:expr, $c:expr) => {{ let _ = ($y, $c); }}; }

    kbar!(90, 0xFF00_FF00u32); // green @90: past the magenta paint, before s_log
    s_log("[ring0] validating BootContext");
    // Guardar el `rsdp` -- solo un numero, no se lee ninguna tabla aqui. Lo
    // necesita el censo de la MADT, que lo pide mas tarde y desde sitios que no
    // tienen el `BootContext` delante (el manejador de syscall, por ejemplo).
    crate::ring0::plat::madt::recordar(ctx.rsdp);
    if !ctx.is_valid() {
        // Make an invalid BootContext VISIBLE (red @90) instead of a silent
        // halt -- otherwise a magic mismatch looks identical to a hang.
        kbar!(90, 0xFFFF_0000u32);
        s_log("[ring0] FATAL: BootContext magic mismatch");
        loop { unsafe { core::arch::asm!("hlt"); } }
    }
    kbar!(110, 0xFFFF_FFFFu32); // white @110: BootContext valid, before percpu

    crate::ring0::dev::console::serial_write("[ring0] BootContext OK, version=");
    crate::ring0::dev::console::serial_write_u64(ctx.version as u64, 10);
    crate::ring0::dev::console::serial_write("\n");
    // Primera entrada de la bitacora, ANTES de que exista framebuffer: CABINA
    // graba desde el minuto cero y lo muestra cuando haya pantalla. Si el
    // kernel muere entre aqui y el shell, el anillo ya tiene lo que paso.
    crate::ring0::cabina::info("ring0", "BootContext valido, kernel arrancando", ctx.version as u64);
    // ** THE RULER STARTS HERE, and not earlier: before this point there is no
    // validated context and the TSC frequency it carries is what every stamp
    // below converts with. See `core::boot_timeline`.
    crate::ring0::core::boot_timeline::start();

    // Kernel-init checkpoints live in the empty band starting at row 140
    // (well below the boot bars that end near row 120), so any new bar is
    // unmistakably kernel progress -- not a repeat of an s1/s2 color.
    // * ANTES que nada que pueda atrapar. Los stubs de trap guardan el estado
    // extendido con XSAVE en un area de tamano FIJO, y el tamano que este CPU
    // necesita solo lo sabe el. Si no cabe, hay que enterarse AHORA y no
    // cuando el primer tick del timer desborde una pila de tarea.
    crate::ring0::cpu_vendor::xsave::init();
    // Y justo detras, la MISMA pregunta sobre las INSTRUCCIONES: si BMO usa
    // algo que este silicio no declara, es un `#UD` esperando a la primera vez
    // que se ejecute esa linea, y eso hay que saberlo aqui y no entonces.
    //
    // Calla si todo cuadra --el censo entero se pide a mano con `ext`--, y por
    // eso no cuesta ni una linea del panel en un arranque sano.
    crate::ring0::core::shell::extensions::aviso_de_arranque();
    crate::ring0::task::percpu::init_bsp();
    kbar!(140, 0xFF00_FF00u32); // green: percpu OK
    crate::ring0::task::scheduler::init(ctx.tsc_freq);
    kbar!(152, 0xFFFF_FFFFu32); // white: scheduler OK
    crate::ring0::mm::phys::init(ctx);
    kbar!(164, 0xFFFF_0000u32); // red: phys::init OK
    crate::ring0::mm::vmm::init();
    kbar!(176, 0xFF00_FFFFu32); // aqua: vmm::init OK
    let (frames_total, frames_free) = crate::ring0::mm::phys::stats();
    crate::ring0::dev::console::serial_write("[ring0] mm ready: frames free=");
    crate::ring0::dev::console::serial_write_u64(frames_free, 10);
    crate::ring0::dev::console::serial_write("/");
    crate::ring0::dev::console::serial_write_u64(frames_total, 10);
    crate::ring0::dev::console::serial_write("\n");
    crate::ring0::cabina::info("mem", "physmap + asignador de frames listos", frames_free);
    crate::ring0::obj::channel::init(ctx);
    crate::ring0::svc::register_all();
    crate::ring0::syscall::init();
    // Arm the on-screen fault reporter before anything can enter Ring 3, so a
    // CPL3 crash paints its vector/RIP/CR2 instead of the silent serial-halt
    // the boot stage installs.
    crate::ring0::core::boot_timeline::mark("mm + scheduler + syscalls");
    crate::ring0::plat::faults::init(ctx);
    let timer_ready = crate::ring0::plat::timer::init(ctx);
    if timer_ready {
        s_log("[ring0] scheduler + BMO Channel + SYSCALL + LAPIC tick ready (BSP)");
        crate::ring0::cabina::info("ring0", "scheduler + canal + syscalls + LAPIC armados", 0);
    } else {
        s_log("[ring0] WARNING: LAPIC tick unavailable; scheduler remains cooperative");
        crate::ring0::cabina::warn("ring0", "sin tick LAPIC: scheduler solo cooperativo", 0);
    }
    kbar!(188, 0xFFFF_FF00u32); // yellow: channel/svc/syscall/timer OK

    // Populate the active BMO CPU profile (today: Ryzen 5 5600X).
    // Identity, SMT/CCX topology, cache hierarchy, TSC calibration and
    // errata/speculation mitigations all live behind the profile --
    // changing CPU or vendor is a profile swap, never a kernel edit.
    let cpu_profile = crate::ring0::cpu_vendor::active();
    (cpu_profile.init)();
    kbar!(200, 0xFFFF_8800u32); // orange: CPU profile + errata (MSR) OK
    crate::ring0::dev::console::serial_write("[cpu] profile: ");
    crate::ring0::dev::console::serial_write(cpu_profile.vendor);
    crate::ring0::dev::console::serial_write(" ");
    crate::ring0::dev::console::serial_write(cpu_profile.name);
    crate::ring0::dev::console::serial_write("\n");
    crate::ring0::cabina::info("cpu", cpu_profile.name, 0);

    // BEX is the only native executable contract admitted by this kernel.
    // The parser is allocation-free so it is safe before the process allocator.
    crate::ring0::task::bex::announce();
    kbar!(212, 0xFF00_FFFFu32); // aqua: bex announce OK, before proc::spawn_init

    // F2: if the boot chain reserved a Ring 3 payload, admit it as the
    // init process. With no payload this is a no-op and the boot flow is
    // exactly the Ring 0 shell as before.
    crate::ring0::task::proc::init(ctx);
    let ring3_tid = crate::ring0::task::proc::spawn_init(ctx);
    if let Some(tid) = ring3_tid {
        crate::ring0::dev::console::serial_write("[ring0] Ring 3 init task ready, tid=");
        crate::ring0::dev::console::serial_write_u64(tid as u64, 10);
        crate::ring0::dev::console::serial_write("\n");
        crate::ring0::cabina::info("ring3", "proceso init admitido", tid as u64);
    } else {
        crate::ring0::cabina::warn("ring3", crate::ring0::task::proc::init_status(), 0);
    }
    kbar!(224, 0xFFFF_FFFFu32); // white: proc init + Ring 3 spawn OK, before splash

    // CPU identity detection (CPUID leaf 0, 1, 0x80000002-04)
    let cpu = crate::ring0::cpu::detect_cpu();
    let cpu_line = match cpu.vendor {
        crate::ring0::cpu::CpuVendor::Amd => "AMD",
        crate::ring0::cpu::CpuVendor::Intel => "Intel",
        crate::ring0::cpu::CpuVendor::Unknown => "Unknown",
    };
    let brand = cpu.brand.as_str();
    // Use a stack buffer to build the log line, then emit to both
    // serial and the framebuffer dashboard.
    let mut line = [0u8; 96];
    let prefix = b"[cpu] ";
    let mid1   = b" | ";
    let mid2   = b" | cores=";
    let mut off = 0;
    for &b in prefix { line[off] = b; off += 1; }
    for &b in brand.as_bytes() { if off < line.len() { line[off] = b; off += 1; } }
    for &b in mid1 { if off < line.len() { line[off] = b; off += 1; } }
    for &b in cpu_line.as_bytes() { if off < line.len() { line[off] = b; off += 1; } }
    for &b in mid2 { if off < line.len() { line[off] = b; off += 1; } }
    if off < line.len() { line[off] = b'0' + (cpu.logical_cores as u8 / 10); off += 1; }
    if off < line.len() { line[off] = b'0' + (cpu.logical_cores as u8 % 10); off += 1; }
    if let Ok(s) = core::str::from_utf8(&line[..off]) {
        s_log(s);
    }

    // Populate FB globals from the context, then bring up the fb driver.
    crate::info::init_from(ctx);
    phase0_fb(ctx);

    // -- Intro cinematica (logo -> preparando -> RING 0 -> RING 3) ----------
    // Escenas centradas con fundido y transicion, al estilo de un arranque
    // moderno. Al terminar aterrizamos en el dashboard, donde el trabajo
    // REAL de cada etapa fluye como log (igual que Windows: la animacion
    // juega, luego apareces en el escritorio).
    // ** EL TRUCO DE SANTA MONICA, y lo pidio el dueno con ese nombre.
    //
    // God of War 2018 no tiene pantallas de carga: el trabajo se hace DEBAJO de
    // una camara que no corta. La carga no se elimina -- se tapa con algo que el
    // jugador queria ver igualmente.
    //
    // Aqui era al reves. El comentario de arriba lo decia con todas las letras:
    // *"la animacion juega, luego apareces en el escritorio"*, o sea el modelo de
    // Windows: **2.400 ms de animacion MAS lo que tarde el hardware**. Y el
    // precio ya estaba confesado en otro sitio -- `boot_timeline` tiene una fila
    // propia para el `GATO_MS` porque sin ella ese segundo y medio se achacaba a
    // la enumeracion del bus PCI.
    //
    // Ahora `intro_empieza` no espera a nada: arranca el reloj y pinta un
    // fotograma. Los `intro_paso(pct)` de mas abajo van repartidos por el
    // arranque de verdad, y `intro_cierra` toca el final cuando ya no hay
    // trabajo que tapar.
    //
    // ** Y el `pct` no es una barra: enciende la CIUDAD. Con lo cual la pantalla
    // de arranque deja de acompanar al arranque y **pasa a serlo**: un
    // subsistema que tarda deja su tramo a oscuras mas tiempo, y eso es
    // informacion, no decorado.
    if crate::info::has_fb() {
        splash::intro_empieza();
    } else {
        s_log("[splash] no framebuffer, skipping splash");
    }
    // ** THE LOGO GETS ITS OWN ROW, and it is the reason the table is worth
    // having at all.
    //
    // `boot_intro()` sits between the first mark and the second, so without
    // this line its `GATO_MS` --1.600 ms of holding the cat on screen, pure
    // boot time by its own admission-- would be reported as part of
    // `pci + cpu census`. The very first table printed would have said that
    // enumerating the PCI bus takes a second and a half, and somebody would
    // have gone looking for it in the PCI code.
    //
    // ** An instrument that mis-attributes is worse than no instrument: it does
    // not leave you ignorant, it sends you somewhere. And this one was about to
    // do it on its first run.
    crate::ring0::core::boot_timeline::mark("splash: logo hold (GATO_MS)");

    // ** AQUI YA NO SE ATERRIZA EN NADA, y esa era la causa del panel encima de
    // la ciudad (2026-08-15).
    //
    // Esta llamada decia "aterrizar en el dashboard persistente" y era cierta
    // **cuando la intro bloqueaba**: la animacion terminaba y despues se
    // aterrizaba. Desde el truco de Santa Monica la intro no espera -- corre
    // repartida entre los `intro_paso` de mas abajo -- asi que este punto dejo de
    // ser "despues de la intro" y paso a ser "en mitad de la intro".
    //
    // El resultado en el Ryzen: un rectangulo del panel comiendose la esquina
    // superior con el rotulo `KERNEL LOG` dentro y el gato cortado por la mitad.
    // Un cambio que nadie toco rompio una llamada que nadie toco.
    //
    // Se aterriza donde toca: `intro_cierra()` y acto seguido
    // `splash_dashboard_init()`, al final de esta funcion. Y por si vuelve a
    // colarse una llamada asi, el panel ahora se niega a pintarse mientras la
    // intro tenga la pantalla -- ver `splash_dashboard_init`.
    if !crate::info::has_fb() {
        // Sin framebuffer no hay intro que tapar y el panel nunca se pinta; se
        // dice una vez para que el arranque a ciegas no sea silencioso.
        phase1_ui(ctx);
    }

    // -- Acto I: RING 0 despierta el hardware (log real) -----------------
    // Los encabezados "==" se pintan en cyan (dash_line_color).
    dash_log("== RING 0 : despertando hardware ==");
    s_log("[ring0] cpu Zen 3 perfilado + GDT/IDT propias");
    {
        let (total, free) = crate::ring0::mm::phys::stats();
        let mut b = [0u8; 48];
        let mut o = 0;
        for &c in b"[ring0] mem ".iter() { if o < b.len() { b[o] = c; o += 1; } }
        let gib = (total * 4096) >> 30;
        let mut tmp = [0u8; 4];
        let mut t = 0;
        let mut v = gib.max(1);
        while v > 0 && t < 4 { tmp[t] = b'0' + (v % 10) as u8; v /= 10; t += 1; }
        while t > 0 { t -= 1; if o < b.len() { b[o] = tmp[t]; o += 1; } }
        for &c in b" GiB physmap listos".iter() { if o < b.len() { b[o] = c; o += 1; } }
        let _ = free;
        if let Ok(s) = core::str::from_utf8(&b[..o]) { s_log(s); }
    }
    s_log("[ring0] scheduler preemptivo + capabilities armados");
    // CABINA abre los ojos y censa el almacenamiento (scan PCI). Va AQUI, en el
    // acto donde el kernel despierta hardware -- antes vivia dentro del render y
    // clavaba ~65k lecturas de config PCI en el primer frame del cockpit.
    crate::ring0::cabina::boot_probe();
    splash::intro_paso(25);
    // * Y se le pregunta al CPU si sabe medirse a si mismo. UNA vez: despues
    // `INFO_CPU_HZ_REAL` solo mira una bandera, porque lo va a pedir un panel
    // que se repinta y un `cpuid` por fotograma no es un panel, es un impuesto.
    // Ver `docs/maestro/AXION_MAESTRO.md`, seccion 9.
    crate::ring0::cpu::frequency::init();
    splash::intro_paso(40);
    // Y lo que GASTA. Mismo trato: se pregunta una vez si el chip sabe
    // contestar, y la unidad se le pregunta a el en vez de suponerla.
    crate::ring0::cpu::power::init();
    splash::intro_paso(45);
    // USB en su lugar narrativo: el kernel despierta teclado y mouse AQUI.
    crate::ring0::core::boot_timeline::mark("pci + cpu census");
    // ** Y AQUI DENTRO LA INTRO SIGUE CORRIENDO SOLA, sin que esta funcion se
    // entere: las esperas del USB son de reloj y pintan mientras giran. Ver
    // `intro_latido` y el `delay_ms` de `dev/usb`. Los `intro_paso` de abajo
    // reparten fotogramas por los tramos que NO esperan, que son los que se
    // quedaban mudos.
    crate::ring0::dev::usb::init(ctx);
    splash::intro_paso(50);
    // * And HERE the kernel keeps them. Until this commit the bus only advanced
    // when somebody asked for a key, so a Ring 3 program that took the input and
    // then hung left the machine with no keyboard, no mouse and no rescue
    // shortcut -- which rode on that same pumping. See the `PUMPING` header in
    // `dev/usb.rs`.
    //
    // Goes AFTER `usb::init` (it needs to know whether there are devices) and
    // after the scheduler is armed, which it already is a few lines above.
    // ** THE SUSPECT. USB enumeration has mandatory waits per spec (port
    // reset, recovery, control transfers), and there are two controllers.
    crate::ring0::core::boot_timeline::mark("usb enumeration");
    let _ = crate::ring0::dev::usb::start_bus_thread();
    // Y el disco: el HBA SATA (no el NVMe -- ahi vive el sistema del dueno) y
    // su tabla de particiones. Ver dev/disk.rs.
    crate::ring0::dev::disk::init();
    splash::intro_paso(55);
    // * Y la tarjeta de red: **solo mirarla**. Encuentra la NIC, elige su BAR de
    // memoria y le pregunta su MAC y su enlace, sin escribirle un byte. Va aqui
    // --con el resto del hardware y antes del disco duro de verdad-- porque su
    // respuesta decide si el driver que viene se empieza sobre suelo firme o
    // sobre una suposicion. Ver `dev/red.rs`.
    crate::ring0::core::boot_timeline::mark("disk + ahci");
    crate::ring0::dev::net::init();

    // *** EL CENSO DE LA PLACA. Cero escrituras, y va DESPUES de la NIC por lo
    // mismo que ella: es una pregunta, no una configuracion.
    //
    // ** Hasta hoy a este firmware se le hacia UNA sola pregunta --cuantos
    // nucleos, la MADT-- y todo lo demas que la placa cuenta de si misma
    // estaba ahi y nadie lo miraba. Esto lo CUENTA; interpretarlo es otro paso.
    //
    // [!] Y la cifra que hay que mirar es la de tablas que NO pasan su suma: en
    // una placa sana es cero, y si no lo es lo que falla no es la placa -- es el
    // mapeo de esas direcciones fisicas.
    crate::ring0::plat::placa::confesar(ctx.rsdp);
    splash::intro_paso(58);
    // * El reloj de la placa, DESPUES de que el TSC este medido: la hora se
    // ancla a el, y anclarla a una frecuencia que todavia vale cero daria un
    // reloj parado. Cuesta ocho lecturas de puerto, una vez en la vida.
    crate::ring0::dev::clock::init();
    splash::intro_paso(60);
    crate::ring0::dev::disk::scan_partitions();
    splash::intro_paso(63);
    // Y el sistema de ficheros: de sectores a ARCHIVOS. Monta la particion de
    // arranque, que es donde vive el BOOTX64.EFI con el que arrancamos.
    crate::ring0::fsys::fs::mount();
    splash::intro_paso(66);
    // El gate: el disco tiene que decir QUIEN ES antes de que se le pueda
    // escribir. Va DESPUES de leer la GPT porque una de las pruebas es que la
    // tabla cuadre con los sectores que el propio disco declara.
    crate::ring0::dev::disk::verify_identity();
    splash::intro_paso(68);
    // Y si convencio, el volumen de datos se monta con escritor. La particion
    // de arranque sigue montada sin el, y asi se queda.
    splash::intro_paso(70);
    crate::ring0::fsys::fs::mount_data();
    // Y ESTRATOS, si alguna particion lleva uno. Solo lectura: el modulo no
    // sabe escribir, asi que montarlo no puede estropear nada.
    crate::ring0::fsys::estratos::mount();
    splash::intro_paso(90);
    crate::ring0::core::boot_timeline::mark("filesystems + identity gate");
    // ** Y AQUI SE CIERRA: el hardware ya esta, no queda trabajo que tapar.
    // Los ojos toman el control, todo se va a negro, y el panel del kernel
    // entra sobre una pantalla limpia. Es el unico tramo de la intro que
    // ESPERA, y se puede porque ya no esconde nada.
    splash::intro_paso(100);
    splash::intro_cierra();
    splash::splash_dashboard_init();
    dash_log("== RING 0 : hardware al mando ==");

    // -- Acto II: RING 3 -- el userspace nace -----------------------------
    dash_log("== RING 3 : userspace ==");
    // Surface the Ring 3 init outcome on the (now cleared) dashboard so the
    // demo's state is visible without serial. If a tid was admitted, the next
    // timer tick will enter CPL3 and its 'ring3>' lines should follow below.
    {
        let mut summary = [0u8; 64];
        let head = b"[ring3] ";
        let status = crate::ring0::task::proc::init_status();
        let mut off = 0;
        for &b in head { if off < summary.len() { summary[off] = b; off += 1; } }
        for &b in status.as_bytes() { if off < summary.len() { summary[off] = b; off += 1; } }
        if let Some(tid) = ring3_tid {
            for &b in b" tid=" { if off < summary.len() { summary[off] = b; off += 1; } }
            if off < summary.len() { summary[off] = b'0' + (tid as u8 % 10); off += 1; }
        }
        if let Ok(s) = core::str::from_utf8(&summary[..off]) { s_log(s); }
    }

    // Y el escritorio, que ya NO viaja dentro del kernel. Va aqui y no en
    // `spawn_init` por una razon dura: `spawn_init` corre en el Acto I, cuando
    // el HBA SATA ni se ha tocado. Este es el primer punto del arranque en el
    // que existe un volumen de datos del que leerlo.
    //
    // * Y se ANUNCIA. El paso de Ring 0 a Ring 3 era invisible: el kernel
    // dejaba de pintar y o aparecia un escritorio o no aparecia nada, sin
    // forma de saber cual de los dos lados habia fallado. Decir que se cede y
    // a quien convierte ese silencio en un acto con testigos.
    // * PRIMERO que acaben los demos, LUEGO la entrega.
    //
    // Los demos de Ring 3 y el escritorio se admitian todos antes de encender
    // el timer, asi que arrancaban **a la vez** -- y `init_hello` reclama la
    // pantalla para demostrar que Ring 3 puede. Ganaba el, pintaba sus tres
    // lineas, terminaba, y al morir el kernel recuperaba la pantalla y
    // repintaba su panel... encima del escritorio que acababa de nacer.
    //
    // De ahi las dos cosas que se veian y nadie explicaba: el aviso de "el
    // dueno de la pantalla MURIO" en cada arranque (era el demo, no el
    // compositor) y el panel del kernel dibujado sobre la ventana.
    //
    // Los demos ya demostraron lo suyo. Ahora se les deja terminar antes de
    // entregar la pantalla, con tope: si uno se cuelga, el escritorio arranca
    // igual -- esperar para siempre a un programa de ejemplo seria cambiar un
    // arranque feo por uno que no llega.
    if timer_ready {
        crate::ring0::plat::timer::enable();
        wait_for_demo_tasks();
    }

    crate::ring0::core::boot_timeline::mark("ring 3 handover");
    // The breakdown, printed where the boot is already being told. It is the
    // column of COSTS that answers "what do I attack first?".
    crate::ring0::core::boot_timeline::report(dash_log);
    dash_log("== RING 3 : LA ENTREGA ==");
    row("se cede", |l| {
        l.txt("la PANTALLA, la ENTRADA y una CONSOLA -- y Ring 0 deja de pintar");
    });
    row("a", |l| { l.txt(COMPOSITOR_PATH); l.txt("   desde el volumen de datos, con su firma"); });
    start_desktop();

    // FINAL checkpoint: bright green at row 236 = kernel finished ALL of
    // phase::main and is entering the shell. If this shows, Ring 0 fully
    // booted on real hardware.
    kbar!(236, 0xFF00_FF00u32);

    super::shell::session::run_shell(ctx);
}
