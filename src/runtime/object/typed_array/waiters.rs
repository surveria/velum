use std::{
    collections::{BTreeMap, VecDeque},
    sync::Arc,
    time::Duration,
};

use parking_lot::{Condvar, Mutex, RwLock};

use crate::error::{Error, Result};

use super::ByteBufferState;

#[derive(Debug)]
pub struct SharedByteBuffer {
    pub(super) state: RwLock<ByteBufferState>,
    waiters: Mutex<BTreeMap<usize, VecDeque<Arc<AtomicWaiter>>>>,
}

#[derive(Debug)]
struct AtomicWaiter {
    notified: Mutex<bool>,
    signal: Condvar,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(in crate::runtime) enum AtomicWaitOutcome {
    Notified,
    TimedOut,
}

impl SharedByteBuffer {
    pub(super) const fn new(state: ByteBufferState) -> Self {
        Self {
            state: RwLock::new(state),
            waiters: Mutex::new(BTreeMap::new()),
        }
    }

    pub(super) fn wait_at(
        &self,
        byte_offset: usize,
        timeout: Option<Duration>,
    ) -> AtomicWaitOutcome {
        let waiter = Arc::new(AtomicWaiter {
            notified: Mutex::new(false),
            signal: Condvar::new(),
        });
        self.waiters
            .lock()
            .entry(byte_offset)
            .or_default()
            .push_back(waiter.clone());

        let mut notified = waiter.notified.lock();
        if let Some(duration) = timeout {
            waiter.signal.wait_for(&mut notified, duration);
        } else {
            while !*notified {
                waiter.signal.wait(&mut notified);
            }
        }
        if *notified {
            return AtomicWaitOutcome::Notified;
        }
        drop(notified);

        let mut waiters = self.waiters.lock();
        if *waiter.notified.lock() {
            return AtomicWaitOutcome::Notified;
        }
        let Some(queue) = waiters.get_mut(&byte_offset) else {
            return AtomicWaitOutcome::TimedOut;
        };
        queue.retain(|candidate| !Arc::ptr_eq(candidate, &waiter));
        if queue.is_empty() {
            waiters.remove(&byte_offset);
        }
        drop(waiters);
        AtomicWaitOutcome::TimedOut
    }

    pub(super) fn notify_at(&self, byte_offset: usize, count: usize) -> Result<usize> {
        let mut waiters = self.waiters.lock();
        let Some(queue) = waiters.get_mut(&byte_offset) else {
            return Ok(0);
        };
        let notify_count = count.min(queue.len());
        for _ in 0..notify_count {
            let Some(waiter) = queue.pop_front() else {
                return Err(Error::runtime("Atomics waiter queue changed unexpectedly"));
            };
            *waiter.notified.lock() = true;
            waiter.signal.notify_one();
        }
        if queue.is_empty() {
            waiters.remove(&byte_offset);
        }
        drop(waiters);
        Ok(notify_count)
    }
}
