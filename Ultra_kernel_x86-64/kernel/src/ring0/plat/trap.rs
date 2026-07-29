//! Unified trap frame — the single context representation for every Ring 0
//! entry (SYSCALL, IRQ, first switch into a task).
//!
//! Both trap entries push 15 GPRs in the same order, then hand control to a
//! Rust dispatcher with a pointer to this frame. Below the frame sits the
//! extended-state image (64-byte aligned) and a back-pointer slot:
//!
//! ```text
//! high ─►  [ss][rsp][rflags][cs][rip]   trap tail (5 u64)
//!          [rax]...[r15]                15 GPRs (pushed rax first)
//!          gpr_base = &frame
//!          [gpr_base back-ptr]          at xsave_base + XSAVE_AREA
//! low  ─►  [xsave XSAVE_AREA B, 64-al]  xsave_base = "context rsp"
//! ```
//!
//! A context is fully described by its `xsave_base`: the shared epilogue does
//! `rsp = xsave_base; xrstor; rsp = [rsp+XSAVE_AREA]; pop 15; iretq`.
//! Context switch = choosing the next `xsave_base`.
//!
//! ## Por qué XSAVE y no FXSAVE
//!
//! `FXSAVE` guarda 512 bytes: x87 y SSE. En este Ryzen el firmware ya dejó
//! `CR4.OSXSAVE` puesto y `XCR0 = 0x7`, o sea **AVX habilitado antes de que el
//! kernel arrancara**. Un programa Ring 3 que usara `YMM` perdía la mitad alta
//! de sus registros en el primer cambio de tarea, sin fault y sin aviso.
//!
//! ## La máscara es todo unos, y eso es lo que lo hace portable
//!
//! `XSAVE`/`XRSTOR` operan sobre `RFBM ∩ XCR0`. Con `RFBM = -1` se guarda
//! **exactamente lo que este CPU tenga habilitado**, sea cual sea. No hay que
//! saber qué CPU es ni leer `XCR0` desde el ensamblador: la máscara es una
//! constante correcta en cualquier máquina. Un valor concreto (0x7) habría
//! sido un número de este Ryzen metido en el camino más crítico del kernel.
//!
//! ## El área es fija, y por eso se verifica al arrancar
//!
//! El ensamblador necesita el desplazamiento del back-pointer como constante,
//! así que el área tiene tamaño fijo. Lo que ocupa de verdad lo decide el CPU
//! (CPUID hoja 0xD), así que `cpu_vendor::xsave::init()` lo comprueba **antes
//! de que exista el primer trap** y se planta si no cabe. Reservar de más
//! cuesta unos KiB; quedarse corto desborda la pila de una tarea.

/// Frame passed to Rust dispatchers. Field order matches the push order:
/// `push rax, rcx, rdx, rbx, rbp, rsi, rdi, r8..r15` leaves `r15` lowest.
#[repr(C)]
pub struct TrapFrame {
    pub r15: u64,
    pub r14: u64,
    pub r13: u64,
    pub r12: u64,
    pub r11: u64,
    pub r10: u64,
    pub r9: u64,
    pub r8: u64,
    pub rdi: u64,
    pub rsi: u64,
    pub rbp: u64,
    pub rbx: u64,
    pub rdx: u64,
    pub rcx: u64,
    pub rax: u64,
    pub rip: u64,
    pub cs: u64,
    pub rflags: u64,
    /// Valid only when the trap came from Ring 3 (or was fabricated).
    pub rsp: u64,
    pub ss: u64,
}

pub const RFLAGS_IF: u64 = 1 << 9;

pub const KERNEL_CS: u64 = 0x08;
/// **0x10, no 0x08.** La GDT que monta `s1_cpu` es
/// `[0]=nulo [1]=codigo0 [2]=datos0 [3]=datos3 [4]=codigo3`, o sea que `0x08`
/// es el selector de **CÓDIGO** de Ring 0 y el de datos es `0x10`.
///
/// Decía `0x08` y eso era un `iretq` condenado: **en modo largo el `iretq` saca
/// `SS:RSP` siempre**, también cuando vuelve al mismo privilegio, y cargar `SS`
/// con un descriptor de código da `#GP(selector)`. El informe lo cantaba solo —
/// `err=0x00000008` es literalmente el selector culpable, dicho por el CPU.
///
/// Sólo mordía al arrancar una tarea de KERNEL (`spawn_kernel`, o sea `ktest`):
/// las de Ring 3 usan `USER_SS = 0x1B`, que sí apunta a datos. Por eso llevaba
/// aquí sin que nadie lo pisara.
///
/// La simetría de abajo es la comprobación: `USER_SS` va a la entrada de datos
/// y `USER_CS` a la de código. Arriba tenía que ser igual.
pub const KERNEL_SS: u64 = 0x10;
pub const USER_CS: u64 = 0x23;
pub const USER_SS: u64 = 0x1B;

