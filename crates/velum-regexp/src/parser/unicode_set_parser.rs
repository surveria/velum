use std::collections::BTreeSet;

use crate::{
    CompileError, CompileErrorKind,
    ast::Node,
    character_class::{CharacterClass, CharacterClassTerm},
};

use super::{Parser, PropertyEscape, predefined_class_term};

struct ClassSet {
    term: CharacterClassTerm,
    strings: BTreeSet<Vec<u32>>,
    expression_depth: usize,
    term_count: usize,
    evaluation_work: usize,
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum SetOperator {
    Intersection,
    Subtraction,
}

impl ClassSet {
    const fn from_term(term: CharacterClassTerm) -> Self {
        Self {
            term,
            strings: BTreeSet::new(),
            expression_depth: 1,
            term_count: 1,
            evaluation_work: 1,
        }
    }

    fn from_strings(
        strings: BTreeSet<Vec<u32>>,
        term_limit: usize,
        offset: usize,
    ) -> Result<Self, CompileError> {
        let mut terms = Vec::new();
        let mut remaining_strings = BTreeSet::new();
        for sequence in strings {
            if let [value] = sequence.as_slice() {
                if terms.len() >= term_limit {
                    return Err(CompileError::new(
                        CompileErrorKind::NodeLimit { limit: term_limit },
                        offset,
                    ));
                }
                terms.push(CharacterClassTerm::Range {
                    start: *value,
                    end: *value,
                });
            } else {
                remaining_strings.insert(sequence);
            }
        }
        let expression_depth = if terms.is_empty() { 1 } else { 2 };
        let term_count = terms.len();
        let evaluation_work = term_count
            .checked_add(1)
            .ok_or_else(|| CompileError::new(CompileErrorKind::SizeOverflow, offset))?;
        Ok(Self {
            term: CharacterClassTerm::Union(terms),
            strings: remaining_strings,
            expression_depth,
            term_count,
            evaluation_work,
        })
    }

    fn union(
        mut self,
        mut right: Self,
        depth_limit: usize,
        term_limit: usize,
        offset: usize,
    ) -> Result<Self, CompileError> {
        let left_is_union = matches!(&self.term, CharacterClassTerm::Union(_));
        let right_is_union = matches!(&right.term, CharacterClassTerm::Union(_));
        let left_nested = if right_is_union {
            self.expression_depth.checked_add(1)
        } else {
            Some(self.expression_depth)
        };
        let right_nested = if left_is_union {
            right.expression_depth.checked_add(1)
        } else {
            Some(right.expression_depth)
        };
        let expression_depth = match (left_is_union, right_is_union) {
            (true, true) => self.expression_depth.max(right.expression_depth),
            (true, false) => self.expression_depth.max(
                right_nested
                    .ok_or_else(|| CompileError::new(CompileErrorKind::SizeOverflow, offset))?,
            ),
            (false, true) => right.expression_depth.max(
                left_nested
                    .ok_or_else(|| CompileError::new(CompileErrorKind::SizeOverflow, offset))?,
            ),
            (false, false) => self
                .expression_depth
                .max(right.expression_depth)
                .checked_add(1)
                .ok_or_else(|| CompileError::new(CompileErrorKind::SizeOverflow, offset))?,
        };
        validate_expression_depth(expression_depth, depth_limit, offset)?;
        let term_count = self
            .term_count
            .checked_add(right.term_count)
            .ok_or_else(|| CompileError::new(CompileErrorKind::SizeOverflow, offset))?;
        validate_term_count(term_count, term_limit, offset)?;
        let evaluation_work = match (left_is_union, right_is_union) {
            (true, true) => self
                .evaluation_work
                .checked_add(right.evaluation_work)
                .and_then(|work| work.checked_sub(1)),
            (true, false) | (false, true) => {
                self.evaluation_work.checked_add(right.evaluation_work)
            }
            (false, false) => self
                .evaluation_work
                .checked_add(right.evaluation_work)
                .and_then(|work| work.checked_add(1)),
        }
        .ok_or_else(|| CompileError::new(CompileErrorKind::SizeOverflow, offset))?;
        self.term = merge_union_terms(self.term, right.term);
        self.strings.append(&mut right.strings);
        self.expression_depth = expression_depth;
        self.term_count = term_count;
        self.evaluation_work = evaluation_work;
        Ok(self)
    }

