// Global variable for channels. capacity one
// API fun: wait_for_timeout
// Internally, it sends on channel, waits for receive

#[cfg(test)]
mod tests;

use async_channel::{Receiver, Sender};
use async_std::future;
use futures::executor::block_on;
use futures::lock::{Mutex, MutexGuard};
use std::time::Duration;

struct Channels {
    sleep_called_tx: Mutex<Sender<()>>,
    sleep_called_rx: Mutex<Receiver<()>>,
    let_sleep_return_tx: Mutex<Sender<()>>,
    let_sleep_return_rx: Mutex<Receiver<()>>,
}

static mut CHANNELS: Option<Channels> = None;

const SLEEP_CALLED_BEFORE_SLEEP_RETURNED_MSG: &str =
    "Sleep called before sleep returned";

// Test Case API

pub fn initialize() {
    let (sleep_called_tx, sleep_called_rx) = async_channel::bounded(1);
    let (let_sleep_return_tx, let_sleep_return_rx) = async_channel::bounded(1);

    let channels = Channels {
        sleep_called_tx: Mutex::new(sleep_called_tx),
        sleep_called_rx: Mutex::new(sleep_called_rx),
        let_sleep_return_tx: Mutex::new(let_sleep_return_tx),
        let_sleep_return_rx: Mutex::new(let_sleep_return_rx),
    };

    unsafe { CHANNELS = Some(channels) }
}

unsafe fn get_channels() -> &'static Channels {
    CHANNELS.as_ref().expect("CHANNELS was None")
}

pub fn wait_for_sleep_called() {
    block_on(async {
        unsafe {
            let sleep_called_rx = get_channels().sleep_called_rx.lock().await;
            let recv_sleep_called = sleep_called_rx.recv();

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
        block_on(async {
            get_channels()
                .let_sleep_return_tx
                .lock()
                .await
                .try_send(())
                .expect("Error sending on let_sleep_return_tx");
        });
    }
}

// Unit Under Test API

pub async fn sleep() {
    unsafe {
        let send_result =
            get_channels().sleep_called_tx.lock().await.try_send(());

        match send_result {
            Ok(_) => (),
            Err(async_channel::TrySendError::Full(_)) => {
                panic!("{}", SLEEP_CALLED_BEFORE_SLEEP_RETURNED_MSG)
            }
            Err(_) => panic!("send on sleep_called_tx failed"),
        }

        get_channels()
            .let_sleep_return_rx
            .lock()
            .await
            .recv()
            .await
            .expect("Error receiving on let_sleep_return_rx")
    }
}
