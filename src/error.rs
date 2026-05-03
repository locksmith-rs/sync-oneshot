/// Error returned by the [`Receiver::recv`](crate::Receiver::recv).
///
/// This error is returned by the receiver when the sender is dropped without sending.
#[derive(Debug)]
pub struct RecvError;

/// Error returned by the [`try_recv`](crate::Receiver::try_recv) function on
/// [`Receiver`](crate::Receiver).
#[derive(Debug, PartialEq, Eq)]
pub enum TryRecvError {
    /// The send half of the channel has not yet sent a value.
    Empty,

    /// The send half of the channel was dropped without sending a value.
    Closed,
}

#[derive(Debug)]
pub enum RecvTimeoutError {
    Timeout,
    Closed,
}
