# Rustic exporter

[coverage]() | [latest-version](gh-release-page) | [ghrc-container](gh-package-page)

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
