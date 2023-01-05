use super::*;

use std::thread;

#[test]
fn test_sleep_called() {
    // Arrange
    initialize();

    // Act
    let join_handle = thread::spawn(|| {
        block_on(async {
            sleep().await;
        })
    });

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
    let result = std::panic::catch_unwind(|| wait_for_sleep_called());

    // Assert
    let error = result.err().unwrap().downcast::<String>().unwrap();
    assert!(error.starts_with("Timeout waiting for sleep to be called"));
}
