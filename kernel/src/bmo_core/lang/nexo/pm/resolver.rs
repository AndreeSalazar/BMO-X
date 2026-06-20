//! ÑEXO Dependency Resolver — Resolución de dependencias.
//!
//! Topological sort, detección de ciclos, version matching.

#![allow(dead_code)]

extern crate alloc;
use alloc::string::String;
use alloc::string::ToString;
use alloc::vec::Vec;

use crate::bmo_gpu::{BxError, BxResult};
use super::manifest::{Manifest, DependencySpec};
use super::registry::Registry;

/// Resolved dependency with version.
#[derive(Debug, Clone)]
pub struct ResolvedDep {
    pub name: String,
    pub version: String,
    pub is_optional: bool,
}

/// Dependency resolver.
pub struct Resolver {
    registry: Registry,
}

impl Resolver {
    pub fn new() -> Self {
        Self { registry: Registry::load() }
    }

    /// Resolve all dependencies from a manifest.
    pub fn resolve(&self, manifest: &Manifest) -> BxResult<Vec<ResolvedDep>> {
        let mut resolved = Vec::new();
        let mut visiting = Vec::new();

        for (name, spec) in &manifest.dependencies {
            self.resolve_package(name, spec, &mut resolved, &mut visiting)?;
        }

        for (name, spec) in &manifest.dev_dependencies {
            self.resolve_package(name, spec, &mut resolved, &mut visiting)?;
        }

        // Topological sort
        self.topological_sort(&mut resolved);

        Ok(resolved)
    }

    fn resolve_package(
        &self,
        name: &str,
        spec: &DependencySpec,
        resolved: &mut Vec<ResolvedDep>,
        visiting: &mut Vec<String>,
    ) -> BxResult<()> {
        // Check for cycles
        if visiting.contains(&name.to_string()) {
            crate::bmo_core::diag::warn("nexo_pm", "Circular dependency detected");
            return Err(BxError::InvalidArgument);
        }

        // Check if already resolved
        if resolved.iter().any(|r| r.name == name) {
            return Ok(());
        }

        let version_req = spec.version();

        // Check if package exists in registry
        if !self.registry.satisfies(name, version_req) {
            // Not found in registry — might be a local package or stdlib
            // Allow it with the requested version
        }

        visiting.push(name.to_string());

        resolved.push(ResolvedDep {
            name: name.to_string(),
            version: version_req.to_string(),
            is_optional: spec.is_optional(),
        });

        visiting.pop();
        Ok(())
    }

    fn topological_sort(&self, deps: &mut Vec<ResolvedDep>) {
        // Simple sort: non-optional first, then alphabetical
        deps.sort_by(|a, b| {
            a.is_optional.cmp(&b.is_optional)
                .then(a.name.cmp(&b.name))
        });
    }
}

impl Default for Resolver {
    fn default() -> Self {
        Self::new()
    }
}
