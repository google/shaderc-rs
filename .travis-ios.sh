#!/bin/bash
set -e
if [ `uname` = Darwin ]
then
    rustup target add x86_64-apple-ios aarch64-apple-ios;
    cargo build --target x86_64-apple-ios
    cargo build --target aarch64-apple-ios
    RUNTIME_ID=$(xcrun simctl list runtimes | grep iOS | cut -d ' ' -f 7 | tail -1)
    export SIM_ID=$(xcrun simctl create My-iphone7 com.apple.CoreSimulator.SimDeviceType.iPhone-7 $RUNTIME_ID)
    xcrun simctl boot $SIM_ID
    cargo install cargo-dinghy
    cargo dinghy test --target x86_64-apple-ios
fi
