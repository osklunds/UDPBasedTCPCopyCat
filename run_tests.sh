#!/bin/bash

# Use 10 threads since unfortunately there are some sleeps in the test cases
RUST_BACKTRACE=1 cargo test "$1" -- --nocapture --test-threads 1 --color always