    fn intersection(
        self,
        right: Self,
        depth_limit: usize,
        term_limit: usize,
        offset: usize,
    ) -> Result<Self, CompileError> {
        let expression_depth = joined_expression_depth(
            self.expression_depth,
            right.expression_depth,
            depth_limit,
            offset,
        )?;
        let term_count = self
            .term_count
            .checked_add(right.term_count)
            .ok_or_else(|| CompileError::new(CompileErrorKind::SizeOverflow, offset))?;
        validate_term_count(term_count, term_limit, offset)?;
        let evaluation_work = self
            .evaluation_work
            .checked_add(right.evaluation_work)
            .and_then(|work| work.checked_add(1))
            .ok_or_else(|| CompileError::new(CompileErrorKind::SizeOverflow, offset))?;
        let term = CharacterClassTerm::Intersection {
            left: Box::new(self.term),
            right: Box::new(right.term),
        };
        let strings = self.strings.intersection(&right.strings).cloned().collect();
        Ok(Self {
            term,
            strings,
            expression_depth,
            term_count,
            evaluation_work,
        })
    }

    fn subtraction(
        self,
        right: Self,
        depth_limit: usize,
        term_limit: usize,
        offset: usize,
    ) -> Result<Self, CompileError> {
        let expression_depth = joined_expression_depth(
            self.expression_depth,
            right.expression_depth,
            depth_limit,
            offset,
        )?;
        let term_count = self
            .term_count
            .checked_add(right.term_count)
            .ok_or_else(|| CompileError::new(CompileErrorKind::SizeOverflow, offset))?;
        validate_term_count(term_count, term_limit, offset)?;
        let evaluation_work = self
            .evaluation_work
            .checked_add(right.evaluation_work)
            .and_then(|work| work.checked_add(1))
            .ok_or_else(|| CompileError::new(CompileErrorKind::SizeOverflow, offset))?;
        let term = CharacterClassTerm::Subtraction {
            left: Box::new(self.term),
            right: Box::new(right.term),
        };
        let strings = self.strings.difference(&right.strings).cloned().collect();
        Ok(Self {
            term,
            strings,
            expression_depth,
            term_count,
            evaluation_work,
        })
    }

    fn complement(self, limit: usize, offset: usize) -> Result<Self, CompileError> {
        if !self.strings.is_empty() {
            return Err(CompileError::new(
                CompileErrorKind::InvalidCharacterClass,
                offset,
            ));
        }
        let expression_depth = self
            .expression_depth
            .checked_add(1)
            .ok_or_else(|| CompileError::new(CompileErrorKind::SizeOverflow, offset))?;
        validate_expression_depth(expression_depth, limit, offset)?;
        let evaluation_work = self
            .evaluation_work
            .checked_add(1)
            .ok_or_else(|| CompileError::new(CompileErrorKind::SizeOverflow, offset))?;
        Ok(Self {
            term: CharacterClassTerm::Complement(Box::new(self.term)),
            strings: BTreeSet::new(),
            expression_depth,
            term_count: self.term_count,
            evaluation_work,
        })
    }
}

fn merge_union_terms(left: CharacterClassTerm, right: CharacterClassTerm) -> CharacterClassTerm {
    match (left, right) {
        (CharacterClassTerm::Union(mut left), CharacterClassTerm::Union(mut right)) => {
            left.append(&mut right);
            CharacterClassTerm::Union(left)
        }
        (CharacterClassTerm::Union(mut terms), right) => {
            terms.push(right);
            CharacterClassTerm::Union(terms)
        }
        (left, CharacterClassTerm::Union(mut terms)) => {
            terms.insert(0, left);
            CharacterClassTerm::Union(terms)
        }
        (left, right) => CharacterClassTerm::Union(vec![left, right]),
    }
}

fn joined_expression_depth(
    left: usize,
    right: usize,
    limit: usize,
    offset: usize,
) -> Result<usize, CompileError> {
    let depth = left
        .max(right)
        .checked_add(1)
        .ok_or_else(|| CompileError::new(CompileErrorKind::SizeOverflow, offset))?;
    validate_expression_depth(depth, limit, offset)?;
    Ok(depth)
}

const fn validate_expression_depth(
    depth: usize,
    limit: usize,
    offset: usize,
) -> Result<(), CompileError> {
    if depth > limit {
        return Err(CompileError::new(
            CompileErrorKind::NestingLimit { limit },
            offset,
        ));
    }
    Ok(())
}

const fn validate_term_count(
    count: usize,
    limit: usize,
    offset: usize,
) -> Result<(), CompileError> {
    if count > limit {
        return Err(CompileError::new(
            CompileErrorKind::NodeLimit { limit },
            offset,
        ));
    }
    Ok(())
}

impl Default for ClassSet {
    fn default() -> Self {
        Self {
            term: CharacterClassTerm::Union(Vec::new()),
            strings: BTreeSet::new(),
            expression_depth: 1,
            term_count: 0,
            evaluation_work: 1,
        }
    }
}

impl Parser<'_> {
    pub(super) fn parse_unicode_set_class_node(&mut self) -> Result<Node, CompileError> {
        let class_offset = self.position;
        let set = self.parse_unicode_set_class()?;
        self.validate_class_term_count(set.term_count, class_offset)?;
        self.validate_class_strings(&set.strings, class_offset)?;
        let codepoint_work = set.evaluation_work;
        let terms = vec![set.term];
        let strings = set.strings.into_iter().map(Vec::into_boxed_slice).collect();
        self.node(Node::Class(CharacterClass {
            inverted: false,
            terms,
            strings,
            codepoint_work,
        }))
    }

