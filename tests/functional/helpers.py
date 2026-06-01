import time
import requests

from prometheus_client import Metric
from prometheus_client.parser import text_string_to_metric_families


def wait_for_http_ready(url: str, timeout: int = 5, interval: float = 0.1):
  start = time.time()
  while (time.time() - start) < timeout:
    try:
      r = requests.get(url)
      if 200 <= r.status_code < 300:
        return
    except requests.exceptions.ConnectionError:
      pass

    time.sleep(interval)

  raise TimeoutError(f"Server not ready within timeout limit {timeout}s")


def get_metrics_from_text(metrics_text) -> list[Metric]:
  return list(text_string_to_metric_families(metrics_text))


def get_metrics_value(metrics: list[Metric], metric_name: str, filter_labels: dict[str, str]) -> float | None:
  for metric in metrics:
    if metric.name == metric_name:
      for sample in metric.samples:
        if all(sample.labels.get(k) == v for k, v in filter_labels.items()):
          return sample.value
  return None
