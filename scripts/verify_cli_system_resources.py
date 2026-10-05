#!/usr/bin/env python3
"""Native Linux release probes for system TLS trust and font discovery."""

from __future__ import annotations

import http.server
import json
import os
from pathlib import Path
import socket
import ssl
import subprocess
import tempfile
import threading
import xml.etree.ElementTree as ET

if __package__:
    from .release_archive import ArchiveVerificationError
    from .release_process import run_checked
else:
    from release_archive import ArchiveVerificationError
    from release_process import run_checked


ICON_SOURCE = b'flowchart TD\nA@{ icon: "probe:cloud", label: "Cloud" }\n'
ICON_BODY = json.dumps({
    "prefix": "probe", "width": 16, "height": 16,
    "icons": {"cloud": {
        "body": '<path data-resource-probe="trusted" d="M1 8H15V14H1z"/>'
    }},
}).encode()
FONT_SOURCE = b'''---
config:
  theme: default
  look: classic
  layout: dagre
  fontFamily: DejaVu Sans
  htmlLabels: false
---
flowchart LR
A[NativeFontEvidence]
'''


def probe_https(binary: Path, certificate: Path, key: Path, *, trusted: bool) -> None:
    requests: list[str] = []

    class Handler(http.server.BaseHTTPRequestHandler):
        def do_GET(self) -> None:
            requests.append(self.path)
            if self.path != "/icons.json":
                self.send_error(404)
                return
            self.send_response(200)
            self.send_header("Content-Type", "application/json")
            self.send_header("Content-Length", str(len(ICON_BODY)))
            self.end_headers()
            self.wfile.write(ICON_BODY)

        def log_message(self, _format: str, *args: object) -> None:
            pass

    context = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
    context.load_cert_chain(certificate, key)
    with http.server.ThreadingHTTPServer(("127.0.0.1", 0), Handler) as server:
        server.socket = context.wrap_socket(server.socket, server_side=True)
        thread = threading.Thread(target=server.serve_forever, daemon=True)
        thread.start()
        try:
            url = f"probe#https://127.0.0.1:{server.server_port}/icons.json"
            if trusted:
                with socket.create_connection(("127.0.0.1", server.server_port), timeout=5) as connection:
                    with ssl.create_default_context().wrap_socket(
                        connection, server_hostname="127.0.0.1",
                    ):
                        pass
            result = subprocess.run(
                [str(binary), "render", "--format", "svg", "--allow-network",
                 "--allow-private-network", "--icon-pack-source", url, "-"],
                input=ICON_SOURCE, capture_output=True, timeout=30, check=False,
            )
        finally:
            server.shutdown()
            thread.join(timeout=5)
    if not trusted:
        if result.returncode == 0 or result.stdout or requests:
            raise ArchiveVerificationError("CLI accepted an untrusted HTTPS certificate")
        return
    if result.returncode != 0 or result.stderr or requests != ["/icons.json"]:
        raise ArchiveVerificationError(
            f"system-trusted HTTPS icon request failed: {result.stderr.decode(errors='replace')}"
        )
    root = ET.fromstring(result.stdout)
    if not any(node.get("data-resource-probe") == "trusted" for node in root.iter()):
        raise ArchiveVerificationError("HTTPS response icon is missing from rendered SVG")


def probe_fonts(binary: Path, root: Path) -> None:
    font = run_checked(
        ["fc-match", "--format=%{file}", "DejaVu Sans"], stdin=b"", cwd=root, runner=subprocess.run,
    ).stdout.decode().strip()
    if not Path(font).is_file() or "DejaVuSans" not in Path(font).name:
        raise ArchiveVerificationError("install fonts-dejavu-core before the font probe")
    for format in ("png", "jpg", "pdf"):
        trace = root / f"font-{format}.trace"
        output = root / f"font.{format}"
        run_checked(
            ["strace", "-f", "-e", "trace=openat", "-o", str(trace),
             str(binary), "render", "--format", format, "--output", str(output), "-"],
            stdin=FONT_SOURCE, cwd=root, runner=subprocess.run,
        )
        opens = trace.read_text(encoding="utf-8")
        if not any(f'"{font}"' in line and "= -1" not in line for line in opens.splitlines()):
            raise ArchiveVerificationError(f"{format} export did not open the host font {font}")
        if not output.is_file() or output.stat().st_size == 0:
            raise ArchiveVerificationError(f"{format} font probe produced no output")
    text = run_checked(
        ["pdftotext", str(root / "font.pdf"), "-"], stdin=b"", cwd=root, runner=subprocess.run,
    ).stdout
    fonts = run_checked(
        ["pdffonts", str(root / "font.pdf")], stdin=b"", cwd=root, runner=subprocess.run,
    ).stdout
    if b"NativeFontEvidence" not in b"".join(text.split()) or b"DejaVuSans" not in fonts:
        raise ArchiveVerificationError("PDF does not contain the selected host font and text")
    print(f"host font discovered by PNG/JPEG/PDF; PDF text verified: {font}")


def verify_system_resources(binary: Path, certificate: Path, key: Path) -> None:
    """Use a CI-installed system CA; never change host trust or override CA paths."""
    overrides = {name for name in ("SSL_CERT_FILE", "SSL_CERT_DIR") if name in os.environ}
    if overrides:
        raise ArchiveVerificationError(f"resource probe requires default system trust: {sorted(overrides)}")
    with tempfile.TemporaryDirectory(prefix="merman-resource-probe-") as directory:
        root = Path(directory)
        untrusted_certificate = root / "untrusted.crt"
        untrusted_key = root / "untrusted.key"
        run_checked(
            ["openssl", "req", "-x509", "-newkey", "rsa:2048", "-nodes", "-days", "1",
             "-subj", "/CN=merman-untrusted-probe", "-addext", "subjectAltName=IP:127.0.0.1",
             "-keyout", str(untrusted_key), "-out", str(untrusted_certificate)],
            stdin=b"", cwd=root, runner=subprocess.run,
        )
        probe_https(binary, untrusted_certificate, untrusted_key, trusted=False)
        probe_https(binary, certificate, key, trusted=True)
        print("HTTPS rejected an untrusted certificate and loaded an icon through system trust")
        probe_fonts(binary, root)