    fn parse_unicode_set_class(&mut self) -> Result<ClassSet, CompileError> {
        let class_offset = self.position;
        if self.peek() != Some(u16::from(b'[')) {
            return Err(self.error(CompileErrorKind::InvalidCharacterClass));
        }
        self.advance_one()?;
        self.enter_depth()?;
        let inverted = if self.peek() == Some(u16::from(b'^')) {
            self.advance_one()?;
            true
        } else {
            false
        };
        let mut set = if self.peek() == Some(u16::from(b']')) {
            ClassSet::default()
        } else {
            self.parse_set_operand()?
        };
        if let Some(operator) = self.peek_set_operator()? {
            set = self.parse_set_operation(set, operator)?;
        } else {
            while self.peek() != Some(u16::from(b']')) {
                if self.peek().is_none() || self.peek_set_operator()?.is_some() {
                    return Err(self.error(CompileErrorKind::InvalidCharacterClass));
                }
                set = set.union(
                    self.parse_set_operand()?,
                    self.limits.max_nesting_depth,
                    self.limits.max_character_class_terms,
                    class_offset,
                )?;
            }
        }
        if self.peek() != Some(u16::from(b']')) {
            return Err(self.error(CompileErrorKind::UnterminatedCharacterClass));
        }
        self.advance_one()?;
        self.depth = self
            .depth
            .checked_sub(1)
            .ok_or_else(|| self.error(CompileErrorKind::SizeOverflow))?;
        if inverted {
            set.complement(self.limits.max_nesting_depth, class_offset)
        } else {
            Ok(set)
        }
    }

    fn parse_set_operation(
        &mut self,
        mut left: ClassSet,
        operator: SetOperator,
    ) -> Result<ClassSet, CompileError> {
        loop {
            if self.peek_set_operator()? != Some(operator) {
                return Err(self.error(CompileErrorKind::InvalidCharacterClass));
            }
            self.advance_one()?;
            self.advance_one()?;
            if self.peek().is_none() || self.peek() == Some(u16::from(b']')) {
                return Err(self.error(CompileErrorKind::InvalidCharacterClass));
            }
            let right = self.parse_set_operand()?;
            left = match operator {
                SetOperator::Intersection => left.intersection(
                    right,
                    self.limits.max_nesting_depth,
                    self.limits.max_character_class_terms,
                    self.position,
                )?,
                SetOperator::Subtraction => left.subtraction(
                    right,
                    self.limits.max_nesting_depth,
                    self.limits.max_character_class_terms,
                    self.position,
                )?,
            };
            match self.peek_set_operator()? {
                Some(next) if next == operator => {}
                None if self.peek() == Some(u16::from(b']')) => return Ok(left),
                Some(_) | None => {
                    return Err(self.error(CompileErrorKind::InvalidCharacterClass));
                }
            }
        }
    }

