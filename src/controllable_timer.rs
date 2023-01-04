// Global variable for channels. capacity one
// API fun: wait_for_timeout
// Internally, it sends on channel, waits for receive

#[cfg(test)]
mod tests;

use async_channel::{Receiver, Sender};
use futures::executor::block_on;

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
    unsafe {
        SLEEP_CALLED_RX
            .as_ref()
            .expect("SLEEP_CALLED_RX was None")
            .recv_blocking()
            .expect("Error receiving from SLEEP_CALLED_RX");
    }
}

pub fn let_sleep_return() {
    unsafe {
        LET_SLEEP_RETURN_TX.as_ref().unwrap().try_send(()).unwrap();
    }
}

// Unit Under Test API

pub async fn sleep() {
    unsafe {
        SLEEP_CALLED_TX.as_ref().unwrap().try_send(()).unwrap();
        LET_SLEEP_RETURN_RX.as_ref().unwrap().recv().await.unwrap();
    }
}
