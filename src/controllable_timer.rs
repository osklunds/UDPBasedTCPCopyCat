// Global variable for channels. capacity one
// API fun: wait_for_timeout
// Internally, it sends on channel, waits for receive

#[cfg(test)]
mod tests;

use async_channel::{Receiver, Sender};
use async_std::future;
use futures::executor::block_on;
use std::time::Duration;

struct Channels {
    sleep_called_tx: Sender<()>,
    sleep_called_rx: Receiver<()>,
    let_sleep_return_tx: Sender<()>,
    let_sleep_return_rx: Receiver<()>,
}

static mut CHANNELS: Option<Channels> = None;

// Test Case API

pub fn initialize() {
    let (sleep_called_tx, sleep_called_rx) = async_channel::bounded(1);
    let (let_sleep_return_tx, let_sleep_return_rx) = async_channel::bounded(1);

    let channels = Channels {
        sleep_called_tx,
        sleep_called_rx,
        let_sleep_return_tx,
        let_sleep_return_rx,
    };

    unsafe { CHANNELS = Some(channels) }
}

unsafe fn get_channels() -> &'static Channels {
    CHANNELS.as_ref().expect("CHANNELS was None")
}

pub fn wait_for_sleep_called() {
    block_on(async {
        unsafe {
            let recv_sleep_called = get_channels().sleep_called_rx.recv();
            let duration = Duration::from_millis(10);
            let timeout_result =
                future::timeout(duration, recv_sleep_called).await;
            let recv_result =
                timeout_result.expect("Timeout waiting for sleep to be called");
            recv_result.expect("Error receiving from sleep_called_rx");
        }
    });
}

pub fn let_sleep_return() {
    unsafe {
        get_channels()
            .let_sleep_return_tx
            .try_send(())
            .expect("Error sending on let_sleep_return_tx");
    }
}

// Unit Under Test API

pub async fn sleep() {
    unsafe {
        get_channels()
            .sleep_called_tx
            .try_send(())
            .expect("Error sending on sleep_called_tx");

        get_channels()
            .let_sleep_return_rx
            .recv()
            .await
            .expect("Error receiving on let_sleep_return_rx")
    }
}
