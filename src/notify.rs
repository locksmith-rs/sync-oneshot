#[cfg(loom)]
use loom::{
    cell::UnsafeCell,
    thread::{self, Thread},
};

#[cfg(not(loom))]
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
    #[inline]
    pub(crate) unsafe fn set_current(&self) {
        let thread = self.thread.get();
        #[cfg(loom)]
        let thread: *mut Option<Thread> = thread.with(|ptr| ptr as *mut _);
        unsafe {
            *thread = Some(thread::current());
        }
    }

    /// SAFETY:
    /// Notify dose not avoid data race.
    /// Need to trace Notify state in multi thread environment.
    #[inline]
    pub(crate) unsafe fn notify(&self) {
        let thread_ptr = self.thread.get();
        #[cfg(loom)]
        let thread_ptr: *mut Option<Thread> = thread_ptr.with(|ptr| ptr as *mut _);

        let mut old_thread: Option<Thread> = unsafe { std::ptr::replace(thread_ptr, None) };
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
        let test_inner = || {
            let notify = Notify::new();
            unsafe {
                notify.set_current();
            }

            unsafe {
                notify.notify();
            }
        };

        #[cfg(loom)]
        loom::model(test_inner);

        #[cfg(not(loom))]
        test_inner();
    }
}
