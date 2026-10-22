# Changelog
All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]
### Changed
- Updated `axum` to 0.8 and `hyper` to 1 (management socket served by axum, `hyperlocal` only used by `megactl`); `megactl` errors now include their cause
- Updated `tonic`/`prost` to 0.14 (code generation via `tonic-prost-build`)
- Updated `thiserror` 2, `dashmap` 6, `config` 0.15, `base64` 0.23, `md5` 0.8, `rand` 0.10, `metrics` 0.24, `metrics-exporter-prometheus` 0.18 (without unused HTTP listener and push gateway)
- Rust toolchain bumped to 1.99.1
- Rust toolchain pinned to 1.92.0 via `rust-toolchain.toml` (Docker image aligned and checked by CI); CI updated to maintained actions, release artifacts published only after tests pass, Dependabot also updates GitHub Actions

## [0.10.7] 2026-10-21
### Fixed
- Build failure with recent Rust toolchains by updating `metrics` to 0.22.4


## [0.10.6] 2026-10-21
### Changed
- Docker image is built with Rust 1.92

### Fixed
- Panic on malformed producer addresses shorter than nonce and tag, now rejected with `400 Bad Request`
- Writes performed with a producer address were not delivered to replicas while the agent was piped

### Added
- `allow_consumer_write` config key (default `true`) to allow or forbid writing into a channel using its consumer address
- `megaphone_consumer_address_writes` metric counting allowed write attempts on existing channels performed with a consumer address

### Deprecated
- Writing into a channel using its consumer address, it will be disabled by default in a future release

## [0.10.5] 2024-04-27

## [0.10.4] 2024-03-22
### Added
- Basic support for protocol negotoation

## [0.10.3] 2024-03-16
### Fixed
- Fix metrics

## [0.10.2] 2024-03-03
### Added
- Add `on-channel-deleted` webhook

## [0.10.1] 2024-03-02
### Added
- Agents warmup period is now configurable with the `agent_warmup_secs` config key

## [0.10.0] 2024-02-26
### Changed
- Introduce `producer_address` to separate consumer and producer roles

### Deprecated
- `channel_id` usages in favor of `producer_address` and `consumer_address`

## [0.9.6] 2024-02-25
### Fixed
- Docker image creation

## [0.9.5] 2024-02-25
### Fixed
- Agent sessions counters

### Changed
- Update dependencies