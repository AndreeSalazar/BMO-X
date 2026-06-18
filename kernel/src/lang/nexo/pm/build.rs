//! ÑEXO Build System — Compilación de paquetes.

#![allow(dead_code)]

use alloc::vec::Vec;

use crate::barex::BxResult;
use super::manifest::Manifest;
use super::resolver::Resolver;

/// Build result.
#[derive(Debug)]
pub struct BuildResult {
    pub success: bool,
    pub compiled_files: usize,
    pub errors: Vec<alloc::string::String>,
}

/// Build system for ÑEXO packages.
pub struct BuildSystem;

impl BuildSystem {
    pub fn new() -> Self { Self }

    /// Build a project from its manifest.
    pub fn build(&self, _manifest: &Manifest) -> BxResult<BuildResult> {
        let resolver = Resolver::new();
        let deps = resolver.resolve(_manifest)?;

        crate::diag::info("nexo_build", "Building package");

        let mut compiled = 0;
        let errors = Vec::new();

        for dep in &deps {
            if dep.is_optional {
                crate::diag::info("nexo_build", "Skipping optional dep");
                continue;
            }
            crate::diag::info("nexo_build", "Resolving dep");
            compiled += 1;
        }

        crate::diag::info("nexo_build", "Compiling package");
        compiled += 1;

        let success = errors.is_empty();
        if success {
            crate::diag::info("nexo_build", "Build succeeded");
        } else {
            crate::diag::warn("nexo_build", "Build failed");
        }

        Ok(BuildResult { success, compiled_files: compiled, errors })
    }

    /// Build a single ÑEXO source file.
    pub fn build_file(&self, source: &[u8]) -> BxResult<Vec<u8>> {
        crate::lang::nexo::compile(source)
    }

    /// Build a single C source file.
    pub fn build_c_file(&self, source: &[u8]) -> BxResult<Vec<u8>> {
        crate::lang::nexo::plugins::languages::c::translator::compile_c_to_native(source)
    }
}

impl Default for BuildSystem {
    fn default() -> Self { Self::new() }
}
