use super::*;

use std::thread;

#[test]
fn test_sleep_called() {
    initialize();

    let join_handle = thread::spawn(|| {
        block_on(async {
            sleep().await;
        })
    });

    wait_for_sleep_called();
    let_sleep_return();
    join_handle.join().unwrap();
}

#[test]
fn test_sleep_not_called() {
    initialize();

    assert!(!wait_for_sleep_called_no_panic());
}
