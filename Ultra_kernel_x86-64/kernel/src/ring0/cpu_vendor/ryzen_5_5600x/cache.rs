//! Cache topology for the Ryzen 5 5600X: lo ESPERADO y lo MEDIDO.
//!
//! [carril]  VERDE     topologia de cache, en una tabla y en una hoja de CPUID
//! [consumo] NADA      pregunta al silicio en el arranque, o es contrato
//!
//! # Las dos mitades (LEY 24: el hardware se PERFILA)
//!
//! ```text
//!    esperado_5600x()   lo que dice la HOJA DE DATOS de AMD para Zen 3.
//!                       Es una suposicion escrita; `PERFIL/CPU.txt` la repite
//!                       y `perfil-campos` exige que digan lo mismo
//!    medir()            lo que CONTESTA el silicio: CPUID 0x8000001D, una
//!                       subhoja por cache. `None` si la hoja no existe
//! ```
//!
//! # [!] 2026-09-19: hasta hoy esto era SOLO la primera mitad
//!
//! Se llamaba `detect_5600x()` y no detectaba nada: devolvia la tabla escrita
//! a mano, y el arranque imprimia `cache: L1d 32K L1i 32K L2 512K L3 32M` como
//! si lo hubiera preguntado. Y la tabla tenia un dato MAL: L1 y L2 compartidas
//! por UN hilo. El 5600X tiene SMT -- 6 nucleos, 12 hilos -- y los dos hilos de
//! un nucleo comparten su L1 y su L2. Es justo el dato del que depende saber si
//! dos hilos se pisan una linea.
//!
//! 5600X (Vermeer), segun AMD:
//!   L1d:  32 KiB,  8 vias, linea de 64 B, por nucleo (2 hilos)
//!   L1i:  32 KiB,  8 vias, linea de 64 B, por nucleo (2 hilos)
//!   L2:  512 KiB,  8 vias, linea de 64 B, por nucleo (2 hilos)
//!   L3:   32 MiB, 16 vias, linea de 64 B, compartida por los 12 hilos (1 CCX)

