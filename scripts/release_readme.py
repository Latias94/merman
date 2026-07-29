"""Generated README installation commands for source and registry release states."""

from __future__ import annotations

import re
from dataclasses import dataclass

try:
    from scripts.release_version import ReleaseVersion
except ModuleNotFoundError:
    from release_version import ReleaseVersion


SOURCE_MODE = "source"
REGISTRY_MODE = "registry"
MODES = {SOURCE_MODE, REGISTRY_MODE}
ROOT_README = "README.md"
MODE_PATTERN = re.compile(
    r"^<!-- merman-release-install-mode: ([a-z-]+) -->$",
    flags=re.MULTILINE,
)
SAFE_REPOSITORY_URL = re.compile(
    r"https?://[A-Za-z0-9.-]+(?::[0-9]{1,5})?"
    r"(?:/[A-Za-z0-9._~/-]+)+"
)


class ReleaseReadmeError(ValueError):
    """A generated README installation projection is malformed or stale."""


@dataclass(frozen=True)
class CargoDependency:
    package: str
    default_features: bool | None = None
    features: tuple[str, ...] = ()
    optional: bool = False


@dataclass(frozen=True)
class ReadmeBlock:
    kind: str
    package: str = ""
    directory: str = ""
    dependencies: tuple[CargoDependency, ...] = ()
    features: tuple[str, ...] = ()
    followup: tuple[str, ...] = ()
    suffix: str = ""


def _dependency(
    package: str,
    *,
    default_features: bool | None = None,
    features: tuple[str, ...] = (),
    optional: bool = False,
) -> CargoDependency:
    return CargoDependency(package, default_features, features, optional)


def _cargo_add_block(package: str) -> ReadmeBlock:
    return ReadmeBlock("cargo-add", package=package)


def _cargo_install_block(
    package: str,
    *,
    features: tuple[str, ...] = (),
    followup: tuple[str, ...] = (),
) -> ReadmeBlock:
    return ReadmeBlock(
        "cargo-install",
        package=package,
        features=features,
        followup=followup,
    )


def _cargo_dependencies_block(
    *dependencies: CargoDependency,
    suffix: str = "",
) -> ReadmeBlock:
    return ReadmeBlock(
        "cargo-dependencies",
        dependencies=dependencies,
        suffix=suffix,
    )


def _npm_install_block(package: str, directory: str) -> ReadmeBlock:
    return ReadmeBlock("npm-install", package=package, directory=directory)


def _pub_dependency_block(package: str, directory: str) -> ReadmeBlock:
    return ReadmeBlock("pub-dependency", package=package, directory=directory)


