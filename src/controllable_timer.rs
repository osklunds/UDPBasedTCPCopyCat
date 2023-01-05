// Global variable for channels. capacity one
// API fun: wait_for_timeout
// Internally, it sends on channel, waits for receive

#[cfg(test)]
mod tests;

use async_channel::{Receiver, Sender};
use async_std::future;
use futures::executor::block_on;
use std::time::Duration;

const WAIT_FOR_SLEEP_CALLED_TIMEOUT_MSG: &str =
    "Waiting for sleep to be called timed out";

pub struct Waiter {
    sleep_called_rx: Receiver<()>,
}

pub struct Returner {
    let_sleep_return_tx: Sender<()>,
}

pub struct Sleeper {
    sleep_called_tx: Sender<()>,
    let_sleep_return_rx: Receiver<()>,
}

// Test Case API

pub fn create() -> (Waiter, Returner, Sleeper) {
    let (sleep_called_tx, sleep_called_rx) = async_channel::bounded(1);
    let (let_sleep_return_tx, let_sleep_return_rx) = async_channel::bounded(1);

    let waiter = Waiter { sleep_called_rx };

    let returner = Returner {
        let_sleep_return_tx,
    };

    let sleeper = Sleeper {
        sleep_called_tx,
        let_sleep_return_rx,
    };

    (waiter, returner, sleeper)
}

impl Waiter {
    pub fn wait_for_sleep_called(self) {
        block_on(async {
            let duration = Duration::from_millis(10);
            let recv_sleep_called = self.sleep_called_rx.recv();
            let timeout_result =
                future::timeout(duration, recv_sleep_called).await;

            let recv_result =
                timeout_result.expect(WAIT_FOR_SLEEP_CALLED_TIMEOUT_MSG);
            recv_result.expect("Error receiving from sleep_called_rx");
        });
    }
}

impl Returner {
    pub fn let_sleep_return(self) {
        block_on(async {
            self.let_sleep_return_tx
                .try_send(())
                .expect("Error sending on let_sleep_return_tx");
        });
    }
}
// Unit Under Test API

impl Sleeper {
    pub async fn sleep(self) {
        self.sleep_called_tx
            .try_send(())
            .expect("send on sleep_called_tx failed");

        self.let_sleep_return_rx
            .recv()
            .await
            .expect("Error receiving on let_sleep_return_rx")
    }
}
