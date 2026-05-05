FROM ubuntu:24.04

ARG DEBIAN_FRONTEND=noninteractive
ARG TARGETARCH
ARG NODE_VERSION=20.19.0
ARG RUST_TOOLCHAIN=1.86.0
ARG SOLANA_CLI=2.3.11
ARG ANCHOR_CLI=0.31.1

ENV HOME=/root
ENV PATH="/opt/node/bin:/root/.cargo/bin:/root/.local/bin:/root/.avm/bin:/root/.local/share/solana/install/active_release/bin:${PATH}"
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

RUN case "${TARGETARCH:-amd64}" in \
        amd64) node_arch="x64" ;; \
        arm64) node_arch="arm64" ;; \
        *) echo "Unsupported Docker target architecture: ${TARGETARCH}" >&2; exit 1 ;; \
    esac \
    && mkdir -p /opt/node \
    && curl -fsSL "https://nodejs.org/dist/v${NODE_VERSION}/node-v${NODE_VERSION}-linux-${node_arch}.tar.xz" \
        | tar -xJ -C /opt/node --strip-components=1 \
    && npm install -g yarn@1.22.22 \
    && node --version \
    && npm --version

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
