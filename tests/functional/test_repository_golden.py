from exporter import start_rustic_exporter
import pytest
import helpers
import textwrap
import logging

REPO_NAME = "test"
REPO_DIR = "./tests/data/golden-repo"
REPO_PASSWORD = "test"
port = helpers.get_free_port()
url = f"http://localhost:{port}/metrics"


@pytest.fixture(scope="module")
def server():
  config_content = textwrap.dedent(f"""\
    [[backup]]
    name = "{REPO_NAME}"
    repository = "{REPO_DIR}"
    password = "{REPO_PASSWORD}"
    [backup.options]
    """)

  server = start_rustic_exporter(config_content, port)
  with server:
    logging.info("Start rustic-exporter")
    yield server


@pytest.mark.parametrize(
  ("metric_name", "repo_id", "value"),
  [
    ("rustic_repository_info", "3b0165bb", 1),
  ],
)
def test_repository_metrics(server, metric_name, repo_id, value):
  helpers.wait_metrics_value(url, metric_name, filter_labels={"repo_id": repo_id}) == value


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
def test_snapshot_metrics(server, metric_name, snapshot_id, value):
  assert helpers.wait_metrics_value(url, metric_name, filter_labels={"snapshot_id": snapshot_id}) == value
