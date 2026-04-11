# sync-oneshot
[![crates.io](https://img.shields.io/crates/v/sync-oneshot.svg)](https://crates.io/crates/sync-oneshot)
[![docs.rs](https://img.shields.io/docsrs/sync-oneshot/latest)](https://docs.rs/sync-oneshot)
[![GitHub License](https://img.shields.io/github/license/locksmith-rs/sync-oneshot)][actions-url]

[actions-url]: https://github.com/locksmith-rs/sync-oneshot/actions?query=branch%3Amain+workflow%3ACI+

A minimal oneshot channel for synchronous Rust.

A oneshot channel is used for sending a single message between threads.  
The `channel` function is used to create a `Sender` and `Receiver`
handle pair that form the channel.

The `Sender` handle is used by the producer to send the value.  
The `Receiver` handle is used by the consumer to receive the value.

Each handle can be used on other threads.

`Sender::send` will no block the calling thread.  
`Receiver::recv` will **block** the calling thread.

## Example
```rust
use std::time::Duration;

let (tx, rx) = sync_oneshot::channel();

std::thread::spawn(move || {
    std::thread::sleep(Duration::from_millis(200));
    tx.send(5).unwrap();
});

// blocking thread until a message available
let val = rx.recv().unwrap();
assert_eq!(val, 5);
```

## Licence
`sync-oneshot` is provided under the MIT license.See [LICENSE](LICENSE)
