
mod tests;

use async_channel::{Receiver, Sender, TryRecvError, TrySendError};

pub fn new() -> (GateController, GateUser) {
    let (close_tx, close_rx) = async_channel::bounded(1);
    let (open_tx, open_rx) = async_channel::bounded(1);

    let controller = GateController {
        close_tx,
        open_tx,
    };
    let user = GateUser {
        close_rx,
        open_rx
    };

    (controller, user)
}

pub struct GateController {
    close_tx: Sender<()>,
    open_tx: Sender<()>,
}

pub struct GateUser {
    close_rx: Receiver<()>,
    open_rx: Receiver<()>,
}

impl GateController {
    pub fn open(&self) {
        self.try_send(&self.open_tx);
    }

    pub fn close(&self) {
        self.try_send(&self.close_tx);
    }

    fn try_send(&self, sender: &Sender<()>) {
        match sender.try_send(()) {
            Ok(_) => (),
            Err(TrySendError::Full(_)) => (),
            Err(TrySendError::Closed(_)) => panic!("Closed"),
        }
    }
}

impl GateUser {
    pub async fn pass(&self) {
        if self.close_rx.try_recv().is_ok() {
            self.open_rx.recv().await.unwrap();
        }
    }
}
