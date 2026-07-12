use std::cell::RefCell;

use crate::{Error, SourceId, SourceSpan};

use super::{
    Token, TokenKind,
    classification::token_kind_can_precede_regexp,
    scanner::{Lexer, LexerCheckpoint, LexicalGoal},
};

struct BufferedToken {
    checkpoint: LexerCheckpoint,
    goal: LexicalGoal,
    token: Token,
}

struct TokenStreamState {
    lexer: Lexer,
    tokens: Vec<BufferedToken>,
}

pub(crate) struct TokenStream {
    state: RefCell<TokenStreamState>,
}

impl TokenStream {
    pub(crate) fn new(source: &str, source_id: SourceId) -> Self {
        Self {
            state: RefCell::new(TokenStreamState {
                lexer: Lexer::new(source, source_id),
                tokens: Vec::new(),
            }),
        }
    }

    pub(crate) fn get(&self, index: usize) -> Option<Token> {
        self.get_with_goal(index, None)
    }

    pub(crate) fn get_with_goal(
        &self,
        index: usize,
        requested_goal: Option<LexicalGoal>,
    ) -> Option<Token> {
        let mut state = self.state.borrow_mut();
        Self::invalidate_conflicting_slash(&mut state, index, requested_goal);
        while state.tokens.len() <= index {
            if let Some(error) = Self::terminal_error(&state) {
                return Some(error);
            }
            let goal = requested_goal
                .filter(|_| state.tokens.len() == index)
                .unwrap_or_else(|| Self::automatic_goal(&state));
            let checkpoint = state.lexer.checkpoint();
            let token = match state.lexer.next_token(goal) {
                Ok(token) => token,
                Err(error) => Token {
                    span: Self::error_span(&error),
                    kind: TokenKind::LexicalError(Box::new(error)),
                    line_terminator_before: false,
                    identifier_escaped: false,
                },
            };
            state.tokens.push(BufferedToken {
                checkpoint,
                goal,
                token,
            });
        }
        state.tokens.get(index).map(|entry| entry.token.clone())
    }

    fn invalidate_conflicting_slash(
        state: &mut TokenStreamState,
        index: usize,
        requested_goal: Option<LexicalGoal>,
    ) {
        let Some(requested_goal) = requested_goal else {
            return;
        };
        let Some(entry) = state.tokens.get(index) else {
            return;
        };
        if entry.goal == requested_goal {
            return;
        }
        let checkpoint = entry.checkpoint.clone();
        state.tokens.truncate(index);
        state.lexer.restore(&checkpoint);
    }

    /// Supplies speculative lookahead only. Parser-selected goals replace it
    /// before a slash-sensitive token becomes part of the accepted grammar.
    fn automatic_goal(state: &TokenStreamState) -> LexicalGoal {
        state.tokens.last().map_or(LexicalGoal::RegExp, |entry| {
            if token_kind_can_precede_regexp(&entry.token.kind) {
                LexicalGoal::RegExp
            } else {
                LexicalGoal::Div
            }
        })
    }

    fn terminal_error(state: &TokenStreamState) -> Option<Token> {
        state.tokens.last().and_then(|entry| {
            matches!(entry.token.kind, TokenKind::LexicalError(_)).then(|| entry.token.clone())
        })
    }

    const fn error_span(error: &Error) -> SourceSpan {
        match error {
            Error::Lex { span, .. }
            | Error::Parse { span, .. }
            | Error::Runtime {
                span: Some(span), ..
            }
            | Error::ResourceLimit {
                span: Some(span), ..
            } => *span,
            Error::JavaScript { .. }
            | Error::JavaScriptError { .. }
            | Error::Runtime { span: None, .. }
            | Error::ResourceLimit { span: None, .. } => SourceSpan::point(SourceId::UNKNOWN, 0),
        }
    }
}
