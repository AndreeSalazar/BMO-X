use crate::fb::{self, FrameBuffer};
use crate::panels::helpers as H;
use crate::query;
use cabina_core::SystemSnapshot;

pub fn render(fb: &mut dyn FrameBuffer, _s: &SystemSnapshot) {
    H::header(fb, "QUERY", 0xFF44FF44);
    let mut y = 40u32;

    y = H::section(fb, y, "DSL examples", 0xFF44FF44);
    let examples: &[&str] = &[
        "errores", "critico", "capa:bmo_core", "modulo:lang",
        "severidad:Panic", "nuevos", "repetidos",
        "memoria:creciendo", "syscalls:lentas",
    ];
    for e in examples {
        fb::draw_text(fb, 16, y, &alloc::format!("query \"{}\"", e), 0xFF00FFFF);
        y += 14;
    }

    y = H::section(fb, y, "Presets (12 smart)", 0xFF44FF44);
    let presets: &[(&str, &str)] = &[
        ("only_errors",    "0"), ("only_critical",  "1"),
        ("layer",          "2"), ("process",        "3"),
        ("thread",         "4"), ("syscall",        "5"),
        ("file",           "6"), ("before_panic",   "7"),
        ("only_new",       "8"), ("only_repeated",  "9"),
        ("memory_growing", "10"), ("slow_syscalls", "11"),
    ];
    for (k, v) in presets {
        y = H::kv(fb, y, k, v, 0xFF00FFAA);
    }

    y = H::section(fb, y, "QueryId (F8)", 0xFF44FF44);
    y = H::kv(fb, y, "0", "OnlyErrors",   0xFFFF0000);
    y = H::kv(fb, y, "1", "OnlyCritical", 0xFFFF8800);
    y = H::kv(fb, y, "2", "Kernel",       0xFFFFFF00);
    y = H::kv(fb, y, "3", "Ring3",        0xFF00FF00);
    y = H::kv(fb, y, "4", "Gpu",          0xFF00FFFF);
    y = H::kv(fb, y, "5", "All",          0xFFCCCCCC);

    y = H::section(fb, y, "Live test", 0xFF44FF44);
    let input = "errores";
    fb::draw_text(fb, 16, y, "input:", 0xFFCCCCCC);
    fb::draw_text(fb, 80, y, input, 0xFFFFFF00);
    y += 16;
    match query::parse(input) {
        Some(q) => {
            let desc = alloc::format!(
                "-> Query {{ entities: {:?}, sev: {:?} }}",
                q.entities, q.severities
            );
            fb::draw_text(fb, 16, y, &desc, 0xFF00FF00);
        }
        None => {
            fb::draw_text(fb, 16, y, "-> None (parse failed)", 0xFFFF0000);
        }
    }
}
