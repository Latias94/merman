#!/usr/bin/env python3
"""Install only the Node loader tarball, resolving its optional packages through a local registry."""

from __future__ import annotations

import argparse
import json
import os
import shutil
import subprocess
import sys
import tempfile
import threading
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from urllib.parse import unquote, urlsplit

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT))
from scripts.node_package_group import load_descriptor, npm_integrity, read_tarball_manifest
from scripts.npm_package_group import npm_executable


def install_root_package(tarball_dir: Path, project: Path, target: str, version: str) -> dict:
    descriptor = load_descriptor(ROOT / "platforms/node/package-surfaces.json")
    target_package = next(item["name"] for item in descriptor["targets"] if item["target"] == target)
    packages: dict[str, dict] = {}
    tarballs: dict[str, Path] = {}
    root_tarball = None
    for tarball in sorted(tarball_dir.glob("*.tgz")):
        manifest, _, _ = read_tarball_manifest(tarball)
        name, package_version = manifest["name"], manifest["version"]
        tarballs[f"/tarballs/{tarball.name}"] = tarball
        packages.setdefault(name, {})[package_version] = {
            **manifest, "dist": {"integrity": npm_integrity(tarball), "tarball": tarball.name},
        }
        if name == descriptor["root"]["name"] and package_version == version:
            root_tarball = tarball
    assert root_tarball is not None, "expected an exact-version loader tarball"

    class Registry(BaseHTTPRequestHandler):
        def log_message(self, *_args) -> None:
            pass

        def do_GET(self) -> None:
            route = unquote(urlsplit(self.path).path)
            if route in tarballs:
                with tarballs[route].open("rb") as source:
                    self.send_response(200)
                    self.send_header("Content-Type", "application/octet-stream")
                    self.send_header("Content-Length", str(os.fstat(source.fileno()).st_size))
                    self.end_headers()
                    shutil.copyfileobj(source, self.wfile, length=1024 * 1024)
                return
            elif route.removeprefix("/") in packages:
                name = route.removeprefix("/")
                versions = packages[name]
                payload = json.dumps({
                    "name": name,
                    "dist-tags": {"latest": next(reversed(versions))},
                    "versions": versions,
                }).encode()
                content_type = "application/json"
            else:
                self.send_error(404)
                return
            self.send_response(200)
            self.send_header("Content-Type", content_type)
            self.send_header("Content-Length", str(len(payload)))
            self.end_headers()
            self.wfile.write(payload)

    project.mkdir(parents=True, exist_ok=True)
    package_json = project / "package.json"
    assert not package_json.exists(), "root-only fixture needs a fresh consumer project"
    package_json.write_text(json.dumps({"private": True, "type": "module"}) + "\n")
    with ThreadingHTTPServer(("127.0.0.1", 0), Registry) as server:
        registry = f"http://127.0.0.1:{server.server_port}"
        for versions in packages.values():
            for manifest in versions.values():
                manifest["dist"]["tarball"] = registry + "/tarballs/" + manifest["dist"]["tarball"]
        thread = threading.Thread(target=lambda: server.serve_forever(poll_interval=0.01), daemon=True)
        thread.start()
        try:
            with tempfile.TemporaryDirectory(prefix="merman-node-npm-") as temporary:
                npmrc = Path(temporary) / "npmrc"
                npmrc.write_text("")
                result = subprocess.run(
                    [npm_executable(), "install", "--ignore-scripts", "--no-audit", "--no-fund",
                     "--include=optional", "--engine-strict", "--registry", registry,
                     "--userconfig", str(npmrc), "--cache", str(Path(temporary) / "cache"),
                     str(root_tarball.resolve())],
                    cwd=project, capture_output=True, text=True, timeout=60,
                )
                if result.returncode:
                    raise RuntimeError(result.stderr or result.stdout)
        finally:
            server.shutdown()
            thread.join()

    consumer = json.loads(package_json.read_text())
    root_name = descriptor["root"]["name"]
    assert set(consumer["dependencies"]) == {root_name}, "install must name only the loader"
    installed_root = json.loads((project / "node_modules" / root_name / "package.json").read_text())
    expected_optional = {item["name"]: version for item in descriptor["targets"]}
    assert installed_root["version"] == version
    assert installed_root["optionalDependencies"] == expected_optional
    installed = sorted(item["name"] for item in descriptor["targets"]
                       if (project / "node_modules" / item["name"] / "package.json").is_file())
    assert installed == [target_package], f"npm selected the wrong platform package(s): {installed}"
    lock = json.loads((project / "package-lock.json").read_text())
    assert lock["packages"][f"node_modules/{root_name}"]["optionalDependencies"] == expected_optional
    selected = lock["packages"][f"node_modules/{target_package}"]
    assert selected["version"] == version and selected["optional"] is True
    assert json.loads((project / "node_modules" / target_package / "package.json").read_text())["version"] == version
    return {
        "root_only": True, "target": target, "version": version,
        "installed_platform_packages": installed, "optional_dependency_version": selected["version"],
        "node_version": subprocess.check_output(["node", "--version"], text=True).strip(),
    }


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--tarball-dir", type=Path, required=True)
    parser.add_argument("--project", type=Path, required=True)
    parser.add_argument("--target", required=True)
    parser.add_argument("--version", required=True)
    arguments = parser.parse_args()
    print(json.dumps(install_root_package(arguments.tarball_dir, arguments.project,
                                        arguments.target, arguments.version)))
