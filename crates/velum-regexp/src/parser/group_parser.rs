use alloc::{boxed::Box, string::String};
use core::cmp::Ordering;

use crate::{CompileError, CompileErrorKind, Flags, ast::Node};

use super::Parser;

enum GroupKind {
    Capturing(Option<String>),
    NonCapturing,
    Lookahead(bool),
    Lookbehind(bool),
    Modifier { set: Flags, unset: Flags },
}

impl Parser<'_> {
    pub(super) fn parse_group(&mut self) -> Result<Node, CompileError> {
        self.advance_one()?;
        self.enter_depth()?;
        let mut kind = self.parse_group_kind()?;
        let capture_name = match &mut kind {
            GroupKind::Capturing(name) => name.take(),
            GroupKind::NonCapturing
            | GroupKind::Lookahead(_)
            | GroupKind::Lookbehind(_)
            | GroupKind::Modifier { .. } => None,
        };
        let capture_id = if matches!(&kind, GroupKind::Capturing(_)) {
            let id = self.capture_count;
            self.capture_count = self
                .capture_count
                .checked_add(1)
                .ok_or_else(|| self.error(CompileErrorKind::SizeOverflow))?;
            if self.capture_count > self.limits.max_captures {
                return Err(self.error(CompileErrorKind::CaptureLimit {
                    limit: self.limits.max_captures,
                }));
            }
            if let Some(name) = capture_name.as_ref()
                && self.capture_name_conflicts(name)
            {
                return Err(self.error(CompileErrorKind::DuplicateCaptureName));
            }
            self.capture_names.push(capture_name);
            self.capture_paths.push(self.alternative_path.clone());
            Some(id)
        } else {
            None
        };
        let modifiers = match &kind {
            GroupKind::Modifier { set, unset } => Some((*set, *unset)),
            GroupKind::Capturing(_)
            | GroupKind::NonCapturing
            | GroupKind::Lookahead(_)
            | GroupKind::Lookbehind(_) => None,
        };
        let outer_flags = self.flags;
        if let Some((set, unset)) = modifiers {
            self.flags = self.flags.apply_modifiers(set, unset);
        }
        let body_result = self.parse_disjunction(true);
        self.flags = outer_flags;
        let body = body_result?;
        if self.peek() != Some(u16::from(b')')) {
            return Err(self.error(CompileErrorKind::UnterminatedGroup));
        }
        self.advance_one()?;
        self.depth = self
            .depth
            .checked_sub(1)
            .ok_or_else(|| self.error(CompileErrorKind::SizeOverflow))?;
        match kind {
            GroupKind::Lookahead(positive) => self.node(Node::Lookahead {
                body: Box::new(body),
                positive,
            }),
            GroupKind::Lookbehind(positive) => self.node(Node::Lookbehind {
                body: Box::new(body),
                positive,
            }),
            GroupKind::Capturing(_) => {
                let id = capture_id.ok_or_else(|| self.error(CompileErrorKind::SizeOverflow))?;
                self.node(Node::Capture {
                    id,
                    body: Box::new(body),
                })
            }
            GroupKind::Modifier { set, unset } => self.node(Node::Modifier {
                body: Box::new(body),
                set,
                unset,
            }),
            GroupKind::NonCapturing => Ok(body),
        }
    }

    fn parse_group_kind(&mut self) -> Result<GroupKind, CompileError> {
        if self.peek() != Some(u16::from(b'?')) {
            return Ok(GroupKind::Capturing(None));
        }
        self.advance_one()?;
        match self.peek() {
            Some(value) if value == u16::from(b':') => {
                self.advance_one()?;
                Ok(GroupKind::NonCapturing)
            }
            Some(value) if value == u16::from(b'=') => {
                self.advance_one()?;
                Ok(GroupKind::Lookahead(true))
            }
            Some(value) if value == u16::from(b'!') => {
                self.advance_one()?;
                Ok(GroupKind::Lookahead(false))
            }
            Some(value) if value == u16::from(b'<') => {
                self.advance_one()?;
                match self.peek() {
                    Some(marker) if marker == u16::from(b'=') => {
                        self.advance_one()?;
                        Ok(GroupKind::Lookbehind(true))
                    }
                    Some(marker) if marker == u16::from(b'!') => {
                        self.advance_one()?;
                        Ok(GroupKind::Lookbehind(false))
                    }
                    _ => Ok(GroupKind::Capturing(Some(self.parse_capture_name()?))),
                }
            }
            Some(0x0069 | 0x006D | 0x0073 | 0x002D) => {
                let (set, unset) = self.parse_modifier_flags()?;
                Ok(GroupKind::Modifier { set, unset })
            }
            Some(_) | None => Err(self.error(CompileErrorKind::UnsupportedSyntax)),
        }
    }

    fn parse_modifier_flags(&mut self) -> Result<(Flags, Flags), CompileError> {
        let offset = self.position;
        let mut set = Flags::default();
        let mut unset = Flags::default();
        let mut removing = false;
        let mut saw_set = false;
        let mut saw_unset = false;
        loop {
            let Some(unit) = self.peek() else {
                return Err(CompileError::new(CompileErrorKind::InvalidModifier, offset));
            };
            if unit == u16::from(b':') {
                if !saw_set && !saw_unset {
                    return Err(CompileError::new(CompileErrorKind::InvalidModifier, offset));
                }
                self.advance_one()?;
                return Ok((set, unset));
            }
            if unit == u16::from(b'-') {
                if removing {
                    return Err(CompileError::new(CompileErrorKind::InvalidModifier, offset));
                }
                removing = true;
                self.advance_one()?;
                continue;
            }
            let (already_set, already_unset) = match unit {
                value if value == u16::from(b'i') => (set.ignore_case(), unset.ignore_case()),
                value if value == u16::from(b'm') => (set.multiline(), unset.multiline()),
                value if value == u16::from(b's') => (set.dot_all(), unset.dot_all()),
                _ => {
                    return Err(CompileError::new(CompileErrorKind::InvalidModifier, offset));
                }
            };
            if already_set || already_unset {
                return Err(CompileError::new(CompileErrorKind::InvalidModifier, offset));
            }
            if removing {
                unset = with_modifier_flag(unset, unit, true);
                saw_unset = true;
            } else {
                set = with_modifier_flag(set, unit, true);
                saw_set = true;
            }
            self.advance_one()?;
        }
    }

    fn capture_name_conflicts(&self, name: &str) -> bool {
        self.capture_names
            .iter()
            .zip(&self.capture_paths)
            .any(|(existing, path)| {
                existing.as_deref() == Some(name)
                    && !alternative_paths_are_disjoint(path, &self.alternative_path)
            })
    }
}

const fn with_modifier_flag(flags: Flags, unit: u16, enabled: bool) -> Flags {
    match unit {
        0x0069 => flags.with_ignore_case(enabled),
        0x006D => flags.with_multiline(enabled),
        0x0073 => flags.with_dot_all(enabled),
        _ => flags,
    }
}

fn alternative_paths_are_disjoint(left: &[(usize, usize)], right: &[(usize, usize)]) -> bool {
    let mut left_index = 0_usize;
    let mut right_index = 0_usize;
    while let (
        Some((left_disjunction, left_alternative)),
        Some((right_disjunction, right_alternative)),
    ) = (left.get(left_index), right.get(right_index))
    {
        match left_disjunction.cmp(right_disjunction) {
            Ordering::Equal => {
                if left_alternative != right_alternative {
                    return true;
                }
                left_index = left_index.saturating_add(1);
                right_index = right_index.saturating_add(1);
            }
            Ordering::Less => left_index = left_index.saturating_add(1),
            Ordering::Greater => right_index = right_index.saturating_add(1),
        }
    }
    false
}
