//! # THE SIGNEDNESS PROBE -- the operations that ask about the top bit
//!
//! ## One more axis, and why it is NOT `probe_widths`
//!
//! That one asks *"what happens to a number when it is STORED in something
//! narrower"* -- narrowing, widening, promoting. Its 16 came back green.
//!
//! This one asks something else: **how the bit pattern is READ when you operate
//! on it**. They are different axes because an `unsigned long` never narrows at
//! all and still has four operations that get it wrong.
//!
//! ## Why this axis, and why now
//!
//! Because DOOM's `angle_t` is an `unsigned int` that wraps on purpose: the
//! whole renderer --`R_PointToAngle`, `R_ScaleFromGlobalAngle`, the BSP walk--
//! lives in arithmetic that only makes sense unsigned. A `>>` that copies the
//! sign bit turns every angle past 180 degrees into garbage, and that does not
//! produce an error: it produces a picture.
//!
//! ## ** What it found: FOUR operations, and all four only at 64 bits
//!
//! ```text
//!   (unsigned long)0x8000000000000000 >> 60   gave 18446744073709551608
//!   ...                               / 2     gave a huge negative
//!   ...                               % 10    same
//!   ...                               > 1     gave 0
//! ```
//!
//! [!] **At 32 bits they were right by accident**, and that part has to be
//! understood or the fix looks unnecessary. The codegen computes everything in
//! `rax`, i.e. in 64 bits: an `unsigned int` with bit 31 set loads with
//! `mov eax` and arrives **zero-extended**, so bit 63 is 0 and `sar` gives the
//! same answer as `shr`. Only an `unsigned long` exposes it.
//!
//! So the defect had been there since day one, hidden by the width of the
//! accumulator. It is a cousin of house pattern 15: it was not hidden by a
//! missing test, it was hidden because **the case that breaks it cannot be
//! written in 32 bits**.
//!
//! ## And the `Shr` arm CONFESSED it in prose
//!
//! *"The right shift is ARITHMETIC (`sar`), which is correct for `int`. An
//! unsigned type would want `shr`; today the codegen does not carry that
//! distinction this far."*
//!
//! And that was false: the distinction DID reach it. `var_type_of` existed, and
//! `Field`, `Arrow` and `IndexPtr` carry their `TypeSpec` inside -- all that
//! was missing was asking. `expr_is_unsigned` was written as a carbon copy of
//! `expr_is_float`, which had been sitting right there the whole time. **A
//! defect confessed in prose is still a defect**, and this is the third time
//! this house has paid for that.

use super::census::{sweep, Cell};

