use alloc::{
    collections::{BTreeMap, VecDeque},
    sync::Arc,
};
use core::time::Duration;

#[cfg(feature = "std")]
use parking_lot::Condvar;

use crate::sync::{Mutex, RwLock};

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
    #[cfg(feature = "std")]
    signal: Condvar,
}

#[derive(Debug)]
pub(in crate::runtime) struct AtomicWaitRegistration {
    shared: Arc<SharedByteBuffer>,
    byte_offset: usize,
    waiter: Arc<AtomicWaiter>,
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

    pub(super) fn register_waiter(
        shared: &Arc<Self>,
        byte_offset: usize,
    ) -> AtomicWaitRegistration {
        let waiter = Arc::new(AtomicWaiter {
            notified: Mutex::new(false),
            #[cfg(feature = "std")]
            signal: Condvar::new(),
        });
        shared
            .waiters
            .lock()
            .entry(byte_offset)
            .or_default()
            .push_back(waiter.clone());
        AtomicWaitRegistration {
            shared: shared.clone(),
            byte_offset,
            waiter,
        }
    }

    fn remove_waiter(&self, byte_offset: usize, waiter: &Arc<AtomicWaiter>) {
        let mut waiters = self.waiters.lock();
        let Some(queue) = waiters.get_mut(&byte_offset) else {
            return;
        };
        queue.retain(|candidate| !Arc::ptr_eq(candidate, waiter));
        if queue.is_empty() {
            waiters.remove(&byte_offset);
        }
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
            #[cfg(feature = "std")]
            waiter.signal.notify_one();
        }
        if queue.is_empty() {
            waiters.remove(&byte_offset);
        }
        drop(waiters);
        Ok(notify_count)
    }
}

impl AtomicWaitRegistration {
    #[cfg(feature = "std")]
    pub(in crate::runtime) fn wait(&self, timeout: Option<Duration>) -> AtomicWaitOutcome {
        let mut notified = self.waiter.notified.lock();
        if let Some(duration) = timeout {
            self.waiter.signal.wait_for(&mut notified, duration);
        } else {
            while !*notified {
                self.waiter.signal.wait(&mut notified);
            }
        }
        if *notified {
            return AtomicWaitOutcome::Notified;
        }
        drop(notified);

        let mut waiters = self.shared.waiters.lock();
        if *self.waiter.notified.lock() {
            return AtomicWaitOutcome::Notified;
        }
        if let Some(queue) = waiters.get_mut(&self.byte_offset) {
            queue.retain(|candidate| !Arc::ptr_eq(candidate, &self.waiter));
            if queue.is_empty() {
                waiters.remove(&self.byte_offset);
            }
        }
        drop(waiters);
        AtomicWaitOutcome::TimedOut
    }

    #[cfg(not(feature = "std"))]
    pub(in crate::runtime) fn wait(&self, _timeout: Option<Duration>) -> AtomicWaitOutcome {
        if *self.waiter.notified.lock() {
            AtomicWaitOutcome::Notified
        } else {
            AtomicWaitOutcome::TimedOut
        }
    }
}

impl Drop for AtomicWaitRegistration {
    fn drop(&mut self) {
        self.shared.remove_waiter(self.byte_offset, &self.waiter);
    }
}