README_BLOCKS = {
    ROOT_README: (
        (
            "CLI",
            _cargo_install_block(
                "merman-cli",
                followup=(
                    "printf 'flowchart LR\\n  Source --> Merman --> SVG\\n' | \\",
                    "  merman-cli render - --output diagram.svg",
                ),
            ),
        ),
        ("RUST", _cargo_add_block("merman")),
        (
            "BASIC_SVG",
            _cargo_dependencies_block(
                _dependency(
                    "merman",
                    default_features=False,
                    features=("svg",),
                ),
            ),
        ),
        (
            "LEAN_CLI",
            _cargo_install_block(
                "merman-cli",
                features=("analysis",),
            ),
        ),
    ),
    "crates/merman-analysis/README.md": (
        ("ANALYSIS_INSTALL", _cargo_add_block("merman-analysis")),
    ),
    "crates/merman-ascii/README.md": (
        (
            "ASCII_DEPENDENCY",
            _cargo_dependencies_block(
                _dependency(
                    "merman",
                    default_features=False,
                    features=("ascii",),
                ),
            ),
        ),
    ),
    "crates/merman-cli/README.md": (
        (
            "CLI_PACKAGE_INSTALL",
            _cargo_install_block("merman-cli"),
        ),
        (
            "CLI_PACKAGE_LEAN_INSTALL",
            _cargo_install_block(
                "merman-cli",
                features=("analysis",),
            ),
        ),
    ),
    "crates/merman-core/README.md": (
        ("CORE_INSTALL", _cargo_add_block("merman-core")),
    ),
    "crates/merman-editor-core/README.md": (
        (
            "EDITOR_CORE_DEPENDENCY",
            _cargo_dependencies_block(
                _dependency(
                    "merman",
                    default_features=False,
                    features=("analysis", "editor"),
                ),
            ),
        ),
    ),
    "crates/merman-export/README.md": (
        (
            "EXPORT_FACADE_DEPENDENCY",
            _cargo_dependencies_block(
                _dependency(
                    "merman",
                    default_features=False,
                    features=("png",),
                ),
            ),
        ),
        (
            "EXPORT_DIRECT_DEPENDENCIES",
            _cargo_dependencies_block(
                _dependency(
                    "merman",
                    default_features=False,
                    features=("svg",),
                ),
                _dependency(
                    "merman-export",
                    default_features=False,
                    features=("png",),
                ),
            ),
        ),
    ),
    "crates/merman-lsp/README.md": (
        (
            "LSP_INSTALL",
            _cargo_install_block(
                "merman-lsp",
                features=("stdio",),
            ),
        ),
        (
            "LSP_LIBRARY_DEPENDENCY",
            _cargo_dependencies_block(
                _dependency("merman-lsp", default_features=False),
            ),
        ),
    ),
    "crates/merman-rustdoc/README.md": (
        (
            "RUSTDOC_DEPENDENCY",
            _cargo_dependencies_block(
                _dependency("merman-rustdoc", optional=True),
                suffix=(
                    '[features]\ndoc-diagrams = ["dep:merman-rustdoc"]\n\n'
                    '[package.metadata.docs.rs]\nfeatures = ["doc-diagrams"]'
                ),
            ),
        ),
        (
            "RUSTDOC_SLIM_DEPENDENCY",
            _cargo_dependencies_block(
                _dependency(
                    "merman-rustdoc",
                    default_features=False,
                    features=("svg",),
                    optional=True,
                ),
            ),
        ),
    ),
    "docs/rendering/RASTER_OUTPUT.md": (
        (
            "RASTER_FACADE_DEPENDENCY",
            _cargo_dependencies_block(
                _dependency(
                    "merman",
                    default_features=False,
                    features=("png", "pdf"),
                ),
            ),
        ),
        (
            "RASTER_ENCODER_DEPENDENCY",
            _cargo_dependencies_block(
                _dependency(
                    "merman",
                    default_features=False,
                    features=("png", "jpeg", "pdf"),
                ),
            ),
        ),
    ),
    "platforms/flutter/README.md": (
        (
            "FLUTTER_PACKAGE_INSTALL",
            _pub_dependency_block("merman", "platforms/flutter"),
        ),
    ),
    "platforms/web/README.md": (
        (
            "WEB_GUIDE_INSTALL",
            _npm_install_block("@mermanjs/web", "full"),
        ),
    ),
    "platforms/web/packages/analysis/README.md": (
        (
            "NPM_ANALYSIS_INSTALL",
            _npm_install_block("@mermanjs/web-analysis", "analysis"),
        ),
    ),
    "platforms/web/packages/ascii/README.md": (
        (
            "NPM_ASCII_INSTALL",
            _npm_install_block("@mermanjs/web-ascii", "ascii"),
        ),
    ),
    "platforms/web/packages/editor/README.md": (
        (
            "NPM_EDITOR_INSTALL",
            _npm_install_block("@mermanjs/web-editor", "editor"),
        ),
    ),
    "platforms/web/packages/full/README.md": (
        (
            "NPM_FULL_INSTALL",
            _npm_install_block("@mermanjs/web", "full"),
        ),
    ),
    "platforms/web/packages/render/README.md": (
        (
            "NPM_RENDER_INSTALL",
            _npm_install_block("@mermanjs/web-render", "render"),
        ),
    ),
}
PROJECTED_README_PATHS = tuple(
    path for path in README_BLOCKS if path != ROOT_README
)


