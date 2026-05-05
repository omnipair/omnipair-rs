FROM ubuntu:24.04

ARG DEBIAN_FRONTEND=noninteractive
ARG NODE_MAJOR=20
ARG RUST_TOOLCHAIN=1.86.0
ARG SOLANA_CLI=2.3.11
ARG ANCHOR_CLI=0.31.1

ENV HOME=/root
ENV PATH="/root/.cargo/bin:/root/.local/bin:/root/.avm/bin:/root/.local/share/solana/install/active_release/bin:${PATH}"
ENV FORK_LAB_BUILD=false

RUN apt-get update -qq && apt-get install -y --no-install-recommends \
    bash \
    build-essential \
    ca-certificates \
    curl \
    git \
    jq \
    libssl-dev \
    libudev-dev \
    pkg-config \
    python3 \
    xz-utils \
    && rm -rf /var/lib/apt/lists/*

RUN curl https://sh.rustup.rs -sSf | sh -s -- -y --default-toolchain "${RUST_TOOLCHAIN}" \
    && rustup component add rustfmt clippy

RUN curl -fsSL "https://deb.nodesource.com/setup_${NODE_MAJOR}.x" | bash - \
    && apt-get update -qq \
    && apt-get install -y --no-install-recommends nodejs \
    && npm install -g yarn@1.22.22 \
    && rm -rf /var/lib/apt/lists/*

RUN sh -c "$(curl -sSfL https://release.anza.xyz/v${SOLANA_CLI}/install)"

RUN cargo install --git https://github.com/coral-xyz/anchor avm --locked --force \
    && avm install "${ANCHOR_CLI}" \
    && avm use "${ANCHOR_CLI}"

RUN curl -sL https://run.surfpool.run/ | bash

WORKDIR /app

COPY package.json package-lock.json ./
RUN npm ci

COPY . .

RUN solana-keygen new --no-bip39-passphrase --force -o /app/deployer-keypair.json \
    && anchor build -- --features "development"

EXPOSE 3011 8898 8899 8900

CMD ["npm", "run", "fork-lab:api"]
