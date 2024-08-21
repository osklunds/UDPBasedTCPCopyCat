use async_channel::{Receiver, Sender};
use async_std::future;
use async_std::net::*;
use async_trait::async_trait;
use futures::executor::block_on;
use futures::lock::{Mutex, MutexGuard};
use std::time::Duration;

use crate::socket::{SleepDuration, Timer, RETRANSMISSION_TIMER};

pub struct MockTimer {
    expect_sleep_tx: Sender<SleepDuration>,
    expect_sleep_rx: Receiver<SleepDuration>,
    sleep_called_tx: Sender<()>,
    sleep_called_rx: Receiver<()>,
    let_sleep_return_tx: Sender<()>,
    let_sleep_return_rx: Receiver<()>,
}

impl MockTimer {
    pub fn new() -> Self {
        let (expect_sleep_tx, expect_sleep_rx) = async_channel::bounded(1);
        let (sleep_called_tx, sleep_called_rx) = async_channel::bounded(1);
        let (let_sleep_return_tx, let_sleep_return_rx) =
            async_channel::bounded(1);

        MockTimer {
            expect_sleep_tx,
            expect_sleep_rx,
            sleep_called_tx,
            sleep_called_rx,
            let_sleep_return_tx,
            let_sleep_return_rx,
        }
    }

    // Sleeping the retransmission timer length is the same as "timer starting"
    // because now the uut is waiting that time for some ACK.
    pub fn expect_start(&self) {
        self.expect_call(SleepDuration::Finite(RETRANSMISSION_TIMER));
    }

    // Sleeping forever is the same as "timer stopping" because now the uut
    // doesn't need the timer, isn't waiting for any ACK.
    pub fn expect_stop(&self) {
        self.expect_call(SleepDuration::Forever);
    }

    fn expect_call(&self, duration: SleepDuration) {
        self.expect_sleep_tx
            .try_send(duration)
            .expect("Expect sleep called, but sleep already expected");
    }

    pub fn wait_for_call(&self) {
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

    pub fn test_end_check(&self) {
        block_on(async {
            // First check that the TC hasn't expected any sleep that hasn't
            // been executed yet
            assert!(
                self.expect_sleep_tx.is_empty(),
                "In test end check, a call to sleep is expected to happen"
            );

            // Then check that no sleep has been made that the TC hasn't
            // waited for
            assert!(
                self.sleep_called_rx.is_empty(),
                "In test end check, a call to sleep has been made that the tc has not waited for"
            );

            // Finally let the sleep return so that a retransmission of
            // everything in the buffer is done, so that non empty
            // buffer is detected.
            self.trigger();
        });
    }

    pub fn trigger(&self) {
        self.expect_start();
        self.let_sleep_return_tx.try_send(()).unwrap();
        self.wait_for_call();
    }
}

#[async_trait]
impl Timer for MockTimer {
    async fn sleep(&self, duration: SleepDuration) {
        let exp_duration = self.expect_sleep_rx.try_recv().expect(&format!(
            "Sleep called but not expectation set up, duration: {:?}",
            duration
        ));
        assert_eq!(exp_duration, duration);
        self.sleep_called_tx.try_send(()).unwrap();
        self.let_sleep_return_rx.recv().await.unwrap();
    }
}