def render_readme(
    text: str,
    release: ReleaseVersion,
    mode: str,
    repository_url: str,
) -> str:
    _require_mode(mode)
    _read_mode(text)
    rendered = MODE_PATTERN.sub(
        f"<!-- merman-release-install-mode: {mode} -->",
        text,
        count=1,
    )
    return _render_document(
        ROOT_README,
        rendered,
        release,
        mode,
        repository_url,
    )


def verify_readme(
    text: str,
    release: ReleaseVersion,
    *,
    mode: str,
    repository_url: str,
) -> str:
    actual_mode = _read_mode(text)
    if actual_mode != mode:
        raise ReleaseReadmeError(
            f"README installation mode is {actual_mode!r}, expected {mode!r}"
        )
    return _verify_document(
        ROOT_README,
        text,
        release,
        mode,
        repository_url,
    )


def projected_readme_paths() -> tuple[str, ...]:
    """README files whose release-sensitive commands follow the root mode."""

    return PROJECTED_README_PATHS


def render_projected_readme(
    path: str,
    text: str,
    release: ReleaseVersion,
    mode: str,
    repository_url: str,
) -> str:
    return _render_document(path, text, release, mode, repository_url)


def verify_projected_readme(
    path: str,
    text: str,
    release: ReleaseVersion,
    *,
    mode: str,
    repository_url: str,
) -> str:
    return _verify_document(path, text, release, mode, repository_url)


def _render_document(
    path: str,
    text: str,
    release: ReleaseVersion,
    mode: str,
    repository_url: str,
) -> str:
    expected = _expected_blocks(path, release, mode, repository_url)
    block_ids = tuple(expected)
    _validate_block_structure(text, block_ids)
    rendered = text
    for block_id, body in expected.items():
        rendered = _replace_block(rendered, block_id, body)
    return rendered


def _verify_document(
    path: str,
    text: str,
    release: ReleaseVersion,
    mode: str,
    repository_url: str,
) -> str:
    expected = _expected_blocks(path, release, mode, repository_url)
    block_ids = tuple(expected)
    _validate_block_structure(text, block_ids)
    stale = [
        block_id
        for block_id, body in expected.items()
        if _read_block(text, block_id) != body
    ]
    if stale:
        raise ReleaseReadmeError(
            f"{path} generated installation blocks are stale: "
            + ", ".join(stale)
        )
    return mode


def _read_mode(text: str) -> str:
    matches = MODE_PATTERN.findall(text)
    if len(matches) != 1:
        raise ReleaseReadmeError(
            "README must contain exactly one merman release installation mode marker"
        )
    mode = matches[0]
    _require_mode(mode)
    return mode


def _require_mode(mode: str) -> None:
    if mode not in MODES:
        raise ReleaseReadmeError(
            f"unsupported README installation mode {mode!r}; "
            f"expected one of {sorted(MODES)}"
        )


def _require_repository_url(repository_url: str) -> None:
    if SAFE_REPOSITORY_URL.fullmatch(repository_url) is None:
        raise ReleaseReadmeError(
            "README repository URL must be an absolute HTTP(S) URL containing "
            f"only command-safe host and path characters, found {repository_url!r}"
        )


def _markers(block_id: str) -> tuple[str, str]:
    return (
        f"<!-- BEGIN GENERATED RELEASE README {block_id} -->",
        f"<!-- END GENERATED RELEASE README {block_id} -->",
    )


def _read_block(text: str, block_id: str) -> str:
    begin, end = _markers(block_id)
    if text.count(begin) != 1 or text.count(end) != 1:
        raise ReleaseReadmeError(
            f"README must contain exactly one generated {block_id} block"
        )
    begin_at = text.index(begin)
    end_at = text.index(end)
    if end_at < begin_at:
        raise ReleaseReadmeError(f"README generated {block_id} markers are reversed")
    body = text[begin_at + len(begin) : end_at]
    if not body.startswith("\n\n") or not body.endswith("\n\n"):
        raise ReleaseReadmeError(
            f"README generated {block_id} block must use marker-delimited lines"
        )
    return body[2:-2]


