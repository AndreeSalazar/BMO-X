//! Embedded C++ standard profiles (C++98 through C++23).

use super::StandardProfile;

pub static CPP98: StandardProfile = StandardProfile {
    iso_number: "ISO/IEC 14882:1998",
    year: 1998,
    short_name: "C++98",
    language: "cpp",
    features: &[
        ("classes", true),
        ("single_inheritance", true),
        ("virtual_functions", true),
        ("vtables", true),
        ("templates", true),
        ("exceptions", true),
        ("namespaces", true),
        ("bool_type", true),
        ("mutable", true),
        ("explicit", true),
        ("RTTI", true),
        ("operator_overloading", true),
        ("function_overloading", true),
        ("default_arguments", true),
        ("references", true),
        ("new_delete", true),
        ("inline_functions", true),
        ("STL_basic", true),
        ("constexpr", false),
        ("auto", false),
        ("lambda", false),
        ("move_semantics", false),
        ("concepts", false),
    ],
    macros: &[("__cplusplus", 199711)],
    parent: None,
};

pub static CPP11: StandardProfile = StandardProfile {
    iso_number: "ISO/IEC 14882:2011",
    year: 2011,
    short_name: "C++11",
    language: "cpp",
    features: &[
        ("auto", true),
        ("decltype", true),
        ("lambda", true),
        ("rvalue_references", true),
        ("move_semantics", true),
        ("constexpr", true),
        ("nullptr", true),
        ("range_for", true),
        ("override", true),
        ("final", true),
        ("static_assert", true),
        ("variadic_templates", true),
        ("noexcept", true),
        ("enum_class", true),
        ("raw_string_literals", true),
        ("explicit_conversion", true),
        ("type_alias", true),
    ],
    macros: &[("__cplusplus", 201103)],
    parent: Some("cpp98"),
};

pub static CPP14: StandardProfile = StandardProfile {
    iso_number: "ISO/IEC 14882:2014",
    year: 2014,
    short_name: "C++14",
    language: "cpp",
    features: &[
        ("generic_lambdas", true),
        ("return_type_deduction", true),
        ("decltype_auto", true),
        ("variable_templates", true),
        ("binary_literals", true),
        ("digit_separators", true),
        ("constexpr_relaxed", true),
        ("make_unique", true),
    ],
    macros: &[("__cplusplus", 201402)],
    parent: Some("cpp11"),
};

pub static CPP17: StandardProfile = StandardProfile {
    iso_number: "ISO/IEC 14882:2017",
    year: 2017,
    short_name: "C++17",
    language: "cpp",
    features: &[
        ("if_constexpr", true),
        ("structured_bindings", true),
        ("fold_expressions", true),
        ("nested_namespaces", true),
        ("inline_variables", true),
        ("if_init_statements", true),
        ("constexpr_lambdas", true),
        ("string_view", true),
        ("optional", true),
        ("variant", true),
        ("any", true),
        ("filesystem", true),
        ("guaranteed_copy_elision", true),
    ],
    macros: &[("__cplusplus", 201703)],
    parent: Some("cpp14"),
};

pub static CPP20: StandardProfile = StandardProfile {
    iso_number: "ISO/IEC 14882:2020",
    year: 2020,
    short_name: "C++20",
    language: "cpp",
    features: &[
        ("concepts", true),
        ("coroutines", true),
        ("ranges", true),
        ("modules", true),
        ("spaceship_operator", true),
        ("consteval", true),
        ("constinit", true),
        ("designated_initializers", true),
        ("char8_t", true),
        ("format_library", true),
        ("span", true),
        ("jthread", true),
    ],
    macros: &[("__cplusplus", 202002)],
    parent: Some("cpp17"),
};

pub static CPP23: StandardProfile = StandardProfile {
    iso_number: "ISO/IEC 14882:2023",
    year: 2023,
    short_name: "C++23",
    language: "cpp",
    features: &[
        ("deducing_this", true),
        ("if_consteval", true),
        ("multidimensional_subscript", true),
        ("static_operator", true),
        ("monadic_optional", true),
        ("expected", true),
        ("flat_map", true),
        ("flat_set", true),
        ("mdspan", true),
        ("print", true),
        ("ranges_zip", true),
        ("stacktrace", true),
        ("assume", true),
    ],
    macros: &[("__cplusplus", 202302)],
    parent: Some("cpp20"),
};

pub static ALL: &[&StandardProfile] = &[&CPP98, &CPP11, &CPP14, &CPP17, &CPP20, &CPP23];

pub fn find(version: &str) -> Option<&'static StandardProfile> {
    match version.to_lowercase().as_str() {
        "c++98" | "cpp98" => Some(&CPP98),
        "c++11" | "cpp11" => Some(&CPP11),
        "c++14" | "cpp14" => Some(&CPP14),
        "c++17" | "cpp17" => Some(&CPP17),
        "c++20" | "cpp20" => Some(&CPP20),
        "c++23" | "cpp23" => Some(&CPP23),
        _ => None,
    }
}
