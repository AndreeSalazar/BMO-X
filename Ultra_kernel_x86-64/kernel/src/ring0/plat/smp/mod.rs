//! **Despertar los otros nucleos.** El bring-up de SMP, del lado bueno de la
//! frontera.
//!
//! === Por que esto esta aqui y no en `s1_cpu` ===
//!
//! Habia un `smp_startup()` en `faggin/s1_cpu` que **nunca se llamo**, y al ir a
//! llamarlo se vio que estaba **del lado equivocado de `ExitBootServices`**:
//! antes de EBS el firmware sigue vivo y **los otros nucleos son suyos** (UEFI
//! los tiene en su *MP Services*), la memoria baja tampoco es nuestra, y lo
//! siguiente que hacia `s1_cpu` era volver a llamar al firmware. Encima hablaba
//! solo por `ser_print!`, y en esta maquina no hay cable serie.
//!
//! Aqui, en el kernel, no hay firmware: la maquina es entera de BMO, la memoria
//! baja **ya esta reservada** (`phys::init` reserva `<1 MiB` con el comentario
//! *"future SMP trampoline lives here"*) y CABINA pinta en pantalla.
//!
//! === Las cuatro piezas, y por que estan separadas ===
//!
//! | | |
//! |---|---|
//! | [`mapa`] | las direcciones y la GDT: **el dato** que los dos lados comparten |
//! | [`tramp`] | el trampolin de 16->64 bits y donde aterriza el AP |
//! | [`lapic`] | mandar IPIs y esperar -- solo lo usa el que despierta |
//! | este fichero | **la orquestacion, y nada mas** |
//!
//! No es orden por el orden: `mapa` existe porque esas direcciones estan
//! escritas **dos veces** --aqui en Rust y a mano dentro del trampolin, que no
//! puede llamar a nada--, y tenerlas en un fichero con nombre propio es lo que
//! hace que se vean juntas. Y `lapic` esta aparte porque es justo lo que el AP
//! **no** puede tocar.
//!
//! === PROBADO EN METAL: `12 de 12` el 2026-08-07 ===
//!
//! Y **sigue sin llamarse en el arranque**, que ya no es cautela sino la
//! decision: se pide con la orden `smp`. INIT+SIPI es la unica operacion del
//! sistema que no se deshace sin reiniciar, asi que si algo va mal se cuelga un
//! comando y no la maquina al encenderla.

pub mod lapic;
pub mod mapa;
/// El reparto de trabajo. Sin esto, los nucleos despiertos no sirven de nada.
pub mod obra;
pub mod tramp;

use core::sync::atomic::Ordering;
use mapa::*;

