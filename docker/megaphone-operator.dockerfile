FROM rust:1.75-bookworm as build

WORKDIR /app

ARG PROTOC_VERSION=v23.2
RUN arch="$(uname -m)" ; \
    version="$PROTOC_VERSION" ; \
    curl --proto '=https' -vsSfLo protoc.zip  "https://github.com/google/protobuf/releases/download/$version/protoc-${version#v}-linux-$arch.zip" && \
    unzip protoc.zip -d /opt/protobuf && \
    chmod 755 /opt/protobuf/bin/protoc

ENV PROTOC=/opt/protobuf/bin/protoc

COPY Cargo.toml Cargo.lock /app/
COPY operator /app/operator
COPY megaphone /app/megaphone

RUN cargo build -p megaphone-operator --release

FROM debian:bookworm-slim as dist

WORKDIR /app
COPY --from=build /app/target/release/megaphone-operator /app/megaphone-operator

CMD ["/app/megaphone-operator"]
