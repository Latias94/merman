ARG RUST_IMAGE=rust:1.95-bullseye
ARG NODE_IMAGE=node:24.13.1-bullseye

FROM ${RUST_IMAGE} AS rust-toolchain

FROM ${NODE_IMAGE}

# The full Node image inherits these tools from buildpack-deps. Verify them
# without refreshing the retired Bullseye package repositories.
RUN set -eu; \
    for tool in cc c++ make pkg-config python3 git; do \
        command -v "$tool"; \
    done; \
    test -s /etc/ssl/certs/ca-certificates.crt

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
