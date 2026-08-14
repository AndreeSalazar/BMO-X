//! # THE KEYMAP PROBE -- the sparse table a program is driven by
//!
//! ## The axis
//!
//! A keyboard table is not a normal array. It is **128 slots of which twenty
//! are written, by designator, out of order, with values above 127** -- and it
//! is read with an index that only exists at run time. Every one of those four
//! words is a place where an emitter can be wrong while the program still
//! compiles, still runs, and still draws a title screen.
//!
//! And it is the shape the whole of DOOM's input rides on: `DG_GetKey` looks up
//! `g_tabla[scancode]`, and if the slot answers zero the key is **silently
//! dropped**. A table that is quietly off by one entry does not crash; it makes
//! a key that does nothing, which reads as a broken keyboard.
//!
//! ## The four things being separated, and why each earns a row
//!
//! | what | why it can be wrong on its own |
//! |---|---|
//! | sparse designators | the cursor has to jump and the gap has to be zero |
//! | **descending** designators | `[0x58]` then `[0x45]` -- the cursor goes BACK |
//! | values over 127 | `unsigned char` read into an `int`: sign or no sign |
//! | index from a variable | a constant index can be folded; a variable cannot |
//!
//! The descending row is the one worth having. A keyboard table is written in
//! reading order --the function keys together, then Pause-- and Set 1 does not
//! number the keyboard in that order, so **F12 is 0x58 and Pause is 0x45**. The
//! table therefore walks backwards halfway through, which no table in
//! `probe_tables` does.
//!
//! ## ** First sweep: all 9 green
//!
//! Written the day DOOM got its full keyboard (2026-08-14). Nothing broken --
//! which is the answer you want before blaming a table for a key that does not
//! work.

use super::census::{sweep, Cell};