    fn parse_set_operand(&mut self) -> Result<ClassSet, CompileError> {
        if self.peek() == Some(u16::from(b'[')) {
            return self.parse_unicode_set_class();
        }
        if self.peek() == Some(u16::from(b'\\')) {
            let escaped_index = self
                .position
                .checked_add(1)
                .ok_or_else(|| self.error(CompileErrorKind::SizeOverflow))?;
            match self.pattern.get(escaped_index).copied() {
                Some(value) if value == u16::from(b'p') || value == u16::from(b'P') => {
                    self.advance_one()?;
                    let offset = self.position;
                    self.advance_one()?;
                    let property = self.parse_property_escape(value == u16::from(b'P'), offset)?;
                    return self.class_set_from_property(property, offset);
                }
                Some(value) if value == u16::from(b'q') => return self.parse_q_disjunction(),
                Some(value)
                    if matches!(value, 0x0064 | 0x0044 | 0x0073 | 0x0053 | 0x0077 | 0x0057) =>
                {
                    self.advance_one()?;
                    self.advance_one()?;
                    return predefined_class_term(value).map(ClassSet::from_term);
                }
                _ => {}
            }
        }
        let start = self.parse_set_character()?;
        if self.peek() == Some(u16::from(b'-')) && !self.next_is(u16::from(b'-'))? {
            self.advance_one()?;
            if self.peek().is_none() || self.peek() == Some(u16::from(b']')) {
                return Err(self.error(CompileErrorKind::InvalidCharacterClass));
            }
            let end = self.parse_set_character()?;
            if start > end {
                return Err(self.error(CompileErrorKind::InvalidCharacterClass));
            }
            return Ok(ClassSet::from_term(CharacterClassTerm::Range {
                start,
                end,
            }));
        }
        Ok(ClassSet::from_term(CharacterClassTerm::Range {
            start,
            end: start,
        }))
    }

    fn class_set_from_property(
        &self,
        property: PropertyEscape,
        offset: usize,
    ) -> Result<ClassSet, CompileError> {
        match property {
            PropertyEscape::CodePoints(term) => Ok(ClassSet::from_term(term)),
            PropertyEscape::Strings(strings) => ClassSet::from_strings(
                strings.into_iter().map(Vec::from).collect(),
                self.limits.max_character_class_terms,
                offset,
            ),
        }
    }

    fn parse_q_disjunction(&mut self) -> Result<ClassSet, CompileError> {
        let escape_offset = self.position;
        self.advance_one()?;
        self.advance_one()?;
        if self.peek() != Some(u16::from(b'{')) {
            return Err(CompileError::new(
                CompileErrorKind::InvalidEscape,
                escape_offset,
            ));
        }
        self.advance_one()?;
        let mut strings = BTreeSet::new();
        let mut current = Vec::new();
        let mut string_units = 0_usize;
        loop {
            match self.peek() {
                Some(value) if value == u16::from(b'}') => {
                    self.advance_one()?;
                    self.insert_class_string(
                        &mut strings,
                        current,
                        &mut string_units,
                        escape_offset,
                    )?;
                    break;
                }
                Some(value) if value == u16::from(b'|') => {
                    self.advance_one()?;
                    self.insert_class_string(
                        &mut strings,
                        core::mem::take(&mut current),
                        &mut string_units,
                        escape_offset,
                    )?;
                }
                Some(_) => {
                    if current.len() >= self.limits.max_class_string_units {
                        return Err(CompileError::new(
                            CompileErrorKind::ClassStringUnitLimit {
                                limit: self.limits.max_class_string_units,
                            },
                            escape_offset,
                        ));
                    }
                    current.push(self.parse_set_character()?);
                }
                None => {
                    return Err(CompileError::new(
                        CompileErrorKind::InvalidEscape,
                        escape_offset,
                    ));
                }
            }
        }
        if self.flags.ignore_case() {
            strings = strings
                .into_iter()
                .map(|sequence| {
                    sequence
                        .into_iter()
                        .map(|value| crate::unicode::canonicalize(value, true))
                        .collect()
                })
                .collect();
        }
        self.validate_class_strings(&strings, escape_offset)?;
        ClassSet::from_strings(
            strings,
            self.limits.max_character_class_terms,
            escape_offset,
        )
    }

    fn parse_set_character(&mut self) -> Result<u32, CompileError> {
        if self.peek() == Some(u16::from(b'\\')) {
            return self.parse_set_character_escape();
        }
        let offset = self.position;
        let Some(unit) = self.peek() else {
            return Err(self.error(CompileErrorKind::UnterminatedCharacterClass));
        };
        if is_set_syntax_character(unit)
            || (is_reserved_double_punctuator(unit) && self.next_is(unit)?)
        {
            return Err(CompileError::new(
                CompileErrorKind::InvalidCharacterClass,
                offset,
            ));
        }
        self.decode_pattern_value()
    }

