# Rustic exporter

[![codecov](https://codecov.io/gh/timtorChen/rustic-exporter/graph/badge.svg?token=34YBCFDN6B)](https://codecov.io/gh/timtorChen/rustic-exporter)
[![github-release](https://img.shields.io/github/v/release/timtorChen/rustic-exporter)](https://github.com/timtorChen/rustic-exporter/releases/latest)
[![github-ghcr](https://img.shields.io/badge/_ghcr.io-download-blue)](https://github.com/timtorChen/rustic-exporter/pkgs/container/rustic-exporter)

⚠️ This project is still under development; use it with caution.

Prometheus exporter for rustic/restic backup.

### Requirements

The backup client should use `restic >= v0.17`, or metrics like `rustic_snapshot_size_bytes` will be dropped.

### Configuration

The example configuration file is in [example/config.toml](example/config.toml).

```
Usage: rustic-exporter [OPTIONS] --config <CONFIG>

Options:
      --interval <INTERVAL>  Frequency of metrics collection in seconds [default: 300]
      --config <CONFIG>      Path to the configuration file
      --host <HOST>          Server host [default: 0.0.0.0]
      --port <PORT>          Server port [default: 8080]
  -h, --help                 Print help
  -V, --version              Print version
```
