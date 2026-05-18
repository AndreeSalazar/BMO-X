//! Lexer — convierte texto fuente en stream de `Token`.

pub mod token;
pub mod scanner;

pub use token::{Token, TokenKind};
pub use scanner::Scanner;
