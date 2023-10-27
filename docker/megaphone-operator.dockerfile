FROM rust:1.73-bookworm as build

WORKDIR /app

COPY operator/Cargo.toml Cargo.lock /app/
COPY operator/src /app/src

RUN cargo build --release

FROM debian:bookworm-slim as dist

WORKDIR /app
COPY --from=build /app/target/release/megaphone-operator /app/megaphone-operator

CMD ["/app/megaphone-operator"]
