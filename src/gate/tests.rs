
use super::*;

use std::time::Duration;
use std::thread;

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
fn can_open_after_creation() {
    let (controller, user) = new();

    controller.open();

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
fn cannot_pass_after_single_close() {
    let (controller, mut user) = new();

    user = assert_can_pass(user);

    controller.close();

    assert_cannot_pass(user);
}

#[test]
fn cannot_pass_after_multiple_closes() {
    let (controller, mut user) = new();

    user = assert_can_pass(user);

    controller.close();
    controller.close();
    controller.close();

    assert_cannot_pass(user);
}

#[test]
fn can_pass_after_single_open() {
    let (controller, mut user) = new();

    user = assert_can_pass(user);

    controller.close();
    controller.open();

    user = assert_can_pass(user);
    user = assert_can_pass(user);
    assert_can_pass(user);
}

#[test]
fn can_pass_after_multiple_opens() {
    let (controller, mut user) = new();

    user = assert_can_pass(user);

    controller.close();

    controller.open();
    controller.open();
    controller.open();

    user = assert_can_pass(user);
    user = assert_can_pass(user);
    assert_can_pass(user);
}

#[test]
fn open_while_trying_to_pass() {
    let (controller, mut user) = new();

    user = assert_can_pass(user);

    controller.close();

    let passer_thread = thread::spawn(move || {
        block_on(user.pass())
    });

    // Make user had a chance to call pass and get stuck
    thread::sleep(Duration::from_millis(5));

    // Make sure user hasn't passed through too early
    assert!(!passer_thread.is_finished());

    controller.open();

    user = passer_thread.join().unwrap();

    assert_can_pass(user);
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
