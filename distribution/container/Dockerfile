FROM ubuntu:26.10@sha256:262df65c0ec6bf8d9dcc87454f4330a05fae4b97825f45854af3b136666584c2

RUN apt-get -o Acquire::Retries=3 -o Acquire::http::Timeout=30 -o Acquire::https::Timeout=30 update --error-on=any \
    && apt-get -o Acquire::Retries=3 -o Acquire::http::Timeout=30 -o Acquire::https::Timeout=30 install --no-install-recommends --yes \
        ca-certificates fontconfig fonts-dejavu-core libgcc-s1 \
    && rm -rf /var/lib/apt/lists/*

# The build context is the extracted, verified CLI release archive.
COPY merman-cli /usr/local/bin/merman-cli
COPY LICENSE-APACHE LICENSE-MIT README.md CHANGELOG.md THIRD_PARTY_NOTICES.md /usr/local/share/doc/merman-cli/
COPY THIRD_PARTY_LICENSES/ /usr/local/share/doc/merman-cli/THIRD_PARTY_LICENSES/
COPY completions/ /usr/local/share/merman-cli/completions/
COPY man/ /usr/local/share/man/man1/

WORKDIR /data
USER 65532:65532
ENTRYPOINT ["/usr/local/bin/merman-cli"]
CMD ["--help"]
