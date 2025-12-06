import tempfile
import pytest


def pytest_addoption(parser):
  parser.addoption("--tempdir", action="store", help="set the base path of tempfiles")


@pytest.fixture(scope="session", autouse=True)
def set_tempdir(request):
  tempdir = request.config.getoption("--tempdir")
  if tempdir is not None:
    tempfile.tempdir = tempdir
