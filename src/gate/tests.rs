
use super::*;

use std::time::Duration;

use async_std::future;
use futures::executor::block_on;
use futures::Future;

#[test]
fn test() {
    let (controller, user) = new();

    assert_can_pass(&user);
    assert_can_pass(&user);
    assert_can_pass(&user);

    controller.close();

    assert_cannot_pass(&user);
}

fn assert_can_pass(user: &GateUser) {
    block_on(async {
        user.pass().await;
    });
}

fn assert_cannot_pass(user: &GateUser) {
    block_on(async {
        assert_timeout(user.pass()).await;
    });
}

async fn assert_timeout<F: Future<Output = T>, T>(f: F) {
    let dur = Duration::from_millis(1);
    assert!(future::timeout(dur, f).await.is_err());
}
