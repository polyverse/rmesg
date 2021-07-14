#!/bin/sh


IMAGE="ghcr.io/polyverse/rust-dev-env"
docker run -v cargo-cache:/root/.cargo/registry -v $PWD:/volume --rm -it --privileged $IMAGE
