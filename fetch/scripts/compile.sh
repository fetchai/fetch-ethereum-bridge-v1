#!/bin/bash

set -e

wd=$(pwd)
cd $1
RUSTFLAGS='-C link-arg=-s' cargo wasm
cp target/wasm32-unknown-unknown/release/*.wasm $wd

if [[ $* == *--test* ]];
then
    cargo test --lib
fi

rm target/ -rf