fn census() -> Vec<Cell> {
    vec![
        Cell {
            // The plain shape: three slots written, everything else zero.
            name: "sparse designators + a gap",
            source: "unsigned char t[128] = { [1] = 27, [0x39] = 32, [0x6F] = 0xD1 };\n\
                     int main() { printf(\"%d %d %d %d\\n\", (int)t[1], (int)t[0x39], (int)t[0x6F], (int)t[2]); return 0; }",
            expects: "27 32 209 0",
        },
        Cell {
            // ** The one that matters: the cursor goes BACKWARDS.
            // `[F11]=0x57, [F12]=0x58, [Pause]=0x45` is the real order of the
            // real table, because Set 1 does not number keys the way a person
            // writes them down.
            name: "designators that go backwards",
            source: "unsigned char t[128] = { [0x57] = 11, [0x58] = 12, [0x45] = 13, [0x46] = 14, [0x6A] = 15 };\n\
                     int main() { printf(\"%d %d %d %d %d\\n\", (int)t[0x57], (int)t[0x58], (int)t[0x45], (int)t[0x46], (int)t[0x6A]); return 0; }",
            expects: "11 12 13 14 15",
        },
        Cell {
            // `KEY_F1` is 0xBB and `KEY_PAUSE` is 0xFF. If the read
            // sign-extends, the first becomes -69 and DOOM sees no key at all.
            name: "values above 127",
            source: "unsigned char t[128] = { [0x3B] = 0xBB, [0x45] = 0xFF, [0x0E] = 0x7F };\n\
                     int main() { printf(\"%d %d %d\\n\", (int)t[0x3B], (int)t[0x45], (int)t[0x0E]); return 0; }",
            expects: "187 255 127",
        },
        Cell {
            // The index is a scancode read from the kernel: a variable, never a
            // constant. A constant index can be folded at compile time and hide
            // a broken address computation.
            name: "index from a variable",
            source: "unsigned char t[128] = { [0x1E] = 97, [0x11] = 119 };\n\
                     int main() { int sc; sc = 0x1E; printf(\"%d %d\\n\", (int)t[sc], (int)t[sc + 0x11 - 0x1E]); return 0; }",
            expects: "97 119",
        },
        Cell {
            // How the table is actually written: both the index and the value
            // are macros, and the value is an arithmetic expression
            // (`KEY_F1` is literally `(0x80+0x3b)`).
            name: "macros as index and as value",
            source: "#define SC_W 0x11\n\
                     #define KEY_UP (0x80+0x2d)\n\
                     unsigned char t[128] = { [SC_W] = KEY_UP };\n\
                     int main() { printf(\"%d\\n\", (int)t[SC_W]); return 0; }",
            expects: "173",
        },
        Cell {
            // TWO tables in parallel, indexed by the same scancode: that is how
            // one key produces both a letter and a movement key.
            name: "two parallel tables, one index",
            source: "unsigned char a[128] = { [0x1E] = 'a', [0x11] = 'w' };\n\
                     unsigned char b[128] = { [0x1E] = 0xA0, [0x11] = 0xAD };\n\
                     int main() { int sc; sc = 0x1E; printf(\"%d %d %d\\n\", (int)a[sc], (int)b[sc], (int)b[0x20]); return 0; }",
            expects: "97 160 0",
        },
        Cell {
            // Character escapes inside an initialiser: the real table carries
            // `'\\'` and `'\''`, and a lexer that eats one backslash too many
            // shifts every entry after it.
            name: "escaped chars in the table",
            source: "unsigned char t[128] = { [0x2B] = '\\\\', [0x28] = '\\'', [0x35] = '/' };\n\
                     int main() { printf(\"%d %d %d\\n\", (int)t[0x2B], (int)t[0x28], (int)t[0x35]); return 0; }",
            expects: "92 39 47",
        },
        Cell {
            // Only the LAST slot written. Asks whether the array keeps its
            // declared size when almost all of it is zero -- the shape the
            // `Bss` split reasons about.
            name: "only the tail slot written",
            source: "unsigned char t[128] = { [0x6F] = 7 };\n\
                     int main() { printf(\"%d %d\\n\", (int)t[0x6F], (int)t[0]); return 0; }",
            expects: "7 0",
        },
        Cell {
            // The pending-key machine: a `static` that survives between calls
            // and hands the SECOND key over on the next call. If the static
            // resets, the second key is lost; if it never clears, the caller
            // spins forever inside one frame.
            name: "static pending slot across calls",
            source: "static int pend;\n\
                     static unsigned char pk;\n\
                     int siguiente(unsigned char *k) { if (pend) { pend = 0; *k = pk; return 1; } return 0; }\n\
                     int main() { unsigned char k; int n; pend = 1; pk = 200; n = 0;\n\
                       while (siguiente(&k)) { n = n + 1; printf(\"%d %d\\n\", n, (int)k); }\n\
                       printf(\"fin %d\\n\", n); return 0; }",
            expects: "1 200\nfin 1",
        },
    ]
}

#[test]
fn the_keymap_census_has_not_changed() {
    sweep(
        &census(),
        CENSUS,
        "EL CENSO DEL TECLADO CAMBIO.\n\
         Este eje estaba verde entero, asi que un ROTO aqui es una REGRESION\n\
         -- y la paga DOOM entero, porque su entrada es una tabla dispersa.\n\
         Si la que cae es 'designators that go backwards', el sospechoso es el\n\
         cursor de `parser/inicializador.rs`, que reposiciona con cada\n\
         designador; si la que cae es 'values above 127', es la carga de un\n\
         `unsigned char` (`movzx`, no `movsx`).",
    );
}

/// **EL CENSO DEL TECLADO, al 2026-08-14.** Verde entero desde el primer
/// barrido.
const CENSUS: &str = "\
sparse designators + a gap     GOOD
designators that go backwards  GOOD
values above 127               GOOD
index from a variable          GOOD
macros as index and as value   GOOD
two parallel tables, one index GOOD
escaped chars in the table     GOOD
only the tail slot written     GOOD
static pending slot across calls GOOD
";
