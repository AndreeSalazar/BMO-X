//! `cabina::query` — DSL de filtros inteligentes.
//!
//! Permite a las apps (Cabina, Ring 3, scripts) filtrar el log con
//! un lenguaje declarativo. Ejemplos:
//!
//! ```text
//! Cabina > solo errores (Fault + Panic)
//! Cabina > ring0 + ring3
//! Cabina > ultimos 5 segundos
//! Cabina > proceso 7
//! Cabina > syscalls lentas (> 100 µs)
//! Cabina > memoria creciendo (heap_used diff > 0)
//! Cabina > eventos antes de panic #42
//! Cabina > solo Warn + Fault
//! Cabina > solo nuevos (seq > last_seen)
//! ```
//!
//! Los filtros se pueden combinar con AND/OR y se pueden guardar como
//! presets (ver `Preset`).

#![allow(dead_code)]

use crate::cabina::event::{Event, Layer, Entity, Severity};
use crate::cabina::telemetry;

/// Un filtro de eventos.
///
/// Cada campo es opcional. Un campo None significa "no filtrar por esto".
/// Los campos se combinan con AND (todos deben pasar).
#[derive(Clone, Debug, Default)]
pub struct Query {
    /// Solo eventos de estas capas (vacío = todas).
    pub layers: alloc::vec::Vec<Layer>,
    /// Solo eventos de estas entidades (vacío = todas).
    pub entities: alloc::vec::Vec<Entity>,
    /// Solo estas severidades (vacío = todas).
    pub severities: alloc::vec::Vec<Severity>,
    /// Solo módulos en esta lista (vacío = todos).
    pub modules: alloc::vec::Vec<alloc::string::String>,
    /// Solo entity_id en este rango (None = todos).
    pub entity_id_min: Option<u32>,
    pub entity_id_max: Option<u32>,
    /// Solo mensajes que contengan este substring.
    pub msg_contains: Option<alloc::string::String>,
    /// Solo eventos con seq >= este.
    pub min_seq: Option<u64>,
    /// Solo eventos con seq <= este.
    pub max_seq: Option<u64>,
    /// Solo eventos de los últimos N ms (None = sin límite).
    pub last_ms: Option<u64>,
    /// Solo eventos desde la última falla (None = sin límite).
    pub since_last_fault: bool,
    /// Solo eventos nuevos (los que no se han visto antes).
    pub only_new: bool,
    /// Solo eventos repetidos.
    pub only_repeated: bool,
    /// Límite de resultados (None = sin límite).
    pub limit: Option<usize>,
}

impl Query {
    pub const fn new() -> Self {
        Self {
            layers: alloc::vec::Vec::new(),
            entities: alloc::vec::Vec::new(),
            severities: alloc::vec::Vec::new(),
            modules: alloc::vec::Vec::new(),
            entity_id_min: None,
            entity_id_max: None,
            msg_contains: None,
            min_seq: None,
            max_seq: None,
            last_ms: None,
            since_last_fault: false,
            only_new: false,
            only_repeated: false,
            limit: None,
        }
    }

    /// `true` si el evento pasa el filtro.
    pub fn matches(&self, ev: &Event) -> bool {
        if !self.layers.is_empty() && !self.layers.contains(&ev.layer) { return false; }
        if !self.entities.is_empty() && !self.entities.contains(&ev.entity) { return false; }
        if !self.severities.is_empty() && !self.severities.contains(&ev.severity) { return false; }
        if !self.modules.is_empty() && !self.modules.iter().any(|m| m == &ev.module) { return false; }
        if let Some(min) = self.entity_id_min {
            if ev.entity_id < min { return false; }
        }
        if let Some(max) = self.entity_id_max {
            if ev.entity_id > max { return false; }
        }
        if let Some(ref s) = self.msg_contains {
            if !ev.msg.contains(s.as_str()) { return false; }
        }
        if let Some(min_seq) = self.min_seq {
            if ev.seq < min_seq { return false; }
        }
        if let Some(max_seq) = self.max_seq {
            if ev.seq > max_seq { return false; }
        }
        if let Some(last_ms) = self.last_ms {
            let tsc = crate::cpu::rdtsc();
            let freq = crate::cpu::tsc_per_sec();
            let now_ns = if freq == 0 { 0 } else { tsc.wrapping_mul(1_000_000_000) / freq };
            if now_ns.saturating_sub(ev.tick_ns) > last_ms * 1_000_000 {
                return false;
            }
        }
        true
    }

