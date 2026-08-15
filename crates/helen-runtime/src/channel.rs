//! Bidirectional message channel (Task 7.1) — port of `helen/runtime/channel.py`.
//!
//! A `Channel` carries messages between two endpoints: the main-thread side
//! (returned by `spawn`) and the spawned-agent side (auto-injected as the
//! agent's last `Channel` parameter). Each endpoint writes to its own outbox
//! and reads from its own inbox (two internal queues — matching Python).
//!
//! Close semantics (Python parity):
//! - `send` after close is silently ignored.
//! - `close`/`cancel` push a sentinel into the outbox to wake a blocked
//!   receiver; the next `receive` returns `Some(sentinel)` which the
//!   interpreter maps to `None` (indistinguishable from a real `None`
//!   message — exactly as in Python).
//! - `receive(timeout)` returns `None` on timeout or when closed.

use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;

/// Message types queued on a `Channel` must be `Send` and able to produce a
/// sentinel value (the wake-up marker pushed on close/cancel).
pub trait Queueable: Send {
    /// The sentinel message (Python: the `None` value).
    fn sentinel() -> Self;
}

/// A bidirectional message channel (two internal FIFO queues).
pub struct Channel<T: Queueable> {
    pub name: String,
    /// Main thread -> spawned agent direction.
    to_spawned: Mutex<VecDeque<T>>,
    /// Spawned agent -> main thread direction.
    from_spawned: Mutex<VecDeque<T>>,
    closed: AtomicBool,
    cancelled: AtomicBool,
    cv: Condvar,
}

impl<T: Queueable> Channel<T> {
    pub fn new(name: impl Into<String>) -> Self {
        Channel {
            name: name.into(),
            to_spawned: Mutex::new(VecDeque::new()),
            from_spawned: Mutex::new(VecDeque::new()),
            closed: AtomicBool::new(false),
            cancelled: AtomicBool::new(false),
            cv: Condvar::new(),
        }
    }

    pub fn is_closed(&self) -> bool {
        self.closed.load(Ordering::SeqCst)
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::SeqCst)
    }

    fn mark_closed(&self) {
        self.closed.store(true, Ordering::SeqCst);
        self.cv.notify_all();
    }

    fn mark_cancelled(&self) {
        self.cancelled.store(true, Ordering::SeqCst);
        self.mark_closed();
    }
}

/// One side of a channel.
pub struct ChannelEndpoint<T: Queueable> {
    channel: Arc<Channel<T>>,
    is_main: bool,
}

impl<T: Queueable> ChannelEndpoint<T> {
    pub fn new(channel: Arc<Channel<T>>, is_main: bool) -> Self {
        ChannelEndpoint { channel, is_main }
    }

    pub fn channel(&self) -> &Arc<Channel<T>> {
        &self.channel
    }

    pub fn is_main_thread(&self) -> bool {
        self.is_main
    }

    /// Cancellation signal (only meaningful for the spawned endpoint).
    pub fn is_cancelled(&self) -> bool {
        self.channel.is_cancelled()
    }

    pub fn is_closed(&self) -> bool {
        self.channel.is_closed()
    }

    fn outbox(&self) -> &Mutex<VecDeque<T>> {
        if self.is_main {
            &self.channel.to_spawned
        } else {
            &self.channel.from_spawned
        }
    }

    fn inbox(&self) -> &Mutex<VecDeque<T>> {
        if self.is_main {
            &self.channel.from_spawned
        } else {
            &self.channel.to_spawned
        }
    }

    /// `send` — silently ignored after the channel is closed.
    pub fn send(&self, msg: T) {
        if self.channel.is_closed() {
            return;
        }
        self.outbox().lock().unwrap().push_back(msg);
        self.channel.cv.notify_all();
    }

    /// `try_receive` — non-blocking; `None` when the inbox is empty.
    pub fn try_receive(&self) -> Option<T> {
        self.inbox().lock().unwrap().pop_front()
    }

