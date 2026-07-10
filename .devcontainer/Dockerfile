# Use Rust base image
FROM mcr.microsoft.com/devcontainers/rust:1@sha256:1707e2a8007968925f110c0961811200e9bb10e0ec055e2734857c59189a8b13

# Install additional dependencies
RUN apt-get update && apt-get install -y \
    build-essential \
    libdbus-1-dev \
    gnome-keyring \
    libxcb1-dev \
    protobuf-compiler \
    && apt-get clean