    /// Aplica el filtro a una lista de eventos.
    pub fn apply(&self, events: &[Event]) -> alloc::vec::Vec<Event> {
        let mut out: alloc::vec::Vec<Event> = events.iter().filter(|e| self.matches(e)).cloned().collect();
        if let Some(limit) = self.limit {
            out.truncate(limit);
        }
        out
    }

    // ─── Presets ────────────────────────────────────────────────

    /// Solo errores: Fault + Panic.
    pub fn only_errors() -> Self {
        Self { severities: alloc::vec![Severity::Fault, Severity::Panic], ..Self::default() }
    }

    /// Solo critical: Warning + Fault + Panic.
    pub fn only_critical() -> Self {
        Self {
            severities: alloc::vec![Severity::Warning, Severity::Fault, Severity::Panic],
            ..Self::default()
        }
    }

    /// Solo una capa específica.
    pub fn layer(l: Layer) -> Self {
        Self { layers: alloc::vec![l], ..Self::default() }
    }

    /// Solo múltiples capas.
    pub fn layers_vec(ls: alloc::vec::Vec<Layer>) -> Self {
        Self { layers: ls, ..Self::default() }
    }

    /// Solo Ring 0 + BMO Core.
    pub fn kernel() -> Self {
        Self { layers: alloc::vec![Layer::Ring0, Layer::BmoCore], ..Self::default() }
    }

    /// Solo eventos de un proceso (PID).
    pub fn process(pid: u32) -> Self {
        Self {
            entities: alloc::vec![Entity::Process],
            entity_id_min: Some(pid),
            entity_id_max: Some(pid),
            ..Self::default()
        }
    }

    /// Solo eventos de un thread (TID).
    pub fn thread(tid: u32) -> Self {
        Self {
            entities: alloc::vec![Entity::Thread],
            entity_id_min: Some(tid),
            entity_id_max: Some(tid),
            ..Self::default()
        }
    }

    /// Solo eventos de un syscall específico.
    pub fn syscall(nr: u32) -> Self {
        Self {
            entities: alloc::vec![Entity::Syscall],
            entity_id_min: Some(nr),
            entity_id_max: Some(nr),
            ..Self::default()
        }
    }

    /// Solo eventos de un archivo (inode).
    pub fn file(inode: u32) -> Self {
        Self {
            entities: alloc::vec![Entity::File],
            entity_id_min: Some(inode),
            entity_id_max: Some(inode),
            ..Self::default()
        }
    }

    /// Solo eventos de los últimos N ms.
    pub fn last_n_ms(n: u64) -> Self {
        Self { last_ms: Some(n), ..Self::default() }
    }

    /// Solo eventos desde un seq específico.
    pub fn since_seq(seq: u64) -> Self {
        Self { min_seq: Some(seq), ..Self::default() }
    }

    /// Solo eventos entre seq_min y seq_max (inclusive).
    pub fn seq_range(min: u64, max: u64) -> Self {
        Self { min_seq: Some(min), max_seq: Some(max), ..Self::default() }
    }

    /// Búsqueda por texto en el mensaje.
    pub fn search(text: &str) -> Self {
        Self { msg_contains: Some(alloc::string::String::from(text)), ..Self::default() }
    }

    /// Limita a N resultados.
    pub fn with_limit(mut self, n: usize) -> Self {
        self.limit = Some(n);
        self
    }

    /// Solo eventos de un módulo.
    pub fn module(name: &str) -> Self {
        Self { modules: alloc::vec![alloc::string::String::from(name)], ..Self::default() }
    }

    /// Builder: agregar layers.
    pub fn with_layers(mut self, ls: alloc::vec::Vec<Layer>) -> Self {
        self.layers = ls;
        self
    }

    /// Builder: agregar severities.
    pub fn with_severities(mut self, ss: alloc::vec::Vec<Severity>) -> Self {
        self.severities = ss;
        self
    }

    // ─── Presets "inteligentes" (Opus) ─────────────────────────

    /// Solo errores nuevos (los que han aparecido desde la última vez que
    /// se consultó el filtro).
    pub fn only_new() -> Self {
        Self { only_new: true, ..Self::default() }
    }

    /// Solo eventos repetidos (mismo msg 2+ veces).
    pub fn only_repeated() -> Self {
        Self { only_repeated: true, ..Self::default() }
    }

    /// Detecta "memoria creciendo": faults GP en ring0 con heap_used
    /// en aumento. v1.8.8: simplificación, solo filtra GP faults.
    pub fn memory_growing() -> Self {
        Self {
            layers: alloc::vec![Layer::Ring0],
            severities: alloc::vec![Severity::Fault],
            ..Self::default()
        }
    }

