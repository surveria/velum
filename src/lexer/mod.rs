mod classification;
mod scanner;
mod source_text;
mod stream;
mod support;
mod template;
mod token;

pub(crate) use source_text::SourceText;
pub use stream::TokenStream;
pub use token::{NumberToken, StringToken, TemplatePart, Token, TokenKind};

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum LexicalGoal {
    Div,
    RegExp,
}
