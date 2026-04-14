#!/usr/bin/env bash

rustup install 1.94.0
rustup default 1.94.0
rustup target add x86_64-unknown-linux-musl
rustup component add rust-src --toolchain 1.94.0-x86_64-unknown-linux-gnu
rustup target add aarch64-unknown-none
rustup target add riscv64gc-unknown-none-elf
rustup target add x86_64-unknown-none

sudo apt update
sudo apt install software-properties-common
sudo add-apt-repository ppa:deadsnakes/ppa
sudo apt install \
    gcc-x86-64-linux-gnu \
    gcc-riscv64-unknown-elf \
    cmake pandoc device-tree-compiler ninja-build \
    texlive-latex-base texlive-latex-recommended \
    texlive-fonts-recommended texlive-fonts-extra \
    libxml2-utils \
    python3.12 python3-pip python3.12-venv \
    qemu-system-arm qemu-system-misc

python3.12 -m venv pyenv
./pyenv/bin/pip install --upgrade pip setuptools wheel
./pyenv/bin/pip install -r requirements.txt

wget -O aarch64-toolchain.tar.gz https://sel4-toolchains.s3.us-east-2.amazonaws.com/arm-gnu-toolchain-12.2.rel1-x86_64-aarch64-none-elf.tar.xz%3Frev%3D28d5199f6db34e5980aae1062e5a6703%26hash%3DF6F5604BC1A2BBAAEAC4F6E98D8DC35B
tar xf aarch64-toolchain.tar.gz
echo "$(pwd)/arm-gnu-toolchain-12.2.rel1-x86_64-aarch64-none-elf/bin" >> $GITHUB_PATH
