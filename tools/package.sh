#!/bin/bash

set -e 

package=$1

echo "Testing package $package"
cargo test --package "$package"

echo "Building .crate package"
cargo package --package "$package"
