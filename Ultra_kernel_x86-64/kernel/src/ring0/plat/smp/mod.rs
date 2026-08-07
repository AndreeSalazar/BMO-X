//! **Despertar los otros núcleos.** El bring-up de SMP, del lado bueno de la
//! frontera.
//!
//! ═══ Por qué esto está aquí y no en `s1_cpu` ═══
//!
//! Había un `smp_startup()` en `faggin/s1_cpu` que **nunca se llamó**, y al ir a
//! llamarlo se vio que estaba **del lado equivocado de `ExitBootServices`**:
//! antes de EBS el firmware sigue vivo y **los otros núcleos son suyos** (UEFI
//! los tiene en su *MP Services*), la memoria baja tampoco es nuestra, y lo
//! siguiente que hacía `s1_cpu` era volver a llamar al firmware. Encima hablaba
//! sólo por `ser_print!`, y en esta máquina no hay cable serie.
//!
//! Aquí, en el kernel, no hay firmware: la máquina es entera de BMO, la memoria
//! baja **ya está reservada** (`phys::init` reserva `<1 MiB` con el comentario
//! *"future SMP trampoline lives here"*) y CABINA pinta en pantalla.
//!
//! ═══ Las cuatro piezas, y por qué están separadas ═══
//!
//! | | |
//! |---|---|
//! | [`mapa`] | las direcciones y la GDT: **el dato** que los dos lados comparten |
//! | [`tramp`] | el trampolín de 16→64 bits y dónde aterriza el AP |
//! | [`lapic`] | mandar IPIs y esperar — sólo lo usa el que despierta |
//! | este fichero | **la orquestación, y nada más** |
//!
//! No es orden por el orden: `mapa` existe porque esas direcciones están
//! escritas **dos veces** —aquí en Rust y a mano dentro del trampolín, que no
//! puede llamar a nada—, y tenerlas en un fichero con nombre propio es lo que
//! hace que se vean juntas. Y `lapic` está aparte porque es justo lo que el AP
//! **no** puede tocar.
//!
//! ═══ ⚠️ SIN PROBAR EN METAL ═══
//!
//! Corre antes de que exista nada, en el único CPU que hay. Por eso **no se
//! llama en el arranque**: se pide con la orden `smp`. Si el trampolín está mal
//! se cuelga un comando, no la máquina al encenderla; la salida es un reinicio a
//! botón.

pub mod lapic;
pub mod mapa;
pub mod tramp;

use core::sync::atomic::Ordering;
use mapa::*;

/// Despierta a los demás núcleos y cuenta cuántos contestan.
///
/// Devuelve `(vivos, esperados)`, ambos **sin contar el BSP**.
///
/// ★ `aviso` se llama **antes de cada SIPI**, con el APIC ID al que le toca. No
/// es adorno: despertar a un AP cuesta hasta 10 ms de espera y esto es lo único
/// que puede colgarse de todo el comando. Sin el aviso, un cuelgue se ve como
/// *"se quedó parado"*; con él se ve **en qué núcleo** se quedó parado, que es
/// la diferencia entre tener un dato y tener que adivinar.
///
/// Va como parámetro y no como una llamada a CABINA desde aquí por dos razones:
/// once líneas seguidas inundarían un anillo de 48 eventos, y **`plat/` no tiene
/// por qué saber pintar**. Quien llama decide cómo se enseña.
pub fn despertar(aviso: impl Fn(u32)) -> (u32, u32) {
    // Por el PERFIL, no por el nombre del fabricante. Ver `cpu_vendor/profile.rs`:
    // la primera versión de esto llamaba a `ryzen_5_5600x::bmo_cpu::topology()`
    // directamente, que es exactamente lo que ese contrato prohíbe.
    let esperados = match (crate::ring0::cpu_vendor::profile::active().nucleos)() {
        Some(n) if n.hilos > 1 => n.hilos - 1,
        _ => {
            crate::ring0::cabina::warn("smp", "sin topologia: no se cuantos nucleos esperar", 0);
            return (0, 0);
        }
    };

    tramp::VIVOS.store(0, Ordering::SeqCst);
    tramp::MASCARA.store(0, Ordering::SeqCst);
    let yo = tramp::apic_id();

    unsafe {
        // 1. El trampolín, a su página.
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

        // 2. La GDT, dentro de la página de datos. El GDTR apunta a ESTA copia y
        //    no a la del kernel: en modo real la base es una dirección FÍSICA y
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
        //    así que hay que dejarla puesta antes de cada SIPI. De ahí que esto
        //    no sea un broadcast.
        //
        // ⚠️ SE SUPONE QUE LOS APIC IDs SON `0..hilos-1`, y hay que decirlo.
        //
        // Es cierto en un Zen 3 de un solo CCD y **es una suposición** en
        // cualquier otra cosa: nada en BMO enumera los APIC IDs de verdad.
        // `Topology::cpus` **parece** ese censo y no lo es —se rellena con 64
        // copias del BSP, ver su doc—, y la fuente buena son las entradas de
        // tipo 0 de la **MADT**, que `s2_mem` localiza y no recorre.
        //
        // Consecuencias de que la suposición falle, para saber qué se está
        // mirando si pasa: a un ID que no existe la IPI se va al vacío —el
        // `Delivery Status` acaba por bajar y se sigue—, y un núcleo real con
        // un ID fuera del rango no se llamaría nunca. Las dos cosas se ven
        // igual desde fuera: **faltan núcleos**. Por eso se imprime la máscara.
        for id in 0..esperados + 1 {
            if id == yo {
                continue;
            }
            core::ptr::write_volatile(
                (DATOS + OFF_PILA) as *mut u64,
                PILAS + (id as u64 + 1) * 0x1000,
            );
            // Se dice ANTES de mandarlo, no después: si el que cuelga es éste,
            // el número ya está en pantalla.
            aviso(id);
            lapic::ipi(id, lapic::INIT);
            lapic::esperar_us(10_000);
            lapic::ipi(id, lapic::SIPI);
            lapic::esperar_us(200);
            lapic::ipi(id, lapic::SIPI);
            lapic::esperar_us(200);
        }
    }

    // 6. Contar, con tope. Un AP que no viene no puede colgar al que pregunta.
    let mut vueltas = 0u32;
    while tramp::VIVOS.load(Ordering::SeqCst) < esperados && vueltas < 1000 {
        lapic::esperar_us(1000);
        vueltas += 1;
    }

    let vivos = tramp::VIVOS.load(Ordering::SeqCst);
    let mascara = tramp::MASCARA.load(Ordering::SeqCst);
    if vivos == esperados {
        crate::ring0::cabina::info("smp", "todos los nucleos contestaron", vivos as u64 + 1);
    } else {
        crate::ring0::cabina::warn(
            "smp",
            "faltan nucleos por contestar",
            (esperados - vivos) as u64,
        );
        crate::ring0::cabina::warn("smp", "mascara de los que SI contestaron", mascara as u64);
    }
    (vivos, esperados)
}

/// `(vivos, máscara)` sin volver a despertar nada.
pub fn vivos() -> (u32, u32) {
    (
        tramp::VIVOS.load(Ordering::SeqCst),
        tramp::MASCARA.load(Ordering::SeqCst),
    )
}
