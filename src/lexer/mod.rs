mod classification;
mod scanner;
mod stream;
mod support;
mod template;
mod token;

pub use stream::TokenStream;
pub use token::{StringToken, TemplatePart, Token, TokenKind};

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum LexicalGoal {
    Div,
    RegExp,
}
