FROM rust:1.73-bookworm as build

WORKDIR /app

COPY megaphone/Cargo.toml Cargo.lock /app/
RUN mkdir src && touch src/lib.rs \
    && cargo build --release \
    && rm -r src

COPY megaphone/src /app/src

RUN cargo build --release

FROM debian:bookworm-slim as dist

WORKDIR /app
COPY --from=build /app/target/release/megaphone /app/megaphone

CMD ["/app/megaphone"]