    /// `receive(timeout)` — blocking; `None` on timeout or when closed.
    pub fn receive(&self, timeout: Option<Duration>) -> Option<T> {
        let mut q = self.inbox().lock().unwrap();
        loop {
            if let Some(v) = q.pop_front() {
                return Some(v);
            }
            if self.channel.is_closed() {
                return None;
            }
            match timeout {
                None => q = self.channel.cv.wait(q).unwrap(),
                Some(d) => {
                    let (guard, res) = self.channel.cv.wait_timeout(q, d).unwrap();
                    q = guard;
                    if res.timed_out() {
                        return None;
                    }
                }
            }
        }
    }

    /// `cancel` — mark cancelled + closed and wake the other side.
    pub fn cancel(&self) {
        self.channel.mark_cancelled();
        self.push_sentinel();
    }

    /// `close` — mark closed and wake the other side.
    pub fn close(&self) {
        self.channel.mark_closed();
        self.push_sentinel();
    }

    fn push_sentinel(&self) {
        let mut q = self.outbox().lock().unwrap();
        q.push_back(T::sentinel());
        self.channel.cv.notify_all();
    }
}

impl<T: Queueable> std::fmt::Debug for ChannelEndpoint<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let side = if self.is_main { "main" } else { "spawned" };
        write!(f, "ChannelEndpoint({:?}, {side})", self.channel.name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Minimal test message type.
    #[derive(Clone, Debug, PartialEq)]
    struct Msg(Option<String>);
    impl Queueable for Msg {
        fn sentinel() -> Self {
            Msg(None)
        }
    }

    fn endpoints() -> (ChannelEndpoint<Msg>, ChannelEndpoint<Msg>) {
        let ch = Arc::new(Channel::<Msg>::new("test"));
        let main = ChannelEndpoint::new(ch.clone(), true);
        let spawned = ChannelEndpoint::new(ch, false);
        (main, spawned)
    }

    #[test]
    fn send_receive_round_trip() {
        let (main, spawned) = endpoints();
        main.send(Msg(Some("hello".into())));
        assert_eq!(spawned.receive(None), Some(Msg(Some("hello".into()))));
        spawned.send(Msg(Some("back".into())));
        assert_eq!(main.receive(None), Some(Msg(Some("back".into()))));
    }

    #[test]
    fn receive_timeout_returns_none() {
        let (main, _spawned) = endpoints();
        assert_eq!(main.receive(Some(Duration::from_millis(50))), None);
    }

    #[test]
    fn try_receive_empty_returns_none() {
        let (main, _spawned) = endpoints();
        assert_eq!(main.try_receive(), None);
    }

    #[test]
    fn send_after_close_is_ignored() {
        let (main, spawned) = endpoints();
        main.close();
        main.send(Msg(Some("lost".into())));
        // Close pushes a sentinel (Python: receive() returns None). A real
        // message sent after close is never delivered.
        assert_eq!(
            spawned.receive(Some(Duration::from_millis(30))),
            Some(Msg::sentinel())
        );
        assert_eq!(spawned.receive(Some(Duration::from_millis(30))), None);
    }

    #[test]
    fn close_wakes_blocked_receiver_with_sentinel() {
        let (main, spawned) = endpoints();
        // Spawn a thread that blocks on receive.
        let spawned2 = {
            // We need a second reference to the spawned endpoint for the thread.
            // Rebuild endpoints sharing the same channel.
            let ch = main.channel().clone();
            ChannelEndpoint::new(ch, false)
        };
        let h = std::thread::spawn(move || spawned2.receive(None));
        // Let the receiver block, then close.
        std::thread::sleep(Duration::from_millis(30));
        main.close();
        assert_eq!(h.join().unwrap(), Some(Msg::sentinel()));
        // Second receive on empty closed channel -> None.
        assert_eq!(spawned.receive(Some(Duration::from_millis(30))), None);
    }

    #[test]
    fn cancel_sets_both_flags() {
        let (main, spawned) = endpoints();
        assert!(!spawned.is_cancelled());
        assert!(!spawned.is_closed());
        main.cancel();
        assert!(spawned.is_cancelled());
        assert!(spawned.is_closed());
    }

    #[test]
    fn fifo_order_preserved() {
        let (main, spawned) = endpoints();
        for i in 0..5 {
            main.send(Msg(Some(i.to_string())));
        }
        for i in 0..5 {
            assert_eq!(spawned.try_receive(), Some(Msg(Some(i.to_string()))));
        }
    }
}
