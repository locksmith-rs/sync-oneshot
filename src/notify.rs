use std::{
    cell::UnsafeCell,
    thread::{self, Thread},
};

/// Mechanisumu for waking blocking thread.
///
/// Provide getting thread and notify to wake up.
pub(crate) struct Notify {
    thread: UnsafeCell<Option<Thread>>,
}

impl Notify {
    pub(crate) fn new() -> Self {
        Self {
            thread: UnsafeCell::new(None),
        }
    }

    /// SAFETY:
    /// Notify dose not avoid data race.
    /// Need to trace Notify state in multi thread environment.
    pub(crate) unsafe fn set_current(&self) {
        let thread = self.thread.get();
        unsafe {
            *thread = Some(thread::current());
        }
    }

    /// SAFETY:
    /// Notify dose not avoid data race.
    /// Need to trace Notify state in multi thread environment.
    pub(crate) unsafe fn notify(&self) {
        let thread_ptr = self.thread.get();

        let mut old_thread = unsafe { std::ptr::replace(thread_ptr, None) };
        if let Some(th) = old_thread.take() {
            th.unpark();
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::notify::Notify;

    #[test]
    fn test() {
        let notify = Notify::new();
        unsafe {
            notify.set_current();
        }

        unsafe {
            notify.notify();
        }
    }
}
