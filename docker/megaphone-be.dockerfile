FROM rust:1.73-bookworm as build

WORKDIR /app

COPY megaphone/Cargo.toml Cargo.lock /app/
COPY megaphone/src /app/src

RUN cargo build --release

FROM debian:bookworm-slim as dist

WORKDIR /app
COPY --from=build /app/target/release/megaphone /app/megaphone
COPY --from=build /app/target/release/megactl /app/megactl

CMD ["/app/megaphone"]