/// Despierta a los demas nucleos y cuenta cuantos contestan.
///
/// Devuelve `(alive, esperados)`, ambos **sin contar el BSP**.
///
/// * `aviso` se llama **antes de cada SIPI**, con el APIC ID al que le toca. No
/// es adorno: despertar a un AP cuesta hasta 10 ms de espera y esto es lo unico
/// que puede colgarse de todo el comando. Sin el aviso, un cuelgue se ve como
/// *"se quedo parado"*; con el se ve **en que nucleo** se quedo parado, que es
/// la diferencia entre tener un dato y tener que adivinar.
///
/// Va como parametro y no como una llamada a CABINA desde aqui por dos razones:
/// once lineas seguidas inundarian un anillo de 48 eventos, y **`plat/` no tiene
/// por que saber pintar**. Quien llama decide como se ensena.
///
/// * `cuantos` es el CONTROL, y es lo que el dueno pidio: **0 no despierta a
/// nadie** y solo contesta el censo, `u32::MAX` despierta a todos, y cualquier
/// otro numero despierta exactamente esos, en el orden en que los lista el
/// firmware.
///
/// Que el caso por defecto sea *"mira y no toques"* no es prudencia: mandar
/// INIT+SIPI es la unica operacion de todo el sistema que cambia el estado del
/// hardware de forma que no se puede deshacer sin reiniciar. Un mando asi se
/// dispara **a proposito**, no por escribir su nombre.
pub fn despertar(cuantos: u32, aviso: impl Fn(u32)) -> (u32, u32) {
    // -- A quien llamar: lo dice el FIRMWARE, no una suposicion ----------
    //
    // La MADT es la lista de nucleos que declara la placa. Antes esto suponia
    // APIC IDs `0..hilos-1`, cierto en un Zen 3 de un CCD y falso en cuanto se
    // cambie de maquina -- y fallando de la peor forma, porque un ID inventado y
    // un nucleo no llamado se ven **igual desde fuera**.
    let censo = super::madt::censo();

    // Por el PERFIL, no por el nombre del fabricante. Ver `cpu_vendor/profile.rs`.
    let por_cpuid = (crate::ring0::cpu_vendor::profile::active().nucleos)().map(|n| n.hilos);

    // * Y se contrastan las dos fuentes. Dicen cosas distintas por naturaleza:
    // CPUID dice lo que el silicio TIENE, la MADT lo que el firmware DECLARA. Si
    // no coinciden, lo normal es que la placa haya apagado algo (SMT off en la
    // BIOS) -- y saberlo aqui evita buscar un fallo en el trampolin cuando lo que
    // pasa es que faltan nucleos a proposito.
    if let (Some(c), Some(hilos)) = (censo.as_ref(), por_cpuid) {
        let declarados = c.ids().len() as u32;
        if declarados != hilos {
            crate::ring0::cabina::warn(
                "smp",
                "el firmware declara otros hilos que el silicio (BIOS?)",
                declarados as u64,
            );
        }
        if c.apagados() > 0 {
            crate::ring0::cabina::warn(
                "smp",
                "nucleos LISTADOS y no habilitados: no se llaman",
                c.apagados() as u64,
            );
        }
    }

    let esperados = match (censo.as_ref(), por_cpuid) {
        // Manda la MADT: es la lista de a quien se puede llamar.
        (Some(c), _) => (c.ids().len() as u32).saturating_sub(1),
        // Sin MADT se sigue, pero **diciendo que se esta suponiendo**.
        (None, Some(hilos)) if hilos > 1 => {
            crate::ring0::cabina::warn(
                "smp",
                "sin MADT: suponiendo APIC IDs 0..hilos-1",
                hilos as u64,
            );
            hilos - 1
        }
        _ => {
            crate::ring0::cabina::warn("smp", "sin MADT ni topologia: no se a quien llamar", 0);
            return (0, 0);
        }
    };

    // * MIRA Y NO TOQUES. Con `cuantos == 0` se sale AQUI, antes de resetear los
    // contadores y antes de escribir un solo byte en la memoria baja. Es lo que
    // hace que el caso por defecto sea de verdad inofensivo y no "inofensivo
    // salvo por lo que ya habia hecho al llegar".
    //
    // Y se contesta lo que hay ahora mismo: si ya se despertaron antes, este
    // camino lo dice sin volver a despertarlos.
    if cuantos == 0 {
        crate::ring0::core::phase::dashboard_log("[smp] censo pedido, no se desperto a nadie");
        return (tramp::VIVOS.load(Ordering::SeqCst), esperados);
    }

    let pedidos = cuantos.min(esperados);
    tramp::VIVOS.store(0, Ordering::SeqCst);
    tramp::MASCARA.store(0, Ordering::SeqCst);
    let yo = tramp::apic_id();

    unsafe {
        // 1. El trampolin, a su pagina.
        let (ini, largo) = tramp::bytes();
        if largo == 0 || largo > 0x1000 {
            crate::ring0::cabina::fault("smp", "el trampolin mide algo imposible", largo as u64);
            return (0, esperados);
        }
        for i in 0..largo {
            core::ptr::write_volatile(
                (TRAMPOLIN as *mut u8).add(i),
                core::ptr::read_volatile(ini.add(i)),
            );
        }

        // 2. La GDT, dentro de la pagina de datos. El GDTR apunta a ESTA copia y
        //    no a la del kernel: en modo real la base es una direccion FISICA y
        //    tiene que caber en 32 bits.
        let gdt = (DATOS + OFF_GDT) as *mut u64;
        for (i, e) in GDT.iter().enumerate() {
            core::ptr::write_volatile(gdt.add(i), *e);
        }
        core::ptr::write_volatile((DATOS + OFF_GDTR) as *mut u16, (GDT.len() * 8 - 1) as u16);
        core::ptr::write_volatile((DATOS + OFF_GDTR + 2) as *mut u64, DATOS + OFF_GDT);

        // 3. La IDT: la del kernel, tal como la tiene puesta el BSP ahora mismo.
        let mut idtr = [0u8; 10];
        core::arch::asm!("sidt [{}]", in(reg) idtr.as_mut_ptr(), options(nostack));
        core::ptr::copy_nonoverlapping(idtr.as_ptr(), (DATOS + OFF_IDTR) as *mut u8, 10);

        // 4. CR3 del kernel y la entrada de 64 bits.
        core::ptr::write_volatile(
            (DATOS + OFF_CR3) as *mut u64,
            crate::ring0::mm::vmm::kernel_pml4(),
        );
        core::ptr::write_volatile(
            (DATOS + OFF_ENTRADA) as *mut u64,
            tramp::smp_ap_entrada as *const () as u64,
        );

        // 5. Y a llamar, de uno en uno. Cada AP recoge SU pila del mismo sitio,
        //    asi que hay que dejarla puesta antes de cada SIPI. De ahi que esto
        //    no sea un broadcast.
        //
        // La pila de cada AP se indexa por el ORDEN de la llamada, no por su
        // APIC ID: con la MADT esos IDs pueden ser cualquier cosa (en un x2APIC
        // son numeros grandes y dispersos), y `PILAS + id * 0x1000` se saldria
        // del primer MiB sin avisar. El orden siempre es 0, 1, 2...
        let mut slot = 0u64;
        let call = |id: u32, slot: &mut u64| {
            *slot += 1;
            core::ptr::write_volatile((DATOS + OFF_PILA) as *mut u64, PILAS + *slot * 0x1000);
            // Se dice ANTES de mandarlo, no despues: si el que cuelga es este,
            // el numero ya esta en pantalla.
            aviso(id);
            lapic::ipi(id, lapic::INIT);
            lapic::esperar_us(10_000);
            lapic::ipi(id, lapic::SIPI);
            lapic::esperar_us(200);
            lapic::ipi(id, lapic::SIPI);
            lapic::esperar_us(200);
        };

        match censo.as_ref() {
            // * La lista del firmware, tal cual. Ni se inventa ni se ordena.
            Some(c) => {
                for &id in c.ids() {
                    if id != yo && (slot as u32) < cuantos {
                        call(id, &mut slot);
                    }
                }
            }
            // El camino de respaldo, ya avisado mas arriba.
            None => {
                for id in 0..esperados + 1 {
                    if id != yo && (slot as u32) < cuantos {
                        call(id, &mut slot);
                    }
                }
            }
        }
    }

    // 6. Contar, con tope. Un AP que no viene no puede colgar al que pregunta.
    // Se espera a los PEDIDOS, no a todos: con `smp 3` esperar a los once seria
    // colgar el comando un segundo entero para contar hasta tres.
    let mut vueltas = 0u32;
    while tramp::VIVOS.load(Ordering::SeqCst) < pedidos && vueltas < 1000 {
        lapic::esperar_us(1000);
        vueltas += 1;
    }

    let alive = tramp::VIVOS.load(Ordering::SeqCst);
    let mascara = tramp::MASCARA.load(Ordering::SeqCst);

    // * TAMBIEN AL KLOG, y esto era un fallo mio de bulto: todo el relato del
    // bring-up iba **solo a CABINA**, que Ring 3 no puede leer. El mensaje del
    // compositor decia "(F11 lo cuenta entero)" y F11 ensena el KLOG, que es
    // otro sitio. O sea: se prometia un dato en una ventana donde no estaba.
    //
    // Son dos sumideros distintos a proposito --CABINA es el narrador con
    // severidad, el klog es la transcripcion-- y quien escribe tiene que elegir
    // los dos cuando quiere que se vea desde fuera.
    crate::ring0::core::phase::dashboard_log(if alive == pedidos {
        "[smp] contestaron todos los que se llamaron"
    } else {
        "[smp] FALTAN nucleos por contestar"
    });

    if alive == pedidos {
        crate::ring0::cabina::info("smp", "contestaron todos los llamados", alive as u64 + 1);
    } else {
        crate::ring0::cabina::warn(
            "smp",
            "faltan nucleos por contestar",
            (pedidos - alive) as u64,
        );
        crate::ring0::cabina::warn("smp", "mascara de los que SI contestaron", mascara as u64);
    }
    (alive, esperados)
}