fn census() -> Vec<Cell> {
    vec![
        Cell {
            // ** THE `angle_t` ONE: `angle >> ANGLETOFINESHIFT` with the top
            // bit set. With `sar` instead of `shr` it comes out negative.
            name: "unsigned >> with the top bit set",
            source: "int main() { unsigned int a; a = 0x80000000; \
                       printf(\"%u\\n\", a >> 19); return 0; }",
            expects: "4096",
        },
        Cell {
            name: "int >> with the top bit (sar)",
            source: "int main() { int a; a = -2147483648; \
                       printf(\"%d\\n\", a >> 19); return 0; }",
            expects: "-4096",
        },
        Cell {
            name: "unsigned long >> with top bit",
            source: "int main() { unsigned long a; a = 0x8000000000000000; \
                       printf(\"%lu\\n\", a >> 60); return 0; }",
            expects: "8",
        },
        Cell {
            // `if (angle < ANG90)` with angles past 180 degrees.
            name: "unsigned < with the top bit set",
            source: "int main() { unsigned int a; unsigned int b; \
                       a = 0x90000000; b = 0x10000000; \
                       printf(\"%d\\n\", (int)(a > b)); return 0; }",
            expects: "1",
        },
        Cell {
            name: "int < with the top bit (signed)",
            source: "int main() { int a; int b; a = -1879048192; b = 268435456; \
                       printf(\"%d\\n\", (int)(a > b)); return 0; }",
            expects: "0",
        },
        Cell {
            // The angle wraps: this is `angle_t` arithmetic in full.
            name: "unsigned wraps around on add",
            source: "int main() { unsigned int a; a = 0xC0000000; a = a + 0x80000000; \
                       printf(\"%u\\n\", a); return 0; }",
            expects: "1073741824",
        },
        Cell {
            name: "unsigned / with the top bit set",
            source: "int main() { unsigned int a; a = 0x80000000; \
                       printf(\"%u\\n\", a / 4); return 0; }",
            expects: "536870912",
        },
        Cell {
            name: "unsigned % with the top bit set",
            source: "int main() { unsigned int a; a = 0x80000007; \
                       printf(\"%u\\n\", a % 10); return 0; }",
            expects: "5",
        },
        Cell {
            // `%u` against `%d` on the same value: the way to see that printf
            // does not pick up a sign either.
            name: "printf %u prints no negative",
            source: "int main() { unsigned int a; a = 3000000000; \
                       printf(\"%u\\n\", a); return 0; }",
            expects: "3000000000",
        },
        Cell {
            // DOOM's `FixedDiv`/`FixedMul`: 64 bits in between, back to 32.
            name: "fixed mul: 64 bits in between",
            source: "int main() { int a; int b; long long p; \
                       a = 65536 * 3; b = 65536 / 2; \
                       p = ((long long)a * (long long)b) >> 16; \
                       printf(\"%d\\n\", (int)p); return 0; }",
            expects: "98304",
        },
        Cell {
            // `FixedDiv` with a negative, which is half of the calls.
            name: "fixed div with a negative",
            source: "int main() { int a; int b; long long r; \
                       a = -65536 * 3; b = 65536 * 2; \
                       r = (((long long)a) << 16) / b; \
                       printf(\"%d\\n\", (int)r); return 0; }",
            expects: "-98304",
        },
        // -- And the same at 64 bits, where the value DOES have the top bit
        Cell {
            name: "unsigned long / with bit 63",
            source: "int main() { unsigned long a; a = 0x8000000000000000; \
                       printf(\"%lu\\n\", a / 2); return 0; }",
            expects: "4611686018427387904",
        },
        Cell {
            // [!] Segunda vez que el censo caza una cuenta MIA y no del
            // compilador: escribi `1` y contesto `3`. `0x8000000000000005` es
            // 9223372036854775813, y acaba en 3. La primera fue en
            // `sonda_de_disposicion`. Dos de dos: la aritmetica a ojo sobre
            // numeros de 19 digitos no es de fiar, y por eso el censo compara
            // un informe entero en vez de creerse un `assert` suelto.
            name: "unsigned long % with bit 63",
            source: "int main() { unsigned long a; a = 0x8000000000000005; \
                       printf(\"%lu\\n\", a % 10); return 0; }",
            expects: "3",
        },
        Cell {
            name: "unsigned long > with bit 63",
            source: "int main() { unsigned long a; unsigned long b; \
                       a = 0x8000000000000000; b = 1; \
                       printf(\"%d\\n\", (int)(a > b)); return 0; }",
            expects: "1",
        },
        Cell {
            // The `bmo_valor` shape: the kernel returns an `unsigned long
            // long` and the program splits it into two halves.
            name: "split a u64 from the kernel",
            source: "int main() { unsigned long long d; d = 0x0000028000000190; \
                       printf(\"%d %d\\n\", (int)(d >> 32), (int)(d & 0xFFFFFFFF)); return 0; }",
            expects: "640 400",
        },
        Cell {
            // `unsigned short` being promoted: the BSP's `children[2]` carry
            // bit 15 as the subsector marker.
            name: "unsigned short bit 15",
            source: "int main() { unsigned short c; c = 0x8005; \
                       printf(\"%d %d\\n\", (int)c, (int)(c & 0x8000)); return 0; }",
            expects: "32773 32768",
        },
    ]
}

#[test]
fn the_signedness_census_has_not_changed() {
    sweep(
        &census(),
        CENSUS,
        "EL CENSUS DEL SIGNO CAMBIO.\n\
         Si una casilla de 64 bits se puso en ROJO, mirar `expr_is_unsigned` en\n\
         el codegen: es lo que decide entre `shr`/`sar`, `div`/`idiv` y\n\
         `setb`/`setl`. Y ojo -- las de 32 bits aciertan aunque la regla este\n\
         mal, porque el valor llega a `rax` extendido con ceros.",
    );
}

/// **EL CENSUS DEL SIGNO, al 2026-08-13.** Verde desde que el codegen pregunta
/// por el tipo antes de elegir la instruccion. Antes, las cuatro filas de
/// `unsigned long` estaban rojas.
const CENSUS: &str = "\
unsigned >> with the top bit set GOOD
int >> with the top bit (sar)  GOOD
unsigned long >> with top bit  GOOD
unsigned < with the top bit set GOOD
int < with the top bit (signed) GOOD
unsigned wraps around on add   GOOD
unsigned / with the top bit set GOOD
unsigned % with the top bit set GOOD
printf %u prints no negative   GOOD
fixed mul: 64 bits in between  GOOD
fixed div with a negative      GOOD
unsigned long / with bit 63    GOOD
unsigned long % with bit 63    GOOD
unsigned long > with bit 63    GOOD
split a u64 from the kernel    GOOD
unsigned short bit 15          GOOD
";
