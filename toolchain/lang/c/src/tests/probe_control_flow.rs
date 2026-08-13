//! # THE CONTROL-FLOW PROBE -- switch, goto and recursion
//!
//! ## The axis, and why this one
//!
//! Counted over DOOM's tree: **85 `switch` and 20 `goto`** in `p_*.c` and
//! `r_*.c` alone, and `R_RenderBSPNode` is **recursive with two calls per
//! node**. The playsim and the renderer are not arithmetic with a loop around
//! it: they are a graph of jumps.
//!
//! And it is a different axis from the ones before it because here nothing has
//! a wrong value -- there are wrong **paths**. A `continue` that lands in the
//! wrong place gives you a program that computes correctly and does something
//! else.
//!
//! ## The rows that are real corner cases
//!
//! Three of the sixteen are not "check that it works", they are known C traps
//! that an emitter written from memory gets wrong:
//!
//! - **`continue` inside a `switch`** belongs to the LOOP, not to the switch.
//! - **`continue` in a `do..while`** jumps to the CONDITION, not to the top of
//!   the body. That is the one almost nobody emits correctly.
//! - **`while (0)` runs zero times and `do..while(0)` runs once.** Emitting the
//!   two from the same template is the natural mistake, and it does not show up
//!   until the condition is false on entry.
//!
//! ## ** Result: clean, all 16
//!
//! Including two-branch recursion with its own locals, which is the exact shape
//! of `R_RenderBSPNode`. When the BSP fails on metal, **control flow is not the
//! suspect**.

use super::census::{sweep, Cell};

