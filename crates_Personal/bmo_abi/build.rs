use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::path::Path;

fn main() {
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
    let asm_dir = Path::new(&manifest_dir).join("..").join("Semantic_ASM").join("bmo");

    let mut syscalls: BTreeMap<u32, (String, u8, String)> = BTreeMap::new();
    // nr → (canonical_name, arg_count, category)

    for entry in fs::read_dir(&asm_dir).unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();
        if path.extension().map_or(false, |e| e == "toml") {
            let category = path.file_stem().unwrap().to_str().unwrap().to_string();
            let content = fs::read_to_string(&path).unwrap();

            for line in content.lines() {
                let line = line.trim();
                if line.is_empty() || line.starts_with('#') {
                    continue;
                }
                if let Some((name, rest)) = line.split_once('=') {
                    let name = name.trim();
                    if let Some((nr_str, arg_str)) = rest.split_once(',') {
                        let nr = u32::from_str_radix(nr_str.trim().trim_start_matches("0x"), 16)
                            .expect(&format!("bad nr in {}: {}", path.display(), nr_str));
                        let arg_count: u8 = arg_str.trim().parse().unwrap();
                        let has_cat = name.starts_with("bmo_") && name[4..].contains('_');

                        if let Some((existing, _, _)) = syscalls.get(&nr) {
                            let existing_has_cat =
                                existing.starts_with("bmo_") && existing[4..].contains('_');
                            if has_cat && !existing_has_cat {
                                syscalls.insert(nr, (name.to_string(), arg_count, category.clone()));
                            }
                        } else {
                            syscalls.insert(nr, (name.to_string(), arg_count, category.clone()));
                        }
                    }
                }
            }
        }
    }

    // Group by category for output
    let mut by_cat: BTreeMap<String, Vec<(u32, String, u8)>> = BTreeMap::new();
    for (nr, (name, ac, cat)) in &syscalls {
        by_cat
            .entry(cat.clone())
            .or_default()
            .push((*nr, name.clone(), *ac));
    }

    // Category comments
    let cat_comment: BTreeMap<&str, &str> = BTreeMap::from([
        ("wm", "Window Manager (0x100..0x10F)"),
        ("draw", "Draw (0x110..0x119)"),
        ("winpaint", "Window Painting (0x120..0x125)"),
        ("compositor", "Compositor (0x130..0x134)"),
        ("io", "Filesystem — short aliases (0x140..0x149)"),
        ("fs", "Filesystem — canonical (0x140..0x149)"),
        ("time", "Time (0x150..0x153)"),
        ("input", "Input (0x160..0x162)"),
        ("audio", "Audio (0x170..0x173)"),
        ("proc", "Process / Thread (0x180..0x188)"),
        ("mem", "Memory + BEFCore (0x190..0x197)"),
        ("ipc", "IPC (0x1A0..0x1A3)"),
        ("surface", "Surface mapping (0x1C0..0x1CF)"),
        ("diag", "Diagnostics (0x1F0..0x1F3)"),
    ]);

    let out_dir = env::var("OUT_DIR").unwrap();
    let out_path = Path::new(&out_dir).join("generated_syscalls.rs");

    let mut o = String::new();
    o.push_str("// Auto-generated from Semantic_ASM/bmo/*.toml — DO NOT EDIT\n");
    // (dead_code allow is on the parent module)

    for (cat, entries) in &by_cat {
        let comment = cat_comment.get(cat.as_str()).copied().unwrap_or(cat.as_str());
        o.push_str("// ═══════════════════════════════════════════════\n");
        o.push_str(&format!("//  {} — {}\n", cat.to_uppercase(), comment));
        o.push_str(&format!("// ═══════════════════════════════════════════════\n"));
        for (nr, name, _) in entries {
            o.push_str(&format!("pub const {}: u32 = {:#X};\n", name_to_const(name), nr));
        }
        o.push_str("\n");
    }

    // Helpers
    o.push_str("// ── Helpers ──\n\n");
    o.push_str("pub const fn is_bmo_api(nr: u32) -> bool {\n");
    o.push_str("    nr >= 0x100 && nr <= 0x1FF\n");
    o.push_str("}\n\n");
    o.push_str("pub const fn is_befcore(nr: u32) -> bool {\n");
    o.push_str("    nr >= 0x194 && nr <= 0x197\n");
    o.push_str("}\n\n");

    // name() function
    o.push_str("pub fn name(nr: u32) -> &'static str {\n");
    o.push_str("    match nr {\n");
    for (nr, (name, _, _)) in &syscalls {
        o.push_str(&format!("        {:#X} => \"{}\",\n", nr, clean_name(name)));
    }
    o.push_str("        _ => \"<unknown bmo_api syscall>\",\n");
    o.push_str("    }\n");
    o.push_str("}\n");

    fs::write(&out_path, o).unwrap();
    println!("cargo:rerun-if-changed={}", asm_dir.display());
}

fn clean_name(name: &str) -> &str {
    name.strip_prefix("bmo_").unwrap_or(name)
}

fn name_to_const(name: &str) -> String {
    let s = clean_name(name);
    let parts: Vec<&str> = s.split('_').filter(|p| !p.is_empty()).collect();
    format!("NR_{}", parts.join("_").to_uppercase())
}
