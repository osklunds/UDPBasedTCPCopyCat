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

const SLEEP_CALLED_BEFORE_SLEEP_RETURNED_MSG: &str =
    "Sleep called before sleep returned";
const WAIT_FOR_SLEEP_CALLED_TIMEOUT_MSG: &str =
    "Waiting for sleep to be called timed out";

pub struct Waiter {
    sleep_called_rx: Mutex<Receiver<()>>,
}

pub struct Returner {
    let_sleep_return_tx: Mutex<Sender<()>>,
}

pub struct Sleeper {
    sleep_called_tx: Mutex<Sender<()>>,
    let_sleep_return_rx: Mutex<Receiver<()>>,
}

// Test Case API

pub fn create() -> (Waiter, Returner, Sleeper) {
    let (sleep_called_tx, sleep_called_rx) = async_channel::bounded(1);
    let (let_sleep_return_tx, let_sleep_return_rx) = async_channel::bounded(1);

    let waiter = Waiter {
        sleep_called_rx: Mutex::new(sleep_called_rx),
    };

    let returner = Returner {
        let_sleep_return_tx: Mutex::new(let_sleep_return_tx),
    };

    let sleeper = Sleeper {
        sleep_called_tx: Mutex::new(sleep_called_tx),
        let_sleep_return_rx: Mutex::new(let_sleep_return_rx),
    };

    (waiter, returner, sleeper)
}

impl Waiter {
    pub fn wait_for_sleep_called(self) {
        block_on(async {
            let sleep_called_rx = self.sleep_called_rx.lock().await;
            let recv_sleep_called = sleep_called_rx.recv();

            let duration = Duration::from_millis(10);
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
                .lock()
                .await
                .try_send(())
                .expect("Error sending on let_sleep_return_tx");
        });
    }
}
// Unit Under Test API

impl Sleeper {
    pub async fn sleep(self) {
        let send_result = self.sleep_called_tx.lock().await.try_send(());

        match send_result {
            Ok(_) => (),
            Err(async_channel::TrySendError::Full(_)) => {
                panic!("{}", SLEEP_CALLED_BEFORE_SLEEP_RETURNED_MSG)
            }
            Err(_) => panic!("send on sleep_called_tx failed"),
        }

        self.let_sleep_return_rx
            .lock()
            .await
            .recv()
            .await
            .expect("Error receiving on let_sleep_return_rx")
    }
}