/// Bytes reservados para la imagen de estado extendido en CADA contexto.
///
/// Es constante porque el ensamblador de los stubs necesita el desplazamiento
/// del back-pointer como inmediato. Múltiplo de 64, que es lo que `XSAVE`
/// exige de alineación.
///
/// **1024 y no 3072.** La primera versión reservó 3072 "por si acaso", como
/// previsión para un CPU con AVX-512 que esta máquina no tiene: el `cpu` dice
/// que con `XCR0 = 0x7` necesita **832**. Esa previsión multiplicó el contexto
/// por 4,6 de golpe y no era gratis — cada trap se lleva eso de la pila antes
/// de que el despachador haga nada, y los contextos empezaron a caer donde no
/// debían.
///
/// Reservar de más no es conservador cuando el margen sale de la pila de cada
/// tarea. Se ajusta a lo que este CPU pide, con holgura, y **el guardia de
/// arranque es lo que hace que eso sea seguro**: si algún día hay un CPU que
/// necesita más, `cpu_vendor::xsave::init()` lo ve por CPUID antes del primer
/// trap y se planta con el motivo, en vez de corromper pilas en silencio.
/// Subir este número es entonces una decisión informada, no una apuesta.
pub const XSAVE_AREA: usize = 1024;

/// Área + back-pointer + margen para realinear a 64. Es lo que los stubs
/// restan de la pila antes del `and rsp, -64`.
pub const XSAVE_RESERVA: usize = XSAVE_AREA + 8 + 64;

/// Los últimos 64 bytes del área NO los toca el CPU: `xsave64` escribe como
/// mucho `area_actual` bytes (832 en este Ryzen) y `xrstor64` lee lo mismo.
/// `cpu_vendor::xsave::init()` se planta al arrancar si algún CPU necesitara
/// meterse aquí, así que este espacio es nuestro por contrato verificado.
///
/// Se usa como SELLO del contexto: una firma fija y el dueño. No es adorno —
/// es la diferencia entre "el iretq murió con cs=0" y "el contexto del tid 3
/// lo pisó algo entre que se guardó y que se restauró".
pub const SELLO_FIRMA: usize = XSAVE_AREA - 16;
pub const SELLO_DUENO: usize = XSAVE_AREA - 8;

/// Firma del sello. Cabe en un `imm32` con signo, que es lo que admite
/// `mov qword ptr [mem], imm` en los stubs.
pub const SELLO_MAGIA: u64 = 0x424D_4F31; // "BMO1"

// ── La cabecera XSAVE, y por qué también se vigila ──────────────────────
//
// El sello cubre el final del área (`+1008`/`+1016`) y el back-pointer cubre
// `+1024`. Los dos EXTREMOS. En medio, en `+512`, está la cabecera XSAVE — y
// no la miraba nadie.
//
// Ese hueco se cobró una foto: `#GP(0)` en el `xrstor64` del epílogo del timer
// con el sello intacto. `XRSTOR64` da `#GP(0)` por cuatro motivos, y quitando
// la alineación (que el `and rsp, -64` garantiza), los otros tres viven todos
// aquí dentro. El informe salía describiendo el sitio donde el CPU se enteró,
// no el sitio donde se rompió — exactamente lo mismo que ya pasaba con el
// `iretq` de `cs=0` antes de que existiera `contexto_podrido`.
//
// Tres comparaciones lo convierten en un informe con nombre.

