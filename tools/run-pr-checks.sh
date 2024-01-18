#!/bin/bash 

set -e

# Run checks before pull request can be merged 

cargo fmt --check
cargo clippy
