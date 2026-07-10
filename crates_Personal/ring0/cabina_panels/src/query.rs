use cabina_core::{Event, Severity, Layer, Entity};

#[derive(Clone, Debug, Default)]
pub struct Query {
    pub layers: alloc::vec::Vec<Layer>,
    pub entities: alloc::vec::Vec<Entity>,
    pub severities: alloc::vec::Vec<Severity>,
    pub modules: alloc::vec::Vec<alloc::string::String>,
    pub entity_id_min: Option<u32>,
    pub entity_id_max: Option<u32>,
    pub msg_contains: Option<alloc::string::String>,
    pub min_seq: Option<u64>,
    pub max_seq: Option<u64>,
    pub last_ms: Option<u64>,
    pub since_last_fault: bool,
    pub only_new: bool,
    pub only_repeated: bool,
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

    pub fn matches(&self, ev: &Event) -> bool {
        if !self.layers.is_empty() && !self.layers.contains(&ev.layer) {
            return false;
        }
        if !self.entities.is_empty() && !self.entities.contains(&ev.entity) {
            return false;
        }
        if !self.severities.is_empty() && !self.severities.contains(&ev.severity) {
            return false;
        }
        if !self.modules.is_empty() {
            let ev_mod = ev.module_str();
            if !self.modules.iter().any(|m| m == ev_mod) {
                return false;
            }
        }
        if let Some(min) = self.entity_id_min {
            if ev.entity_id < min {
                return false;
            }
        }
        if let Some(max) = self.entity_id_max {
            if ev.entity_id > max {
                return false;
            }
        }
        if let Some(ref s) = self.msg_contains {
            let ev_msg = ev.msg_str();
            if !ev_msg.contains(s.as_str()) {
                return false;
            }
        }
        if let Some(min_seq) = self.min_seq {
            if ev.seq < min_seq {
                return false;
            }
        }
        if let Some(max_seq) = self.max_seq {
            if ev.seq > max_seq {
                return false;
            }
        }
        true
    }

    pub fn apply(&self, events: &[Event]) -> alloc::vec::Vec<Event> {
        let mut out: alloc::vec::Vec<Event> =
            events.iter().filter(|e| self.matches(e)).cloned().collect();
        if let Some(limit) = self.limit {
            out.truncate(limit);
        }
        out
    }

    pub fn only_errors() -> Self {
        Self {
            severities: alloc::vec![Severity::Fault, Severity::Panic],
            ..Self::default()
        }
    }

    pub fn only_critical() -> Self {
        Self {
            severities: alloc::vec![Severity::Warning, Severity::Fault, Severity::Panic],
            ..Self::default()
        }
    }

    pub fn layer(l: Layer) -> Self {
        Self {
            layers: alloc::vec![l],
            ..Self::default()
        }
    }

    pub fn kernel() -> Self {
        Self {
            layers: alloc::vec![Layer::Ring0, Layer::BmoCore],
            ..Self::default()
        }
    }

    pub fn process(pid: u32) -> Self {
        Self {
            entities: alloc::vec![Entity::Process],
            entity_id_min: Some(pid),
            entity_id_max: Some(pid),
            ..Self::default()
        }
    }

    pub fn thread(tid: u32) -> Self {
        Self {
            entities: alloc::vec![Entity::Thread],
            entity_id_min: Some(tid),
            entity_id_max: Some(tid),
            ..Self::default()
        }
    }

    pub fn syscall(nr: u32) -> Self {
        Self {
            entities: alloc::vec![Entity::Syscall],
            entity_id_min: Some(nr),
            entity_id_max: Some(nr),
            ..Self::default()
        }
    }

    pub fn file(inode: u32) -> Self {
        Self {
            entities: alloc::vec![Entity::File],
            entity_id_min: Some(inode),
            entity_id_max: Some(inode),
            ..Self::default()
        }
    }

    pub fn search(text: &str) -> Self {
        Self {
            msg_contains: Some(alloc::string::String::from(text)),
            ..Self::default()
        }
    }

    pub fn with_limit(mut self, n: usize) -> Self {
        self.limit = Some(n);
        self
    }

    pub fn module(name: &str) -> Self {
        Self {
            modules: alloc::vec![alloc::string::String::from(name)],
            ..Self::default()
        }
    }

    pub fn with_layers(mut self, ls: alloc::vec::Vec<Layer>) -> Self {
        self.layers = ls;
        self
    }

    pub fn with_severities(mut self, ss: alloc::vec::Vec<Severity>) -> Self {
        self.severities = ss;
        self
    }

    pub fn before_panic(seq: u64) -> Self {
        Self {
            max_seq: Some(seq),
            severities: alloc::vec![Severity::Info, Severity::Trace, Severity::Warning],
            ..Self::default()
        }
    }

    pub fn memory_growing() -> Self {
        Self {
            layers: alloc::vec![Layer::Ring0],
            severities: alloc::vec![Severity::Fault],
            ..Self::default()
        }
    }

    pub fn slow_syscalls() -> Self {
        Self {
            entities: alloc::vec![Entity::Syscall],
            min_seq: Some(0),
            ..Self::default()
        }
    }
}

pub fn apply(events: &[Event], q: &Query) -> alloc::vec::Vec<Event> {
    q.apply(events)
}

pub fn count(events: &[Event], q: &Query) -> usize {
    events.iter().filter(|e| q.matches(e)).count()
}

#[cfg(test)]
mod tests {
    use super::*;
    use cabina_core::{Event, Layer, Entity, Severity};

