from exporter import start_rustic_exporter

import pytest

import helpers
import textwrap

import logging
import requests
import time


@pytest.fixture(scope="module")
def metrics():
  repo_name = "test"
  repo_dir = "./tests/data/golden-repo"
  repo_password = "test"
  rustic_exporter_url = "http://localhost:8080/metrics"

  exporter_config_content = textwrap.dedent(f"""\
    [[backup]]
    name = "{repo_name}"
    repository = "{repo_dir}"
    password = "{repo_password}"
    [backup.options]
    """)
  exporter = start_rustic_exporter(config_content=exporter_config_content)
  with exporter:
    logging.info("Start rustic-exporter")
    helpers.wait_for_http_ready(url=rustic_exporter_url)

    # TODO wait for metrics all ready
    time.sleep(5)
    res = requests.get(rustic_exporter_url)
    metrics = helpers.get_metrics_from_text(res.text)
    return metrics


@pytest.mark.parametrize(
  ("metric_name", "repo_id", "value"),
  [
    ("rustic_repository_info", "3b0165bb", 1),
  ],
)
def test_repository_metrics(metrics, metric_name, repo_id, value):
  assert helpers.get_metrics_value(metrics, metric_name, filter_labels={"repo_id": repo_id}) == value


@pytest.mark.parametrize(
  ("metric_name", "snapshot_id", "value"),
  [
    ("rustic_snapshot_files_total", "9e3db981", 1),
    ("rustic_snapshot_size_bytes", "9e3db981", 1024),
    ("rustic_snapshot_timestamp", "9e3db981", 1780212689.337172),
    ("rustic_snapshot_backup_start_timestamp", "9e3db981", 1780212689.345597),
    ("rustic_snapshot_backup_end_timestamp", "9e3db981", 1780212689.397221),
    ("rustic_snapshot_backup_duration_seconds", "9e3db981", 0.051624),
    ("rustic_snapshot_files_total", "f797d9b5", 2),
    ("rustic_snapshot_size_bytes", "f797d9b5", 3072),
    ("rustic_snapshot_timestamp", "f797d9b5", 1780214434.184508),
    ("rustic_snapshot_backup_start_timestamp", "f797d9b5", 1780214434.190304),
    ("rustic_snapshot_backup_end_timestamp", "f797d9b5", 1780214434.231709),
    ("rustic_snapshot_backup_duration_seconds", "f797d9b5", 0.041405),
  ],
)
def test_snapshot_metrics(metrics, metric_name, snapshot_id, value):
  assert helpers.get_metrics_value(metrics, metric_name, filter_labels={"snapshot_id": snapshot_id}) == value
