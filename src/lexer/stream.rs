use crate::{Error, SourceId, SourceSpan};

use super::{
    LexicalGoal, SourceText, Token, TokenKind,
    classification::token_kind_can_precede_regexp,
    scanner::{Lexer, LexerCheckpoint},
};

struct BufferedToken {
    checkpoint: LexerCheckpoint,
    goal: LexicalGoal,
    goal_sensitive: bool,
    token: Token,
}

struct TokenStreamState {
    lexer: Lexer,
    tokens: Vec<BufferedToken>,
}

pub struct TokenStream {
    state: TokenStreamState,
}

impl TokenStream {
    pub(crate) fn from_source_text(
        source: SourceText,
        source_id: SourceId,
        allow_html_comments: bool,
    ) -> Self {
        let mut stream = Self {
            state: TokenStreamState {
                lexer: Lexer::new(source, source_id, allow_html_comments),
                tokens: Vec::new(),
            },
        };
        stream.fill_provisional_suffix();
        stream
    }

    pub(crate) fn get(&mut self, index: usize) -> Option<&Token> {
        self.get_with_goal(index, None)
    }

    pub(crate) fn cached(&self, index: usize) -> Option<&Token> {
        self.state.tokens.get(index).map(|entry| &entry.token)
    }

    pub(crate) fn get_with_goal(
        &mut self,
        index: usize,
        requested_goal: Option<LexicalGoal>,
    ) -> Option<&Token> {
        let invalidated =
            Self::invalidate_conflicting_slash(&mut self.state, index, requested_goal);
        while self.state.tokens.len() <= index {
            if Self::has_terminal_error(&self.state) {
                return None;
            }
            let goal = requested_goal
                .filter(|_| self.state.tokens.len() == index)
                .unwrap_or_else(|| Self::automatic_goal(&self.state));
            Self::push_next(&mut self.state, goal);
        }
        if invalidated {
            self.fill_provisional_suffix();
        }
        self.state.tokens.get(index).map(|entry| &entry.token)
    }

    fn fill_provisional_suffix(&mut self) {
        while !Self::has_terminal_token(&self.state) {
            let goal = Self::automatic_goal(&self.state);
            Self::push_next(&mut self.state, goal);
        }
    }

    fn push_next(state: &mut TokenStreamState, goal: LexicalGoal) {
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
        let goal_sensitive = Self::goal_sensitive(&state.lexer, &token);
        state.tokens.push(BufferedToken {
            checkpoint,
            goal,
            goal_sensitive,
            token,
        });
    }

    fn invalidate_conflicting_slash(
        state: &mut TokenStreamState,
        index: usize,
        requested_goal: Option<LexicalGoal>,
    ) -> bool {
        let Some(requested_goal) = requested_goal else {
            return false;
        };
        let Some(entry) = state.tokens.get(index) else {
            return false;
        };
        if entry.goal == requested_goal || !entry.goal_sensitive {
            return false;
        }
        let checkpoint = entry.checkpoint.clone();
        state.tokens.truncate(index);
        state.lexer.restore(&checkpoint);
        true
    }

    fn goal_sensitive(lexer: &Lexer, token: &Token) -> bool {
        matches!(
            token.kind,
            TokenKind::Slash | TokenKind::SlashEqual | TokenKind::RegExp { .. }
        ) || matches!(token.kind, TokenKind::LexicalError(_))
            && lexer.is_slash_offset(token.span.start())
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

    fn has_terminal_error(state: &TokenStreamState) -> bool {
        state
            .tokens
            .last()
            .is_some_and(|entry| matches!(entry.token.kind, TokenKind::LexicalError(_)))
    }

    fn has_terminal_token(state: &TokenStreamState) -> bool {
        state.tokens.last().is_some_and(|entry| {
            matches!(
                entry.token.kind,
                TokenKind::Eof | TokenKind::LexicalError(_)
            )
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
