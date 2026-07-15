use std::ops::Range;

use crate::{
    error::{Error, Result},
    value::Value,
};

use super::{VmStorageKind, storage_ledger::VmStorageLedger};

/// Realm-owned state behind the Annex B legacy `RegExp` constructor accessors.
///
/// String values remain in the VM string heap and are exposed as direct roots.
/// Match spans are UTF-16 code-unit ranges into `match_subject`.
#[derive(Debug)]
pub(super) struct RealmRegExpStatics {
    storage_ledger: VmStorageLedger,
    input: Option<Value>,
    match_subject: Option<Value>,
    match_span: Option<Range<usize>>,
    captures: Vec<Option<Range<usize>>>,
}

impl RealmRegExpStatics {
    pub(super) const fn new(storage_ledger: VmStorageLedger) -> Self {
        Self {
            storage_ledger,
            input: None,
            match_subject: None,
            match_span: None,
            captures: Vec::new(),
        }
    }

    pub(super) fn replace_input(&mut self, input: Value) -> Result<()> {
        Self::require_string(&input)?;
        let replacement_count = self
            .association_count()
            .saturating_add(usize::from(self.input.is_none()));
        let reservation = self.storage_ledger.reserve_replacement(
            VmStorageKind::Association,
            self.association_count(),
            0,
            replacement_count,
            0,
        )?;
        reservation.commit()?;
        self.input = Some(input);
        Ok(())
    }

    pub(super) fn replace_match(
        &mut self,
        subject: Value,
        match_span: Range<usize>,
        captures: Vec<Option<Range<usize>>>,
    ) -> Result<()> {
        let subject_length = Self::require_string(&subject)?;
        Self::validate_span(&match_span, subject_length)?;
        for span in captures.iter().flatten() {
            Self::validate_span(span, subject_length)?;
        }
        let replacement_count = captures
            .len()
            .checked_add(3)
            .ok_or_else(|| Error::limit("legacy RegExp association count overflowed"))?;
        let reservation = self.storage_ledger.reserve_replacement(
            VmStorageKind::Association,
            self.association_count(),
            0,
            replacement_count,
            0,
        )?;
        reservation.commit()?;
        self.input = Some(subject.clone());
        self.match_subject = Some(subject);
        self.match_span = Some(match_span);
        self.captures = captures;
        Ok(())
    }

    pub(super) const fn input(&self) -> Option<&Value> {
        self.input.as_ref()
    }

    pub(super) const fn match_subject(&self) -> Option<&Value> {
        self.match_subject.as_ref()
    }

    pub(super) const fn match_span(&self) -> Option<&Range<usize>> {
        self.match_span.as_ref()
    }

    pub(super) fn captures(&self) -> &[Option<Range<usize>>] {
        &self.captures
    }

    pub(super) fn anchor_values(&self) -> impl Iterator<Item = &Value> {
        self.input.iter().chain(self.match_subject.iter())
    }

    pub(super) fn association_count(&self) -> usize {
        usize::from(self.input.is_some())
            .saturating_add(usize::from(self.match_subject.is_some()))
            .saturating_add(usize::from(self.match_span.is_some()))
            .saturating_add(self.captures.len())
    }

    fn require_string(value: &Value) -> Result<usize> {
        let Value::String(value) = value else {
            return Err(Error::runtime("legacy RegExp state requires a string"));
        };
        Ok(value.as_utf16().len())
    }

    fn validate_span(span: &Range<usize>, subject_length: usize) -> Result<()> {
        if span.start <= span.end && span.end <= subject_length {
            return Ok(());
        }
        Err(Error::runtime(
            "legacy RegExp match span is outside its subject",
        ))
    }
}
