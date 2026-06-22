//! Java exceptions — try/catch lowered to BMO control flow.
//!
//! Strategy: each `try { ... } catch (T e) { ... }` block becomes:
//!   1. Save the current exception handler (setjmp-style).
//!   2. Execute the try body.
//!   3. On throw, restore the saved state and jump to the catch body.
//!
//! In BMO, we use the BMO exception ABI (BmoStatus + handler table).

#![allow(dead_code)]

extern crate alloc;
use alloc::string::String;
use alloc::string::ToString;
use alloc::vec::Vec;

use super::ast::*;

/// One entry in the exception handler table.
pub struct CatchHandler {
    /// BMO exception class name to match.
    pub catch_type: String,
    /// Offset to jump to if this handler matches.
    pub handler_offset: u32,
}

/// Plan for compiling a try/catch/finally block.
pub struct ExceptionPlan {
    pub handlers: Vec<CatchHandler>,
    pub has_finally: bool,
}

pub fn plan_exception(jstmt: &JStmt) -> Option<ExceptionPlan> {
    if let JStmt::Try { catches, finally, .. } = jstmt {
        let handlers = catches.iter().map(|c| CatchHandler {
            catch_type: c.catch_type.clone().unwrap_or_else(|| "Exception".to_string()),
            handler_offset: 0, // filled by codegen
        }).collect();
        Some(ExceptionPlan {
            handlers,
            has_finally: finally.is_some(),
        })
    } else { None }
}

