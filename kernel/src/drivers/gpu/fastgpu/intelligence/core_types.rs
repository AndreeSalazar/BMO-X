//! Core Intelligence Types
//! Auto-generated. Do not edit.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfidenceLevel {
    Confirmed,
    Inferred,
    Experimental,
    Unknown,
}

#[derive(Debug, Clone, Copy)]
pub struct RegisterDescriptor {
    pub offset: u32,
    pub name: &'static str,
    pub source: &'static str,
    pub confidence: ConfidenceLevel,
}

#[derive(Debug, Clone, Copy)]
pub struct EngineDescriptor {
    pub name: &'static str,
    pub mmio_base: u32,
    pub requires_falcon: bool,
    pub requires_authenticated_boot: bool,
    pub confidence: ConfidenceLevel,
}

#[derive(Debug, Clone, Copy)]
pub struct SequenceStep {
    pub step_num: u32,
    pub actor: &'static str,
    pub action: &'static str,
}

#[derive(Debug, Clone, Copy)]
pub struct BootSequence {
    pub name: &'static str,
    pub target: &'static str,
    pub steps: &'static [SequenceStep],
}
