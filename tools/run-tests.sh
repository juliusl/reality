#!/bin/bash
set -e 

cargo update
cargo test --features full