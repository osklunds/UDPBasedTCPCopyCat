
use super::*;

use std::time::Duration;

use async_std::future;
use futures::executor::block_on;
use futures::Future;

#[test]
fn test() {
    let (controller, user) = new();

    block_on(async {
        user.pass().await;
        user.pass().await;
    });

    controller.close();

    block_on(async {
        assert_timeout(user.pass()).await;
    });

    println!("{:?}", "done");
}

async fn assert_timeout<F: Future<Output = T>, T>(f: F) {
    let dur = Duration::from_millis(1);
    assert!(future::timeout(dur, f).await.is_err());
}
