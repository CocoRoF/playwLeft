"""Smoke tests — verify the package installs and imports correctly.

These tests don't require a browser and should pass in any CI environment.
"""

import playwleft


def test_version_exists():
    """__version__ should be a non-empty string."""
    assert isinstance(playwleft.__version__, str)
    assert len(playwleft.__version__) > 0


def test_version_format():
    """__version__ should look like semver (x.y.z)."""
    parts = playwleft.__version__.split(".")
    assert len(parts) == 3
    for part in parts:
        assert part.isdigit()


def test_all_public_classes_importable():
    """Every name in __all__ should be importable."""
    for name in playwleft.__all__:
        assert hasattr(playwleft, name), f"{name} listed in __all__ but not found"


def test_playwleft_instantiation():
    """PlaywLeft() should create an instance without errors."""
    pw = playwleft.PlaywLeft()
    assert pw is not None


def test_chromium_browser_type():
    """PlaywLeft().chromium() should return a BrowserType."""
    pw = playwleft.PlaywLeft()
    bt = pw.chromium()
    assert isinstance(bt, playwleft.BrowserType)
