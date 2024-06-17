FROM rust:1.79-bookworm as build

WORKDIR /app

ARG PROTOC_VERSION=v25.3
RUN arch="$(uname -m)" ; \
    version="$PROTOC_VERSION" ; \
    curl --proto '=https' -vsSfLo protoc.zip  "https://github.com/google/protobuf/releases/download/$version/protoc-${version#v}-linux-$arch.zip" && \
    unzip protoc.zip -d /opt/protobuf && \
    chmod 755 /opt/protobuf/bin/protoc

ENV PROTOC=/opt/protobuf/bin/protoc

COPY Cargo.toml build.rs Cargo.lock /app/
COPY proto /app/proto
COPY src /app/src

RUN cargo build --all-features --release

FROM debian:bookworm-slim as dist

RUN apt-get update && apt-get install -y --no-install-recommends \
    libssl3 \
    && \
    apt-get clean && \
    rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY --from=build /app/target/release/megaphone-broker /app/megaphone
COPY --from=build /app/target/release/megactl /app/megactl

CMD ["/app/megaphone"]
