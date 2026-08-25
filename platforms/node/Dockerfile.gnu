ARG RUST_IMAGE=rust:1.95-bullseye
ARG NODE_IMAGE=node:24-bullseye

FROM ${RUST_IMAGE} AS rust-toolchain

FROM ${NODE_IMAGE}

RUN apt-get update \
    && apt-get install --yes --no-install-recommends build-essential ca-certificates git pkg-config python3 \
    && rm -rf /var/lib/apt/lists/*

COPY --from=rust-toolchain /usr/local/cargo /usr/local/cargo
COPY --from=rust-toolchain /usr/local/rustup /usr/local/rustup

ENV CARGO_HOME=/usr/local/cargo
ENV RUSTUP_HOME=/usr/local/rustup
ENV PATH=/usr/local/cargo/bin:${PATH}

RUN node --version \
    && npm --version \
    && rustc --version \
    && cargo --version \
    && node -p "process.report.getReport().header.glibcVersionRuntime"

WORKDIR /work