/// `XSTATE_BV`: qué componentes trae la imagen. `XRSTOR` da `#GP` si enciende
/// alguno que `XCR0` no tenga habilitado.
///
/// ★ **`XSAVE` NO lo escribe entero, y ésa fue la causa raíz.** Lo que hace es
///
/// ```text
///     XSTATE_BV ← (XSTATE_BV_viejo AND NOT RFBM) OR (XINUSE AND RFBM)
/// ```
///
/// con `RFBM = EDX:EAX AND XCR0`. Los stubs pasan `EDX:EAX = -1`, así que
/// `RFBM = XCR0 = 0x7` en este CPU — y **todos los bits fuera de `XCR0` se
/// conservan del valor anterior**. Un área tallada sobre basura hereda esa
/// basura en los bits altos, sobrevive al guardado, y `XRSTOR` la rechaza con
/// `#GP(0)` por declarar componentes que el CPU no tiene habilitados.
///
/// La firma en los volcados: `0x5F0FCB` y `0x37B`, los dos "el valor viejo con
/// los tres bits bajos puestos a 3" — y 3 es exactamente `XINUSE & 7` (x87 y
/// SSE en uso, AVX en estado inicial). Dos fotos, el mismo patrón.
///
/// Por eso los prólogos ponen a cero **la cabecera entera** (512..575), no sólo
/// los reservados: ni `XSTATE_BV` ni los reservados los inicializa el CPU.
pub const XSAVE_BV: usize = 512;
/// `XCOMP_BV` y los 48 bytes reservados que le siguen: `520..=575`. **Siete
/// palabras que valen cero siempre.**
///
/// ★ **`XSAVE` NO las escribe todas, y eso costó dos pantallas azules.**
/// `XSAVE` escribe `XSTATE_BV`, pone `XCOMP_BV` a cero, y **deja los 48 bytes
/// reservados (528..575) como estaban**. `XRSTOR`, en cambio, da `#GP(0)` si no
/// son todos cero. O sea que ponerlos a cero es deber del SOFTWARE, una vez, al
/// crear el área.
///
/// `fabricate` siempre lo hizo bien —pone a cero los 1024 bytes antes de nada—
/// pero los **stubs de entrada** tallan su área en la pila con `sub`+`and`, o
/// sea encima de lo que hubiera ahí, y hacían `xsave64` directamente. La basura
/// que trajera la pila en esos 48 bytes sobrevivía al guardado y reventaba en el
/// `xrstor64` del epílogo. Intermitente, y peor en la pila de arranque, donde lo
/// que hay debajo cambia en cada vuelta. Ésa era la asimetría.
///
/// Ahora los prólogos ponen a cero **exactamente estas siete palabras**, que son
/// exactamente las que el epílogo verifica. Un valor distinto de cero aquí ya no
/// puede venir de la pila: sólo de que alguien haya escrito encima del área. Por
/// eso la comprobación no tiene un solo falso positivo posible.
pub const XSAVE_CERO_DESDE: usize = 520;
pub const XSAVE_CERO_PALABRAS: usize = 7;

/// El complemento de `XCR0` tal y como estaba al arrancar: `!xcr0`.
///
/// Guardado del revés a propósito. El epílogo sólo necesita `XSTATE_BV & !XCR0`
/// y comparar contra cero; precalcular el `not` aquí ahorra una instrucción en
/// **el camino más caliente del kernel**, que es el que recorre cada cambio de
/// contexto.
///
/// ★ Empieza en 0, y eso es lo correcto: `algo & 0 == 0`, así que hasta que
/// `cpu_vendor::xsave::init()` lo rellene, la guardia está **inerte**. Un
/// guardián que se dispara antes de saber contra qué compara no protege nada:
/// para la máquina en cada trap por un motivo inventado.
///
/// ★ Y sale de `XGETBV`, no de una constante. Escribir aquí `!0x7` habría sido
/// meter un número de este Ryzen concreto en el camino crítico del kernel, que
/// es justo lo que se evitó con `RFBM = -1`.
#[unsafe(no_mangle)]
pub static mut XSAVE_NO_XCR0: u64 = 0;

/// Lo llama `cpu_vendor::xsave::init()` cuando ya ha leído `XCR0`, y sólo esa
/// vez: después de aquí los epílogos empiezan a mirar la cabecera.
pub fn armar_guardia_cabecera(xcr0: u64) {
    unsafe {
        core::ptr::addr_of_mut!(XSAVE_NO_XCR0).write_volatile(!xcr0);
    }
}

// ── Quién talla áreas, y dónde ──────────────────────────────────────────
//
// La guardia de cabecera dice QUÉ contexto se rompió y DE QUIÉN era. Lo que no
// puede decir es **quién escribió encima**, y ésa es justo la pregunta que
// queda cuando el sello aparece intacto: un sello sin consumir significa que el
// contexto se guardó bien y que el vándalo llegó después, mientras su dueño
// estaba descolocado.
//
// Cada stub de entrada talla su área en la pila y la publica. Anotando las
// últimas, el informe puede enseñar si dos áreas se solapan — y de qué tarea es
// cada una. Dos bases separadas por menos de `XSAVE_AREA` en la misma pila es
// la respuesta entera, sin interpretación.
//
// Es un anillo de cuatro y sin lock: se escribe desde el despachador de cada
// trap, con las interrupciones ya apagadas, y se lee sólo desde un informe de
// fallo terminal. Un dato de diagnóstico que se pierda no rompe nada; uno que
// se cuelgue tomando un lock dentro de un manejador de fallos, sí.

