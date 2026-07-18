mod class_parser;
mod escape_parser;
mod name_parser;
mod unicode_set_parser;

use core::cmp::Ordering;

use crate::{
    CompileError, CompileErrorKind, CompileLimits, Flags,
    ast::{Node, ParsedPattern},
    character_class::{CharacterClassTerm, DIGIT_RANGES, SPACE_RANGES, WORD_RANGES},
};

pub struct Parser<'a> {
    pattern: &'a [u16],
    flags: Flags,
    limits: CompileLimits,
    position: usize,
    depth: usize,
    node_count: usize,
    capture_count: usize,
    total_capture_count: usize,
    has_named_capture: bool,
    capture_names: Vec<Option<String>>,
    disjunction_count: usize,
    alternative_path: Vec<(usize, usize)>,
    capture_paths: Vec<Vec<(usize, usize)>>,
}

enum PropertyEscape {
    CodePoints(CharacterClassTerm),
    Strings(Vec<Box<[u32]>>),
}

enum GroupKind {
    Capturing(Option<String>),
    NonCapturing,
    Lookahead(bool),
    Lookbehind(bool),
    Modifier { set: Flags, unset: Flags },
}

impl<'a> Parser<'a> {
    pub(super) fn parse(
        pattern: &'a [u16],
        flags: Flags,
        limits: CompileLimits,
    ) -> Result<ParsedPattern, CompileError> {
        if flags.unicode() && flags.unicode_sets() {
            return Err(CompileError::new(
                CompileErrorKind::IncompatibleUnicodeFlags,
                0,
            ));
        }
        if pattern.len() > limits.max_pattern_units {
            return Err(CompileError::new(
                CompileErrorKind::PatternTooLong {
                    limit: limits.max_pattern_units,
                },
                limits.max_pattern_units,
            ));
        }
        let capture_scan = scan_capturing_groups(pattern)?;
        let mut parser = Self {
            pattern,
            flags,
            limits,
            position: 0,
            depth: 0,
            node_count: 0,
            capture_count: 0,
            total_capture_count: capture_scan.count,
            has_named_capture: capture_scan.has_named,
            capture_names: Vec::new(),
            disjunction_count: 0,
            alternative_path: Vec::new(),
            capture_paths: Vec::new(),
        };
        let mut root = parser.parse_disjunction(false)?;
        if parser.position != pattern.len() {
            return Err(parser.error(CompileErrorKind::UnexpectedToken));
        }
        resolve_backreferences(&mut root, parser.capture_count, &parser.capture_names)?;
        Ok(ParsedPattern {
            root,
            capture_count: parser.capture_count,
            capture_names: parser.capture_names,
        })
    }

    fn parse_disjunction(&mut self, in_group: bool) -> Result<Node, CompileError> {
        let disjunction_id = self.disjunction_count;
        self.disjunction_count = self
            .disjunction_count
            .checked_add(1)
            .ok_or_else(|| self.error(CompileErrorKind::SizeOverflow))?;
        let mut alternatives = Vec::new();
        let mut alternative_id = 0_usize;
        loop {
            self.alternative_path.push((disjunction_id, alternative_id));
            let alternative_result = self.parse_alternative();
            self.alternative_path
                .pop()
                .ok_or_else(|| self.error(CompileErrorKind::SizeOverflow))?;
            alternatives.push(alternative_result?);
            if self.peek() != Some(u16::from(b'|')) {
                break;
            }
            self.advance_one()?;
            alternative_id = alternative_id
                .checked_add(1)
                .ok_or_else(|| self.error(CompileErrorKind::SizeOverflow))?;
        }
        if !in_group && self.peek() == Some(u16::from(b')')) {
            return Err(self.error(CompileErrorKind::UnexpectedToken));
        }
        if alternatives.len() == 1 {
            alternatives
                .pop()
                .ok_or_else(|| self.error(CompileErrorKind::SizeOverflow))
        } else {
            self.node(Node::Alternation(alternatives))
        }
    }

    fn parse_alternative(&mut self) -> Result<Node, CompileError> {
        let mut terms = Vec::new();
        while let Some(unit) = self.peek() {
            if matches!(unit, value if value == u16::from(b'|') || value == u16::from(b')')) {
                break;
            }
            terms.push(self.parse_term()?);
        }
        match terms.len() {
            0 => self.node(Node::Empty),
            1 => terms
                .pop()
                .ok_or_else(|| self.error(CompileErrorKind::SizeOverflow)),
            _ => self.node(Node::Concat(terms)),
        }
    }

