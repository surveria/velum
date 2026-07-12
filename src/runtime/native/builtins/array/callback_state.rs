use crate::value::Value;

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(super) enum ArrayCallbackAction {
    Continue,
    Stop,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(super) enum ReduceDirection {
    Forward,
    Reverse,
}

#[derive(Debug, Clone)]
pub(super) struct ReduceState {
    pub(super) accumulator: Value,
    next: usize,
    end: usize,
    pub(super) started: bool,
}

impl ReduceState {
    pub(super) const fn with_next(
        accumulator: Value,
        next: usize,
        end: usize,
        started: bool,
    ) -> Self {
        Self {
            accumulator,
            next,
            end,
            started,
        }
    }

    pub(super) const fn next_index(&mut self, direction: ReduceDirection) -> Option<usize> {
        match direction {
            ReduceDirection::Forward => self.next_forward_index(),
            ReduceDirection::Reverse => self.next_reverse_index(),
        }
    }

    const fn next_forward_index(&mut self) -> Option<usize> {
        if self.next >= self.end {
            return None;
        }
        let index = self.next;
        self.next = self.next.saturating_add(1);
        Some(index)
    }

    const fn next_reverse_index(&mut self) -> Option<usize> {
        if self.next == 0 {
            return None;
        }
        self.next = self.next.saturating_sub(1);
        Some(self.next)
    }
}
