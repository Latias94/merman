#!/usr/bin/env python3
"""Prepare and qualify an Android Maven handoff without publishing it."""

from __future__ import annotations

import argparse
import hashlib
import importlib.util
import os
import shutil
import subprocess
import sys
import xml.etree.ElementTree as ET
import zipfile
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
SPEC = importlib.util.spec_from_file_location(
    "android_binding_verifier", ROOT / "scripts" / "verify-platform-bindings.py",
)
assert SPEC is not None and SPEC.loader is not None
verifier = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(verifier)
ARTIFACT_ID = verifier.ANDROID_MAVEN_COORDINATES[1]


def selected_metadata(module_root: Path, group_id: str, version: str) -> bytes:
    staged = ET.parse(module_root / "maven-metadata.xml").getroot()
    if (staged.findtext("groupId"), staged.findtext("artifactId")) != (group_id, ARTIFACT_ID):
        raise RuntimeError("staged Maven metadata coordinates do not match the selected publication")
    if version not in [entry.text for entry in staged.findall("versioning/versions/version")]:
        raise RuntimeError(f"staged Maven metadata does not include selected version {version}")
    metadata = ET.Element("metadata")
    ET.SubElement(metadata, "groupId").text = group_id
    ET.SubElement(metadata, "artifactId").text = ARTIFACT_ID
    versioning = ET.SubElement(metadata, "versioning")
    ET.SubElement(versioning, "latest").text = version
    ET.SubElement(versioning, "release").text = version
    ET.SubElement(ET.SubElement(versioning, "versions"), "version").text = version
    last_updated = staged.findtext("versioning/lastUpdated")
    if last_updated:
        ET.SubElement(versioning, "lastUpdated").text = last_updated
    return ET.tostring(metadata, encoding="utf-8", xml_declaration=True)


def write_archive(path: Path, entries: dict[str, Path | bytes]) -> None:
    with zipfile.ZipFile(path, "w", compression=zipfile.ZIP_DEFLATED) as archive:
        for name, content in sorted(entries.items()):
            info = zipfile.ZipInfo(name)
            info.compress_type = zipfile.ZIP_DEFLATED
            info.external_attr = 0o100644 << 16
            if isinstance(content, Path):
                with content.open("rb") as source, archive.open(info, "w", force_zip64=True) as destination:
                    shutil.copyfileobj(source, destination)
            else:
                archive.writestr(info, content)
    path.with_name(path.name + ".sha256").write_text(
        verifier._sha256_and_size(path)[0] + "\n", encoding="ascii",
    )


def package_repository(args: argparse.Namespace) -> None:
    relative_module = Path(*args.group_id.split("."), ARTIFACT_ID)
    module_root = args.repository / relative_module
    llvm_nm = verifier.resolve_android_llvm_nm(args.android_ndk_home) if args.android_ndk_home else None
    selected = verifier.assert_android_maven_publication(
        module_root, group_id=args.group_id, version=args.version, llvm_nm=llvm_nm,
        android_root=args.android_root,
        require_signatures=args.central_bundle,
    )
    entries: dict[str, Path | bytes] = {
        (relative_module / args.version / path.relative_to(selected)).as_posix(): path
        for path in selected.rglob("*") if path.is_file()
    }
    metadata = selected_metadata(module_root, args.group_id, args.version)
    # Central owns aggregate metadata; its upload bundle contains version directories only.
    central_entries = dict(entries)
    metadata_path = (relative_module / "maven-metadata.xml").as_posix()
    entries[metadata_path] = metadata
    for algorithm in ("md5", "sha1", "sha256", "sha512"):
        entries[metadata_path + "." + algorithm] = (
            hashlib.new(algorithm, metadata).hexdigest() + "\n"
        ).encode("ascii")
    args.output_dir.mkdir(parents=True, exist_ok=True)
    base = f"{ARTIFACT_ID}-{args.version}"
    write_archive(args.output_dir / f"{base}-maven.zip", entries)
    if args.central_bundle:
        write_archive(args.output_dir / f"{base}-central.zip", central_entries)
    print(f"Prepared {args.group_id}:{ARTIFACT_ID}:{args.version} in {args.output_dir}")


