#!/usr/bin/env python3
"""Build and relocate a Flutter macOS app from the frozen thin release package."""

import argparse
from functools import partial
from http.server import SimpleHTTPRequestHandler, ThreadingHTTPServer
import json
from pathlib import Path
import platform
import shutil
import subprocess
import sys
import tempfile
from threading import Thread

sys.path.insert(0, str(Path(__file__).resolve().parents[3]))
from platforms.wasi.common import validate_svg

MATH_EVIDENCE_PREFIX = "MERMAN_PACKAGED_MATH="


def validate_app_math(stdout: str) -> int:
    evidence = [json.loads(line.removeprefix(MATH_EVIDENCE_PREFIX))
                for line in stdout.splitlines() if line.startswith(MATH_EVIDENCE_PREFIX)]
    if len(evidence) != 1:
        raise RuntimeError("packaged app must report exactly one math SVG result")
    data = evidence[0]["svg"].encode("utf-8")
    validate_svg(data, evidence[0]["metadata"], mathematical=True)
    return len(data)


class AssetHandler(SimpleHTTPRequestHandler):
    def log_message(self, *_args):
        pass

    def do_GET(self):
        self.server.downloads.append(self.path)
        super().do_GET()


def qualify(package_root: Path, assets_root: Path, output: Path):
    if platform.system() != "Darwin":
        raise RuntimeError("the Flutter release packaged-app owner requires macOS")
    if (package_root / "native").exists():
        raise RuntimeError("packaged app qualification requires an actual thin facade")
    manifest = json.loads((package_root / "hook/native-assets.json").read_text())
    host_is_arm64 = platform.machine().lower() in {"aarch64", "arm64"}
    target = "macos/arm64" if host_is_arm64 else "macos/x86_64"
    entry = manifest["targets"][target]
    with tempfile.TemporaryDirectory(prefix="merman-packaged-app-") as temporary:
        root = Path(temporary)
        package = root / "package"
        shutil.copytree(package_root, package, ignore=shutil.ignore_patterns(".dart_tool", "build"))
        consumer = root / "consumer"
        subprocess.run([
            "flutter", "create", "--no-pub", "--platforms=macos", "--empty",
            "--project-name=merman_packaged_app", str(consumer),
        ], check=True)
        shutil.copy2(package / "example/smoke.dart", consumer / "lib/smoke.dart")
        (consumer / "lib/main.dart").write_text(
            "import 'dart:convert';\n"
            "import 'dart:io';\n"
            "import 'package:flutter/widgets.dart';\n"
            "import 'package:merman/merman.dart';\n"
            "import 'smoke.dart' as smoke;\n"
            "void main() {\n"
            "  try {\n"
            "    WidgetsFlutterBinding.ensureInitialized();\n"
            "    smoke.main([]);\n"
            "    final engine = MermanEngine();\n"
            "    try {\n"
            "      final result = engine.execute(MermanOperation.svg, "
            "'flowchart TD\\nA[\"\\$\\$x^2\\$\\$\"] --> B');\n"
            f"      stdout.writeln('{MATH_EVIDENCE_PREFIX}' + jsonEncode({{"
            "'svg': result.utf8Text, 'metadata': result.metadata.jsonObject}));\n"
            "    } finally {\n"
            "      engine.close();\n"
            "    }\n"
            "    exit(0);\n"
            "  } catch (error, stack) {\n"
            "    stderr.writeln('$error\\n$stack');\n"
            "    exit(1);\n"
            "  }\n"
            "}\n"
        )
        release_config = consumer / "macos/Runner/Configs/Release.xcconfig"
        excluded_non_host_arch = "x86_64" if host_is_arm64 else "arm64"
        with release_config.open("a") as config:
            config.write(f"\nEXCLUDED_ARCHS = {excluded_non_host_arch}\n")
        server = ThreadingHTTPServer(("127.0.0.1", 0), partial(AssetHandler, directory=str(assets_root.resolve())))
        server.downloads = []
        thread = Thread(target=server.serve_forever, daemon=True)
        thread.start()
        try:
            (consumer / "pubspec.yaml").write_text(
                "name: merman_packaged_app\n"
                "environment:\n  sdk: '>=3.10.0 <4.0.0'\n"
                "dependencies:\n  flutter:\n    sdk: flutter\n"
                f"  merman:\n    path: {json.dumps(str(package))}\n"
                "hooks:\n  user_defines:\n    merman:\n"
                f"      mirror: http://127.0.0.1:{server.server_port}/\n"
            )
            subprocess.run(["flutter", "pub", "get"], cwd=consumer, check=True)
            subprocess.run(["flutter", "build", "macos", "--release", "--no-pub"], cwd=consumer, check=True)
        finally:
            server.shutdown()
            server.server_close()
            thread.join()
        if server.downloads != [f"/{entry['asset']}"]:
            raise RuntimeError(f"consumer must fetch exactly the selected target: {server.downloads}")
        relocated = root / "relocated/merman_packaged_app.app"
        relocated.parent.mkdir()
        shutil.move(consumer / "build/macos/Build/Products/Release/merman_packaged_app.app", relocated)
        native_framework = relocated / "Contents/Frameworks/merman_ffi.framework/merman_ffi"
        if not native_framework.is_file():
            raise RuntimeError("packaged app is missing its selected Merman native framework")
        shutil.rmtree(consumer)
        shutil.rmtree(package)
        binary = relocated / "Contents/MacOS/merman_packaged_app"
        executed = subprocess.run([str(binary)], cwd=relocated, check=True,
                                  stdout=subprocess.PIPE, text=True)
        math_svg_bytes = validate_app_math(executed.stdout)
        receipt = {
            "schema_version": 1, "source_sha": manifest["source_sha"],
            "product": "flutter-macos-app",
            "source_tree": manifest["source_tree"], "version": manifest["version"],
            "target": target, "asset_sha256": entry["sha256"],
            "selected_download_bytes": entry["size"],
            "selected_library_bytes": entry["library_size"],
            "downloads": server.downloads, "relocated_app_executed": True,
            "math_vector_glyphs": True, "math_svg_bytes": math_svg_bytes,
            "absent_codecs": ["jpeg", "pdf", "png"],
        }
        output.parent.mkdir(parents=True, exist_ok=True)
        output.write_text(json.dumps(receipt, indent=2, sort_keys=True) + "\n")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--package-root", type=Path, required=True)
    parser.add_argument("--assets-root", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    qualify(args.package_root, args.assets_root, args.output)


if __name__ == "__main__":
    main()
