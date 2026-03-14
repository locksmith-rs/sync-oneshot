use std::{
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    thread,
};

use crate::{notify::Notify, slot::Slot};

mod error;
mod notify;
mod slot;

pub use error::RecvError;

/// Create synchronous oneshot channel.
pub fn channel<T>() -> (Sender<T>, Receiver<T>) {
    let inner = Arc::new(Inner {
        state: AtomicUsize::new(0),
        value: Slot::new(),
        notify: Notify::new(),
    });

    (
        Sender {
            inner: Some(inner.clone()),
        },
        Receiver { inner: Some(inner) },
    )
}

pub struct Sender<T> {
    inner: Option<Arc<Inner<T>>>,
}

pub struct Receiver<T> {
    inner: Option<Arc<Inner<T>>>,
}

struct Inner<T> {
    state: AtomicUsize,
    value: Slot<T>,
    notify: Notify,
}

unsafe impl<T> Send for Sender<T> where T: Send {}
unsafe impl<T> Send for Receiver<T> where T: Send {}

/*
 *
 * ===== impl Sender =====
 *
 */
impl<T> Sender<T> {
    pub fn send(mut self, value: T) -> Result<(), T> {
        // take inner
        // The case inner None is unreachable
        let inner = self.inner.take().unwrap();

        // set value
        unsafe {
            // SAFTY:
            // Receiver don't access inner value until set status as VALUE_SENT
            inner.value.set(value);
        }

        // set state as VALUE_SEND and notify
        let prev_state = inner.set_complete();

        if prev_state.is_closed() {
            // SAFTY:
            // Receiver already has been droped. So can access inner value.
            return Err(unsafe { inner.consumu_value().unwrap() });
        }

        if prev_state.is_waiting() {
            unsafe {
                inner.notify();
            }
        }

        Ok(())
    }
}

impl<T> Drop for Sender<T> {
    fn drop(&mut self) {
        if let Some(inner) = self.inner.take() {
            let prev_state = inner.set_complete();

            if prev_state.is_waiting() {
                unsafe {
                    inner.notify.notify();
                }
            }
        }
    }
}

/*
 *
 * ===== impl Receiver =====
 *
 */
impl<T> Receiver<T> {
    pub fn recv(mut self) -> Result<T, RecvError> {
        let inner = self.inner.take().unwrap();

        loop {
            let mut state = inner.state.load(Ordering::Relaxed);
            if State(state).is_complete() {
                let value = unsafe { inner.value.take() };
                return value.ok_or(RecvError::ShutDown);
            }

            unsafe {
                // SAFETY:
                // Notify::notify dose not call untill state is WAITNG.
                // So we can access notify.
                inner.notify.set_current();
            }

            match inner.state.compare_exchange(
                state,
                state | WAITING,
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => thread::park(),
                Err(actual) => state = actual,
            }
        }
    }
}

impl<T> Drop for Receiver<T> {
    fn drop(&mut self) {
        // if inner is some, Receiver::recv is not called before drop.
        // Drop value or change state
        if let Some(inner) = self.inner.take() {
            loop {
                let mut state = inner.state.load(Ordering::Relaxed);
                if State(state).is_complete() {
                    unsafe {
                        inner.consumu_value();
                        break;
                    }
                }

                match inner.state.compare_exchange(
                    state,
                    state | CLOSED,
                    Ordering::AcqRel,
                    Ordering::Acquire, // can Relaxed?
                ) {
                    Ok(_) => break,
                    Err(actual) => state = actual,
                }
            }
        }
    }
}

/*
 *
 * ===== impl Inner =====
 *
 */
impl<T> Inner<T> {
    fn set_complete(&self) -> State {
        let mut state = self.state.load(Ordering::Relaxed);
        loop {
            if State(state).is_closed() {
                break;
            }

            match self.state.compare_exchange_weak(
                state,
                state | VALUE_SENT,
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => break,
                Err(actual) => state = actual,
            }
        }
        State(state)
    }

    unsafe fn notify(&self) {
        unsafe {
            self.notify.notify();
        }
    }

    unsafe fn consumu_value(&self) -> Option<T> {
        unsafe { self.value.take() }
    }
}

struct State(usize);

const WAITING: usize = 0b0001;
const VALUE_SENT: usize = 0b0010;
const CLOSED: usize = 0b0100;

impl State {
    fn is_closed(&self) -> bool {
        self.0 & CLOSED == CLOSED
    }

    fn is_waiting(&self) -> bool {
        self.0 & WAITING == WAITING
    }

    fn is_complete(&self) -> bool {
        self.0 & VALUE_SENT == VALUE_SENT
    }
}

#[cfg(test)]
mod tests {
    use std::{thread, time::Duration};

    use crate::channel;

    #[test]
    fn test_local() {
        let (tx, rx) = channel();

        tx.send(5).unwrap();

        let result = rx.recv().unwrap();
        assert_eq!(result, 5);
    }

    #[test]
    fn test_thread_tx() {
        let (tx, rx) = channel();

        std::thread::spawn(move || {
            thread::sleep(Duration::from_millis(100));
            tx.send(5).unwrap();
        });

        let result = rx.recv().unwrap();
        assert_eq!(result, 5);
    }

    #[test]
    fn test_thread_rx() {
        let (tx, rx) = channel();

        let result = std::thread::spawn(move || rx.recv().unwrap());

        thread::sleep(Duration::from_millis(100));
        tx.send(5).unwrap();
        assert_eq!(result.join().unwrap(), 5);
    }

    #[test]
    fn test_rx_already_closed() {
        let (tx, rx) = channel();

        drop(rx);

        let result = tx.send(5);
        assert!(result.is_err());
    }

    #[test]
    fn test_tx_already_closed() {
        let (tx, rx) = channel::<i32>();
        drop(tx);

        assert!(rx.recv().is_err());
    }

    #[test]
    fn test_tx_already_closed_wait() {
        let (tx, rx) = channel::<i32>();

        std::thread::spawn(move || {
            thread::sleep(Duration::from_millis(100));
            drop(tx);
        });

        assert!(rx.recv().is_err());
    }
}
