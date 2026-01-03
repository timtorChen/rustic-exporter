from contextlib import contextmanager
import subprocess
import tempfile


@contextmanager
def start_rustic_exporter(config_content: str, port: int = 8080):
  with tempfile.NamedTemporaryFile() as config_file:
    config_file.write(config_content.encode())
    config_file.flush()
    process = subprocess.Popen(["./target/release/rustic-exporter", "--config", config_file.name, "--port", str(port)])
    try:
      yield process
    finally:
      process.terminate()
      process.wait()