    fn parse_term(&mut self) -> Result<Node, CompileError> {
        let atom_offset = self.position;
        let atom = self.parse_atom()?;
        let assertion = matches!(
            atom,
            Node::AssertStart
                | Node::AssertEnd
                | Node::WordBoundary(_)
                | Node::Lookahead { .. }
                | Node::Lookbehind { .. }
        );
        let Some((min, max)) = self.parse_quantifier_bounds()? else {
            return Ok(atom);
        };
        if assertion {
            return Err(CompileError::new(
                CompileErrorKind::InvalidQuantifier,
                atom_offset,
            ));
        }
        let greedy = if self.peek() == Some(u16::from(b'?')) {
            self.advance_one()?;
            false
        } else {
            true
        };
        if self.next_starts_quantifier()? {
            return Err(self.error(CompileErrorKind::InvalidQuantifier));
        }
        self.node(Node::Repeat {
            body: Box::new(atom),
            min,
            max,
            greedy,
        })
    }

    fn parse_atom(&mut self) -> Result<Node, CompileError> {
        let Some(unit) = self.peek() else {
            return Err(self.error(CompileErrorKind::UnexpectedToken));
        };
        match unit {
            value if value == u16::from(b'.') => {
                self.advance_one()?;
                self.node(Node::Any)
            }
            value if value == u16::from(b'^') => {
                self.advance_one()?;
                self.node(Node::AssertStart)
            }
            value if value == u16::from(b'$') => {
                self.advance_one()?;
                self.node(Node::AssertEnd)
            }
            value if value == u16::from(b'(') => self.parse_group(),
            value if value == u16::from(b'[') => self.parse_character_class(),
            value if value == u16::from(b'\\') => self.parse_escape(),
            value
                if value == u16::from(b'*')
                    || value == u16::from(b'+')
                    || value == u16::from(b'?')
                    || value == u16::from(b')') =>
            {
                Err(self.error(CompileErrorKind::UnexpectedToken))
            }
            _ => {
                let value = self.decode_pattern_value()?;
                self.node(Node::Literal(value))
            }
        }
    }