    fn parse_set_character_escape(&mut self) -> Result<u32, CompileError> {
        self.advance_one()?;
        let escape_offset = self.position;
        let Some(unit) = self.peek() else {
            return Err(self.error(CompileErrorKind::InvalidEscape));
        };
        self.advance_one()?;
        let value = match unit {
            0x0062 => 0x0008,
            0x006E => 0x000A,
            0x0072 => 0x000D,
            0x0074 => 0x0009,
            0x0076 => 0x000B,
            0x0066 => 0x000C,
            0x0063 => {
                let Some(control) = self
                    .peek()
                    .and_then(super::escape_parser::control_letter_value)
                else {
                    return Err(CompileError::new(
                        CompileErrorKind::InvalidEscape,
                        escape_offset,
                    ));
                };
                self.advance_one()?;
                control
            }
            0x0078 => self.parse_fixed_hex(2)?,
            0x0075 if self.peek() == Some(u16::from(b'{')) => self.parse_braced_hex()?,
            0x0075 => self.parse_unicode_escape_value(escape_offset)?,
            0x0030
                if !self
                    .peek()
                    .is_some_and(super::escape_parser::is_decimal_digit) =>
            {
                0
            }
            value if is_set_syntax_character(value) || is_reserved_double_punctuator(value) => {
                u32::from(value)
            }
            _ => {
                return Err(CompileError::new(
                    CompileErrorKind::InvalidEscape,
                    escape_offset,
                ));
            }
        };
        Ok(value)
    }

    fn peek_set_operator(&self) -> Result<Option<SetOperator>, CompileError> {
        if self.peek() == Some(u16::from(b'&')) && self.next_is(u16::from(b'&'))? {
            Ok(Some(SetOperator::Intersection))
        } else if self.peek() == Some(u16::from(b'-')) && self.next_is(u16::from(b'-'))? {
            Ok(Some(SetOperator::Subtraction))
        } else {
            Ok(None)
        }
    }

    fn next_is(&self, expected: u16) -> Result<bool, CompileError> {
        let next = self
            .position
            .checked_add(1)
            .ok_or_else(|| self.error(CompileErrorKind::SizeOverflow))?;
        Ok(self.pattern.get(next).copied() == Some(expected))
    }

    fn validate_class_strings(
        &self,
        strings: &BTreeSet<Vec<u32>>,
        pattern_offset: usize,
    ) -> Result<(), CompileError> {
        if strings.len() > self.limits.max_class_strings {
            return Err(CompileError::new(
                CompileErrorKind::ClassStringLimit {
                    limit: self.limits.max_class_strings,
                },
                pattern_offset,
            ));
        }
        let units = strings.iter().try_fold(0_usize, |total, string| {
            total
                .checked_add(string.len())
                .ok_or_else(|| CompileError::new(CompileErrorKind::SizeOverflow, pattern_offset))
        })?;
        if units > self.limits.max_class_string_units {
            return Err(CompileError::new(
                CompileErrorKind::ClassStringUnitLimit {
                    limit: self.limits.max_class_string_units,
                },
                pattern_offset,
            ));
        }
        Ok(())
    }

    fn insert_class_string(
        &self,
        strings: &mut BTreeSet<Vec<u32>>,
        sequence: Vec<u32>,
        total_units: &mut usize,
        pattern_offset: usize,
    ) -> Result<(), CompileError> {
        if strings.contains(&sequence) {
            return Ok(());
        }
        if strings.len() >= self.limits.max_class_strings {
            return Err(CompileError::new(
                CompileErrorKind::ClassStringLimit {
                    limit: self.limits.max_class_strings,
                },
                pattern_offset,
            ));
        }
        let next_units = total_units
            .checked_add(sequence.len())
            .ok_or_else(|| CompileError::new(CompileErrorKind::SizeOverflow, pattern_offset))?;
        if next_units > self.limits.max_class_string_units {
            return Err(CompileError::new(
                CompileErrorKind::ClassStringUnitLimit {
                    limit: self.limits.max_class_string_units,
                },
                pattern_offset,
            ));
        }
        if !strings.insert(sequence) {
            return Err(CompileError::new(
                CompileErrorKind::SizeOverflow,
                pattern_offset,
            ));
        }
        *total_units = next_units;
        Ok(())
    }

    const fn validate_class_term_count(
        &self,
        count: usize,
        pattern_offset: usize,
    ) -> Result<(), CompileError> {
        validate_term_count(count, self.limits.max_character_class_terms, pattern_offset)
    }
}

const fn is_set_syntax_character(value: u16) -> bool {
    matches!(
        value,
        0x0028 | 0x0029 | 0x002D | 0x002F | 0x005B | 0x005C | 0x005D | 0x007B | 0x007C | 0x007D
    )
}

const fn is_reserved_double_punctuator(value: u16) -> bool {
    matches!(
        value,
        0x0021
            | 0x0023..=0x0025
            | 0x0026
            | 0x002A..=0x002E
            | 0x003A..=0x003B
            | 0x003C..=0x003E
            | 0x003F..=0x0040
            | 0x005E
            | 0x0060
            | 0x007E
    )
}
