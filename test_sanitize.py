
KEYWORDS = ["type", "Self", "in", "for", "const", "as", "match", "where", "move", "dyn", "use", "mod", "pub", "crate", "unsafe", "trait", "impl", "fn", "let"]

def sanitize_name(name):
    name = name.strip().rstrip(';')
    if name in KEYWORDS or name.startswith('_') or name == "":
        if name == "" or name == "_": return "reserved_padding"
        return f"field_{name.lstrip('_')}"
    if name and name[0].isdigit(): return f"field_{name}"
    name = "".join(c if c.isalnum() or c == '_' else '_' for c in name)
    return name

print(f"Sanitized 'type': '{sanitize_name('type')}'")
