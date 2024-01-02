FROM rust:1.75-bookworm as build

WORKDIR /app

ARG PROTOC_VERSION=v23.2
RUN arch="$(uname -m)" ; \
    version="$PROTOC_VERSION" ; \
    curl --proto '=https' -vsSfLo protoc.zip  "https://github.com/google/protobuf/releases/download/$version/protoc-${version#v}-linux-$arch.zip" && \
    unzip protoc.zip -d /opt/protobuf && \
    chmod 755 /opt/protobuf/bin/protoc

ENV PROTOC=/opt/protobuf/bin/protoc

COPY megaphone/Cargo.toml megaphone/build.rs Cargo.lock /app/
COPY megaphone/proto /app/proto
COPY megaphone/src /app/src
COPY megaphone/examples /app/examples

RUN cargo build --all-features --release

FROM debian:bookworm-slim as dist

WORKDIR /app
COPY --from=build /app/target/release/server /app/megaphone
COPY --from=build /app/target/release/megactl /app/megactl

CMD ["/app/megaphone"]
