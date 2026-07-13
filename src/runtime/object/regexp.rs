use std::rc::Rc;

use crate::{
    error::{Error, Result},
    regexp_syntax::RegExpFlags,
};

#[derive(Debug, Clone)]
pub struct RegExpValue {
    state: Rc<RegExpState>,
}

#[derive(Debug)]
struct RegExpState {
    pattern_units: Box<[u16]>,
    parsed_flags: RegExpFlags,
    compiled: regress::Regex,
    storage_payload_bytes: usize,
}

impl RegExpValue {
    pub(in crate::runtime) fn new_utf16(
        pattern_units: Vec<u16>,
        parsed_flags: RegExpFlags,
        compiled: regress::Regex,
    ) -> Result<Self> {
        let pattern_payload_bytes = pattern_units
            .len()
            .checked_mul(core::mem::size_of::<u16>())
            .ok_or_else(|| Error::limit("RegExp pattern payload bytes overflowed"))?;
        let compiled_payload_bytes = compiled
            .retained_payload_bytes()
            .ok_or_else(|| Error::limit("compiled RegExp payload bytes overflowed"))?;
        let storage_payload_bytes = pattern_payload_bytes
            .checked_add(compiled_payload_bytes)
            .ok_or_else(|| Error::limit("RegExp storage payload bytes overflowed"))?;
        Ok(Self {
            state: Rc::new(RegExpState {
                pattern_units: pattern_units.into_boxed_slice(),
                parsed_flags,
                compiled,
                storage_payload_bytes,
            }),
        })
    }

    pub fn pattern_utf16(&self) -> &[u16] {
        &self.state.pattern_units
    }

    pub(in crate::runtime) fn parsed_flags(&self) -> RegExpFlags {
        self.state.parsed_flags
    }

    pub(in crate::runtime) fn compiled(&self) -> &regress::Regex {
        &self.state.compiled
    }

    pub(in crate::runtime) fn storage_payload_bytes(&self) -> usize {
        self.state.storage_payload_bytes
    }
}