use super::cpuid::cpuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CacheType { Data, Instruction, Unified, Unknown }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CacheInfo {
    pub level: u8,
    pub size_kb: u32,
    pub line_size_bytes: u8,
    /// `0` = totalmente asociativa.
    pub associativity: u8,
    pub shared_threads: u8,
    pub cache_type: CacheType,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CacheTopology {
    pub l1d: Option<CacheInfo>,
    pub l1i: Option<CacheInfo>,
    pub l2:  Option<CacheInfo>,
    pub l3:  Option<CacheInfo>,
}

const fn fila(level: u8, size_kb: u32, associativity: u8, shared_threads: u8, cache_type: CacheType) -> CacheInfo {
    CacheInfo { level, size_kb, line_size_bytes: 64, associativity, shared_threads, cache_type }
}

/// **Lo ESPERADO**: la hoja de datos de AMD. `PERFIL/CPU.txt` tiene la misma
/// tabla y `toolchain/tools/perfil-campos` compara las dos.
pub const fn esperado_5600x() -> CacheTopology {
    CacheTopology {
        l1d: Some(fila(1, 32, 8, 2, CacheType::Data)),
        l1i: Some(fila(1, 32, 8, 2, CacheType::Instruction)),
        l2:  Some(fila(2, 512, 8, 2, CacheType::Unified)),
        l3:  Some(fila(3, 32 * 1024, 16, 12, CacheType::Unified)),
    }
}

/// **Lo MEDIDO**: CPUID 0x8000001D, "Cache Topology Information" (AMD APM
/// vol. 3). Una subhoja por cache hasta que el tipo es 0.
///
/// ```text
///    EAX[4:0]    tipo (0 fin, 1 datos, 2 instrucciones, 3 unificada)
///    EAX[7:5]    nivel
///    EAX[9]      totalmente asociativa
///    EAX[25:14]  hilos que la comparten, menos uno
///    EBX[11:0]   linea en bytes, menos uno
///    EBX[21:12]  particiones fisicas, menos uno
///    EBX[31:22]  vias, menos uno
///    ECX         conjuntos, menos uno
///    tamano = vias * particiones * linea * conjuntos
/// ```
///
/// `None` si la hoja no existe: hace falta `TopologyExtensions`
/// (CPUID 0x80000001 ECX[22]) y que 0x80000000 llegue a 0x8000001D. Sin ella
/// NO se rellena con lo esperado -- un hueco dicho es mejor que una suposicion
/// que parece una medida.
pub fn medir() -> Option<CacheTopology> {
    let (max_ext, _, _, _) = cpuid(0x8000_0000, 0);
    if max_ext < 0x8000_001D {
        return None;
    }
    let (_, _, ecx, _) = cpuid(0x8000_0001, 0);
    if ecx & (1 << 22) == 0 {
        return None;
    }
    let mut t = CacheTopology { l1d: None, l1i: None, l2: None, l3: None };
    let mut sub = 0u32;
    while sub < 8 {
        let (eax, ebx, ecx, _) = cpuid(0x8000_001D, sub);
        let Some(info) = decodificar(eax, ebx, ecx) else { break };
        match (info.level, info.cache_type) {
            (1, CacheType::Data) => t.l1d = Some(info),
            (1, CacheType::Instruction) => t.l1i = Some(info),
            (2, _) => t.l2 = Some(info),
            (3, _) => t.l3 = Some(info),
            _ => {}
        }
        sub += 1;
    }
    Some(t)
}

/// Una subhoja de 0x8000001D. `None` = tipo 0, se acabo la lista.
pub const fn decodificar(eax: u32, ebx: u32, ecx: u32) -> Option<CacheInfo> {
    let cache_type = match eax & 0x1F {
        0 => return None,
        1 => CacheType::Data,
        2 => CacheType::Instruction,
        3 => CacheType::Unified,
        _ => CacheType::Unknown,
    };
    let level = ((eax >> 5) & 0x7) as u8;
    let total_asoc = eax & (1 << 9) != 0;
    let hilos = ((eax >> 14) & 0xFFF) + 1;
    let linea = (ebx & 0xFFF) + 1;
    let particiones = ((ebx >> 12) & 0x3FF) + 1;
    let vias = ((ebx >> 22) & 0x3FF) + 1;
    let conjuntos = ecx as u64 + 1;
    let bytes = vias as u64 * particiones as u64 * linea as u64 * conjuntos;
    Some(CacheInfo {
        level,
        size_kb: (bytes / 1024) as u32,
        line_size_bytes: if linea > 255 { 255 } else { linea as u8 },
        associativity: if total_asoc { 0 } else if vias > 255 { 255 } else { vias as u8 },
        shared_threads: if hilos > 255 { 255 } else { hilos as u8 },
        cache_type,
    })
}

/// **Lo que viaja a Ring 3** (`INFO_CPU_CACHE_*`): una cache en 64 bits.
///
/// ```text
///    bits  0..23   tamano en KiB
///    bits 24..31   linea en bytes
///    bits 32..39   vias (0 = totalmente asociativa)
///    bits 40..47   hilos que la comparten
///    bit  62       NO coincide con lo esperado
///    bit  63       medida (si es 0, el campo entero es 0: no se pudo medir)
/// ```
pub const fn empaquetar(medida: Option<CacheInfo>, esperada: Option<CacheInfo>) -> u64 {
    let Some(m) = medida else { return 0 };
    let difiere = match esperada {
        Some(e) => !(e.size_kb == m.size_kb
            && e.line_size_bytes == m.line_size_bytes
            && e.associativity == m.associativity
            && e.shared_threads == m.shared_threads),
        None => true,
    };
    (m.size_kb as u64 & 0xFF_FFFF)
        | ((m.line_size_bytes as u64) << 24)
        | ((m.associativity as u64) << 32)
        | ((m.shared_threads as u64) << 40)
        | ((difiere as u64) << 62)
        | (1u64 << 63)
}
