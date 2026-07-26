//! El estado extendido del CPU: qué hay, cuánto ocupa y si el perfil acertó.
//!
//! ## Por qué existe este módulo
//!
//! Hoy el cambio de contexto usa `FXSAVE`, que guarda 512 bytes: x87 y SSE.
//! Un programa Ring 3 que use **AVX** tiene la mitad alta de sus registros
//! `YMM` fuera de esa foto — se pierde en el primer cambio de tarea, sin
//! fault, sin aviso y sin forma de notarlo salvo por un resultado que no
//! cuadra. Es corrupción esperando a ocurrir.
//!
//! ## Lo que este módulo hace y lo que NO
//!
//! **Hace**: preguntarle al procesador qué componentes de estado tiene, cuánto
//! ocupa su área y qué instrucciones de guardado ofrece; y contrastarlo con lo
//! que el perfil del CPU esperaba.
//!
//! **NO hace**: tocar `CR4.OSXSAVE` ni escribir `XCR0`. Y es deliberado:
//! habilitar el estado extendido ANTES de que el cambio de contexto sepa
//! guardarlo haría el problema **peor**, porque AVX pasaría de ser inusable
//! (`#UD`, ruidoso) a ser usable y corromperse en silencio. Primero se mide,
//! después se cambia el guardado, y solo entonces se habilita.
//!
//! ## El perfil es una expectativa, no la verdad
//!
//! Todos los números salen de `CPUID` hoja 0xD. Lo que declara el perfil sirve
//! **solo para avisar** si el silicio no coincide. Un kernel cuyo perfil
//! dictara el tamaño del área sería un kernel que se rompe el día que alguien
//! enchufe otro CPU — y se rompería corrompiendo registros, que es la peor
//! forma de romperse.

/// Componentes de estado que puede haber en XCR0. Los nombres son los de la
/// spec; los huecos son componentes que este kernel no espera ver.
fn nombre_componente(bit: u32) -> &'static str {
    match bit {
        0 => "x87",
        1 => "SSE (XMM)",
        2 => "AVX (YMM alto)",
        3 => "MPX bndregs",
        4 => "MPX bndcsr",
        5 => "AVX-512 opmask",
        6 => "AVX-512 ZMM alto",
        7 => "AVX-512 ZMM 16-31",
        9 => "PKRU",
        11 => "CET usuario",
        12 => "CET supervisor",
        17 => "AMX tilecfg",
        18 => "AMX tiledata",
        _ => "desconocido",
    }
}

pub const MAX_COMPONENTES: usize = 16;

#[derive(Clone, Copy)]
pub struct Componente {
    pub bit: u32,
    pub tam: u32,
    pub offset: u32,
}

impl Componente {
    const VACIO: Componente = Componente { bit: 0, tam: 0, offset: 0 };
    pub fn nombre(&self) -> &'static str { nombre_componente(self.bit) }
}

/// Lo que el procesador declara sobre su estado extendido.
#[derive(Clone, Copy)]
pub struct Informe {
    /// CPUID.1:ECX[26] — el procesador implementa XSAVE.
    pub xsave: bool,
    /// CR4.OSXSAVE — el sistema lo ha habilitado. Hoy: `false`, a propósito.
    pub osxsave: bool,
    /// Máscara de componentes que el CPU soporta (CPUID.D.0:EDX:EAX).
    pub soportado: u64,
    /// XCR0 actual. Solo se puede leer si `osxsave`; si no, vale 0.
    pub xcr0: u64,
    /// Bytes que ocupa el área para el XCR0 ACTUAL (CPUID.D.0:EBX).
    pub area_actual: u32,
    /// Bytes que ocuparía con TODO lo soportado habilitado (CPUID.D.0:ECX).
    pub area_maxima: u32,
    /// Variantes de guardado disponibles (CPUID.D.1:EAX).
    pub xsaveopt: bool,
    pub xsavec: bool,
    pub xsaves: bool,
    pub componentes: [Componente; MAX_COMPONENTES],
    pub n_componentes: usize,
}

impl Informe {
    pub const VACIO: Informe = Informe {
        xsave: false, osxsave: false, soportado: 0, xcr0: 0,
        area_actual: 0, area_maxima: 0,
        xsaveopt: false, xsavec: false, xsaves: false,
        componentes: [Componente::VACIO; MAX_COMPONENTES], n_componentes: 0,
    };

