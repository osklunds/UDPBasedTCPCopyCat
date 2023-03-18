use async_channel::{Receiver, Sender};
use async_std::future;
use async_std::net::*;
use async_trait::async_trait;
use futures::executor::block_on;
use futures::lock::{Mutex, MutexGuard};
use std::time::Duration;

use crate::socket::{SleepDuration, Timer, RETRANSMISSION_TIMER};

pub struct MockTimer {
    sleep_expected: Mutex<Option<SleepDuration>>,
    sleep_called_tx: Sender<()>,
    sleep_called_rx: Receiver<()>,
    let_sleep_return_tx: Sender<()>,
    let_sleep_return_rx: Receiver<()>,
}

impl MockTimer {
    pub fn new() -> Self {
        let sleep_expected = Mutex::new(None);
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

    pub fn expect_forever_sleep(&self) {
        block_on(async {
            let mut locked_sleep_expected = self.sleep_expected.lock().await;
            assert!(locked_sleep_expected.is_none());
            *locked_sleep_expected = Some(SleepDuration::Forever);
        });
    }

    pub fn expect_sleep(&self) {
        block_on(async {
            let mut locked_sleep_expected = self.sleep_expected.lock().await;
            assert!(locked_sleep_expected.is_none());
            *locked_sleep_expected =
                Some(SleepDuration::Finite(RETRANSMISSION_TIMER));
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
            assert!(locked_sleep_expected.is_none());
            *locked_sleep_expected =
                Some(SleepDuration::Finite(RETRANSMISSION_TIMER));

            self.let_sleep_return_tx.try_send(()).unwrap();
        });
    }

    pub fn test_end_check(&self) {
        block_on(async {
            // First check that the TC hasn't expected any sleep that hasn't
            // been executed yet
            let locked_sleep_expected = self.sleep_expected.lock().await;
            assert!(locked_sleep_expected.is_none());

            // Then check that no sleep has been made that the TC hasn't
            // waited for
            assert!(self.sleep_called_rx.is_empty());

            // Finally let the sleep return so that a retransmission of
            // everything in the buffer is done, so that non empty
            // buffer is detected.
            self.let_sleep_return_tx.try_send(()).unwrap();
        });
    }
}

#[async_trait]
impl Timer for MockTimer {
    async fn sleep(&self, duration: SleepDuration) {
        let mut locked_sleep_expected = self.sleep_expected.lock().await;
        assert_eq!(*locked_sleep_expected, Some(duration));
        *locked_sleep_expected = None;
        drop(locked_sleep_expected);

        self.sleep_called_tx.try_send(()).unwrap();
        self.let_sleep_return_rx.recv().await.unwrap();
    }
}
