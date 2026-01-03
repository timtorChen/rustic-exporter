from contextlib import contextmanager
import tempfile
from testcontainers.core.container import DockerContainer
from testcontainers.core.wait_strategies import HttpWaitStrategy
import os


@contextmanager
def start_prometheus(tag: str, config_content: str, host_port: int = 9090, timeout: int = 10):
  with tempfile.NamedTemporaryFile() as config_file:
    config_file.write(config_content.encode())
    config_file.flush()

    # tempfile default permission to 600
    # let docker daemon to read the config file
    os.chmod(config_file.name, 0o644)
    container = (
      DockerContainer(f"docker.io/prom/prometheus:{tag}")
      .with_volume_mapping(config_file.name, "/etc/prometheus/prometheus.yml")
      .with_bind_ports(container=9090, host=host_port)
      .waiting_for(HttpWaitStrategy(9090, "/-/healthy").for_status_code(200).with_startup_timeout(timeout))
      .with_kwargs(user=f"{os.getuid()}:{os.getgid()}")
      .with_kwargs(group_add=["65534"])
      .with_kwargs(extra_hosts={"host.docker.internal": "host-gateway"})
    )
    with container:
      yield container


def rustic_backup(tag: str, repo_dir: str, data_dir: str, password: str = "dummy", timeout: int = 10):
  container = (
    DockerContainer(f"ghcr.io/rustic-rs/rustic:{tag}")
    .with_volume_mapping(data_dir, "/data", "ro")
    .with_volume_mapping(repo_dir, "/repo", "rw")
    .with_command(["backup", "--init", "/data", "--repository", "/repo", "--password", password])
    .with_kwargs(user=f"{os.getuid()}:{os.getgid()}")
  )
  with container:
    container.get_wrapped_container().wait(timeout=timeout)


def restic_backup(tag: str, repo_dir: str, data_dir: str, password: str = "dummy", timeout: int = 10):
  init_container = (
    DockerContainer(f"docker.io/restic/restic:{tag}")
    .with_volume_mapping(repo_dir, "/repo", "rw")
    .with_env("RESTIC_PASSWORD", password)
    .with_command(["init", "--repo", "/repo"])
    .with_kwargs(user=f"{os.getuid()}:{os.getgid()}")
  )

  backup_container = (
    DockerContainer(f"docker.io/restic/restic:{tag}")
    .with_volume_mapping(data_dir, "/data", "ro")
    .with_volume_mapping(repo_dir, "/repo", "rw")
    .with_env("RESTIC_PASSWORD", password)
    .with_command(["backup", "--repo", "/repo", "/data"])
    .with_kwargs(user=f"{os.getuid()}:{os.getgid()}")
  )

  with init_container:
    init_container.get_wrapped_container().wait(timeout=timeout)
    with backup_container:
      backup_container.get_wrapped_container().wait(timeout=timeout)
