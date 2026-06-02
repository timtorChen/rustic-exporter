from prometheus_client import Metric
from prometheus_client.parser import text_string_to_metric_families
import requests
import time
import socket


def wait_for_condition(fn, timeout=5, interval=0.1):
  start = time.time()
  while (time.time() - start) < timeout:
    try:
      last = fn()
      if last is not None:
        return last
    except Exception:
      pass
    time.sleep(interval)
  raise TimeoutError(f"Condition not met within timeout limit {timeout}s")


def get_metrics_from_text(metrics_text) -> list[Metric]:
  return list(text_string_to_metric_families(metrics_text))


def get_metrics_value(metrics: list[Metric], metric_name: str, filter_labels: dict[str, str]) -> float:
  for metric in metrics:
    if metric.name == metric_name:
      for sample in metric.samples:
        if all(sample.labels.get(k) == v for k, v in filter_labels.items()):
          return sample.value
  raise KeyError(f"Metrics not found: {metric_name}, labels: {filter_labels}")


def wait_metrics_value(url: str, metric_name: str, filter_labels: dict[str, str]) -> float:
  def check():
    res = requests.get(url)
    res.raise_for_status()
    metrics = get_metrics_from_text(res.text)
    return get_metrics_value(metrics, metric_name, filter_labels)

  return wait_for_condition(check)


def get_free_port() -> int:
  with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as s:
    s.bind(("", 0))
    return s.getsockname()[1]
