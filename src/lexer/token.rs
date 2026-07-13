use std::rc::Rc;

use crate::{Error, SourceSpan, value::JsBigInt};

#[derive(Debug, Clone, PartialEq)]
pub struct Token {
    pub kind: TokenKind,
    pub span: SourceSpan,
    pub line_terminator_before: bool,
    pub identifier_escaped: bool,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct StringToken {
    pub cooked: Rc<[u16]>,
    pub escape_free: bool,
    pub legacy_escape: bool,
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub struct NumberToken {
    pub value: f64,
    pub legacy: bool,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct TemplatePart {
    pub cooked: Option<Rc<[u16]>>,
    pub raw: Rc<[u16]>,
}

impl Token {
    pub const fn offset(&self) -> usize {
        self.span.start()
    }

    pub fn is_unescaped_identifier_named(&self, expected: &str) -> bool {
        !self.identifier_escaped
            && matches!(&self.kind, TokenKind::Identifier(name) if name.as_ref() == expected)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum TokenKind {
    #[doc(hidden)]
    LexicalError(Box<Error>),
    Number(NumberToken),
    BigInt(JsBigInt),
    String(StringToken),
    NoSubstitutionTemplate(TemplatePart),
    TemplateHead(TemplatePart),
    TemplateMiddle(TemplatePart),
    TemplateTail(TemplatePart),
    RegExp {
        pattern: Rc<str>,
        flags: Rc<str>,
    },
    Identifier(Rc<str>),
    /// A `#name` private identifier; the text keeps its leading `#` so
    /// private names can never collide with public identifier names.
    PrivateName(Rc<str>),
    Let,
    Const,
    Var,
    If,
    Else,
    Do,
    While,
    For,
    Switch,
    Case,
    Default,
    Class,
    Extends,
    Break,
    Continue,
    Debugger,
    Try,
    Catch,
    Finally,
    Throw,
    Return,
    Function,
    Async,
    Await,
    Super,
    Import,
    Export,
    Enum,
    With,
    New,
    This,
    In,
    InstanceOf,
    Typeof,
    Void,
    Delete,
    True,
    False,
    Null,
    Plus,
    PlusPlus,
    PlusEqual,
    Minus,
    MinusMinus,
    MinusEqual,
    Star,
    StarStar,
    StarEqual,
    StarStarEqual,
    Slash,
    SlashEqual,
    Percent,
    PercentEqual,
    Bang,
    Tilde,
    Arrow,
    Equal,
    EqualEqual,
    BangEqual,
    StrictEqual,
    StrictNotEqual,
    Less,
    LessEqual,
    LessLess,
    LessLessEqual,
    Greater,
    GreaterEqual,
    GreaterGreater,
    GreaterGreaterEqual,
    GreaterGreaterGreater,
    GreaterGreaterGreaterEqual,
    Ampersand,
    AmpersandEqual,
    Pipe,
    PipeEqual,
    Caret,
    CaretEqual,
    AndAnd,
    AndAndEqual,
    OrOr,
    OrOrEqual,
    QuestionQuestion,
    QuestionQuestionEqual,
    QuestionDot,
    Question,
    Colon,
    Dot,
    DotDotDot,
    LParen,
    RParen,
    LBrace,
    RBrace,
    LBracket,
    RBracket,
    Semicolon,
    Comma,
    Eof,
}