    fn parse_group(&mut self) -> Result<Node, CompileError> {
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
                if (removing || !saw_set) && !saw_unset {
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

    fn parse_quantifier_bounds(&mut self) -> Result<Option<(u32, Option<u32>)>, CompileError> {
        match self.peek() {
            Some(value) if value == u16::from(b'*') => {
                self.advance_one()?;
                Ok(Some((0, None)))
            }
            Some(value) if value == u16::from(b'+') => {
                self.advance_one()?;
                Ok(Some((1, None)))
            }
            Some(value) if value == u16::from(b'?') => {
                self.advance_one()?;
                Ok(Some((0, Some(1))))
            }
            Some(value) if value == u16::from(b'{') => self.parse_braced_quantifier(),
            _ => Ok(None),
        }
    }

    fn parse_braced_quantifier(&mut self) -> Result<Option<(u32, Option<u32>)>, CompileError> {
        let checkpoint = self.position;
        self.advance_one()?;
        let Some(min) = self.parse_decimal()? else {
            self.position = checkpoint;
            return Ok(None);
        };
        let max = match self.peek() {
            Some(value) if value == u16::from(b'}') => Some(min),
            Some(value) if value == u16::from(b',') => {
                self.advance_one()?;
                self.parse_decimal()?
            }
            _ => {
                self.position = checkpoint;
                return Ok(None);
            }
        };
        if self.peek() != Some(u16::from(b'}')) {
            self.position = checkpoint;
            return Ok(None);
        }
        self.advance_one()?;
        if min > self.limits.max_repeat_count
            || max.is_some_and(|value| value > self.limits.max_repeat_count)
        {
            return Err(CompileError::new(
                CompileErrorKind::RepeatLimit {
                    limit: self.limits.max_repeat_count,
                },
                checkpoint,
            ));
        }
        if max.is_some_and(|value| value < min) {
            return Err(CompileError::new(
                CompileErrorKind::InvalidQuantifier,
                checkpoint,
            ));
        }
        Ok(Some((min, max)))
    }

    fn parse_decimal(&mut self) -> Result<Option<u32>, CompileError> {
        let mut value = 0_u32;
        let mut found = false;
        while let Some(unit) = self.peek() {
            if !(u16::from(b'0')..=u16::from(b'9')).contains(&unit) {
                break;
            }
            found = true;
            let digit = u32::from(unit - u16::from(b'0'));
            value = value
                .checked_mul(10)
                .and_then(|current| current.checked_add(digit))
                .ok_or_else(|| {
                    self.error(CompileErrorKind::RepeatLimit {
                        limit: self.limits.max_repeat_count,
                    })
                })?;
            self.advance_one()?;
        }
        Ok(found.then_some(value))
    }

    fn next_starts_quantifier(&mut self) -> Result<bool, CompileError> {
        match self.peek() {
            Some(value)
                if value == u16::from(b'*')
                    || value == u16::from(b'+')
                    || value == u16::from(b'?') =>
            {
                Ok(true)
            }
            Some(value) if value == u16::from(b'{') => {
                let checkpoint = self.position;
                let parsed = self.parse_braced_quantifier()?.is_some();
                self.position = checkpoint;
                Ok(parsed)
            }
            _ => Ok(false),
        }
    }

    fn decode_pattern_value(&mut self) -> Result<u32, CompileError> {
        let start = self.position;
        let Some(first) = self.peek() else {
            return Err(self.error(CompileErrorKind::UnexpectedToken));
        };
        self.advance_one()?;
        if !self.flags.has_unicode_mode() || !(0xD800..=0xDBFF).contains(&first) {
            return Ok(u32::from(first));
        }
        let Some(second) = self.peek() else {
            return Ok(u32::from(first));
        };
        if !(0xDC00..=0xDFFF).contains(&second) {
            return Ok(u32::from(first));
        }
        self.advance_one()?;
        let high = u32::from(first - 0xD800);
        let low = u32::from(second - 0xDC00);
        0x1_0000_u32
            .checked_add(
                high.checked_mul(0x400)
                    .ok_or_else(|| CompileError::new(CompileErrorKind::SizeOverflow, start))?,
            )
            .and_then(|value| value.checked_add(low))
            .ok_or_else(|| CompileError::new(CompileErrorKind::SizeOverflow, start))
    }

    fn enter_depth(&mut self) -> Result<(), CompileError> {
        self.depth = self
            .depth
            .checked_add(1)
            .ok_or_else(|| self.error(CompileErrorKind::SizeOverflow))?;
        if self.depth > self.limits.max_nesting_depth {
            return Err(self.error(CompileErrorKind::NestingLimit {
                limit: self.limits.max_nesting_depth,
            }));
        }
        Ok(())
    }

    fn node(&mut self, node: Node) -> Result<Node, CompileError> {
        self.node_count = self
            .node_count
            .checked_add(1)
            .ok_or_else(|| self.error(CompileErrorKind::SizeOverflow))?;
        if self.node_count > self.limits.max_nodes {
            return Err(self.error(CompileErrorKind::NodeLimit {
                limit: self.limits.max_nodes,
            }));
        }
        Ok(node)
    }

    fn peek(&self) -> Option<u16> {
        self.pattern.get(self.position).copied()
    }

    fn advance_one(&mut self) -> Result<(), CompileError> {
        self.position = self
            .position
            .checked_add(1)
            .ok_or_else(|| self.error(CompileErrorKind::SizeOverflow))?;
        Ok(())
    }

    const fn error(&self, kind: CompileErrorKind) -> CompileError {
        CompileError::new(kind, self.position)
    }
}

const fn is_syntax_character(value: u16) -> bool {
    matches!(
        value,
        0x005E
            | 0x0024
            | 0x005C
            | 0x002E
            | 0x002A
            | 0x002B
            | 0x003F
            | 0x0028
            | 0x0029
            | 0x005B
            | 0x005D
            | 0x007B
            | 0x007D
            | 0x007C
            | 0x002F
    )
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

const fn predefined_class_term(value: u16) -> Result<CharacterClassTerm, CompileError> {
    let (ranges, inverted) = match value {
        0x0064 => (DIGIT_RANGES, false),
        0x0044 => (DIGIT_RANGES, true),
        0x0073 => (SPACE_RANGES, false),
        0x0053 => (SPACE_RANGES, true),
        0x0077 => (WORD_RANGES, false),
        0x0057 => (WORD_RANGES, true),
        _ => {
            return Err(CompileError::new(
                CompileErrorKind::InvalidCharacterClass,
                0,
            ));
        }
    };
    Ok(CharacterClassTerm::StaticRanges {
        ranges,
        inverted,
        complement_before_case_fold: false,
    })
}

struct CaptureScan {
    count: usize,
    has_named: bool,
}

fn scan_capturing_groups(pattern: &[u16]) -> Result<CaptureScan, CompileError> {
    let mut index = 0_usize;
    let mut in_class = false;
    let mut scan = CaptureScan {
        count: 0,
        has_named: false,
    };
    while let Some(unit) = pattern.get(index).copied() {
        index = index
            .checked_add(1)
            .ok_or_else(|| CompileError::new(CompileErrorKind::SizeOverflow, index))?;
        if unit == u16::from(b'\\') {
            if index < pattern.len() {
                index = index
                    .checked_add(1)
                    .ok_or_else(|| CompileError::new(CompileErrorKind::SizeOverflow, index))?;
            }
            continue;
        }
        if unit == u16::from(b'[') && !in_class {
            in_class = true;
            continue;
        }
        if unit == u16::from(b']') && in_class {
            in_class = false;
            continue;
        }
        if unit != u16::from(b'(') || in_class {
            continue;
        }
        let question = pattern.get(index).copied() == Some(u16::from(b'?'));
        let angle_index = index
            .checked_add(1)
            .ok_or_else(|| CompileError::new(CompileErrorKind::SizeOverflow, index))?;
        let marker_index = angle_index
            .checked_add(1)
            .ok_or_else(|| CompileError::new(CompileErrorKind::SizeOverflow, index))?;
        let named = question
            && pattern.get(angle_index).copied() == Some(u16::from(b'<'))
            && !matches!(pattern.get(marker_index).copied(), Some(marker) if marker == u16::from(b'=') || marker == u16::from(b'!'));
        if question && !named {
            continue;
        }
        scan.count = scan
            .count
            .checked_add(1)
            .ok_or_else(|| CompileError::new(CompileErrorKind::SizeOverflow, index))?;
        scan.has_named |= named;
    }
    Ok(scan)
}

fn resolve_backreferences(
    node: &mut Node,
    capture_count: usize,
    capture_names: &[Option<String>],
) -> Result<(), CompileError> {
    match node {
        Node::Backreference { id, pattern_offset } if *id >= capture_count => Err(
            CompileError::new(CompileErrorKind::InvalidBackreference, *pattern_offset),
        ),
        Node::NamedBackreference {
            name,
            pattern_offset,
        } => {
            let ids = capture_names
                .iter()
                .enumerate()
                .filter_map(|(id, candidate)| (candidate.as_ref() == Some(name)).then_some(id))
                .collect::<Vec<_>>();
            match ids.as_slice() {
                [] => {
                    return Err(CompileError::new(
                        CompileErrorKind::UnknownCaptureName,
                        *pattern_offset,
                    ));
                }
                [id] => {
                    *node = Node::Backreference {
                        id: *id,
                        pattern_offset: *pattern_offset,
                    };
                }
                _ => {
                    *node = Node::BackreferenceSet { ids };
                }
            }
            Ok(())
        }
        Node::Concat(nodes) | Node::Alternation(nodes) => {
            for child in nodes {
                resolve_backreferences(child, capture_count, capture_names)?;
            }
            Ok(())
        }
        Node::Capture { body, .. }
        | Node::Repeat { body, .. }
        | Node::Lookahead { body, .. }
        | Node::Lookbehind { body, .. }
        | Node::Modifier { body, .. } => resolve_backreferences(body, capture_count, capture_names),
        Node::Empty
        | Node::Literal(_)
        | Node::Backreference { .. }
        | Node::BackreferenceSet { .. }
        | Node::Class(_)
        | Node::Any
        | Node::WordBoundary(_)
        | Node::AssertStart
        | Node::AssertEnd => Ok(()),
    }
}
