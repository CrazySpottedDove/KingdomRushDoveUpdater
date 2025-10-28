#!/bin/bash
for f in ./target/x86_64-pc-windows-gnu/release/deps/*.rlib; do
    nm -C "$f" | grep -q GetSystemTimePreciseAsFileTime && echo "$f"
done
