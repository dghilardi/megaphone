# Changelog
All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]
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