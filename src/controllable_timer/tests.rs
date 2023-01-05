use super::*;

use std::thread;

#[test]
fn test_sleep_called() {
    // Arrange
    initialize();

    // Act

    let join_handle = thread::spawn(|| block_on(async { sleep().await }));

    // Assert
    wait_for_sleep_called();
    let_sleep_return();
    join_handle.join().unwrap();
}

#[test]
fn test_sleep_not_called() {
    // Arrange
    initialize();

    // Act
    let error = catch_panic(wait_for_sleep_called);

    // Assert
    assert!(error.starts_with("Timeout waiting for sleep to be called"));
}

#[test]
fn test_sleep_called_twice() {
    // Arrange
    initialize();

    thread::spawn(|| block_on(async { sleep().await }));
    thread::sleep(Duration::from_millis(1));

    // Act
    let error = catch_panic(|| block_on(async { sleep().await }));

    // Assert
    assert!(error.starts_with("Error sending on SLEEP_CALLED_TX: Full"));
}

fn catch_panic(f: fn() -> ()) -> String {
    let result = std::panic::catch_unwind(f);
    *result.err().unwrap().downcast::<String>().unwrap()
}
