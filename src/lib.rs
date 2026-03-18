//! A minimal oneshot channel for synchronous Rust.
//!
//! A oneshot channel is used for sending a single message between threads.
//! The [`channel`] function is used to create a [`Sender`] and [`Receiver`]
//! handle pair that form the channel.
//!
//! The [`Sender`] handle is used by the producer to send the value.  
//! The [`Receiver`] handle is used by the consumer to receive the value.
//!
//! Each handle can be used on other threads.
//!
//! [`Sender::send`] will no block the calling thread.  
//! [`Receiver::recv`] will **block** the calling thread.
//!
//! # Example
//! ```rust
//! # use std::time::Duration;
//! let (tx, rx) = sync_oneshot::channel();
//!
//! std::thread::spawn(move || {
//!     std::thread::sleep(Duration::from_millis(200));
//!     tx.send(5).unwrap();
//! });
//!
//! // blocking thread until a message available
//! let val = rx.recv().unwrap();
//! assert_eq!(val, 5);
//! ```
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

/// Creates a new oneshot channel, returning the sender/receiver halves.
///
/// The [`Sender`] is used by the producer to send the value.
/// The [`Receiver`] handle is used by the consumer to receive the value.
///
/// [`send`](Sender::send) will no block the calling thread. [`recv`](Receiver::recv)
/// will **block** until a message is available.
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

/// Sends a value to the associated [`Receiver`].
///
/// This is created by the [`channel`] function.  
/// Messages can be sent using [`send`](Sender::send).
pub struct Sender<T> {
    inner: Option<Arc<Inner<T>>>,
}

/// Receive a value from the associated [`Sender`].
///
/// This is created by the [`channel`] function.  
/// Messages sent to the channel can be retrieved using [`recv`](Receiver::recv).
/// [`recv`](Receiver::recv) method blocks thread.
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
    /// Attempts to send a value on this channel, returning it back if it could not be sent.
    ///
    /// A successful send occurs when it is determined that the other end of the
    /// channel has not hung up already. An unsuccessful send would be one where
    /// the corresponding receiver has already been deallocated. Note that a
    /// return value of [`Err`] means that the data will never be received, but
    /// a return value of [`Ok`] does *not* mean that the data will be received.
    /// It is possible for the corresponding receiver to hang up immediately
    /// after this function returns [`Ok`].
    ///
    /// This method will never block the current thread.
    /// # Example
    /// ```rust
    /// let (tx, rx) = sync_oneshot::channel();
    /// std::thread::spawn(move || {
    ///     if let Err(e) = tx.send(5) {
    ///         println!("the receiver dropped");
    ///     }
    /// });
    ///
    /// match rx.recv() {
    ///     Ok(v) => println!("got = {:?}", v),
    ///     Err(_) => println!("the sender dropped"),
    /// }
    /// ```
    pub fn send(mut self, value: T) -> Result<(), T> {
        // take inner
        // The case inner None is unreachable
        let inner = self.inner.take().unwrap();

        // set value
        unsafe {
            // SAFETY:
            // Receiver don't access inner value until set status as VALUE_SENT
            inner.value.set(value);
        }

        // set state as VALUE_SEND and notify
        let prev_state = inner.set_complete();

        if prev_state.is_closed() {
            // SAFETY:
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
    /// Attempts to wait for a value on this receiver, returning an error if
    /// the corresponding channel has hung up.
    ///
    /// This function will always block the current thread if there is no data
    /// available. Once a message is sent to the corresponding [`Sender`],
    /// this receiver will wake up and return that message.
    ///
    /// If the corresponding [`Sender`] has disconnected, or it disconnects while
    /// this call is blocking, this call will wake up and return [`Err`] to
    /// indicate that no more messages can ever be received on this channel.
    /// # Example
    /// ```rust
    /// let (tx, rx) = sync_oneshot::channel();
    ///
    /// let th_handle = std::thread::spawn(move || {
    ///     tx.send(5).unwrap();
    /// });
    ///
    /// th_handle.join().unwrap();
    ///
    /// assert_eq!(5, rx.recv().unwrap());
    /// ```
    pub fn recv(mut self) -> Result<T, RecvError> {
        let inner = self.inner.take().unwrap();

        let mut state = inner.state.load(Ordering::Acquire);
        loop {
            if State(state).is_complete() {
                let value = unsafe { inner.value.take() };
                return value.ok_or(RecvError);
            }

            unsafe {
                // SAFETY:
                // Notify::notify dose not call untill state is WAITNG.
                // So we can access notify.

                // Prevent double write due to spurious wake-up.
                if !State(state).is_waiting() {
                    inner.notify.set_current();
                }
            }

            match inner.state.compare_exchange(
                state,
                state | WAITING,
                Ordering::Release,
                Ordering::Acquire,
            ) {
                Ok(_) => {
                    thread::park();
                    state = inner.state.load(Ordering::Acquire);
                }
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
            let mut state = inner.state.load(Ordering::Acquire);
            loop {
                if State(state).is_complete() {
                    unsafe {
                        inner.consumu_value();
                        break;
                    }
                }

                match inner.state.compare_exchange(
                    state,
                    state | CLOSED,
                    Ordering::Relaxed,
                    Ordering::Acquire,
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
                Ordering::Relaxed,
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
