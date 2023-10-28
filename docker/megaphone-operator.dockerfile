FROM rust:1.73-bookworm as build

WORKDIR /app

COPY Cargo.toml Cargo.lock /app/
COPY operator /app/operator
COPY megaphone /app/megaphone

RUN cargo build -p megaphone-operator --release

FROM debian:bookworm-slim as dist

WORKDIR /app
COPY --from=build /app/target/release/megaphone-operator /app/megaphone-operator

CMD ["/app/megaphone-operator"]
