"""Smoke tests — verify the package installs and imports correctly.

These tests don't require a browser and should pass in any CI environment.
"""

import playleft


def test_version_exists():
    """__version__ should be a non-empty string."""
    assert isinstance(playleft.__version__, str)
    assert len(playleft.__version__) > 0


def test_version_format():
    """__version__ should look like semver (x.y.z)."""
    parts = playleft.__version__.split(".")
    assert len(parts) == 3
    for part in parts:
        assert part.isdigit()


def test_all_public_classes_importable():
    """Every name in __all__ should be importable."""
    for name in playleft.__all__:
        assert hasattr(playleft, name), f"{name} listed in __all__ but not found"


def test_playleft_instantiation():
    """PlaywLeft() should create an instance without errors."""
    pw = playleft.PlaywLeft()
    assert pw is not None


def test_chromium_browser_type():
    """PlaywLeft().chromium() should return a BrowserType."""
    pw = playleft.PlaywLeft()
    bt = pw.chromium()
    assert isinstance(bt, playleft.BrowserType)
