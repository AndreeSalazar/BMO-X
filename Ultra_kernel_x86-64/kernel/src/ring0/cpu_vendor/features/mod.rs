//! `cpu_vendor::features` -- what this silicon offers, and what BMO takes.
//!
//! [carril]  VERDE     junta lo que ofrece con lo que se coge
//!
//! Sits **on top of the CPU profile that is already registered**: `profile.rs`
//! declares what we expect, `xsave.rs` asks the silicon about the extended
//! *state*, and this folder asks it about the instruction *set* -- the third
//! side of the same question.
//!
//! # Why it is a folder and not a file
//!
//! Because there are two jobs and mixing them is how this exact kind of table
//! goes wrong:
//!
//! ```text
//!    silicon.rs   what the CPU DECLARES   -- facts, from CPUID and CR4
//!    usage.rs     what BMO USES           -- contract, written by hand
//!    mod.rs       the census that joins them, and the CONFLICTS
//! ```
//!
//! ** That split is rule 5 made visible in the directory listing: *hardcode
//! CONTRACTS, ask the hardware for FACTS. Never the other way round.* A single
//! file would let one side quietly start deriving from the other, and then the
//! census would agree with itself instead of with the machine.
//!
//! # ** The drift is stopped by the COMPILER, not by discipline
//!
//! [`Feat`] is an enum, and both [`silicon::has`] and [`usage::of`] answer it
//! with an exhaustive `match`. **Adding a row breaks the build in both files
//! until both answer it.** That matters because the lesson already paid for in
//! this codebase is that *a guard which checks half gives a confidence it has
//! not earned* -- and a hand-written "BMO does not use AVX2" is exactly the
//! sort of line that stays there for a year after it stopped being true.
//!
//! [!] What the compiler CANNOT check is whether a [`usage::Use::Yes`] is true.
//! That column is written by hand against the tree. The rule that keeps it
//! honest is in `usage.rs`: **a `Yes` without a place named is a `Yes` that
//! lies.**
//!
//! # What the census is FOR, beyond being a pretty table
//!
//! One column is a list. Two columns are a decision. And the subtraction
//! between them has a name -- [`Censo::conflictos`] -- because *used and not
//! present* is not a curiosity: it is an instruction that will `#UD` on this
//! machine. Same shape as `xsave::coincide`.

pub mod silicon;
pub mod usage;

pub use silicon::Silicon;
pub use usage::Use;

/// The features this census covers.
///
/// ** They are ENUMERATED, not invented: each group is a family of the AMD64
/// manual, and inside a group everything relevant to BMO is listed even when
/// the answer is a plain no. A census that only lists what is interesting is a
/// census that already decided the answer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Feat {
    // -- vectors and maths --
    Sse2, Sse41, Sse42, Avx, Avx2, Fma, F16c,
    // -- bits: counting and scanning --
    Popcnt, Lzcnt, Bmi1, Bmi2, Movbe, Adx,
    // -- randomness --
    Rdrand, Rdseed,
    // -- cryptography --
    Aes, Pclmul, Sha,
    // -- extended state --
    Xsave, Osxsave, Xsaveopt, Xsavec, Xsaves,
    // -- memory and cache --
    Erms, Clflushopt, Clwb, Clzero, Pdpe1gb,
    // -- time and waiting --
    Rdtscp, InvariantTsc, Monitor, Monitorx,
    // -- protection the CPU offers for free --
    Nx, Smep, Smap, Umip,
}

/// Which family a feature belongs to. Only for grouping the report: a wall of
/// thirty-six rows with no headings is unreadable, and unreadable is the same
/// as absent when the way you debug is a photograph of the screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Group {
    Vector,
    Bits,
    Random,
    Crypto,
    State,
    Memory,
    Time,
    Guard,
}

impl Group {
    pub const fn title(self) -> &'static str {
        match self {
            Group::Vector => "vectores y matematica",
            Group::Bits => "bits: contar y escanear",
            Group::Random => "azar",
            Group::Crypto => "criptografia",
            Group::State => "estado extendido",
            Group::Memory => "memoria y cache",
            Group::Time => "tiempo y espera",
            Group::Guard => "proteccion que el CPU regala",
        }
    }
}

