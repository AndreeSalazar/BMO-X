//! C++ Lexer — extends the C lexer with C++ keywords and tokens.
//!
//! Approach: delegate to the C lexer first, then post-process tokens
//! to add C++-specific entries. This keeps the C lexer unchanged
//! while allowing the C++ parser to see extra tokens.

#![allow(dead_code)]

extern crate alloc;
use alloc::vec::Vec;

use crate::bmo_gpu::BxResult;
use super::super::c::lexer::{CLexer, CToken};

/// C++ token — extends `CToken` with class-related variants.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CppToken {
    C(CToken),
    /// `class Foo { ... };`
    Class,
    /// `public:`, `private:`, `protected:`
    Access(CppAccess),
    /// `virtual`
    Virtual,
    /// `override`
    Override,
    /// `final`
    Final,
    /// `nullptr`
    Nullptr,
    /// `this`
    This,
    /// `new Type(args)`
    New,
    /// `delete ptr`
    Delete,
    /// `~Foo()` (destructor)
    Tilde,
    /// `Foo::method`
    Scope,
    /// `Foo<T>` (template — recognized but lowered to Foo)
    Lt, Cgt,
    /// `using namespace foo;`
    Using,
    /// `namespace foo { ... }`
    Namespace,
    /// `try { ... } catch (...) { ... }`
    Try,
    /// `catch (Type e) { ... }`
    Catch,
    /// `throw expr;`
    Throw,
    /// `explicit` (constructor)
    Explicit,
    /// `inline` (function)
    Inline,
    /// `friend`
    Friend,
    /// `operator` (overload — recognized, lowered to regular call)
    Operator,
    /// `template <typename T>`
    Template,
    /// `typename`
    Typename,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CppAccess {
    Public,
    Private,
    Protected,
}

/// C++ Lexer.
pub struct CppLexer {
    inner: CLexer,
}

impl CppLexer {
    pub fn new(source: &[u8]) -> Self {
        Self { inner: CLexer::new(source) }
    }

    /// Tokenize C++ source. Returns a stream of C++ tokens.
    pub fn tokenize(&mut self) -> BxResult<Vec<CppToken>> {
        let c_tokens = self.inner.tokenize()?;
        let mut out = Vec::with_capacity(c_tokens.len());

        for tok in c_tokens {
            let cpp_tok = match &tok {
                CToken::Ident(name) => {
                    let lower = name.to_ascii_lowercase();
                    match lower.as_str() {
                        "class" => CppToken::Class,
                        "public" => CppToken::Access(CppAccess::Public),
                        "private" => CppToken::Access(CppAccess::Private),
                        "protected" => CppToken::Access(CppAccess::Protected),
                        "virtual" => CppToken::Virtual,
                        "override" => CppToken::Override,
                        "final" => CppToken::Final,
                        "nullptr" => CppToken::Nullptr,
                        "this" => CppToken::This,
                        "new" => CppToken::New,
                        "delete" => CppToken::Delete,
                        "using" => CppToken::Using,
                        "namespace" => CppToken::Namespace,
                        "try" => CppToken::Try,
                        "catch" => CppToken::Catch,
                        "throw" => CppToken::Throw,
                        "explicit" => CppToken::Explicit,
                        "inline" => CppToken::Inline,
                        "friend" => CppToken::Friend,
                        "operator" => CppToken::Operator,
                        "template" => CppToken::Template,
                        "typename" => CppToken::Typename,
                        _ => CppToken::C(tok),
                    }
                }
                CToken::StructOp => CppToken::Scope,
                CToken::Tilde => CppToken::Tilde,
                CToken::Lt => CppToken::Lt,
                CToken::Gt => CppToken::Cgt,
                _ => CppToken::C(tok),
            };
            out.push(cpp_tok);
        }
        Ok(out)
    }
}