fn census() -> Vec<Cell> {
    vec![
        Cell {
            name: "switch: three cases and default",
            source: "int f(int x) { switch (x) { case 1: return 10; case 2: return 20; \
                       default: return 99; } return -1; }\n\
                     int main() { printf(\"%d %d %d\\n\", f(1), f(2), f(7)); return 0; }",
            expects: "10 20 99",
        },
        Cell {
            // ** Fallthrough: without `break` it falls into the case below.
            // DOOM uses it deliberately in `P_SpecialThing` and in the menu.
            name: "switch without break falls through",
            source: "int main() { int n; int x; n = 0; x = 1; \
                       switch (x) { case 1: n = n + 1; case 2: n = n + 10; break; \
                         case 3: n = n + 100; } \
                       printf(\"%d\\n\", n); return 0; }",
            expects: "11",
        },
        Cell {
            name: "switch with default in the middle",
            source: "int f(int x) { int r; r = 0; \
                       switch (x) { case 1: r = 1; break; default: r = 9; break; \
                         case 2: r = 2; break; } return r; }\n\
                     int main() { printf(\"%d %d %d\\n\", f(1), f(2), f(5)); return 0; }",
            expects: "1 2 9",
        },
        Cell {
            name: "switch with scattered values",
            source: "int f(int x) { switch (x) { case 0: return 1; case 100: return 2; \
                       case -5: return 3; case 1000000: return 4; } return 0; }\n\
                     int main() { printf(\"%d %d %d %d %d\\n\", f(0), f(100), f(-5), \
                       f(1000000), f(7)); return 0; }",
            expects: "1 2 3 4 0",
        },
        Cell {
            // ** `continue` INSIDE a switch: the `continue` belongs to the
            // LOOP, not to the switch. Mixing them up is a classic.
            name: "continue inside a switch",
            source: "int main() { int i; int n; n = 0; \
                       for (i = 0; i < 5; i++) { \
                         switch (i) { case 2: continue; default: break; } \
                         n = n + 1; } \
                       printf(\"%d\\n\", n); return 0; }",
            expects: "4",
        },
        Cell {
            // ** `continue` in a `do..while` goes to the CONDITION, not to the
            // top of the body. The corner case almost nobody emits right.
            name: "continue in a do..while",
            source: "int main() { int i; int n; i = 0; n = 0; \
                       do { i = i + 1; if (i == 2) { continue; } n = n + 1; } \
                       while (i < 4); \
                       printf(\"%d %d\\n\", i, n); return 0; }",
            expects: "4 3",
        },
        Cell {
            name: "break leaves ONE loop, not two",
            source: "int main() { int i; int j; int n; n = 0; \
                       for (i = 0; i < 3; i++) { for (j = 0; j < 3; j++) { \
                         if (j == 1) { break; } n = n + 1; } } \
                       printf(\"%d\\n\", n); return 0; }",
            expects: "3",
        },
        Cell {
            // The real use of `goto` in C: leaving two loops at once.
            name: "goto leaves two loops",
            source: "int main() { int i; int j; int n; n = 0; \
                       for (i = 0; i < 4; i++) { for (j = 0; j < 4; j++) { \
                         n = n + 1; if (n == 5) { goto fuera; } } } \
                       fuera: printf(\"%d\\n\", n); return 0; }",
            expects: "5",
        },
        Cell {
            name: "backward goto is a loop",
            source: "int main() { int n; n = 0; \
                       otra: n = n + 1; if (n < 6) { goto otra; } \
                       printf(\"%d\\n\", n); return 0; }",
            expects: "6",
        },
        Cell {
            // ** `R_RenderBSPNode` is recursive and calls itself twice per node.
            name: "recursion: factorial",
            source: "int fact(int n) { if (n <= 1) { return 1; } return n * fact(n - 1); }\n\
                     int main() { printf(\"%d\\n\", fact(10)); return 0; }",
            expects: "3628800",
        },
        Cell {
            // The exact shape of the BSP: two recursive calls per level, and
            // the result depends on both.
            name: "TWO-branch recursion (the BSP)",
            source: "int arbol(int n) { if (n <= 0) { return 1; } \
                       return arbol(n - 1) + arbol(n - 1); }\n\
                     int main() { printf(\"%d\\n\", arbol(10)); return 0; }",
            expects: "1024",
        },
        Cell {
            // Real depth: E1M1's BSP goes about 20 levels down, but what is
            // being tested is that the stack frame does not walk over itself.
            name: "deep recursion (200)",
            source: "int baja(int n) { if (n == 0) { return 0; } return 1 + baja(n - 1); }\n\
                     int main() { printf(\"%d\\n\", baja(200)); return 0; }",
            expects: "200",
        },
        Cell {
            // With LOCALS inside, which is what really exercises the frame:
            // every level has to get its own.
            name: "recursion with own locals",
            source: "int suma(int n) { int mio; if (n == 0) { return 0; } \
                       mio = n * 2; return mio + suma(n - 1); }\n\
                     int main() { printf(\"%d\\n\", suma(100)); return 0; }",
            expects: "10100",
        },
        Cell {
            name: "mutual recursion",
            source: "int impar(int n);\n\
                     int par(int n) { if (n == 0) { return 1; } return impar(n - 1); }\n\
                     int impar(int n) { if (n == 0) { return 0; } return par(n - 1); }\n\
                     int main() { printf(\"%d %d\\n\", par(10), par(7)); return 0; }",
            expects: "1 0",
        },
        Cell {
            name: "return from inside a switch",
            source: "int f(int x) { int i; \
                       for (i = 0; i < 10; i++) { switch (i) { case 3: \
                         if (x) { return i * 100; } break; } } return -1; }\n\
                     int main() { printf(\"%d %d\\n\", f(1), f(0)); return 0; }",
            expects: "300 -1",
        },
        Cell {
            // `while` with the condition false on entry: ZERO iterations. And
            // its twin `do..while`, which runs ONE. Emitting both the same way
            // is the mistake.
            name: "while runs 0, do..while runs 1",
            source: "int main() { int a; int b; a = 0; b = 0; \
                       while (0) { a = a + 1; } \
                       do { b = b + 1; } while (0); \
                       printf(\"%d %d\\n\", a, b); return 0; }",
            expects: "0 1",
        },
    ]
}

#[test]
fn the_control_flow_census_has_not_changed() {
    sweep(
        &census(),
        CENSUS,
        "EL CENSUS DEL FLUJO CAMBIO.\n\
         Este eje estaba limpio entero, asi que un ROTO aqui es una REGRESION.\n\
         Si cae una de recursion, mirar el marco de pila (`codegen/marco.rs`);\n\
         si cae una de `switch` o `goto`, el enlazado (`codegen/enlazado.rs`).",
    );
}

/// **EL CENSUS DEL FLUJO, al 2026-08-13.** Verde entero desde el primer barrido.
const CENSUS: &str = "\
switch: three cases and default GOOD
switch without break falls through GOOD
switch with default in the middle GOOD
switch with scattered values   GOOD
continue inside a switch       GOOD
continue in a do..while        GOOD
break leaves ONE loop, not two GOOD
goto leaves two loops          GOOD
backward goto is a loop        GOOD
recursion: factorial           GOOD
TWO-branch recursion (the BSP) GOOD
deep recursion (200)           GOOD
recursion with own locals      GOOD
mutual recursion               GOOD
return from inside a switch    GOOD
while runs 0, do..while runs 1 GOOD
";