def _validate_block_structure(text: str, block_ids: tuple[str, ...]) -> None:
    previous_end = -1
    for block_id in block_ids:
        begin, end = _markers(block_id)
        _read_block(text, block_id)
        begin_at = text.index(begin)
        end_at = text.index(end) + len(end)
        if begin_at < previous_end:
            raise ReleaseReadmeError(
                "README generated installation blocks must appear once, "
                f"without nesting, in this order: {', '.join(block_ids)}"
            )
        previous_end = end_at


def _replace_block(text: str, block_id: str, body: str) -> str:
    begin, end = _markers(block_id)
    _read_block(text, block_id)
    start = text.index(begin) + len(begin)
    finish = text.index(end, start)
    return text[:start] + "\n\n" + body + "\n\n" + text[finish:]


def _fence(language: str, body: str) -> str:
    return f"```{language}\n{body}\n```"


def _cargo_dependency(
    dependency: CargoDependency,
    release: ReleaseVersion,
    mode: str,
    repository_url: str,
) -> str:
    fields = [f'version = "={release.canonical}"']
    if mode == SOURCE_MODE:
        fields.append(f'git = "{repository_url}"')
    if dependency.default_features is not None:
        fields.append(
            "default-features = "
            + ("true" if dependency.default_features else "false")
        )
    if dependency.features:
        features = ", ".join(f'"{feature}"' for feature in dependency.features)
        fields.append(f"features = [{features}]")
    if dependency.optional:
        fields.append("optional = true")
    return f"{dependency.package} = {{ {', '.join(fields)} }}"


def _render_block(
    block: ReadmeBlock,
    release: ReleaseVersion,
    mode: str,
    repository_url: str,
) -> str:
    if block.kind == "cargo-add":
        command = (
            f"cargo add {block.package} --git {repository_url}"
            if mode == SOURCE_MODE
            else f"cargo add {block.package}@={release.canonical}"
        )
        return _fence("sh", command)
    if block.kind == "cargo-install":
        command = (
            f"cargo install --git {repository_url} --locked {block.package}"
            if mode == SOURCE_MODE
            else (
                f"cargo install {block.package} "
                f"--version {release.canonical} --locked"
            )
        )
        if block.features:
            command += (
                " \\\n  --no-default-features --features "
                + ",".join(block.features)
            )
        return _fence("sh", "\n".join((command, *block.followup)))
    if block.kind == "cargo-dependencies":
        body = "[dependencies]\n" + "\n".join(
            _cargo_dependency(dependency, release, mode, repository_url)
            for dependency in block.dependencies
        )
        if block.suffix:
            body += "\n\n" + block.suffix
        return _fence("toml", body)
    if block.kind == "npm-install":
        commands = (
            (
                "npm ci --prefix /path/to/merman/platforms/web",
                "npm run build --prefix /path/to/merman/platforms/web",
                "npm install /path/to/merman/platforms/web/packages/"
                + block.directory,
            )
            if mode == SOURCE_MODE
            else (f"npm install {block.package}@{release.canonical}",)
        )
        return _fence("sh", "\n".join(commands))
    if block.kind == "pub-dependency":
        body = (
            "dependencies:\n"
            f"  {block.package}:\n"
            "    git:\n"
            f"      url: {repository_url}\n"
            f"      path: {block.directory}"
            if mode == SOURCE_MODE
            else "dependencies:\n"
            f"  {block.package}: {release.canonical}"
        )
        return _fence("yaml", body)
    raise AssertionError(f"unsupported README block kind {block.kind!r}")


def _expected_blocks(
    path: str,
    release: ReleaseVersion,
    mode: str,
    repository_url: str,
) -> dict[str, str]:
    _require_mode(mode)
    _require_repository_url(repository_url)

    try:
        blocks = README_BLOCKS[path]
    except KeyError as exc:
        raise ReleaseReadmeError(
            f"unsupported projected README path {path!r}"
        ) from exc
    return {
        block_id: _render_block(block, release, mode, repository_url)
        for block_id, block in blocks
    }
