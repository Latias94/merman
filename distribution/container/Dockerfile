FROM ubuntu:24.04@sha256:534baea6a22c03a63003dbc8dbe78fe34bc0d7e595d9a9dc9834884ff530eb55

RUN apt-get update \
    && apt-get install --no-install-recommends --yes \
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
