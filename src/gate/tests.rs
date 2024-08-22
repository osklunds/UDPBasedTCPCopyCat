
use super::*;

use std::time::Duration;

use async_std::future;
use async_std::future::TimeoutError;
use futures::executor::block_on;
use futures::Future;

#[test]
fn test() {
    let (controller, user) = new();

    let user = assert_can_pass(user);
    let user = assert_can_pass(user);
    let user = assert_can_pass(user);

    controller.close();

    assert_cannot_pass(user);
}

fn assert_can_pass(user: GateUser) -> GateUser {
    block_on(async {
        user.pass().await
    })
}

fn assert_cannot_pass(user: GateUser) {
    block_on(async {
        let dur = Duration::from_millis(1);
        let result = future::timeout(dur, user.pass()).await;
        assert!(result.is_err());
    });
}
