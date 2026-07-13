use crate::lexer::TokenKind;

use super::Parser;

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
enum Delimiter {
    Paren,
    Brace,
    Bracket,
}

impl Delimiter {
    const fn opening(kind: &TokenKind) -> Option<Self> {
        match kind {
            TokenKind::LParen => Some(Self::Paren),
            TokenKind::LBrace => Some(Self::Brace),
            TokenKind::LBracket => Some(Self::Bracket),
            _ => None,
        }
    }

    const fn closing(kind: &TokenKind) -> Option<Self> {
        match kind {
            TokenKind::RParen => Some(Self::Paren),
            TokenKind::RBrace => Some(Self::Brace),
            TokenKind::RBracket => Some(Self::Bracket),
            _ => None,
        }
    }
}

impl Parser {
    /// Returns the offset of the closing delimiter paired with the opening
    /// token at `opening_offset`. Mismatched delimiter kinds are never treated
    /// as balanced, so every grammar lookahead shares the same boundary.
    pub(super) fn balanced_closing_offset(&mut self, opening_offset: usize) -> Option<usize> {
        let opening = Delimiter::opening(self.peek_kind(opening_offset)?)?;
        let mut delimiters = vec![opening];
        let mut offset = opening_offset.checked_add(1)?;
        loop {
            let kind = self.peek_kind(offset)?;
            if let Some(closing) = Delimiter::closing(kind) {
                if delimiters.pop() != Some(closing) {
                    return None;
                }
                if delimiters.is_empty() {
                    return Some(offset);
                }
            } else if let Some(opening) = Delimiter::opening(kind) {
                delimiters.push(opening);
            } else if matches!(kind, TokenKind::Eof | TokenKind::LexicalError(_)) {
                return None;
            }
            offset = offset.checked_add(1)?;
        }
    }
}
