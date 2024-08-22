
use super::*;

use std::time::Duration;

use async_std::future;
use async_std::future::TimeoutError;
use futures::executor::block_on;
use futures::Future;

#[test]
fn can_pass_after_creation() {
    let (_controller, user) = new();

    assert_can_pass(user);
}

#[test]
fn can_pass_multiple_times() {
    let (_controller, mut user) = new();

    user = assert_can_pass(user);
    user = assert_can_pass(user);
    assert_can_pass(user);
}

#[test]
fn cannot_pass_after_close() {
    let (controller, mut user) = new();

    user = assert_can_pass(user);

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
