//! Embedded C standard profiles (C89 through C23).

use super::StandardProfile;

pub static C89: StandardProfile = StandardProfile {
    iso_number: "ISO/IEC 9899:1990",
    year: 1990,
    short_name: "C89",
    language: "c",
    features: &[
        ("void_type", true), ("signed_unsigned", true), ("enum", true),
        ("struct_union", true), ("function_prototypes", true),
        ("void_pointers", true), ("const", true), ("volatile", true),
        ("string_literals_concat", true), ("trigraphs", true),
        ("long_long", false), ("inline", false), ("restrict", false),
        ("_Bool", false), ("compound_literals", false),
    ],
    macros: &[("__STDC__", 1), ("__STDC_VERSION__", 0)],
    parent: None,
};

pub static C99: StandardProfile = StandardProfile {
    iso_number: "ISO/IEC 9899:1999",
    year: 1999,
    short_name: "C99",
    language: "c",
    features: &[
        ("_Complex", true), ("_Bool", true), ("long_long", true),
        ("inline", true), ("restrict", true), ("variadic_macros", true),
        ("compound_literals", true), ("designated_initializers", true),
        ("flexible_array_members", true), ("mixed_declarations_and_code", true),
        ("line_comments", true), ("variable_length_arrays", true),
        ("implicit_int", false), ("implicit_function_decl", false),
    ],
    macros: &[("__STDC__", 1), ("__STDC_VERSION__", 199901)],
    parent: Some("c89"),
};

pub static C11: StandardProfile = StandardProfile {
    iso_number: "ISO/IEC 9899:2011",
    year: 2011,
    short_name: "C11",
    language: "c",
    features: &[
        ("_Atomic", true), ("_Generic", true), ("_Static_assert", true),
        ("_Thread_local", true), ("_Noreturn", true), ("_Alignas", true),
        ("_Alignof", true), ("unicode_strings", true),
        ("anonymous_structs", true), ("anonymous_unions", true),
        ("quick_exit", true), ("aligned_alloc", true),
        ("implicit_int", false), ("implicit_function_decl", false),
    ],
    macros: &[("__STDC__", 1), ("__STDC_VERSION__", 201112)],
    parent: Some("c99"),
};

pub static C17: StandardProfile = StandardProfile {
    iso_number: "ISO/IEC 9899:2018",
    year: 2018,
    short_name: "C17",
    language: "c",
    features: &[
        ("implicit_int", false), ("implicit_function_decl", false),
    ],
    macros: &[("__STDC__", 1), ("__STDC_VERSION__", 201710)],
    parent: Some("c11"),
};

pub static C23: StandardProfile = StandardProfile {
    iso_number: "ISO/IEC 9899:2024",
    year: 2024,
    short_name: "C23",
    language: "c",
    features: &[
        ("typeof", true), ("constexpr", true), ("auto_type", true),
        ("nullptr", true), ("_embed", true), ("digit_separators", true),
        ("binary_literals", true), ("attributes", true),
        ("empty_initializer", true), ("improved_integer_promotions", true),
        ("implicit_int", false), ("implicit_function_decl", false),
    ],
    macros: &[("__STDC__", 1), ("__STDC_VERSION__", 202311)],
    parent: Some("c17"),
};

pub static ALL: &[&StandardProfile] = &[&C89, &C99, &C11, &C17, &C23];

pub fn find(version: &str) -> Option<&'static StandardProfile> {
    match version.to_lowercase().as_str() {
        "c89" | "c90" => Some(&C89),
        "c99" => Some(&C99),
        "c11" => Some(&C11),
        "c17" | "c18" => Some(&C17),
        "c23" | "c24" => Some(&C23),
        _ => None,
    }
}