    /// Detecta "syscalls lentas": placeholders para futuro
    /// (cuando tengamos timestamps por syscall).
    pub fn slow_syscalls() -> Self {
        Self {
            entities: alloc::vec![Entity::Syscall],
            min_seq: Some(0),
            ..Self::default()
        }
    }

    /// "Eventos antes de panic #N": todos los eventos con seq < N
    /// y severidad <= Warning.
    pub fn before_panic(seq: u64) -> Self {
        Self {
            max_seq: Some(seq),
            severities: alloc::vec![Severity::Info, Severity::Trace, Severity::Warning],
            ..Self::default()
        }
    }
}

/// Aplica un query a una lista de eventos.
pub fn apply(events: &[Event], q: &Query) -> alloc::vec::Vec<Event> {
    q.apply(events)
}

/// Cuenta eventos que matchean el query.
pub fn count(events: &[Event], q: &Query) -> usize {
    events.iter().filter(|e| q.matches(e)).count()
}

/// Devuelve un preset "inteligente" segun palabras clave.
///
/// v1.8.8: simplificación. Reconoce:
/// - "errores" / "fault" / "panic" → only_errors
/// - "warn" / "warning" / "critical" → only_critical
/// - "ring0" / "kernel" → layer(Ring0)
/// - "ring3" / "user" → layer(Ring3)
/// - "bmo_core" / "core" → layer(BmoCore)
/// - "bmo_gpu" / "gpu" → layer(BmoGpu)
/// - "lang" → layer(Lang)
/// - "fs" / "filesystem" → layer(Fs)
/// - "net" / "network" → layer(Net)
/// - "panic <N>" → before_panic(N)
/// - "proc <PID>" / "proceso <PID>" → process(PID)
/// - "thread <TID>" → thread(TID)
/// - "ultimos <N> ms" → last_n_ms(N)
/// - "syscall <NR>" → syscall(NR)
pub fn parse(s: &str) -> Option<Query> {
    let s = s.trim();
    let low = s.to_ascii_lowercase();
    if low.contains("errores") || low.contains("fault") || low.contains("panic") {
        return Some(Query::only_errors());
    }
    if low.contains("warn") || low.contains("critical") {
        return Some(Query::only_critical());
    }
    if low.starts_with("ring0") || low.starts_with("kernel") {
        return Some(Query::layer(Layer::Ring0));
    }
    if low.starts_with("ring3") || low.starts_with("user") {
        return Some(Query::layer(Layer::Ring3));
    }
    if low.starts_with("bmo_core") || low.starts_with("core") {
        return Some(Query::layer(Layer::BmoCore));
    }
    if low.starts_with("bmo_gpu") || low.starts_with("gpu") {
        return Some(Query::layer(Layer::BmoGpu));
    }
    if low.starts_with("lang") {
        return Some(Query::layer(Layer::Lang));
    }
    if low.starts_with("fs") || low.contains("filesystem") {
        return Some(Query::layer(Layer::Fs));
    }
    if low.starts_with("net") || low.contains("network") {
        return Some(Query::layer(Layer::Net));
    }
    // Parsear "panic <N>"
    if let Some(rest) = low.strip_prefix("panic ") {
        if let Ok(n) = rest.trim().parse::<u64>() {
            return Some(Query::before_panic(n));
        }
    }
    // Parsear "proceso <PID>" / "process <PID>"
    for prefix in &["proceso ", "process ", "proc "] {
        if let Some(rest) = low.strip_prefix(prefix) {
            if let Ok(pid) = rest.trim().parse::<u32>() {
                return Some(Query::process(pid));
            }
        }
    }
    // Parsear "thread <TID>"
    if let Some(rest) = low.strip_prefix("thread ") {
        if let Ok(tid) = rest.trim().parse::<u32>() {
            return Some(Query::thread(tid));
        }
    }
    // Parsear "ultimos <N> ms" / "últimos <N> ms"
    for prefix in &["ultimos ", "últimos ", "last "] {
        if let Some(rest) = low.strip_prefix(prefix) {
            if let Some(ms_str) = rest.strip_suffix(" ms") {
                if let Ok(n) = ms_str.trim().parse::<u64>() {
                    return Some(Query::last_n_ms(n));
                }
            }
        }
    }
    // Parsear "syscall <NR>"
    if let Some(rest) = low.strip_prefix("syscall ") {
        if let Ok(nr) = rest.trim().parse::<u32>() {
            return Some(Query::syscall(nr));
        }
    }
    None
}