    pub fn comps(&self) -> &[Componente] { &self.componentes[..self.n_componentes] }

    /// ¿Tiene este CPU estado que `FXSAVE` NO guarda?
    ///
    /// Es la pregunta que importa: si la respuesta es sí, hay componentes que
    /// el cambio de contexto está perdiendo hoy.
    pub fn hay_estado_sin_guardar(&self) -> bool {
        // Los bits 0 y 1 (x87 y SSE) son justo lo que FXSAVE cubre.
        self.soportado & !0b11 != 0
    }

    /// El área que haría falta para guardar todo lo que este CPU soporta,
    /// redondeada a 64 bytes (la alineación que exige XSAVE).
    pub fn area_necesaria(&self) -> u32 {
        (self.area_maxima + 63) & !63
    }
}

#[inline]
fn cpuid(hoja: u32, subhoja: u32) -> (u32, u32, u32, u32) {
    let (mut eax, mut ebx, mut ecx, mut edx): (u32, u32, u32, u32);
    unsafe {
        core::arch::asm!(
            // rbx lo reserva LLVM: se salva y se restaura a mano.
            "mov {tmp:r}, rbx",
            "cpuid",
            "mov {ebx:e}, ebx",
            "mov rbx, {tmp:r}",
            tmp = out(reg) _,
            ebx = out(reg) ebx,
            inout("eax") hoja => eax,
            inout("ecx") subhoja => ecx,
            out("edx") edx,
            options(nostack, preserves_flags),
        );
    }
    (eax, ebx, ecx, edx)
}

#[inline]
fn cr4() -> u64 {
    let v: u64;
    unsafe { core::arch::asm!("mov {}, cr4", out(reg) v, options(nomem, nostack, preserves_flags)); }
    v
}

/// `XGETBV(0)`. **Solo se puede llamar con CR4.OSXSAVE puesto**: si no, es
/// `#UD`. Por eso `medir()` lo consulta antes.
#[inline]
unsafe fn xgetbv0() -> u64 {
    let (lo, hi): (u32, u32);
    core::arch::asm!("xgetbv", in("ecx") 0u32, out("eax") lo, out("edx") hi,
        options(nomem, nostack, preserves_flags));
    ((hi as u64) << 32) | lo as u64
}

/// Le pregunta al procesador. No cambia nada.
pub fn medir() -> Informe {
    let mut inf = Informe::VACIO;

    // CPUID.1:ECX[26] = XSAVE implementado. Sin esto, la hoja 0xD no existe y
    // preguntarla devolvería basura.
    let (_, _, ecx1, _) = cpuid(1, 0);
    inf.xsave = ecx1 & (1 << 26) != 0;
    if !inf.xsave { return inf; }
    inf.osxsave = cr4() & (1 << 18) != 0;
    if inf.osxsave {
        inf.xcr0 = unsafe { xgetbv0() };
    }

    let (eax0, ebx0, ecx0, edx0) = cpuid(0x0D, 0);
    inf.soportado = ((edx0 as u64) << 32) | eax0 as u64;
    inf.area_actual = ebx0;
    inf.area_maxima = ecx0;

    let (eax1, _, _, _) = cpuid(0x0D, 1);
    inf.xsaveopt = eax1 & (1 << 0) != 0;
    inf.xsavec = eax1 & (1 << 1) != 0;
    inf.xsaves = eax1 & (1 << 3) != 0;

    // Subhojas 2 en adelante: una por componente. El tamaño y el sitio de cada
    // uno los dice el CPU; no se calculan ni se suponen.
    for bit in 2..64u32 {
        if inf.soportado & (1u64 << bit) == 0 { continue; }
        if inf.n_componentes >= MAX_COMPONENTES { break; }
        let (tam, offset, _, _) = cpuid(0x0D, bit);
        if tam == 0 { continue; }
        inf.componentes[inf.n_componentes] = Componente { bit, tam, offset };
        inf.n_componentes += 1;
    }
    inf
}

/// Qué dijo el contraste entre el silicio y el perfil.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Veredicto {
    /// El CPU no implementa XSAVE. Nada que hacer (ni nada que perder).
    SinXsave,
    /// El silicio coincide con lo que el perfil esperaba.
    Coincide,
    /// El silicio NO coincide. Manda el silicio; el perfil está desfasado.
    Difiere,
}

