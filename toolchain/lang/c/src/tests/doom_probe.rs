//! The shapes DOOM needs, and nothing else.
//!
//! Part of the BMO C test bench. The helpers (`run_c`, `run_c_con_pp`,
//! `run_c_sembrado`, `ejecutar_bef`) live in `tests/mod.rs`.
//!
//! # Where this list came from
//!
//! Not from reading the standard and guessing which corner mattered. DOOM's 81
//! core translation units were compiled ONE AT A TIME and the first error of
//! each was tallied -- 81 first-errors in one run instead of one at a time,
//! which turns "it does not compile" into a distribution.
//!
//! The answer was that 81 failures were **five** causes, and each one is a row
//! here. That is the point of the exercise and the reason these tests are
//! grouped by origin rather than by feature: what they have in common is the
//! measurement that found them.
//!
//! The enum rows live in `enumeraciones.rs`, next to the enum tests that were
//! already there.

use super::*;

// =============== The preprocessor ===============

/// A comment dies BEFORE the directive is read (C11 phase 3, then phase 4).
///
/// `#if 0 // UNUSED` is how DOOM disables a block, in 19 files. The comment
/// was reaching the expression evaluator, which then blamed the expression.
#[test]
fn a_comment_after_if_is_not_part_of_the_expression() {
    let out = run_c_con_pp(
        "#if 0 // UNUSED\n\
         int dead(void) { return 1; }\n\
         #endif\n\
         int main() { printf(\"alive\\n\"); return 0; }",
    );
    assert_eq!(out.trim(), "alive");
}

/// And the same rule on an `#include`, where it showed up as a path with a
/// comment glued to it: `file not found: m_argv.h" // haleyjd 20110212`.
///
/// The stripper has to know what a string is, or `#define` bodies that carry a
/// URL lose half their text. Both directions are checked here.
#[test]
fn a_comment_dies_but_a_slash_inside_a_string_survives() {
    let out = run_c_con_pp(
        "#define PATH \"http://x/y\" // a comment, this one is real\n\
         int main() { printf(\"%s\\n\", PATH); return 0; }",
    );
    assert_eq!(out.trim(), "http://x/y");
}

/// An identifier that survives macro expansion in a `#if` is `0` (C11
/// 6.10.1p4). It is how a portable program says "not this platform", and DOOM
/// says it -- `#if ORIGCODE`, `#if _WIN64` -- in 45 files.
#[test]
fn an_undefined_identifier_in_if_is_zero() {
    let out = run_c_con_pp(
        "#if NEVER_DEFINED_ANYWHERE\n\
         int dead(void) { return 1; }\n\
         #endif\n\
         int main() { printf(\"alive\\n\"); return 0; }",
    );
    assert_eq!(out.trim(), "alive");
}

/// Parentheses, and precedence.
///
/// The evaluator this replaces split on the first operator of a fixed list,
/// anywhere in the string. `(0 == 0)` it could not evaluate at all; `1 == 0 &&
/// 0` it evaluated as `1 == (0 && 0)`, which is TRUE -- and a `#if` that
/// answers wrongly does not fail, it compiles the other half of the file.
#[test]
fn if_expressions_have_parentheses_and_precedence() {
    let out = run_c_con_pp(
        "#if (0 == 0)\n\
         #define A 1\n\
         #endif\n\
         #if 1 == 0 && 0\n\
         #define B 1\n\
         #endif\n\
         int main() { \n\
         #ifdef A\n\
         printf(\"a\");\n\
         #endif\n\
         #ifdef B\n\
         printf(\"b\");\n\
         #endif\n\
         printf(\"\\n\"); return 0; }",
    );
    assert_eq!(out.trim(), "a");
}

// =============== Aggregates without a tag ===============

/// `typedef struct { ... } name;` -- the way a one-off type is declared in C,
/// and the first error in 34 of DOOM's files ("expected struct name, got
/// OpenBrace").
#[test]
fn typedef_of_an_anonymous_struct_is_a_type() {
    let out = run_c(
        "typedef struct { int x; int y; } point_t; \
         int main() { point_t p; p.x = 3; p.y = 4; printf(\"%d\\n\", p.x + p.y); return 0; }",
    );
    assert_eq!(out, "7\n");
}

/// The same for a union, which is what `d_think.h` is built out of -- the
/// thinker's action is a union of three function pointers.
#[test]
fn typedef_of_an_anonymous_union_is_a_type() {
    let out = run_c(
        "typedef union { int i; unsigned int u; } word_t; \
         int main() { word_t w; w.i = -1; printf(\"%x\\n\", w.u); return 0; }",
    );
    assert_eq!(out, "ffffffff\n");
}

/// Two untagged structs are two DIFFERENT types, and their layouts must not
/// collide. They are keyed by a generated tag, so this is the test that the
/// generator does not hand out the same name twice.
#[test]
fn two_anonymous_structs_keep_their_own_layouts() {
    let out = run_c(
        "typedef struct { int a; } small_t; \
         typedef struct { int a; int b; int c; } big_t; \
         int main() { small_t s; big_t b; s.a = 1; b.a = 2; b.b = 3; b.c = 4; \
         printf(\"%d %d\\n\", s.a, b.a + b.b + b.c); return 0; }",
    );
    assert_eq!(out, "1 9\n");
}

// =============== A typedef of a pointer to function ===============

