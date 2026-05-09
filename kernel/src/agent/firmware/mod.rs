//! GA106 Firmware Intelligence Subsystem.
//!
//! Responsible for extracting, cataloging, and analyzing NVIDIA firmware blobs
//! to build a knowledge database for FastOS.

pub mod metadata;
pub mod scanner;
pub mod embedded;
pub mod registry;
