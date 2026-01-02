import os
from container import start_prometheus, rustic_backup, restic_backup
from exporter import start_rustic_exporter
import logging
import textwrap
import tempfile
import requests

import pytest
import time

os.environ["TESTCONTAINERS_RYUK_DISABLED"] = "true"


def validate_prometheus_scrape(
  rustic_version=None,
  restic_version=None,
  prometheus_version=None,
  rustic_exporter_port: int = 8080,
  prometheus_port: int = 9090,
  prometheus_scrape_interval: float = 1,
  validate_probe_interval: float = 1,
  validate_probe_timeout: float = 1,
  validate_timeout: float = 10,
):
  with (
    tempfile.TemporaryDirectory() as repo_dir,
    tempfile.TemporaryDirectory() as data_dir,
  ):
    repo_name = "test"
    repo_password = "test"
    exporter_config_content = textwrap.dedent(f"""\
    [[backup]]
    name = "{repo_name}"
    repository = "{repo_dir}"
    password = "{repo_password}"
    [backup.options]
    """)
    prometheus_config_content = textwrap.dedent(f"""\
    global:
      scrape_interval: {prometheus_scrape_interval}s
    scrape_configs:
      - job_name: job-1
        static_configs:
          - targets: ['host.docker.internal:{rustic_exporter_port}']
    """)

    logging.info("Prepare repository")
    if rustic_version is not None:
      rustic_backup(tag=rustic_version, repo_dir=repo_dir, data_dir=data_dir, password=repo_password)
    else:
      restic_backup(tag=restic_version, repo_dir=repo_dir, data_dir=data_dir, password=repo_password)

    exporter = start_rustic_exporter(config_content=exporter_config_content, port=rustic_exporter_port)
    prom = start_prometheus(tag=prometheus_version, host_port=prometheus_port, config_content=prometheus_config_content)
    with exporter:
      logging.info("Start rustic-exporter")
      with prom:
        logging.info("Start prometheus")
        metrics = [
          "rustic_repository_info",
          "rustic_snapshot_info",
          "rustic_snapshot_files_total",
          "rustic_snapshot_timestamp",
          "rustic_snapshot_size_bytes",
          "rustic_snapshot_backup_start_timestamp",
          "rustic_snapshot_backup_end_timestamp",
          "rustic_snapshot_backup_duration_seconds",
        ]

        deadline = time.time() + validate_timeout
        while time.time() < deadline:
          all_ready = True
          for m in metrics:
            res = requests.get(
              f"http://localhost:{prometheus_port}/api/v1/query?query={m}", timeout=validate_probe_timeout
            )
            if not res.ok or not len(res.json()["data"]["result"]) > 0:
              all_ready = False
              break
          if all_ready:
            return
          time.sleep(validate_probe_interval)
        pytest.fail("Metrics is not ready in time")


restic_versions = ["0.17.0", "0.18.0", "0.18.1"]
rustic_versions = ["v0.9.5", "v0.10.0", "v0.10.1"]
prometheus_versions = [f"v3.{minor}.0" for minor in range(2, 9)]


@pytest.mark.parametrize("restic_version", restic_versions, ids=lambda v: f"restic={v}")
@pytest.mark.parametrize("prometheus_version", prometheus_versions, ids=lambda v: f"prometheus={v}")
def test_restic_prometheus_scrape(restic_version, prometheus_version, request):
  i = request.node.callspec.indices["restic_version"]
  j = request.node.callspec.indices["prometheus_version"]
  rustic_exporter_port = 1100 + i
  prometheus_port = 1200 + j

  validate_prometheus_scrape(
    restic_version=restic_version,
    rustic_exporter_port=rustic_exporter_port,
    prometheus_port=prometheus_port,
    prometheus_version=prometheus_version,
    validate_timeout=10,
  )


@pytest.mark.parametrize("rustic_version", rustic_versions, ids=lambda v: f"rustic={v}")
@pytest.mark.parametrize("prometheus_version", prometheus_versions, ids=lambda v: f"prometheus={v}")
def test_rustic_prometheus_scrape(rustic_version, prometheus_version, request):
  i = request.node.callspec.indices["rustic_version"]
  j = request.node.callspec.indices["prometheus_version"]
  rustic_exporter_port = 1300 + i
  prometheus_port = 1400 + j

  validate_prometheus_scrape(
    rustic_version=rustic_version,
    rustic_exporter_port=rustic_exporter_port,
    prometheus_port=prometheus_port,
    prometheus_version=prometheus_version,
    validate_timeout=10,
  )