def qualify_repository(args: argparse.Namespace) -> None:
    verifier._assert_sha256(args.archive)
    args.work_dir = args.work_dir.resolve()
    args.work_dir.mkdir(parents=True, exist_ok=False)
    repository = args.work_dir / "repository"
    with zipfile.ZipFile(args.archive) as archive:
        archive.extractall(repository)
    module_root = repository.joinpath(*args.group_id.split("."), ARTIFACT_ID)
    selected = verifier.assert_android_maven_publication(
        module_root, group_id=args.group_id, version=args.version,
        llvm_nm=verifier.resolve_android_llvm_nm(args.android_ndk_home),
    )
    objdump = verifier.resolve_android_ndk_tool("llvm-objdump", args.android_ndk_home)
    verifier.assert_android_16kb_alignment(
        selected / f"{ARTIFACT_ID}-{args.version}.aar", objdump,
    )
    consumer = args.work_dir / "consumer"
    android = ROOT / "platforms" / "android"
    shutil.copytree(android / "consumer", consumer, ignore=shutil.ignore_patterns(".gradle", "build"))
    (consumer / "gradle").mkdir(exist_ok=True)
    shutil.copy2(android / "gradle" / "libs.versions.toml", consumer / "gradle" / "libs.versions.toml")
    smoke = consumer / "src" / "androidTest" / "kotlin" / "io" / "merman" / "examples"
    smoke.mkdir(parents=True, exist_ok=True)
    shutil.copy2(android / "examples" / "MermanSmoke.kt", smoke / "MermanSmoke.kt")
    wrapper = android / ("gradlew.bat" if os.name == "nt" else "gradlew")
    command = (["cmd.exe", "/d", "/s", "/c", str(wrapper)] if os.name == "nt" else [str(wrapper)])
    command += [
        "-p", str(consumer), "--no-daemon", "--max-workers=2", "--stacktrace",
        f"-PmermanRepository={repository.as_uri()}",
        f"-PmermanGroup={args.group_id}", f"-PmermanVersion={args.version}",
    ]
    subprocess.run([*command, "-PmermanMetadata=pom", "verifyMermanResolution"], check=True)
    subprocess.run([
        *command, "-PmermanMetadata=gradle",
        "verifyMermanResolution", "assembleDebug", "assembleDebugAndroidTest",
    ], check=True)
    apk = verifier._single_path(
        list((consumer / "build" / "outputs" / "apk" / "debug").glob("*.apk")),
        "external Maven consumer APK",
    )
    verifier.assert_android_16kb_alignment(apk, objdump, zipalign=args.zipalign)
    if args.connected:
        subprocess.run([*command, "-PmermanMetadata=gradle", "connectedDebugAndroidTest"], check=True)
    print(f"Qualified external Maven consumer: {consumer}")


def sign_repository(args: argparse.Namespace) -> None:
    module_root = args.repository.joinpath(*args.group_id.split("."), ARTIFACT_ID)
    selected = verifier.assert_android_maven_publication(
        module_root, group_id=args.group_id, version=args.version,
        android_root=args.android_root,
        llvm_nm=verifier.resolve_android_llvm_nm(args.android_ndk_home),
    )
    passphrase = os.environ.get("MERMAN_ANDROID_SIGNING_PASSWORD", "")
    for artifact in sorted(selected.iterdir()):
        if artifact.suffix not in {".aar", ".pom", ".module", ".jar"}:
            continue
        subprocess.run([
            "gpg", "--batch", "--yes", "--pinentry-mode", "loopback",
            "--passphrase-fd", "0", "--armor", "--detach-sign",
            "--local-user", args.signing_key_id,
            "--output", str(artifact.with_name(artifact.name + ".asc")), str(artifact),
        ], input=passphrase + "\n", text=True, check=True)
        for signed in (artifact, artifact.with_name(artifact.name + ".asc")):
            for algorithm, digest in verifier.android_maven_checksums(signed).items():
                signed.with_name(signed.name + "." + algorithm).write_text(
                    digest + "\n",
                    encoding="ascii",
                )
    verifier.assert_android_maven_sidecars(selected, require_signatures=True)


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    subparsers = parser.add_subparsers(dest="command", required=True)
    package = subparsers.add_parser("package", help="Archive one verified Maven publication.")
    package.add_argument("--repository", type=Path, required=True)
    package.add_argument("--group-id", required=True)
    package.add_argument("--version", required=True)
    package.add_argument("--output-dir", type=Path, required=True)
    package.add_argument("--android-ndk-home")
    package.add_argument("--android-root", type=Path, default=ROOT / "platforms/android", help="Selected Android facade data; no scripts from this directory are executed.")
    package.add_argument("--central-bundle", action="store_true", help="Require GPG signatures and prepare the official Central upload layout.")
    qualify = subparsers.add_parser("qualify", help="Resolve the extracted Maven repository from an external Android app.")
    qualify.add_argument("--archive", type=Path, required=True)
    qualify.add_argument("--group-id", required=True)
    qualify.add_argument("--version", required=True)
    qualify.add_argument("--work-dir", type=Path, required=True)
    qualify.add_argument("--android-ndk-home", required=True)
    qualify.add_argument("--zipalign", type=Path, required=True)
    qualify.add_argument("--connected", action="store_true")
    sign = subparsers.add_parser("sign", help="Sign an already built publication locally; no registry calls.")
    sign.add_argument("--repository", type=Path, required=True)
    sign.add_argument("--group-id", required=True)
    sign.add_argument("--version", required=True)
    sign.add_argument("--signing-key-id", required=True)
    sign.add_argument("--android-ndk-home", required=True)
    sign.add_argument("--android-root", type=Path, default=ROOT / "platforms/android", help="Selected Android facade data; no scripts from this directory are executed.")
    args = parser.parse_args(argv)
    try:
        if args.command == "qualify":
            qualify_repository(args)
        elif args.command == "sign":
            sign_repository(args)
        else:
            package_repository(args)
        return 0
    except (RuntimeError, OSError, ValueError, ET.ParseError, subprocess.CalledProcessError, zipfile.BadZipFile) as error:
        print(f"error: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
