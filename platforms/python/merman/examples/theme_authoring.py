"""Run the installed-wheel Theme v2 probe without the other wheel checks."""

import merman
from wheel_smoke import assert_theme_v2


def run_theme_authoring_smoke() -> None:
    assert_theme_v2(merman.Merman())
    print("Python Theme v2 authoring smoke passed")


if __name__ == "__main__":
    run_theme_authoring_smoke()
