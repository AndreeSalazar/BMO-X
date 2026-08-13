//! **THE CENSUS HARNESS** -- sweep a matrix of cells and compare the whole
//! report against what was written down last time.
//!
//! ## Why this is a module and not a copied function
//!
//! The pattern was born in `probe_language` and it worked: it turned four
//! reboots of the Ryzen into one 0.25 s run. The moment a SECOND axis showed up
//! --aggregate layout, which is what DOOM needs in `R_Init`-- the choice was to
//! copy forty lines or to lift them here.
//!
//! Copying would have been house pattern 26 wearing a new coat: two copies of
//! one rule, and the second one gets fixed whenever somebody remembers.
//!
//! ## The three properties to preserve, none of them accidental
//!
//! 1. **A broken cell does not hide the others.** Each one runs inside a
//!    `catch_unwind`, so one that does not even compile is recorded and the
//!    sweep carries on. A census that stops at the first hole is not a census.
//! 2. **The suite stays GREEN with open defects**, because the census tells the
//!    truth -- including what does not work. A `BROKEN` row with its exact
//!    symptom beside it beats a line in a `TODO`.
//! 3. ** **The moment reality changes, the test fails.** Fix a `BROKEN` or
//!    break a `GOOD` and the report stops matching the constant, so it has to
//!    be updated. That is the cure for the document that lies, which this house
//!    has already paid for more than once.

use super::*;

/// One cell of the census: what it is called, the program that exercises it,
/// and what that program has to print.
pub(super) struct Cell {
    pub name: &'static str,
    pub source: &'static str,
    pub expects: &'static str,
}

/// Run every cell and compare the report against `expected`.
///
/// [!] The panic hook is silenced for the duration of the sweep: without that,
/// the output fills with backtraces from the broken cells and the report --the
/// thing you actually need to read-- is lost among them.
pub(super) fn sweep(cells: &[Cell], expected: &str, warning: &str) {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(|_| {}));

    let mut report = String::new();
    for c in cells {
        let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| run_c_con_pp(c.source)));
        let verdict = match r {
            Ok(out) if out.trim() == c.expects => "GOOD".to_string(),
            Ok(out) => format!("BROKEN gives {:?}, wants {:?}", out.trim(), c.expects),
            Err(_) => "DOES NOT COMPILE or blows up".to_string(),
        };
        // The width is 30 and not that of the longest name on purpose: the few
        // that overflow push their verdict one column out, and that marks them
        // in the report without needing a flag.
        report.push_str(&format!("{:<30} {}\n", c.name, verdict));
    }

    std::panic::set_hook(previous);

    assert_eq!(report.trim_end(), expected.trim_end(), "\n\n{warning}\n");
}
