#!/usr/bin/env python3
"""Qualify an already built Apple package through a disposable localhost HTTPS fixture."""

import argparse
from contextlib import contextmanager
import errno
from functools import partial
import hashlib
from http.server import SimpleHTTPRequestHandler, ThreadingHTTPServer
import json
import os
from pathlib import Path
import ssl
import subprocess
import sys
import tempfile
from threading import Thread

REPO_ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(REPO_ROOT))
from scripts.apple_package import package_apple


class FixtureHandler(SimpleHTTPRequestHandler):
    def log_message(self, format, *args):
        with self.server.request_log.open("a", encoding="utf-8") as log:
            log.write(f"{self.log_date_time_string()} {format % args}\n")


def fixture_server(directory: Path, request_log: Path):
    handler = partial(FixtureHandler, directory=str(directory))
    try:
        server = ThreadingHTTPServer(("127.0.0.1", 8443), handler)
    except OSError as error:
        if error.errno != errno.EADDRINUSE:
            raise
        server = ThreadingHTTPServer(("127.0.0.1", 0), handler)
    server.request_log = request_log
    return server


def validate_checksum_failure(returncode: int, diagnostic: str, work: Path):
    matched = any("checksum of downloaded artifact of binary target 'MermanFFI'" in line
                  and "does not match checksum specified by the manifest" in line
                  for line in diagnostic.splitlines())
    if returncode == 0 or (work / "qualification.json").exists() or not matched:
        raise RuntimeError("the corrupted fixture ZIP must fail SwiftPM's binary-target checksum check")


def run(command, *, check=True, **kwargs):
    print("RUN " + " ".join(str(arg) for arg in command), flush=True)
    return subprocess.run([str(arg) for arg in command], cwd=REPO_ROOT, check=check, **kwargs)


@contextmanager
def trusted_fixture_certificate(cert: Path):
    keychain = Path("/Library/Keychains/System.keychain")
    # SHA-1 is the Keychain certificate selector, not an artifact identity.
    certificate_id = hashlib.sha1(ssl.PEM_cert_to_DER_cert(cert.read_text())).hexdigest()
    try:
        run(["sudo", "-n", "security", "add-trusted-cert", "-d", "-r", "trustRoot",
             "-k", keychain, cert], timeout=30)
        yield
    finally:
        try:
            removed = run(["sudo", "-n", "security", "remove-trusted-cert", "-d", cert],
                          check=False, timeout=30)
        finally:
            deleted = run(["sudo", "-n", "security", "delete-certificate", "-Z", certificate_id, keychain],
                          check=False, timeout=30)
        if removed.returncode or deleted.returncode:
            raise RuntimeError("could not remove the fixture certificate trust or Keychain entry")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--work-dir", type=Path, required=True, help="Fresh directory outside tracked source")
    parser.add_argument("--version", required=True)
    parser.add_argument("--source-commit", required=True)
    parser.add_argument("--xcframework", type=Path, default=REPO_ROOT / "platforms/apple/Merman.xcframework")
    args = parser.parse_args()
    if sys.platform != "darwin":
        parser.error("controlled Apple HTTPS qualification requires macOS, Xcode and an iOS simulator")
    work = args.work_dir.resolve()
    work.mkdir(parents=True, exist_ok=False)
    with tempfile.TemporaryDirectory(prefix="merman-apple-https-") as temporary:
        root = Path(temporary)
        www = root / "www"
        www.mkdir()
        server = fixture_server(www, work / "https-requests.log")
        thread = None
        cert, key = root / "cert.pem", root / "key.pem"
        try:
            url = f"https://localhost:{server.server_port}"
            assets = package_apple(repo_root=REPO_ROOT, xcframework=args.xcframework.resolve(),
                                   output_dir=www, version=args.version, source_commit=args.source_commit,
                                   artifact_url=f"{url}/Merman.xcframework-v{args.version}.zip")
            tree = assets["remote_package_dir"]
            run(["git", "-c", "init.templateDir=", "init", "--object-format=sha1", tree])
            run(["git", "-C", tree, "add", "."])
            run(["git", "-C", tree, "-c", "user.name=Apple qualification",
                 "-c", "user.email=apple-qualification@example.invalid", "-c", "commit.gpgsign=false",
                 "commit", "-m", "Fixture delivery"])
            revision = run(["git", "-C", tree, "rev-parse", "HEAD"],
                           capture_output=True, text=True).stdout.strip()
            run(["git", "clone", "--bare", "--no-hardlinks", tree, www / "Merman.git"])
            run(["git", "--git-dir", www / "Merman.git", "update-server-info"])
            config = root / "tls.cnf"
            config.write_text("""[req]
distinguished_name=dn
[dn]
[server]
subjectAltName=DNS:localhost,IP:127.0.0.1
basicConstraints=critical,CA:TRUE
keyUsage=critical,digitalSignature,keyEncipherment,keyCertSign
extendedKeyUsage=serverAuth
""", encoding="utf-8")
            run(["openssl", "req", "-x509", "-newkey", "rsa:2048", "-nodes", "-days", "1",
                 "-subj", "/CN=localhost", "-config", config, "-extensions", "server",
                 "-keyout", key, "-out", cert])
            context = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
            context.load_cert_chain(cert, key)
            server.socket = context.wrap_socket(server.socket, server_side=True)
            with trusted_fixture_certificate(cert):
                thread = Thread(target=server.serve_forever, daemon=True)
                thread.start()
                command = [sys.executable, REPO_ROOT / "scripts/apple_qualify.py", "--package-archive",
                           assets["remote_package_archive"], "--version", args.version, "--simulator",
                           "--remote-package-url", f"{url}/Merman.git", "--remote-package-revision", revision]
                env = dict(os.environ, GIT_SSL_CAINFO=str(cert))
                with (work / "proof.log").open("w") as log:
                    run(command + ["--work-dir", work / "proof"], env=env, stdout=log, stderr=subprocess.STDOUT)
                report = json.loads((work / "proof/qualification.json").read_text())
                # Only the disposable served ZIP changes; the manifest and release staging stay frozen.
                with assets["binary_archive"].open("ab") as binary:
                    binary.write(b"intentional checksum mismatch")
                with (work / "wrong-checksum.log").open("w") as log:
                    negative = run(command + ["--work-dir", work / "wrong-checksum"], check=False,
                                   env=env, stdout=log, stderr=subprocess.STDOUT)
                validate_checksum_failure(negative.returncode, (work / "wrong-checksum.log").read_text(),
                                          work / "wrong-checksum")
                report.update(version=args.version, source_commit=args.source_commit,
                              checksum_rejection={"returncode": negative.returncode, "log": "wrong-checksum.log"})
        finally:
            if thread is not None and thread.is_alive():
                server.shutdown()
                thread.join()
            server.server_close()
    (work / "qualification.json").write_text(json.dumps(report, indent=2) + "\n")


if __name__ == "__main__":
    main()
