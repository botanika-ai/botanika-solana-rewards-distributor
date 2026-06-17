FROM ubuntu:22.04

ENV DEBIAN_FRONTEND=noninteractive

# system deps
RUN apt update && apt install -y \
    curl git build-essential pkg-config libssl-dev \
    libudev-dev llvm clang cmake

# -----------------------
# Rust (latest stable)
# -----------------------
RUN curl https://sh.rustup.rs -sSf | sh -s -- -y
ENV PATH="/root/.cargo/bin:$PATH"

RUN rustup update stable
RUN rustup default stable

# -----------------------
# Anza / Solana toolchain (latest)
# -----------------------
RUN sh -c "$(curl -sSfL https://release.anza.xyz/stable/install)"

ENV PATH="/root/.local/share/solana/install/active_release/bin:$PATH"

# verify
RUN rustc --version && cargo --version && solana --version

# -----------------------
# Anchor (AVM)
# -----------------------
RUN cargo install --git https://github.com/coral-xyz/anchor avm --locked
RUN avm install latest
RUN avm use latest

WORKDIR /app

CMD ["bash"]