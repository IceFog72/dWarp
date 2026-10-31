#!/bin/bash
# Use the softbuffer CPU renderer so the window uses no GPU/VRAM
export WARP_CPU_RENDERER=1

# Memory management: suggest the OS to be aggressive about reclaiming memory
export MALLOC_TRIM_THRESHOLD_=131072

# Enable logging to verify performance and adapter selection
export RUST_LOG="info,wgpu=warn,wgpu_hal=warn,naga=warn,warp::terminal::view=info"

echo "Starting Warp with the CPU renderer (Zero VRAM)..."
echo "Target: target/release/warp-oss"

# Build and run the OSS version
# Using --release for better CPU renderer performance
cargo run --release -p warp --bin warp-oss --features gui -- "$@"