/// `(alive, mascara)` sin volver a despertar nada.
pub fn alive() -> (u32, u32) {
    (
        tramp::VIVOS.load(Ordering::SeqCst),
        tramp::MASCARA.load(Ordering::SeqCst),
    )
}

// ============ ** EN QUE ESTA CADA NUCLEO, Y POR QUE ============
//
// === El hueco que tapa ===
//
// Hasta hoy la unica pregunta que este modulo sabia contestar era **cuantos**
// estan en pie. Y "once en pie" no dice nada de lo que importa: ni si estan
// trabajando, ni si estan girando en vacio, ni por que uno no contesto.
//
// > **Un nucleo despierto que no hace nada y uno que trabaja se miden igual
// > desde fuera: los dos estan "en pie".** Desde dentro tambien, hasta que
// > alguien escribe esta tabla.
//
// El motivo viaja con el estado a proposito. `nucleo 3: DORMIDO` sin el porque
// es exactamente lo que hace indepurable un sistema al mes siguiente -- y aqui
// los motivos no son adorno: **"parado por orden" y "no contesto al trampolin"
// se ven igual desde el contador y se arreglan en sitios opuestos**.

/// En que estado esta un nucleo. Ver `docs/AXION_MAESTRO.md`.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Estado {
    /// El BSP. Dueno del kernel entero, y no se negocia.
    Maestro,
    /// En pie y aceptando faenas. **Gira** mientras espera: ver el coste.
    Obrero,
    /// Se le mando parar. Sin IPI **no vuelve**: ver `obra::parar`.
    Dormido,
    /// Se le llamo y no contesto. El fallo esta en el trampolin o en su pila.
    Ausente,
    /// Todavia no se ha llamado a nadie. No es un fallo: es que no se ha pedido.
    SinDespertar,
    /// **Existe y este kernel no puede decir en que esta.** El censo de vivos
    /// son 32 bits; a partir del nucleo 32 el bit se comparte. Ver `estado_de`.
    NoSeSabe,
}

