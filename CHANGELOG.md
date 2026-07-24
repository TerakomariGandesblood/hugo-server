# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.7.1](https://github.com/TerakomariGandesblood/my-servers/compare/v0.7.0...v0.7.1) - 2026-07-22

### <!-- 2 -->🚜 Refactor

- make non_blocking non-lossy
- add non_blocking stderr_writer

## [0.7.0](https://github.com/TerakomariGandesblood/my-servers/compare/v0.6.2...v0.7.0) - 2026-07-22

### <!-- 0 -->🚀 Features

- add allow_upload_ip_nets config
- record connect info
- add opentelemetry

### <!-- 2 -->🚜 Refactor

- change otel.name

### <!-- 7 -->⚙️ Miscellaneous Tasks

- remove cd

## [0.6.2](https://github.com/TerakomariGandesblood/my-servers/compare/v0.6.1...v0.6.2) - 2026-06-18

### <!-- 1 -->🐛 Bug Fixes

- update CD workflow to use only file_server

### <!-- 7 -->⚙️ Miscellaneous Tasks

- disable github release

## [0.6.1](https://github.com/TerakomariGandesblood/hugo-server/compare/v0.6.0...v0.6.1) - 2026-06-18

### <!-- 0 -->🚀 Features

- add CatchPanicLayer

## [0.5.2](https://github.com/TerakomariGandesblood/hugo-server/compare/v0.5.1...v0.5.2) - 2026-06-01

### <!-- 2 -->🚜 Refactor

- remove http cache

## [0.5.1](https://github.com/TerakomariGandesblood/hugo-server/compare/v0.5.0...v0.5.1) - 2026-04-20

### <!-- 0 -->🚀 Features

- add timeout to command execution
- add http cache

### <!-- 1 -->🐛 Bug Fixes

- conditionally upload Algolia records during website update

### <!-- 2 -->🚜 Refactor

- remove metrics tracking

## [0.5.0](https://github.com/TerakomariGandesblood/hugo-server/compare/v0.4.0...v0.5.0) - 2026-04-15

### <!-- 2 -->🚜 Refactor

- big change

## [0.4.0](https://github.com/TerakomariGandesblood/hugo-server/compare/v0.3.0...v0.4.0) - 2026-03-24

### <!-- 0 -->🚀 Features

- use utc time in log

### <!-- 2 -->🚜 Refactor

- remove build and system info

## [0.3.0](https://github.com/TerakomariGandesblood/hugo-server/compare/v0.2.1...v0.3.0) - 2026-03-24

### <!-- 0 -->🚀 Features

- support https and refactor

### <!-- 1 -->🐛 Bug Fixes

- update config file name to .config.toml
- reorder config load and log init

## [0.2.1](https://github.com/TerakomariGandesblood/hugo-server/compare/v0.2.0...v0.2.1) - 2026-03-05

### <!-- 1 -->🐛 Bug Fixes

- wrong thread::spawn

## [0.2.0](https://github.com/TerakomariGandesblood/hugo-server/compare/v0.1.0...v0.2.0) - 2026-03-05

### <!-- 0 -->🚀 Features

- support upload algolia records