/// Contrasta el informe con lo que el perfil activo esperaba, y lo narra.
///
/// Cuando llegue otro CPU —cualquiera— esto es lo que avisa. No se rompe: usa
/// lo que declara el silicio y **dice en alto** que el perfil ya no describe
/// la máquina. Un perfil desfasado tiene que ser una línea ámbar en CABINA, no
/// un montón de registros corrompidos tres semanas después.
pub fn verificar(inf: &Informe) -> Veredicto {
    use crate::ring0::cabina;
    if !inf.xsave {
        cabina::info("cpu", "el procesador no implementa XSAVE", 0);
        return Veredicto::SinXsave;
    }
    let p = super::profile::active();
    let mismos = inf.soportado == p.xsave_componentes;
    let misma_area = inf.area_maxima == p.xsave_area;

    if mismos && misma_area {
        cabina::info("cpu", "XSAVE coincide con el perfil del CPU", inf.area_maxima as u64);
        return Veredicto::Coincide;
    }
    if !mismos {
        cabina::warn("cpu", "el perfil esperaba otros componentes de XSAVE", inf.soportado);
    }
    if !misma_area {
        cabina::warn("cpu", "el area de XSAVE no es la que el perfil decia", inf.area_maxima as u64);
    }
    cabina::warn("cpu", "manda el silicio: el perfil esta desfasado", p.xsave_area as u64);
    Veredicto::Difiere
}

static mut INFORME: Informe = Informe::VACIO;
static mut MEDIDO: bool = false;

/// Mide, verifica y **comprueba que el área reservada da de sí**.
///
/// Se llama lo PRIMERO de `phase::main`, antes de percpu, del scheduler y del
/// timer. La razón es dura: los stubs de trap guardan el estado extendido con
/// `xsave64` en el área de tamaño fijo `trap::XSAVE_AREA`, y si este CPU
/// necesitara más, el primer tick del timer escribiría más allá del área y se
/// llevaría por delante la pila de la tarea. Enterarse de eso *después* sería
/// enterarse por una corrupción, no por un mensaje.
pub fn init() {
    let inf = medir();
    unsafe { INFORME = inf; MEDIDO = true; }
    let _ = verificar(&inf);

    if !inf.xsave {
        // Sin XSAVE los stubs no pueden funcionar: `xsave64` daría #UD en el
        // primer trap. Cualquier x86-64 con soporte de 64 bits lo tiene desde
        // hace más de una década, pero suponerlo por escrito es distinto de
        // suponerlo en silencio.
        pararse("este CPU no implementa XSAVE y los stubs lo necesitan", 0);
    }

    // Lo que hace falta AHORA: el área para los componentes que XCR0 tiene
    // habilitados. No la máxima teórica — esa incluye componentes que este
    // CPU soporta pero nadie ha encendido.
    let necesario = inf.area_actual as usize;
    let reservado = crate::ring0::trap::XSAVE_AREA;
    if necesario > reservado {
        pararse("el area de XSAVE reservada se queda corta en este CPU", necesario as u64);
    }
    crate::ring0::cabina::info("cpu", "area de contexto suficiente", necesario as u64);

    if inf.hay_estado_sin_guardar() {
        // Ya no es un aviso de peligro: es la constancia de QUÉ se está
        // guardando de más respecto a lo que guardaba FXSAVE.
        crate::ring0::cabina::info(
            "cpu",
            "estado extendido mas alla de x87/SSE: ahora se preserva",
            inf.soportado & !0b11,
        );
    }
}

/// Se planta con un motivo legible. Mejor una máquina parada que una que
/// corrompe pilas de tarea en cada cambio de contexto.
fn pararse(motivo: &str, valor: u64) -> ! {
    crate::ring0::cabina::panic_ev("cpu", motivo, valor);
    crate::ring0::dev::console::serial_write("[cpu] FATAL: ");
    crate::ring0::dev::console::serial_write(motivo);
    crate::ring0::dev::console::serial_write("\n");
    loop {
        unsafe { core::arch::asm!("cli", "hlt", options(nomem, nostack)); }
    }
}

pub fn informe() -> Informe { unsafe { if MEDIDO { INFORME } else { Informe::VACIO } } }
