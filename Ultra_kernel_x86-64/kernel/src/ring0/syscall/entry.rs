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
use super::meter;

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
        // ** SELLO A -- el primer instante en que hay registros libres.
        //
        // No antes: hasta que `rax` y `rdx` estan en la pila, `rdtsc` --que
        // escribe los dos-- destruiria valores del usuario. Todo lo que queda
        // por debajo (`syscall`, `swapgs`, el cambio de pila y los 20 pushes)
        // es por eso IMPOSIBLE de sellar desde aqui y sale en el `resto`.
        // Ver el reparto entero en `meter.rs`.
        "rdtsc", "shl rdx, 32", "or rax, rdx",
        "mov qword ptr [rip+{ultimo}], rax",
        "sub rsp, {reserva}",
        "and rsp, -64",                // XSAVE exige 64 bytes de alineacion
        "mov [rsp+{area}], rbp",       // back-pointer to the GPR block
        "mov qword ptr [rsp+{firma}], {magia}", // sello del contexto
        //
        // ===== ** AQUI YA NO SE GUARDA EL ESTADO EXTENDIDO. ================
        //
        // Estaban aqui la cabecera a cero (8 stores) y el `xsaveopt64`, y se
        // hacian EN TODAS LAS PUERTAS. Se han bajado a la via lenta --la
        // etiqueta `5:`-- porque en la puerta normal **no guardan nada que
        // haga falta**, y esto no es una apuesta: es una propiedad del target.
        //
        // El kernel se compila `x86_64-unknown-none`, cuya ficha dice
        // literalmente `-mmx,-sse,-sse2,-avx,+soft-float` y `rustc-abi:
        // softfloat`. O sea que entre el `syscall` que entra y el `iretq` que
        // sale **nadie puede tocar un registro xmm, mm ni x87**. Guardar un
        // estado del que se sabe que no va a cambiar, para restaurarlo
        // identico cuatrocientos ciclos despues, es trabajo que no hace nada.
        //
        // ** Y NO SE DA POR BUENO DESDE LA FICHA: se comprobo en el BINARIO.
        // Un barrido del `bmo-kernel` entero --755 KB-- por `%xmm`, `%mm` y
        // `%st()`, y por los mnemonicos de SSE y x87, da **cero**. La premisa
        // esta medida, no supuesta. Si algun dia alguien mete una sola
        // instruccion SSE en Ring 0, esta optimizacion se vuelve un fallo de
        // corrupcion silenciosa -- por eso el barrido se queda escrito aqui
        // como lo que hay que repetir antes de dudar de esta linea.
        //
        // Lo que SI se queda arriba, y por que:
        //   - la reserva y el `and rsp,-64`: el area tiene que existir por si
        //     `dispatch` cambia de tarea, y entonces se talla justo ahi.
        //   - el back-pointer y el sello: los lee el planificador cuando
        //     consuma un cambio (`trap::seal`), y para entonces ya es tarde
        //     para escribirlos.
        // Son cuatro instrucciones contra las once que se han ido.
        //
        // ** SELLO B -- cierra GUARDA, que ahora mide el prologo pelado.
        "rdtsc", "shl rdx, 32", "or rax, rdx",
        "sub rax, qword ptr [rip+{ultimo}]",
        "add qword ptr [rip+{guarda}], rax",
        "mov gs:[0x10], rsp",          // publish this context
        "cld",
        "mov rdi, rbp",
        "call {dispatch}",
        // ** SELLO D' -- abre RESTAURA, y por eso `rax` hay que salvarlo: trae
        // la base del contexto que toca ejecutar y `rdtsc` lo pisaria. `rcx`
        // sirve de hueco porque el `rcx` del usuario esta en la pila desde el
        // push de arriba y lo recupera su `pop`.
        //
        // Va aqui y no dentro de `meter::stop` para no cambiar `stop`: asi
        // `dispatch` sigue midiendose exactamente igual que esta manana y ese
        // 319 vale de control. Lo que queda entre el `stop` y este sello --dos
        // sumas volatiles y el epilogo de `dispatch`-- cae en el `resto`.
        "mov rcx, rax",
        "rdtsc", "shl rdx, 32", "or rax, rdx",
        "mov qword ptr [rip+{ultimo}], rax",
        "mov rax, rcx",
        //
        // ======== ** LA BIFURCACION ==========================================
        //
        // `dispatch` devuelve la base del contexto que toca ejecutar, que es
        // `percpu::trap_rsp()`. Nosotros publicamos AHI nuestra propia base
        // cuatro lineas antes, y el `call` deja `rsp` justo donde estaba. O sea
        // que **`rax == rsp` significa exactamente "nadie cambio de tarea"** --
        // una comparacion de registros, sin una sola lectura de memoria.
        //
        // Y esa pregunta es la que decide LAS DOS COSAS que cuesta una puerta:
        //
        //   igual    -> el contexto que entro es el que sale. Nada toco el
        //               estado extendido (el kernel es softfloat), asi que no
        //               hay nada que guardar ni nada que restaurar. VIA RAPIDA.
        //   distinto -> el nuestro se va y hay que guardarlo de verdad, y el
        //               que entra hay que devolverlo entero. VIA LENTA.
        //
        // ** El planificador no ha cambiado ni una linea para esto, y eso fue
        // una decision: `yield_current` lo llaman DOS clases de sitio --la
        // puerta (`syscall/mod.rs`) y una tarea de kernel (`dev/usb/bus.rs`)--
        // asi que meter la condicion alli obligaba a marcar cada sitio de
        // llamada con quien habia publicado el contexto. Aqui la condicion no
        // hay que declararla: **se lee**.
        "cmp rax, rsp",
        "jne 5f",
        //
        // -------- VIA RAPIDA: misma tarea ------------------------------------
        //
        // No hay `xrstor64`, y por eso tampoco hay comprobacion de sello ni de
        // cabecera: esas nacieron para que un area corrupta no reventara EN el
        // `xrstor64`, y aqui no hay `xrstor64` que proteger. El marco al que se
        // vuelve lo construimos nosotros hace un instante, con las
        // interrupciones enmascaradas, y no ha salido de esta pila.
        //
        // Tampoco se comprueba el CS: la via rapida vuelve a quien entro por
        // `syscall`, y su CS es el `0x23` que **empujamos como constante** en
        // el prologo. Comprobarlo seria preguntarle a un literal.
        //
        // ** LO QUE SI SE HACE, Y ES EL UNICO STORE DE ESTA VIA: borrar el
        // sello. La via lenta lo borra al consumir un contexto para que una
        // direccion caducada no vuelva a pasar por buena, y en cuanto se
        // ejecuta el `iretq` de abajo esta area es pila libre. Sin este store,
        // un `trap_rsp` rancio --el caso que `scheduler::Saliente` documenta--
        // apuntaria a un area consumida CON el sello puesto, y en vez de
        // plantarse con nombre restauraria basura. Costo tres dias y tres
        // fotos descubrir eso; no se tira por un store.
        "mov qword ptr [rsp+{firma}], 0",
        "mov rsp, rbp",                // rbp es callee-saved: `dispatch` lo devuelve intacto
        // ** SELLO E de la via rapida. Mismo motivo que el de la lenta: rax,
        // rcx y rdx los sobrescriben los pops de la linea siguiente.
        "rdtsc", "shl rdx, 32", "or rax, rdx",
        "sub rax, qword ptr [rip+{ultimo}]",
        "add qword ptr [rip+{restaura}], rax",
        "pop r15", "pop r14", "pop r13", "pop r12", "pop r11",
        "pop r10", "pop r9", "pop r8", "pop rdi", "pop rsi",
        "pop rbp", "pop rbx", "pop rdx", "pop rcx", "pop rax",
        "swapgs",
        "iretq",
        //
        // -------- VIA LENTA: cambio de contexto ------------------------------
        //
        // ** AQUI ES DONDE SE GUARDA EL ESTADO EXTENDIDO, y sigue estando a
        // tiempo: como el kernel no toca xmm, el estado del usuario que se va
        // **todavia esta vivo en los registros** en este instante. El
        // planificador ya anoto nuestra area como su `context_rsp` y le puso el
        // sello (`trap::seal`); lo unico que faltaba era el contenido, y es lo
        // que se escribe ahora.
        //
        // Las interrupciones siguen enmascaradas desde el `syscall`, asi que
        // entre la anotacion del planificador y este guardado no cabe nadie.
        "5: mov rcx, rax",             // el destino, a salvo de rax/rdx
        // La cabecera a cero ANTES del xsave: `XSAVE` no escribe los 48 bytes
        // reservados y `XRSTOR` da #GP(0) si no son cero. El area se talla
        // sobre la pila, asi que sin esto hereda lo que hubiera debajo.
        // Incluye el XSTATE_BV de +512: `XSAVE` CONSERVA los bits fuera de
        // `RFBM = EDX:EAX AND XCR0`, y esa basura sobrevive al guardado.
        "mov qword ptr [rsp+{bv}], 0",
        "mov qword ptr [rsp+{cero}], 0",
        "mov qword ptr [rsp+{cero}+8], 0",
        "mov qword ptr [rsp+{cero}+16], 0",
        "mov qword ptr [rsp+{cero}+24], 0",
        "mov qword ptr [rsp+{cero}+32], 0",
        "mov qword ptr [rsp+{cero}+40], 0",
        "mov qword ptr [rsp+{cero}+48], 0",
        // RFBM = -1: guarda lo que XCR0 tenga habilitado, sea lo que sea.
        //
        // ** Sigue siendo `xsaveopt64` y no `xsave64`: formato ESTANDAR (el
        // `xrstor64` de abajo no cambia una letra), la cabecera se pone a cero
        // igual, y si la *modified optimization* no le sirve escribe entero --
        // el caso de duda se resuelve escribiendo de mas, nunca de menos. Lo
        // que si ha cambiado es lo que vale: los 45 ciclos que compro el
        // 16-08 se cobraban en TODAS las puertas, y ahora solo en las que
        // cambian de tarea. **Esta pieza se traga aquella**, y esta bien: la
        // de antes abarataba el trabajo, esta lo quita.
        //
        // [!] En un CPU sin XSAVEOPT esto es `#UD`. Lo vigila el censo:
        // `usage.rs` declara la fila como USADA, asi que seria un CONFLICTO y
        // el arranque lo grita en CABINA. Ver `features/mod.rs`.
        "mov eax, -1", "mov edx, -1",
        "xsaveopt64 [rsp]",
        "mov rax, rcx",
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
        // ** SELLO E -- cierra RESTAURA. Ultimo punto con registros libres:
        // `rax`, `rcx` y `rdx` los sobrescriben los `pop` de tres lineas mas
        // abajo, asi que pisarlos aqui no cuesta nada. Despues de esos pops ya
        // no hay donde escribir, y por eso los 15 pops, las dos comprobaciones
        // de CS, el `swapgs` y el `iretq` caen tambien en el `resto`.
        "rdtsc", "shl rdx, 32", "or rax, rdx",
        "sub rax, qword ptr [rip+{ultimo}]",
        "add qword ptr [rip+{restaura}], rax",
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
        // El reparto DENTRO del stub. Son tres casillas de `meter.rs` escritas
        // desde aqui porque este es el unico sitio donde el codigo esta -- y
        // porque el metro es de la puerta, no de este fichero. Ver `meter.rs`.
        ultimo = sym meter::ETAPA_ULTIMO,
        guarda = sym meter::CICLOS_GUARDA,
        restaura = sym meter::CICLOS_RESTAURA,
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
