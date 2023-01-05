use super::*;

use std::thread;

#[test]
fn test_sleep_called() {
    // Arrange
    let (waiter, returner, sleeper) = create();

    // Act
    spawn_thread_calling_sleep(sleeper);

    // Assert
    waiter.wait_for_sleep_called();
    returner.let_sleep_return();
}

#[test]
fn test_sleep_not_called() {
    // Arrange
    let (waiter, _returner, _sleeper) = create();

    // Act
    let error = catch_panic(move || {
        waiter.wait_for_sleep_called();
    });

    // Assert
    assert!(error.starts_with(WAIT_FOR_SLEEP_CALLED_TIMEOUT_MSG));
}

fn spawn_thread_calling_sleep(sleeper: Sleeper) -> thread::JoinHandle<()> {
    thread::spawn(|| block_on_sleep(sleeper))
}

fn block_on_sleep(sleeper: Sleeper) {
    block_on(async { sleeper.sleep().await })
}

fn catch_panic<F: FnOnce() + std::panic::UnwindSafe>(f: F) -> String {
    let result = std::panic::catch_unwind(f);
    *result.err().unwrap().downcast::<String>().unwrap()
}