pub const PUBLICACIONES: usize = 4;
static mut PUB_BASE: [u64; PUBLICACIONES] = [0; PUBLICACIONES];
static mut PUB_TID: [u32; PUBLICACIONES] = [0; PUBLICACIONES];
static mut PUB_N: usize = 0;

/// `XSTATE_BV` tal y como estaba al ENTRAR en el despachador.
///
/// Parte en dos la ventana entre el `xsave64` del prólogo y la guardia del
/// epílogo, que es donde ahora se sabe que ocurre la corrupción:
///
/// - si esto ya viene podrido, el vándalo está en las cuatro instrucciones
///   entre el `xsave64` y el `call` — o sea, el `xsave64` no escribió lo que
///   creemos, o la base no es la que creemos;
/// - si viene sano y el epílogo lo encuentra roto, el vándalo está DENTRO del
///   despachador, y hay que mirar qué toca el planificador y el servicio de
///   estuarios.
///
/// Una comparación en el informe que descarta la mitad del código, sea cual sea
/// el resultado. Es más barato que seguir razonando sobre el volcado.
static mut CAB_AL_ENTRAR: u64 = 0;

/// La cabecera y la base leídas **por el propio stub, en la instrucción
/// siguiente al `xsave64`**.
///
/// Última capa de indirección que queda por quitar. `bv0` ya demostró que la
/// cabecera viene podrida al entrar al despachador, pero para llegar a ese dato
/// se pasa por `percpu::trap_rsp()` y por el `rax` que devuelve el
/// despachador — dos sitios donde una dirección podría no ser la que creemos.
///
/// Esto lo lee el ensamblador del propio `rsp` que acaba de usar `xsave64`, sin
/// nadie en medio. Con las dos juntas la pregunta queda contestada sin
/// interpretación posible:
///
/// - `bv_x` con bits fuera de `XCR0` y `base_x` == el área del informe → el
///   `xsave64` NO está escribiendo la cabecera donde creemos. El problema es la
///   instrucción o la base, no el planificador.
/// - `base_x` distinta del área del informe → hay dos direcciones en juego y el
///   contexto que se guarda no es el que se restaura.
///
/// `rax`/`rdx` están muertos ahí (valían -1 para el RFBM y los pops los
/// recuperan), así que se pueden usar sin salvar nada.
#[unsafe(no_mangle)]
pub static mut BV_TRAS_XSAVE: u64 = 0;
#[unsafe(no_mangle)]
pub static mut BASE_TRAS_XSAVE: u64 = 0;

pub fn tras_xsave() -> (u64, u64) {
    unsafe {
        (
            core::ptr::addr_of!(BV_TRAS_XSAVE).read_volatile(),
            core::ptr::addr_of!(BASE_TRAS_XSAVE).read_volatile(),
        )
    }
}

pub fn cabecera_al_entrar() -> u64 {
    unsafe { core::ptr::addr_of!(CAB_AL_ENTRAR).read_volatile() }
}

/// Anota que este trap talló su área en `base`, para la tarea `tid`, y toma la
/// foto de la cabecera antes de que el despachador haga nada.
pub fn registrar_publicacion(base: u64, tid: u32) {
    if base == 0 {
        return;
    }
    unsafe {
        let i = PUB_N % PUBLICACIONES;
        core::ptr::addr_of_mut!(PUB_BASE).cast::<u64>().add(i).write_volatile(base);
        core::ptr::addr_of_mut!(PUB_TID).cast::<u32>().add(i).write_volatile(tid);
        PUB_N = PUB_N.wrapping_add(1);
        let bv = ((base + XSAVE_BV as u64) as *const u64).read_volatile();
        core::ptr::addr_of_mut!(CAB_AL_ENTRAR).write_volatile(bv);
    }
}

/// Las últimas áreas publicadas, de la más reciente a la más antigua.
pub fn publicaciones() -> [(u64, u32); PUBLICACIONES] {
    let mut r = [(0u64, 0u32); PUBLICACIONES];
    unsafe {
        for k in 0..PUBLICACIONES {
            // `PUB_N - 1` es la última escrita; se va hacia atrás.
            let i = PUB_N.wrapping_sub(1).wrapping_sub(k) % PUBLICACIONES;
            r[k] = (
                core::ptr::addr_of!(PUB_BASE).cast::<u64>().add(i).read_volatile(),
                core::ptr::addr_of!(PUB_TID).cast::<u32>().add(i).read_volatile(),
            );
        }
    }
    r
}

