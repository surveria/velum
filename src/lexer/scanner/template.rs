use super::Lexer;
use crate::{
    error::{Error, Result},
    lexer::{
        TemplatePart, TokenKind,
        classification::EscapeContext,
        support::{
            LINE_SEPARATOR, PARAGRAPH_SEPARATOR, TEMPLATE_SUBSTITUTION_START, push_utf16_char,
        },
        template::{TemplatePartPosition, TemplateSubstitutionState, template_part_value},
    },
};

impl Lexer {
    pub(super) fn template_literal(&mut self, offset: usize) -> Result<()> {
        self.advance();
        self.template_part(offset, TemplatePartPosition::Head)
    }

    fn template_part(&mut self, offset: usize, position: TemplatePartPosition) -> Result<()> {
        let mut output = Vec::new();
        let mut cooked_valid = true;
        let raw_start = offset
            .checked_add(1)
            .ok_or_else(|| Error::lex("template literal raw span overflowed", offset))?;

        while let Some((current_offset, ch)) = self.peek() {
            self.advance();
            if let Some(unit) = self.source.surrogate_at(current_offset) {
                output.push(unit);
                continue;
            }
            match ch {
                '`' => {
                    let cooked = cooked_valid.then_some(output);
                    let part =
                        template_part_value(&self.source, cooked, raw_start, current_offset)?;
                    return self.end_template_part(position, part, offset);
                }
                '$' if self.peek_char() == Some(TEMPLATE_SUBSTITUTION_START) => {
                    self.advance();
                    let cooked = cooked_valid.then_some(output);
                    let part =
                        template_part_value(&self.source, cooked, raw_start, current_offset)?;
                    return self.begin_template_substitution(
                        position,
                        part,
                        offset,
                        current_offset,
                    );
                }
                '\\' => {
                    if self.template_escape(current_offset, &mut output).is_err() {
                        cooked_valid = false;
                        output.clear();
                    }
                }
                '\n' => push_utf16_char(&mut output, '\n'),
                '\r' => {
                    if self.peek_char() == Some('\n') {
                        self.advance();
                    }
                    push_utf16_char(&mut output, '\n');
                }
                LINE_SEPARATOR | PARAGRAPH_SEPARATOR => push_utf16_char(&mut output, ch),
                other => push_utf16_char(&mut output, other),
            }
        }

        Err(Error::lex("unterminated template literal", offset))
    }

    fn end_template_part(
        &mut self,
        position: TemplatePartPosition,
        part: TemplatePart,
        offset: usize,
    ) -> Result<()> {
        match position {
            TemplatePartPosition::Head => {
                self.push(TokenKind::NoSubstitutionTemplate(part), offset);
            }
            TemplatePartPosition::Continuation => {
                if self.template_substitutions.pop().is_none() {
                    return Err(Error::lex(
                        "template substitution state underflowed",
                        offset,
                    ));
                }
                self.push(TokenKind::TemplateTail(part), offset);
            }
        }
        Ok(())
    }

    fn begin_template_substitution(
        &mut self,
        position: TemplatePartPosition,
        part: TemplatePart,
        offset: usize,
        substitution_offset: usize,
    ) -> Result<()> {
        match position {
            TemplatePartPosition::Head => {
                self.push(TokenKind::TemplateHead(part), offset);
                self.template_substitutions.push(TemplateSubstitutionState {
                    open_braces: 0,
                    substitution_offset,
                });
            }
            TemplatePartPosition::Continuation => {
                let Some(substitution) = self.template_substitutions.last_mut() else {
                    return Err(Error::lex(
                        "template substitution state underflowed",
                        offset,
                    ));
                };
                substitution.open_braces = 0;
                substitution.substitution_offset = substitution_offset;
                self.push(TokenKind::TemplateMiddle(part), offset);
            }
        }
        Ok(())
    }

    pub(super) fn substitution_brace_open(&mut self, offset: usize) -> Result<()> {
        if let Some(substitution) = self.template_substitutions.last_mut() {
            substitution.open_braces =
                substitution.open_braces.checked_add(1).ok_or_else(|| {
                    Error::lex("template substitution brace depth overflowed", offset)
                })?;
        }
        Ok(())
    }

    pub(super) fn right_brace_or_template_continuation(&mut self, offset: usize) -> Result<()> {
        match self.template_substitutions.last_mut() {
            Some(substitution) if substitution.open_braces == 0 => {
                self.advance();
                self.template_part(offset, TemplatePartPosition::Continuation)
            }
            Some(substitution) => {
                substitution.open_braces = substitution.open_braces.saturating_sub(1);
                self.simple(TokenKind::RBrace);
                Ok(())
            }
            None => {
                self.simple(TokenKind::RBrace);
                Ok(())
            }
        }
    }

    fn template_escape(&mut self, slash_offset: usize, output: &mut Vec<u16>) -> Result<()> {
        if self.escape_sequence(slash_offset, output, EscapeContext::Template)? {
            return Err(Error::lex(
                "legacy escape sequence is not allowed in a template literal",
                slash_offset,
            ));
        }
        Ok(())
    }
}
