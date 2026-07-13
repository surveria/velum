use super::Lexer;
use crate::{
    error::{Error, Result},
    lexer::TokenKind,
};

impl Lexer {
    pub(super) fn simple(&mut self, kind: TokenKind) {
        let offset = self.peek().map_or(self.source.len(), |(offset, _)| offset);
        self.advance();
        self.push(kind, offset);
    }

    pub(super) fn plus_or_increment(&mut self, offset: usize) {
        self.advance();
        if self.match_char('+') {
            self.push(TokenKind::PlusPlus, offset);
        } else if self.match_char('=') {
            self.push(TokenKind::PlusEqual, offset);
        } else {
            self.push(TokenKind::Plus, offset);
        }
    }

    pub(super) fn minus_or_decrement(&mut self, offset: usize) {
        self.advance();
        if self.match_char('-') {
            self.push(TokenKind::MinusMinus, offset);
        } else if self.match_char('=') {
            self.push(TokenKind::MinusEqual, offset);
        } else {
            self.push(TokenKind::Minus, offset);
        }
    }

    pub(super) fn star_or_power(&mut self, offset: usize) {
        self.advance();
        if !self.match_char('*') {
            if self.match_char('=') {
                self.push(TokenKind::StarEqual, offset);
            } else {
                self.push(TokenKind::Star, offset);
            }
            return;
        }
        if self.match_char('=') {
            self.push(TokenKind::StarStarEqual, offset);
        } else {
            self.push(TokenKind::StarStar, offset);
        }
    }

    pub(super) fn dot_token(&mut self, offset: usize) -> Result<()> {
        self.advance();
        if !self.match_char('.') {
            self.push(TokenKind::Dot, offset);
            return Ok(());
        }
        if !self.match_char('.') {
            return Err(Error::lex("unexpected '..'", offset));
        }
        self.push(TokenKind::DotDotDot, offset);
        Ok(())
    }

    pub(super) fn bang_token(&mut self, offset: usize) {
        self.advance();
        if self.match_char('=') {
            if self.match_char('=') {
                self.push(TokenKind::StrictNotEqual, offset);
            } else {
                self.push(TokenKind::BangEqual, offset);
            }
        } else {
            self.push(TokenKind::Bang, offset);
        }
    }

    pub(super) fn equal_token(&mut self, offset: usize) {
        self.advance();
        if self.match_char('>') {
            self.push(TokenKind::Arrow, offset);
        } else if self.match_char('=') {
            if self.match_char('=') {
                self.push(TokenKind::StrictEqual, offset);
            } else {
                self.push(TokenKind::EqualEqual, offset);
            }
        } else {
            self.push(TokenKind::Equal, offset);
        }
    }

    pub(super) fn ampersand_token(&mut self, offset: usize) {
        self.advance();
        if self.match_char('&') {
            if self.match_char('=') {
                self.push(TokenKind::AndAndEqual, offset);
            } else {
                self.push(TokenKind::AndAnd, offset);
            }
        } else if self.match_char('=') {
            self.push(TokenKind::AmpersandEqual, offset);
        } else {
            self.push(TokenKind::Ampersand, offset);
        }
    }

    pub(super) fn pipe_token(&mut self, offset: usize) {
        self.advance();
        if self.match_char('|') {
            if self.match_char('=') {
                self.push(TokenKind::OrOrEqual, offset);
            } else {
                self.push(TokenKind::OrOr, offset);
            }
        } else if self.match_char('=') {
            self.push(TokenKind::PipeEqual, offset);
        } else {
            self.push(TokenKind::Pipe, offset);
        }
    }

    pub(super) fn question_token(&mut self, offset: usize) {
        self.advance();
        if self.match_char('?') {
            if self.match_char('=') {
                self.push(TokenKind::QuestionQuestionEqual, offset);
            } else {
                self.push(TokenKind::QuestionQuestion, offset);
            }
        } else if self.peek_char() == Some('.')
            && !self.peek_next_char().is_some_and(|ch| ch.is_ascii_digit())
        {
            self.advance();
            self.push(TokenKind::QuestionDot, offset);
        } else {
            self.push(TokenKind::Question, offset);
        }
    }

    pub(super) fn less_token(&mut self, offset: usize) {
        self.advance();
        if self.match_char('<') {
            if self.match_char('=') {
                self.push(TokenKind::LessLessEqual, offset);
            } else {
                self.push(TokenKind::LessLess, offset);
            }
        } else if self.match_char('=') {
            self.push(TokenKind::LessEqual, offset);
        } else {
            self.push(TokenKind::Less, offset);
        }
    }

    pub(super) fn greater_token(&mut self, offset: usize) {
        self.advance();
        if self.match_char('>') {
            self.greater_shift_token(offset);
        } else if self.match_char('=') {
            self.push(TokenKind::GreaterEqual, offset);
        } else {
            self.push(TokenKind::Greater, offset);
        }
    }

    fn greater_shift_token(&mut self, offset: usize) {
        if self.match_char('>') {
            if self.match_char('=') {
                self.push(TokenKind::GreaterGreaterGreaterEqual, offset);
            } else {
                self.push(TokenKind::GreaterGreaterGreater, offset);
            }
        } else if self.match_char('=') {
            self.push(TokenKind::GreaterGreaterEqual, offset);
        } else {
            self.push(TokenKind::GreaterGreater, offset);
        }
    }

    pub(super) fn simple_or_equal(&mut self, offset: usize, plain: TokenKind, assigned: TokenKind) {
        self.advance();
        if self.match_char('=') {
            self.push(assigned, offset);
        } else {
            self.push(plain, offset);
        }
    }
}
