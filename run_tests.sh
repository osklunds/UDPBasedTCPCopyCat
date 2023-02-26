#!/bin/bash

RUST_BACKTRACE=1 cargo test "$1" -- --nocapture --test-threads 1 --color always