impl Estado {
    pub fn nombre(self) -> &'static str {
        match self {
            Estado::Maestro => "MAESTRO",
            Estado::Obrero => "OBRERO",
            Estado::Dormido => "DORMIDO",
            Estado::Ausente => "AUSENTE",
            Estado::SinDespertar => "-",
            Estado::NoSeSabe => "?",
        }
    }

    /// La razon, en el idioma del sistema. **Es la mitad util de la tabla.**
    pub fn motivo(self) -> &'static str {
        match self {
            Estado::Maestro => "el kernel entero: drivers, CABINA, los static mut",
            Estado::Obrero => "acepta faenas; nunca toca un driver. GIRA al esperar",
            Estado::Dormido => "parado por orden; sin IPI no vuelve",
            Estado::Ausente => "se le llamo y no contesto: trampolin o pila",
            Estado::SinDespertar => "nadie lo ha llamado todavia",
            Estado::NoSeSabe => "existe, pero el censo solo cubre 32: ver tramp::MASCARA",
        }
    }
}

/// **El estado del nucleo con APIC id `id`.**
///
/// [!] El `id 0` es el BSP y se contesta sin mirar la mascara: es el nucleo que
/// esta ejecutando esta misma funcion. Preguntarselo a la mascara daria
/// `AUSENTE` --el BSP nunca pasa por el trampolin, asi que nunca pone su bit--
/// y el panel diria que el nucleo que lo esta pintando no existe.
pub fn estado_de(id: u32) -> Estado {
    if id == 0 {
        return Estado::Maestro;
    }
    let (vivos, mascara) = alive();
    if vivos == 0 {
        return Estado::SinDespertar;
    }
    // ** [!] HASTA 32 NUCLEOS, Y NI UNO MAS -- dicho, no supuesto.
    //
    // `MASCARA` son 32 bits y el bit se elige con `id & 31`, o sea que en una
    // maquina con mas de 32 hilos **el nucleo 33 comparte bit con el 1**. Su
    // cabecera en `tramp.rs` lo declara y dice *"ese problema todavia no
    // existe"* -- y era cierto en un 5600X de doce hilos.
    //
    // Deja de serlo en cuanto alguien arranque esto en un EPYC. Y lo que haria
    // esta funcion entonces no es fallar: es **contestar el estado de otro
    // nucleo** con toda tranquilidad, que es mucho peor.
    //
    // > Un limite que no se dice no es un limite: es una mentira esperando la
    // > maquina que la destape.
    if id >= 32 {
        return Estado::NoSeSabe;
    }
    if mascara & (1u32 << id) == 0 {
        return Estado::Ausente;
    }
    if obra::parados() {
        Estado::Dormido
    } else {
        Estado::Obrero
    }
}