impl Feat {
    /// The short name, as the manual spells it. It is what a person will search
    /// for after seeing it on screen, so it is not translated.
    pub const fn name(self) -> &'static str {
        match self {
            Feat::Sse2 => "SSE2",
            Feat::Sse41 => "SSE4.1",
            Feat::Sse42 => "SSE4.2",
            Feat::Avx => "AVX",
            Feat::Avx2 => "AVX2",
            Feat::Fma => "FMA",
            Feat::F16c => "F16C",
            Feat::Popcnt => "POPCNT",
            Feat::Lzcnt => "LZCNT",
            Feat::Bmi1 => "BMI1",
            Feat::Bmi2 => "BMI2",
            Feat::Movbe => "MOVBE",
            Feat::Adx => "ADX",
            Feat::Rdrand => "RDRAND",
            Feat::Rdseed => "RDSEED",
            Feat::Aes => "AES-NI",
            Feat::Pclmul => "PCLMULQDQ",
            Feat::Sha => "SHA-NI",
            Feat::Xsave => "XSAVE",
            Feat::Osxsave => "CR4.OSXSAVE",
            Feat::Xsaveopt => "XSAVEOPT",
            Feat::Xsavec => "XSAVEC",
            Feat::Xsaves => "XSAVES",
            Feat::Erms => "ERMS",
            Feat::Clflushopt => "CLFLUSHOPT",
            Feat::Clwb => "CLWB",
            Feat::Clzero => "CLZERO",
            Feat::Pdpe1gb => "PAGINAS 1G",
            Feat::Rdtscp => "RDTSCP",
            Feat::InvariantTsc => "TSC INVARIANTE",
            Feat::Monitor => "MONITOR/MWAIT",
            Feat::Monitorx => "MONITORX",
            Feat::Nx => "NX",
            Feat::Smep => "SMEP",
            Feat::Smap => "SMAP",
            Feat::Umip => "UMIP",
        }
    }

    pub const fn group(self) -> Group {
        match self {
            Feat::Sse2 | Feat::Sse41 | Feat::Sse42 | Feat::Avx | Feat::Avx2
            | Feat::Fma | Feat::F16c => Group::Vector,
            Feat::Popcnt | Feat::Lzcnt | Feat::Bmi1 | Feat::Bmi2 | Feat::Movbe
            | Feat::Adx => Group::Bits,
            Feat::Rdrand | Feat::Rdseed => Group::Random,
            Feat::Aes | Feat::Pclmul | Feat::Sha => Group::Crypto,
            Feat::Xsave | Feat::Osxsave | Feat::Xsaveopt | Feat::Xsavec
            | Feat::Xsaves => Group::State,
            Feat::Erms | Feat::Clflushopt | Feat::Clwb | Feat::Clzero
            | Feat::Pdpe1gb => Group::Memory,
            Feat::Rdtscp | Feat::InvariantTsc | Feat::Monitor | Feat::Monitorx => Group::Time,
            Feat::Nx | Feat::Smep | Feat::Smap | Feat::Umip => Group::Guard,
        }
    }
}

/// Every feature, in report order. The one list both sides are checked against.
pub const ALL: &[Feat] = &[
    Feat::Sse2, Feat::Sse41, Feat::Sse42, Feat::Avx, Feat::Avx2, Feat::Fma, Feat::F16c,
    Feat::Popcnt, Feat::Lzcnt, Feat::Bmi1, Feat::Bmi2, Feat::Movbe, Feat::Adx,
    Feat::Rdrand, Feat::Rdseed,
    Feat::Aes, Feat::Pclmul, Feat::Sha,
    Feat::Xsave, Feat::Osxsave, Feat::Xsaveopt, Feat::Xsavec, Feat::Xsaves,
    Feat::Erms, Feat::Clflushopt, Feat::Clwb, Feat::Clzero, Feat::Pdpe1gb,
    Feat::Rdtscp, Feat::InvariantTsc, Feat::Monitor, Feat::Monitorx,
    Feat::Nx, Feat::Smep, Feat::Smap, Feat::Umip,
];

/// One row of the census: the two answers side by side.
#[derive(Debug, Clone, Copy)]
pub struct Fila {
    pub feat: Feat,
    /// The silicon declares it.
    pub hay: bool,
    /// BMO takes it, and where.
    pub uso: Use,
}

