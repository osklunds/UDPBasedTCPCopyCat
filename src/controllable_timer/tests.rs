use super::*;

use std::thread;
use std::time::Duration;

#[test]
fn test_sleep_called() {
    initialize();

    let join_handle = thread::spawn(|| {
        block_on(async {
            println!("Calling sleep");
            sleep().await;
            println!("Sleep returned");
        })
    });

    wait_for_sleep_called();
    println!("Sleep was called");
    let_sleep_return();
    join_handle.join().unwrap();
}