/// `typedef void (*action_t)(void);`
///
/// The declarator was already understood for variables and parameters; only
/// the typedef could not use it, and said "expected typedef name, got
/// OpenParen" -- pointing at the parenthesis instead of at the gap.
#[test]
fn a_pointer_to_function_can_be_typedefd() {
    let out = run_c(
        "typedef int (*op_t)(int, int); \
         int add(int a, int b) { return a + b; } \
         int main() { op_t f; f = add; printf(\"%d\\n\", f(19, 23)); return 0; }",
    );
    assert_eq!(out, "42\n");
}

/// And it can be a PARAMETER, which is the half that was missing: a callback
/// could be declared, stored and called, but not passed. `p_map.c` and
/// `p_sight.c` are built on passing the traverser in -- 24 files.
#[test]
fn a_pointer_to_function_can_be_a_parameter() {
    let out = run_c(
        "int apply(int (*f)(int), int v) { return f(v); } \
         int twice(int v) { return v * 2; } \
         int main() { printf(\"%d\\n\", apply(twice, 21)); return 0; }",
    );
    assert_eq!(out, "42\n");
}

// =============== Declarators: the comma, and the brackets ===============

/// `int alpha, beta;` at FILE scope.
///
/// It worked inside a function and not outside it, so the difference was the
/// scope and not the syntax. The message -- "expected type, got Comma" --
/// sends you to look at the type, which is perfect.
#[test]
fn several_globals_share_one_type_after_a_comma() {
    let out = run_c(
        "int alpha, beta; int *ptr, plain; \
         int main() { alpha = 1; beta = 2; plain = 39; ptr = &alpha; \
         printf(\"%d\\n\", *ptr + beta + plain); return 0; }",
    );
    assert_eq!(out, "42\n");
}

/// The same inside a struct: `int data1, data2, data3, data4;`
///
/// That is `d_event.h`, the event every input in DOOM travels in. One line,
/// and it was the first error in twenty files.
#[test]
fn several_members_share_one_type_after_a_comma() {
    let out = run_c(
        "typedef struct { int data1, data2, data3, data4; } event_t; \
         int main() { event_t e; e.data1 = 1; e.data2 = 2; e.data3 = 3; e.data4 = 36; \
         printf(\"%d\\n\", e.data1 + e.data2 + e.data3 + e.data4); return 0; }",
    );
    assert_eq!(out, "42\n");
}

/// `int t[] = { ... }` -- the list is what says how long the array is.
#[test]
fn an_array_without_a_size_takes_it_from_its_initializer() {
    let out = run_c(
        "int table[] = { 10, 20, 12 }; \
         int main() { printf(\"%d\\n\", table[0] + table[1] + table[2]); return 0; }",
    );
    assert_eq!(out, "42\n");
}

/// Two dimensions, and they fold from the right: `[2][4]` is two arrays of
/// four. DOOM's `doomdata.h` stores a node's bounding boxes exactly like that,
/// and `tables.h` its gamma tables -- between them, 39 files.
#[test]
fn a_two_dimensional_array_is_read_whole() {
    let out = run_c(
        "int grid[2][3] = { { 1, 2, 3 }, { 10, 20, 30 } }; \
         int main() { \
           printf(\"%d %d %d %d %d %d\\n\", grid[0][0], grid[0][1], grid[0][2], \
                  grid[1][0], grid[1][1], grid[1][2]); return 0; }",
    );
    assert_eq!(out, "1 2 3 10 20 30\n");
}

/// A size that cannot be computed is an ERROR, not a 1.
///
/// It used to fall back to one element: the program compiled, the array had a
/// single slot, and every write past the first landed on the next variable.
#[test]
fn an_array_size_that_is_not_constant_is_rejected() {
    let err = compile_source_to_bef("int n; int t[n]; int main() { return 0; }")
        .expect_err("a variable is not a constant size");
    assert!(err.message.contains("constante"), "message: {}", err.message);
}

/// `typedef byte digest_t[20];` -- a typedef OF an array.
///
/// `sha1.h` has one, `net_defs.h` includes it and `doomstat.h` includes that,
/// which is how one line reached most of the game.
#[test]
fn an_array_can_be_typedefd() {
    let out = run_c(
        "typedef int digest_t[4]; \
         int main() { digest_t d; d[0] = 40; d[3] = 2; printf(\"%d\\n\", d[0] + d[3]); return 0; }",
    );
    assert_eq!(out, "42\n");
}

// =============== And two words that meant nothing ===============

/// `inline` is a REQUEST, and the standard lets a compiler ignore it. BMO C
/// does not inline, so honouring it and ignoring it produce the same program --
/// the only difference was that the word made the file stop.
#[test]
fn inline_is_accepted_and_changes_nothing() {
    let out = run_c(
        "inline int twice(int v) { return v * 2; } \
         int main() { printf(\"%d\\n\", twice(21)); return 0; }",
    );
    assert_eq!(out, "42\n");
}

/// A line ending in `\` is not a line (C11 phase 2). Without this, a macro
/// written across several lines defines a body of `\` and drops the rest into
/// the file as code -- and the error lands on a line that should not exist.
#[test]
fn a_backslash_joins_the_next_line() {
    let out = run_c_con_pp(
        "#define PICK(a, b) \\\n\
         \x20   ((a) > (b) ? (a) : (b))\n\
         int main() { printf(\"%d\\n\", PICK(11, 42)); return 0; }",
    );
    assert_eq!(out.trim(), "42");
}