/// GPR block (15*8) + back-pointer slot (8) + alignment slack (8).
const FRAME_BYTES_BELOW_TAIL: usize = 15 * 8 + 8 + 8;
/// Bytes consumed from the stack top: tail (40) + GPRs + back-ptr + xsave
/// + alignment slack.
const CONTEXT_BYTES: usize = 40 + FRAME_BYTES_BELOW_TAIL + XSAVE_AREA + 64;

/// Fabricate the initial context of a task on its own kernel stack.
/// Returns the `fxsave_base` that becomes the task's `context_rsp`.
///
/// `user` selects a Ring 3 frame (iretq switches stack + CPL); kernel tasks
/// get a same-privilege frame whose post-iretq RSP lands 8 mod 16, matching
/// the SysV post-`call` alignment Rust code expects.
pub unsafe fn fabricate(
    stack_top: u64,
    entry: u64,
    arg: u64,
    user: bool,
    user_rsp: u64,
) -> u64 {
    let gpr_base = (stack_top - 168) & !0xF | 0x8; // ≡ 8 (mod 16)
    // 64-alineado: XSAVE lo exige y da #GP si no.
    let xsave_base = (gpr_base - (XSAVE_AREA as u64 + 8)) & !63;

    core::ptr::write_bytes(xsave_base as *mut u8, 0, XSAVE_AREA);
    // Cabecera XSAVE (offset 512): XSTATE_BV = 0 significa "todos los
    // componentes en estado INICIAL", que es exactamente lo que quiere una
    // tarea nueva — XRSTOR los pone a sus valores por defecto sin leer nada
    // más. XCOMP_BV = 0 declara formato estándar, que es el que usan los
    // stubs (`xsave64`, no `xsavec64`).
    //
    // MXCSR sí hay que ponerlo aunque XSTATE_BV sea 0: XRSTOR lo carga del
    // área igualmente y da #GP si tiene bits reservados. Un área a ceros
    // dejaría MXCSR = 0, o sea TODAS las excepciones de coma flotante
    // desenmascaradas — la tarea moriría en la primera división inexacta.
    (xsave_base as *mut u16).write_volatile(0x037F); // FCW
    ((xsave_base + 24) as *mut u32).write_volatile(0x1F80); // MXCSR
    ((xsave_base + XSAVE_AREA as u64) as *mut u64).write_volatile(gpr_base); // back-ptr
    sellar(xsave_base, 0);

    let frame = &mut *(gpr_base as *mut TrapFrame);
    core::ptr::write_bytes(frame as *mut TrapFrame as *mut u8, 0, core::mem::size_of::<TrapFrame>());
    frame.rdi = arg;
    frame.rip = entry;
    frame.rflags = RFLAGS_IF | 0x2;
    if user {
        frame.cs = USER_CS;
        frame.ss = USER_SS;
        frame.rsp = user_rsp;
    } else {
        frame.cs = KERNEL_CS;
        frame.ss = KERNEL_SS;
        frame.rsp = gpr_base + 144; // informational (same-CPL iretq ignores it)
    }
    xsave_base
}

/// Minimum kernel stack size that fits one fabricated context plus working
/// room for the task's own frames.
pub const MIN_TASK_STACK: usize = CONTEXT_BYTES + 4096;

/// Pone el sello en un contexto: firma + tid del dueño.
///
/// Lo llama `fabricate` al crear el contexto y el planificador cada vez que
/// guarda uno saliente. Los stubs ponen la firma en ensamblador nada más
/// publicar el contexto; el dueño lo pone Rust, que es quien sabe de tids.
pub fn sellar(xsave_base: u64, tid: u32) {
    if xsave_base == 0 {
        return;
    }
    unsafe {
        ((xsave_base + SELLO_FIRMA as u64) as *mut u64).write_volatile(SELLO_MAGIA);
        ((xsave_base + SELLO_DUENO as u64) as *mut u64).write_volatile(tid as u64);
    }
}

/// `(firma, dueño)` tal y como están AHORA en un contexto. Para el reporter.
pub fn leer_sello(xsave_base: u64) -> (u64, u64) {
    if xsave_base == 0 {
        return (0, 0);
    }
    unsafe {
        (
            ((xsave_base + SELLO_FIRMA as u64) as *const u64).read_volatile(),
            ((xsave_base + SELLO_DUENO as u64) as *const u64).read_volatile(),
        )
    }
}