    fn make_event(severity: Severity, layer: Layer, entity: Entity, module: &str, id: u32, msg: &str, seq: u64, value: u64) -> Event {
        let mut ev = Event::new(severity, layer, entity, module, id, msg, value);
        ev.seq = seq;
        ev
    }

    #[test]
    fn only_errors_filters_info() {
        let q = Query::only_errors();
        let info = make_event(Severity::Info, Layer::Ring0, Entity::Module, "test", 0, "ok", 1, 0);
        let fault = make_event(Severity::Fault, Layer::Ring0, Entity::Module, "test", 0, "fail", 2, 0);
        assert!(!q.matches(&info));
        assert!(q.matches(&fault));
    }

    #[test]
    fn only_critical_includes_warning() {
        let q = Query::only_critical();
        let warn = make_event(Severity::Warning, Layer::Ring0, Entity::Module, "test", 0, "warn", 1, 0);
        let trace = make_event(Severity::Trace, Layer::Ring0, Entity::Module, "test", 0, "trace", 2, 0);
        assert!(q.matches(&warn));
        assert!(!q.matches(&trace));
    }

    #[test]
    fn layer_filter() {
        let q = Query::layer(Layer::Ring3);
        let r0 = make_event(Severity::Info, Layer::Ring0, Entity::Module, "test", 0, "r0", 1, 0);
        let r3 = make_event(Severity::Info, Layer::Ring3, Entity::Module, "test", 0, "r3", 2, 0);
        assert!(!q.matches(&r0));
        assert!(q.matches(&r3));
    }

    #[test]
    fn entity_filter() {
        let q = Query::syscall(42);
        let sc = make_event(Severity::Info, Layer::Ring0, Entity::Syscall, "sys", 42, "ok", 1, 0);
        let other = make_event(Severity::Info, Layer::Ring0, Entity::Module, "test", 0, "no", 2, 0);
        assert!(q.matches(&sc));
        assert!(!q.matches(&other));
    }

    #[test]
    fn search_text() {
        let q = Query::search("oom");
        let ev = make_event(Severity::Fault, Layer::Ring0, Entity::Module, "mem", 0, "out of memory: oom_killer", 1, 0);
        assert!(q.matches(&ev));
        let ev2 = make_event(Severity::Info, Layer::Ring0, Entity::Module, "cpu", 0, "all good", 2, 0);
        assert!(!q.matches(&ev2));
    }

    #[test]
    fn seq_range() {
        let q = Query { min_seq: Some(5), max_seq: Some(10), ..Query::default() };
        let ev1 = make_event(Severity::Info, Layer::Ring0, Entity::Module, "t", 0, "x", 3, 0);
        let ev2 = make_event(Severity::Info, Layer::Ring0, Entity::Module, "t", 0, "x", 7, 0);
        let ev3 = make_event(Severity::Info, Layer::Ring0, Entity::Module, "t", 0, "x", 11, 0);
        assert!(!q.matches(&ev1));
        assert!(q.matches(&ev2));
        assert!(!q.matches(&ev3));
    }

    #[test]
    fn apply_limits_results() {
        let q = Query { limit: Some(2), ..Query::default() };
        let events = alloc::vec![
            make_event(Severity::Info, Layer::Ring0, Entity::Module, "a", 0, "1", 1, 0),
            make_event(Severity::Info, Layer::Ring0, Entity::Module, "b", 0, "2", 2, 0),
            make_event(Severity::Info, Layer::Ring0, Entity::Module, "c", 0, "3", 3, 0),
        ];
        let out = q.apply(&events);
        assert_eq!(out.len(), 2);
    }

    #[test]
    fn parse_errores() {
        let q = parse("errores").unwrap();
        assert_eq!(q.severities, alloc::vec![Severity::Fault, Severity::Panic]);
    }

    #[test]
    fn parse_ring0() {
        let q = parse("ring0").unwrap();
        assert_eq!(q.layers, alloc::vec![Layer::Ring0]);
    }

    #[test]
    fn parse_proceso() {
        let q = parse("proceso 7").unwrap();
        assert_eq!(q.entity_id_min, Some(7));
    }

    #[test]
    fn parse_unknown_returns_none() {
        assert!(parse("xyzzy").is_none());
    }

    #[test]
    fn matches_with_module_filter() {
        let mut q = Query::default();
        q.modules = alloc::vec!["fs".into()];
        let ev = make_event(Severity::Info, Layer::Ring0, Entity::Module, "fs", 0, "file opened", 1, 0);
        let ev2 = make_event(Severity::Info, Layer::Ring0, Entity::Module, "net", 0, "packet", 2, 0);
        assert!(q.matches(&ev));
        assert!(!q.matches(&ev2));
    }
}

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
    if let Some(rest) = low.strip_prefix("panic ") {
        if let Ok(n) = rest.trim().parse::<u64>() {
            return Some(Query::before_panic(n));
        }
    }
    for prefix in &["proceso ", "process ", "proc "] {
        if let Some(rest) = low.strip_prefix(prefix) {
            if let Ok(pid) = rest.trim().parse::<u32>() {
                return Some(Query::process(pid));
            }
        }
    }
    if let Some(rest) = low.strip_prefix("thread ") {
        if let Ok(tid) = rest.trim().parse::<u32>() {
            return Some(Query::thread(tid));
        }
    }
    if let Some(rest) = low.strip_prefix("syscall ") {
        if let Ok(nr) = rest.trim().parse::<u32>() {
            return Some(Query::syscall(nr));
        }
    }
    None
}
