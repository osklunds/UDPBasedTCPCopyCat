// Global variable for channels. capacity one
// API fun: wait_for_timeout
// Internally, it sends on channel, waits for receive

#[cfg(test)]
mod tests;

use async_channel::{Receiver, Sender};
use async_std::future;
use futures::executor::block_on;
use std::time::Duration;

static mut SLEEP_CALLED_TX: Option<Sender<()>> = None;
static mut SLEEP_CALLED_RX: Option<Receiver<()>> = None;
static mut LET_SLEEP_RETURN_TX: Option<Sender<()>> = None;
static mut LET_SLEEP_RETURN_RX: Option<Receiver<()>> = None;

// Test Case API

pub fn initialize() {
    let (sleep_called_tx, sleep_called_rx) = async_channel::bounded(1);
    let (let_sleep_return_tx, let_sleep_return_rx) = async_channel::bounded(1);

    unsafe {
        SLEEP_CALLED_TX = Some(sleep_called_tx);
        SLEEP_CALLED_RX = Some(sleep_called_rx);
        LET_SLEEP_RETURN_TX = Some(let_sleep_return_tx);
        LET_SLEEP_RETURN_RX = Some(let_sleep_return_rx);
    }
}

pub fn wait_for_sleep_called() {
    assert!(
        wait_for_sleep_called_no_panic(),
        "Timeout waiting for sleep to be called"
    );
}

fn wait_for_sleep_called_no_panic() -> bool {
    let timeout_result = block_on(async {
        unsafe {
            let recv_sleep_called = SLEEP_CALLED_RX
                .as_ref()
                .expect("SLEEP_CALLED_RX was None")
                .recv();
            let duration = Duration::from_millis(10);
            future::timeout(duration, recv_sleep_called).await
        }
    });
    match timeout_result {
        Ok(recv_result) => {
            recv_result.expect("Error receiving from SLEEP_CALLED_RX");
            true
        }
        Err(_) => false,
    }
}

pub fn let_sleep_return() {
    unsafe {
        LET_SLEEP_RETURN_TX
            .as_ref()
            .expect("LET_SLEEP_RETURN_TX was None")
            .try_send(())
            .expect("Error sending on LET_SLEEP_RETURN_TX");
    }
}

// Unit Under Test API

pub async fn sleep() {
    unsafe {
        SLEEP_CALLED_TX
            .as_ref()
            .expect("SLEEP_CALLED_TX was None")
            .try_send(())
            .expect("Error sending on SLEEP_CALLED_TX");

        LET_SLEEP_RETURN_RX
            .as_ref()
            .expect("LET_SLEEP_RETURN_RX was None")
            .recv()
            .await
            .expect("Error receiving on LET_SLEEP_RETURN_RX")
    }
}