/// **CORE o THREAD: es un nucleo fisico o el hermano SMT de otro?**
///
/// En ingles a peticion del dueno, y aqui las dos palabras hacen falta juntas:
/// `12 hilos` no dice si son doce nucleos o seis con SMT, y **la diferencia
/// decide el reparto** -- una faena de calculo denso quiere seis obreros, no
/// doce (ver `docs/AXION_MAESTRO.md`, apartado 3).
///
/// == De donde sale, y que se esta suponiendo ==
///
/// Del PERFIL, no de una tabla: si `hilos == nucleos * 2`, SMT esta encendido y
/// **el bit bajo del APIC ID separa a los dos hermanos** -- es como los numera
/// x86 cuando los IDs son contiguos, que es el caso de este Ryzen y de
/// cualquier maquina de un solo CCD.
///
/// [!] Y por eso `NO_SE` existe. Con IDs dispersos --x2APIC en una placa
/// grande-- el bit bajo deja de significar eso, y **contestar `CORE` a un
/// hermano SMT seria peor que no contestar**: el reparto creeria tener el doble
/// de unidades de ejecucion de las que hay. Lo correcto seria leer
/// `CPUID.8000001E`, y hasta que exista una maquina donde probarlo, esto se
/// calla en vez de adivinar.
pub fn tipo_de(id: u32) -> &'static str {
    let Some(t) = (crate::ring0::cpu_vendor::profile::active().nucleos)() else {
        return "?";
    };
    let (nucleos, hilos) = (t.nucleos as u32, t.hilos as u32);
    if nucleos == 0 || hilos == 0 {
        return "?";
    }
    // Sin SMT no hay hermanos: todos son nucleos.
    if hilos == nucleos {
        return "CORE";
    }
    if hilos != nucleos * 2 {
        // Ni uno ni dos hilos por nucleo: no se sabe repartirlos por el ID.
        return "?";
    }
    // El censo solo cubre 32; mas alla el propio ID ya no es de fiar aqui
    // tampoco. Ver `estado_de`.
    if id >= 32 {
        return "?";
    }
    if id & 1 == 0 {
        "CORE"
    } else {
        "THREAD"
    }
}

/// **Cuantos nucleos estan girando en vacio ahora mismo.**
///
/// Es el numero del ahorro, y hoy es incomodo a proposito: un obrero en espera
/// **gira** (`pause`), no duerme, asi que este contador es literalmente
/// *"cuantos nucleos estan al 100% sin hacer nada"*. Con los doce en pie son
/// once.
///
/// Existe para que el dia que entre `MWAIT` la mejora se pueda **medir** en vez
/// de suponerla. Ver el apartado 5 de `docs/AXION_MAESTRO.md`.
pub fn girando() -> u32 {
    if obra::parados() {
        0
    } else {
        alive().0
    }
}
