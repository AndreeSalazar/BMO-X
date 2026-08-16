//! **THE DOOR ITSELF** -- the naked entry stub and its installation.
//!
//! === Why this is a file of its own ===
//!
//! Because it is the only code in the kernel that runs with **no frame, no
//! stack of its own and no compiler help**. `#[unsafe(naked)]` means exactly
//! that: rustc emits the bytes and nothing else, so every register that must
//! survive has to be saved here by hand, in the right order, before anything
//! Rust-shaped can happen.
//!
//! That makes it the highest-consequence twenty lines in the tree and the ones
//! least like their neighbours. Sitting between the operation table and the
//! dispatcher it read as more of the same; on its own the warning is the file.
//!
//! [!] `syscall` destroys `rcx` and `r11` -- the CPU puts the return address in
//! one and RFLAGS in the other. Anything that assumed they survive is already
//! wrong by the time the first Rust line runs, which is why the emulator fills
//! them with poison.

use core::arch::{asm, naked_asm};
use crate::ring0::task::percpu;
use super::ops::*;
use super::dispatch;

#[unsafe(naked)]
unsafe extern "C" fn syscall_entry() -> ! {
    naked_asm!(
        "swapgs",
        "mov gs:[0x08], rsp",          // stash user RSP
        "mov rsp, gs:[0x00]",          // per-CPU syscall stack
        // Trap tail: ss, rsp, rflags, cs, rip (SYSCALL contract values).
        "push 0x1B",                   // user SS
        "push qword ptr gs:[0x08]",    // user RSP
        "push r11",                    // user RFLAGS
        "push 0x23",                   // user CS
        "push rcx",                    // user RIP
        // 15 GPRs (push order; pops restore the reverse).
        "push rax", "push rcx", "push rdx", "push rbx", "push rbp",
        "push rsi", "push rdi", "push r8", "push r9", "push r10",
        "push r11", "push r12", "push r13", "push r14", "push r15",
        "mov rbp, rsp",
        "sub rsp, {reserva}",
        "and rsp, -64",                // XSAVE exige 64 bytes de alineacion
        // La cabecera a cero ANTES del xsave64: ver el prologo del timer. En
        // corto: `XSAVE` no escribe los 48 bytes reservados de la cabecera y
        // `XRSTOR` da #GP(0) si no son cero. El area se talla sobre la pila,
        // asi que sin esto hereda la basura de lo que hubiera debajo.
        //
        // Incluye el XSTATE_BV de +512: `XSAVE` CONSERVA los bits que caen
        // fuera de `RFBM = EDX:EAX AND XCR0`, asi que la basura de ahi
        // sobrevive al guardado. Ver el prologo del timer.
        "mov qword ptr [rsp+{bv}], 0",
        "mov qword ptr [rsp+{cero}], 0",
        "mov qword ptr [rsp+{cero}+8], 0",
        "mov qword ptr [rsp+{cero}+16], 0",
        "mov qword ptr [rsp+{cero}+24], 0",
        "mov qword ptr [rsp+{cero}+32], 0",
        "mov qword ptr [rsp+{cero}+40], 0",
        "mov qword ptr [rsp+{cero}+48], 0",
        "mov [rsp+{area}], rbp",       // back-pointer to the GPR block
        "mov qword ptr [rsp+{firma}], {magia}", // sello del contexto
        // RFBM = -1: guarda lo que XCR0 tenga habilitado, sea lo que sea.
        // rax y rdx ya estan salvados en el bloque de GPR de arriba.
        "mov eax, -1", "mov edx, -1",
        // ** `xsaveopt64` Y NO `xsave64`, desde el 2026-08-16.
        //
        // === De donde sale este cambio ===
        //
        // El metro (`syscall/meter.rs`) midio en el Ryzen que una puerta cuesta
        // 2663 ciclos y que **2345 de ellos --el 88%-- estan en este stub**, no
        // en el Rust. Y el censo de extensiones (`ext`) enseno que XSAVEOPT
        // esta EN ESTE SILICIO Y SIN USAR. Las dos cosas se midieron el mismo
        // dia y esta linea es donde se juntan.
        //
        // === Que hace distinto ===
        //
        // XSAVEOPT trae la *modified optimization*: puede NO escribir los
        // componentes que no se han tocado desde el ultimo `xrstor64` hecho
        // **desde esta misma direccion**. Y esa condicion se cumple justo en el
        // caso que domina: una puerta que vuelve a LA MISMA tarea.
        //
        // ** Lo que lo hace valido aqui no es una suposicion, es el target: el
        // kernel se compila `-sse,-avx,+soft-float` (`rustc-abi: softfloat`),
        // o sea que **no toca un solo registro xmm**. Entre el `xrstor64` de la
        // puerta anterior y este `xsaveopt64` nadie modifico el estado
        // extendido, que es exactamente la condicion que la instruccion mira.
        //
        // === Por que es SEGURO, y no es la cirugia que se aplazo ===
        //
        //   1. **Formato ESTANDAR**, igual que `xsave64`. El `xrstor64` del
        //      epilogo no cambia ni una letra. (`XSAVEC`/`XSAVES` usan formato
        //      COMPACTADO y obligarian a tocar tambien la restauracion -- que
        //      es el codigo que produjo el `#GP en xrstor`. Por eso NO se
        //      tocan hoy.)
        //   2. La cabecera se sigue poniendo a cero arriba y `RFBM` sigue
        //      siendo -1: el `XSTATE_BV` se escribe con las mismas reglas.
        //   3. Si el ultimo `xrstor64` vino de OTRA direccion --un cambio de
        //      tarea, o el camino del timer-- la optimizacion simplemente no se
        //      aplica y escribe entero. El caso de duda se resuelve escribiendo
        //      de mas, nunca de menos.
        //
        // [!] La optimizacion esta PERMITIDA, no garantizada: el CPU puede
        // escribir igual. Por eso esto no se declara una mejora hasta que
        // `run c/coste.bex` ensene el reparto nuevo -- el mismo instrumento que
        // encontro el problema es el que dice si se arreglo.
        //
        // [!] Y si algun dia se arranca en un CPU sin XSAVEOPT, esto es `#UD`
        // en la primera puerta. Lo vigila el censo: `usage.rs` declara esta
        // fila como USADA, asi que alli seria un CONFLICTO y el arranque lo
        // grita en CABINA. Ver `features/mod.rs`.
        "xsaveopt64 [rsp]",
        "mov gs:[0x10], rsp",          // publish this context
        "cld",
        "mov rdi, rbp",
        "call {dispatch}",
        // Shared trap epilogue: rax = xsave-base of the context to run.
        "mov rsp, rax",
        "cmp qword ptr [rsp+{firma}], {magia}",
        "jne 3f",
        // La CABECERA, antes de borrar el sello: asi el informe la lee intacta
        // y puede decir de quien era el contexto. rax/rdx se pueden pisar aqui
        // -- los recuperan los pops de abajo.
        "mov rdx, qword ptr [rsp+{bv}]",
        "and rdx, qword ptr [rip+{no_xcr0}]",
        "jnz 8f",
        "mov rax, qword ptr [rsp+{cero}]",
        "or rax, qword ptr [rsp+{cero}+8]",
        "or rax, qword ptr [rsp+{cero}+16]",
        "or rax, qword ptr [rsp+{cero}+24]",
        "or rax, qword ptr [rsp+{cero}+32]",
        "or rax, qword ptr [rsp+{cero}+40]",
        "or rax, qword ptr [rsp+{cero}+48]",
        "jnz 8f",
        // UN SOLO USO: al restaurarlo se borra el sello. Un contexto que ya
        // se consumio no puede volver a pasar por bueno -- si alguien lo
        // intenta, se planta con nombre en vez de reventar en el xrstor.
        "mov qword ptr [rsp+{firma}], 0",
        "mov eax, -1", "mov edx, -1",
        "xrstor64 [rsp]",
        "mov rsp, [rsp+{area}]",
        "pop r15", "pop r14", "pop r13", "pop r12", "pop r11",
        "pop r10", "pop r9", "pop r8", "pop rdi", "pop rsi",
        "pop rbp", "pop rbx", "pop rdx", "pop rcx", "pop rax",
        "cmp qword ptr [rsp+8], 0x08", // returning to kernel CS?
        "je 1f",
        "cmp qword ptr [rsp+8], 0x23", // ...o a usuario. Cualquier otra cosa
        "jne 4f",                      // no es un contexto, es basura.
        "swapgs",
        "1: iretq",
        "3: mov rdi, {m_sello}", "mov rsi, rsp", "and rsp, -16", "call {podrido}",
        "4: mov rdi, {m_cs}", "mov rsi, rsp", "and rsp, -16", "call {podrido}",
        "8: mov rdi, {m_cab}", "mov rsi, rsp", "and rsp, -16", "call {podrido}",
        dispatch = sym dispatch,
        podrido = sym crate::ring0::plat::faults::contexto_podrido,
        no_xcr0 = sym crate::ring0::plat::trap::XSAVE_NO_XCR0,
        area = const crate::ring0::plat::trap::XSAVE_AREA,
        firma = const crate::ring0::plat::trap::SELLO_FIRMA,
        magia = const crate::ring0::plat::trap::SELLO_MAGIA,
        bv = const crate::ring0::plat::trap::XSAVE_BV,
        cero = const crate::ring0::plat::trap::XSAVE_CERO_DESDE,
        m_sello = const crate::ring0::plat::faults::PODRIDO_SELLO,
        m_cs = const crate::ring0::plat::faults::PODRIDO_CS,
        m_cab = const crate::ring0::plat::faults::PODRIDO_CABECERA,
        reserva = const crate::ring0::plat::trap::XSAVE_RESERVA,
    );
}

unsafe fn wrmsr(msr: u32, value: u64) {
    asm!(
        "wrmsr",
        in("ecx") msr,
        in("eax") value as u32,
        in("edx") (value >> 32) as u64,
        options(nostack),
    );
}

pub fn init() {
    let star = (SYSRET_SELECTOR_BASE << 48) | (KERNEL_CS << 32);
    unsafe {
        wrmsr(MSR_STAR, star);
        wrmsr(MSR_LSTAR, syscall_entry as *const () as u64);
        // Do not let hostile user flags trigger #DB/#AC before the entry stub
        // has switched away from the user stack. Interrupts stay masked for
        // the whole dispatch; the iretq restores the user IF.
        wrmsr(
            MSR_SFMASK,
            RFLAGS_TF | RFLAGS_IF | RFLAGS_DF | RFLAGS_NT | RFLAGS_AC,
        );
    }
}
