//! Python builtins — names that are available in every Python scope.
//!
//! These are linked to BMO runtime functions when the program runs.

#![allow(dead_code)]

pub const BUILTINS: &[&str] = &[
    "print", "len", "range", "int", "str", "float", "bool",
    "list", "dict", "tuple", "set",
    "abs", "min", "max", "sum", "sorted", "reversed",
    "enumerate", "zip", "map", "filter",
    "isinstance", "type", "id", "hash",
    "open", "input", "chr", "ord", "hex", "bin", "oct",
    "repr", "format",
    "True", "False", "None",
];
