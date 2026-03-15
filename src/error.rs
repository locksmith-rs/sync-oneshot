/// Error returned by the [`Receiver::recv`](crate::Receiver::recv).
///
/// This error is returned by the receiver when the sender is dropped without sending.
#[derive(Debug)]
pub enum RecvError {
    ShutDown,
}
