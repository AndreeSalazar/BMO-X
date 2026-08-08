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
