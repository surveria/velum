use crate::lexer::TokenKind;
use crate::lexer::support::{is_identifier_part, is_identifier_start};

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub(super) enum IdentifierPosition {
    Start,
    Part,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub(super) enum EscapeContext {
    String,
    Template,
}

pub(super) fn identifier_position_allows(ch: char, position: IdentifierPosition) -> bool {
    match position {
        IdentifierPosition::Start => is_identifier_start(ch),
        IdentifierPosition::Part => is_identifier_part(ch),
    }
}

pub(super) fn identifier_kind(text: String, escaped: bool) -> TokenKind {
    if escaped && text == "async" {
        return TokenKind::Identifier(Rc::from(text.into_boxed_str()));
    }
    match text.as_str() {
        "let" => TokenKind::Let,
        "const" => TokenKind::Const,
        "var" => TokenKind::Var,
        "if" => TokenKind::If,
        "else" => TokenKind::Else,
        "do" => TokenKind::Do,
        "while" => TokenKind::While,
        "for" => TokenKind::For,
        "switch" => TokenKind::Switch,
        "case" => TokenKind::Case,
        "default" => TokenKind::Default,
        "class" => TokenKind::Class,
        "extends" => TokenKind::Extends,
        "break" => TokenKind::Break,
        "continue" => TokenKind::Continue,
        "debugger" => TokenKind::Debugger,
        "try" => TokenKind::Try,
        "catch" => TokenKind::Catch,
        "finally" => TokenKind::Finally,
        "throw" => TokenKind::Throw,
        "return" => TokenKind::Return,
        "function" => TokenKind::Function,
        "async" => TokenKind::Async,
        "await" => TokenKind::Await,
        "super" => TokenKind::Super,
        "import" => TokenKind::Import,
        "export" => TokenKind::Export,
        "enum" => TokenKind::Enum,
        "with" => TokenKind::With,
        "new" => TokenKind::New,
        "this" => TokenKind::This,
        "in" => TokenKind::In,
        "instanceof" => TokenKind::InstanceOf,
        "typeof" => TokenKind::Typeof,
        "void" => TokenKind::Void,
        "delete" => TokenKind::Delete,
        "true" => TokenKind::True,
        "false" => TokenKind::False,
        "null" => TokenKind::Null,
        _ => TokenKind::Identifier(Rc::from(text.into_boxed_str())),
    }
}

pub(super) const fn token_kind_can_precede_regexp(kind: &TokenKind) -> bool {
    !matches!(
        kind,
        TokenKind::Number(_)
            | TokenKind::BigInt(_)
            | TokenKind::String(_)
            | TokenKind::NoSubstitutionTemplate(_)
            | TokenKind::TemplateTail(_)
            | TokenKind::RegExp { .. }
            | TokenKind::Identifier(_)
            | TokenKind::PrivateName(_)
            | TokenKind::This
            | TokenKind::True
            | TokenKind::False
            | TokenKind::Null
            | TokenKind::PlusPlus
            | TokenKind::MinusMinus
            | TokenKind::RParen
            | TokenKind::RBracket
            | TokenKind::RBrace
    )
}
use std::rc::Rc;