impl Fila {
    /// ** Used and not present. Not a curiosity: an instruction that will
    /// `#UD` on this machine the first time it runs.
    pub const fn conflicto(&self) -> bool {
        self.uso.is_yes() && !self.hay
    }

    /// Present and unused. Not a fault -- it is the list this census exists to
    /// produce, and every entry needs a reason written next to it.
    pub const fn desaprovechado(&self) -> bool {
        !self.uso.is_yes() && self.hay
    }
}

/// The whole census.
///
/// # ** THE THREE NUMBERS THAT MUST BE ZERO, AND WHY THEY ARE NUMBERS
///
/// The obvious place for `no row is mute` and `no feature is listed twice` is a
/// `#[cfg(test)]` block. **It would never run.** `cargo test` cannot build this
/// crate -- `no_std` plus the test harness gives a duplicate `panic_impl` -- so
/// a test written here is a test that exists and is never executed, which is
/// the exact pattern this codebase has paid for more than once.
///
/// So they are not tests: they are **counters that have to be zero, printed by
/// the report**. Same shape as `FatVolume::fallos_mudos()` and the xHCI parking
/// stats. A counter that must be zero is a giant needle; a test that never runs
/// is a comfort nobody earned.
pub struct Censo {
    pub filas: [Fila; 36],
    pub hay: u32,
    pub usadas: u32,
    /// ** Used and not declared by this silicon: an instruction that will
    /// `#UD` here. Must be zero.
    pub conflictos: u32,
    /// ** Rows whose note says nothing. A `Yes` with no place named cannot be
    /// verified and a `No` with no reason teaches nothing. Must be zero.
    pub mudas: u32,
    /// ** A feature listed twice in [`ALL`]: it would be counted twice in every
    /// total and nothing else would complain. Must be zero.
    pub repetidas: u32,
    /// ** [`ALL`] and the array do not measure the same: the census would stop
    /// short and report a smaller world than the one it looked at. Must be zero.
    pub sin_sitio: u32,
}

/// A note shorter than this is filler, not a reason.
const NOTA_MINIMA: usize = 10;

/// Ask the silicon once, join it with the declared usage, and count.
pub fn censar() -> Censo {
    let s = silicon::leer();
    // `Sse2` as filler: every slot is overwritten in the loop below, and any
    // slot that is not gets counted by `sin_sitio`.
    let vacia = Fila { feat: Feat::Sse2, hay: false, uso: usage::of(Feat::Sse2) };
    let mut c = Censo {
        filas: [vacia; 36],
        hay: 0,
        usadas: 0,
        conflictos: 0,
        mudas: 0,
        repetidas: 0,
        sin_sitio: 0,
    };

    // Lo que no cabe se CUENTA, no se descarta: una lista mas larga que su
    // array daria un censo corto que parece completo.
    if ALL.len() != c.filas.len() {
        c.sin_sitio = if ALL.len() > c.filas.len() {
            (ALL.len() - c.filas.len()) as u32
        } else {
            (c.filas.len() - ALL.len()) as u32
        };
    }

    let mut i = 0;
    while i < ALL.len() && i < c.filas.len() {
        let f = ALL[i];
        let fila = Fila { feat: f, hay: silicon::has(f, &s), uso: usage::of(f) };
        if fila.hay {
            c.hay += 1;
        }
        if fila.uso.is_yes() {
            c.usadas += 1;
        }
        if fila.conflicto() {
            c.conflictos += 1;
        }
        if fila.uso.nota().len() <= NOTA_MINIMA {
            c.mudas += 1;
        }
        // O(n^2) sobre treinta y seis, y solo cuando alguien escribe `ext`.
        let mut j = 0;
        while j < i {
            if ALL[j].name() == f.name() {
                c.repetidas += 1;
            }
            j += 1;
        }
        c.filas[i] = fila;
        i += 1;
    }
    c
}

impl Censo {
    /// Todos los contadores que tienen que ser cero, sumados. Existe para que
    /// el arranque pueda mirar UNA cifra en vez de acordarse de cuatro.
    pub const fn averias(&self) -> u32 {
        self.conflictos + self.mudas + self.repetidas + self.sin_sitio
    }
}
