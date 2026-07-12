//! Embedded COBOL standard profiles (COBOL-85 through COBOL 2023).

use super::StandardProfile;

pub static COBOL85: StandardProfile = StandardProfile {
    iso_number: "ISO 1989:1985",
    year: 1985, short_name: "COBOL-85", language: "cobol",
    features: &[
        ("scope_terminators", true), ("inline_perform", true),
        ("nested_programs", false), ("evaluate", false),
        ("intrinsic_functions", false), ("object_oriented", false),
        ("recursive_programs", false), ("screen_section", false),
        ("free_format", false), ("report_writer", false),
        ("national_locale", false), ("dynamic_length_tables", false),
    ],
    macros: &[("COBOL_STANDARD", 1985)],
    parent: None,
};

pub static COBOL2002: StandardProfile = StandardProfile {
    iso_number: "ISO/IEC 1989:2002",
    year: 2002, short_name: "COBOL 2002", language: "cobol",
    features: &[
        ("scope_terminators", true), ("inline_perform", true),
        ("nested_programs", true), ("evaluate", true),
        ("intrinsic_functions", true), ("object_oriented", true),
        ("recursive_programs", true), ("screen_section", false),
        ("free_format", true), ("report_writer", false),
        ("national_locale", true), ("dynamic_length_tables", false),
        ("exception_handling", true), ("user_functions", true),
        ("float_binary", true), ("local_storage_section", true),
    ],
    macros: &[("COBOL_STANDARD", 2002)],
    parent: Some("cobol85"),
};

pub static COBOL2014: StandardProfile = StandardProfile {
    iso_number: "ISO/IEC 1989:2014",
    year: 2014, short_name: "COBOL 2014", language: "cobol",
    features: &[
        ("scope_terminators", true), ("inline_perform", true),
        ("nested_programs", true), ("evaluate", true),
        ("intrinsic_functions", true), ("object_oriented", true),
        ("recursive_programs", true), ("screen_section", false),
        ("free_format", true), ("report_writer", false),
        ("national_locale", true), ("dynamic_length_tables", true),
        ("exception_handling", true), ("user_functions", true),
        ("float_binary", true), ("local_storage_section", true),
        ("method_overloading", true), ("factory_methods", true),
        ("property_methods", true), ("enumerations", true),
        ("constant_entries", true), ("external_repository", true),
    ],
    macros: &[("COBOL_STANDARD", 2014)],
    parent: Some("cobol2002"),
};

pub static COBOL2023: StandardProfile = StandardProfile {
    iso_number: "ISO/IEC 1989:2023",
    year: 2023, short_name: "COBOL 2023", language: "cobol",
    features: &[
        ("scope_terminators", true), ("inline_perform", true),
        ("nested_programs", true), ("evaluate", true),
        ("intrinsic_functions", true), ("object_oriented", true),
        ("recursive_programs", true), ("screen_section", true),
        ("free_format", true), ("report_writer", true),
        ("national_locale", true), ("dynamic_length_tables", true),
        ("exception_handling", true), ("user_functions", true),
        ("float_binary", true), ("local_storage_section", true),
        ("method_overloading", true), ("factory_methods", true),
        ("property_methods", true), ("enumerations", true),
        ("constant_entries", true), ("external_repository", true),
        ("json_parse", true), ("json_generate", true),
        ("xml_parse", true), ("xml_generate", true),
        ("utf8_support", true), ("conditional_compilation", true),
    ],
    macros: &[("COBOL_STANDARD", 2023)],
    parent: Some("cobol2014"),
};

pub static ALL: &[&StandardProfile] = &[&COBOL85, &COBOL2002, &COBOL2014, &COBOL2023];

pub fn find(version: &str) -> Option<&'static StandardProfile> {
    match version.to_lowercase().as_str() {
        "cobol-85" | "cobol85" => Some(&COBOL85),
        "cobol-2002" | "cobol2002" => Some(&COBOL2002),
        "cobol-2014" | "cobol2014" => Some(&COBOL2014),
        "cobol-2023" | "cobol2023" => Some(&COBOL2023),
        _ => None,
    }
}
