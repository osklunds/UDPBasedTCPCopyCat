use async_channel::{Receiver, Sender};
use async_std::future;
use async_std::net::*;
use async_trait::async_trait;
use futures::executor::block_on;
use futures::lock::{Mutex, MutexGuard};
use std::time::Duration;

use crate::socket::{Timer, RETRANSMISSION_TIMER};

pub struct MockTimer {
    sleep_expected: Mutex<bool>,
    sleep_called_tx: Sender<()>,
    sleep_called_rx: Receiver<()>,
    let_sleep_return_tx: Sender<()>,
    let_sleep_return_rx: Receiver<()>,
}

impl MockTimer {
    pub fn new() -> Self {
        let sleep_expected = Mutex::new(false);
        let (sleep_called_tx, sleep_called_rx) = async_channel::bounded(1);
        let (let_sleep_return_tx, let_sleep_return_rx) =
            async_channel::bounded(1);

        MockTimer {
            sleep_expected,
            sleep_called_tx,
            sleep_called_rx,
            let_sleep_return_tx,
            let_sleep_return_rx,
        }
    }

    pub fn expect_call_to_sleep(&self) {
        block_on(async {
            let mut locked_sleep_expected = self.sleep_expected.lock().await;
            assert!(!*locked_sleep_expected);
            *locked_sleep_expected = true;
        });
    }

    pub fn wait_for_call_to_sleep(&self) {
        block_on(async {
            let duration = Duration::from_millis(10);
            let recv_sleep_called = self.sleep_called_rx.recv();
            let timeout_result =
                future::timeout(duration, recv_sleep_called).await;

            let recv_result =
                timeout_result.expect("Timeout waiting for sleep to be called");
            recv_result.expect("Error receiving from sleep_called_rx");
        });
    }

    pub fn trigger_and_expect_new_call(&self) {
        block_on(async {
            let mut locked_sleep_expected = self.sleep_expected.lock().await;
            assert!(!*locked_sleep_expected);
            *locked_sleep_expected = true;

            self.let_sleep_return_tx.try_send(()).unwrap();
        });
    }

    // TODO: Move to drop when the Socket process is closed/FIN-ed
    pub fn test_end_check(&self) {
        block_on(async {
            let locked_sleep_expected = self.sleep_expected.lock().await;
            assert!(!*locked_sleep_expected);

            assert!(self.sleep_called_rx.is_empty());
        });
    }
}

#[async_trait]
impl Timer for MockTimer {
    async fn sleep(&self, duration: Duration) {
        assert_eq!(RETRANSMISSION_TIMER, duration);

        let mut locked_sleep_expected = self.sleep_expected.lock().await;
        assert!(*locked_sleep_expected);
        *locked_sleep_expected = false;
        drop(locked_sleep_expected);

        self.sleep_called_tx.try_send(()).unwrap();
        self.let_sleep_return_rx.recv().await.unwrap();
    }
}
