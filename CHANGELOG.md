# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.2.3](https://github.com/nightwatch-astro/esp-idf-improv-wifi/compare/v0.2.2...v0.2.3) - 2026-04-04

### Miscellaneous

- pin GitHub Actions to commit SHAs
- pin GitHub Actions to commit SHAs
- pin GitHub Actions to commit SHAs
- add CODEOWNERS for CI security
- add pre-commit config with Rust hooks ([#13](https://github.com/nightwatch-astro/esp-idf-improv-wifi/pull/13))

### Performance

- *(ci)* replace rust-cache with sccache ([#16](https://github.com/nightwatch-astro/esp-idf-improv-wifi/pull/16))

## [0.2.2](https://github.com/nightwatch-astro/esp-idf-improv-wifi/compare/v0.2.1...v0.2.2) - 2026-03-30

### Bug Fixes

- *(ci)* remove semver-check job for ESP-IDF embedded crate
- *(ci)* restore dependabot config with grouping

### Miscellaneous

- *(deps)* bump dorny/paths-filter from 3 to 4 ([#11](https://github.com/nightwatch-astro/esp-idf-improv-wifi/pull/11))

### Ci

- add minor+patch grouping to dependabot

## [0.2.1](https://github.com/nightwatch-astro/esp-idf-improv-wifi/compare/v0.2.0...v0.2.1) - 2026-03-29

### Refactoring

- use thiserror derive for ParseError ([#7](https://github.com/nightwatch-astro/esp-idf-improv-wifi/pull/7))

## [0.2.0](https://github.com/nightwatch-astro/esp-idf-improv-wifi/compare/v0.1.1...v0.2.0) - 2026-03-29

### Features

- *(ci)* add release environment to publish job
- *(ci)* trusted publishing + publish_no_verify

### Miscellaneous

- add .gitattributes for linguist-generated patterns
- align CI with org standards, add GitHub templates, update README
- align CI — reusable release, dependabot labels ([#4](https://github.com/nightwatch-astro/esp-idf-improv-wifi/pull/4))

### Refactoring

- *(ci)* use shared reusable release workflow

### Ci

- skip semver-check when no Rust code changes
- auto-merge minor dependency updates

## [0.1.1](https://github.com/nightwatch-astro/esp-idf-improv-wifi/compare/v0.1.0...v0.1.1) - 2026-03-28

### Miscellaneous

- switch to Apache-2.0 license, update README ([#2](https://github.com/nightwatch-astro/esp-idf-improv-wifi/pull/2))
