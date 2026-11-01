#!/usr/bin/env python3
"""Focused boundaries for Android ELF/APK qualification and candidate dispatch."""

from __future__ import annotations

import hashlib
import json
import os
import importlib.util
import shutil
import subprocess
import sys
import tempfile
import textwrap
import unittest
import zipfile
from pathlib import Path
from unittest import mock

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from scripts.test_release_workflow_security import workflow_job
from scripts.test_verify_platform_bindings import verify_platform_bindings as verifier


ROOT = Path(__file__).resolve().parents[1]
WORKFLOW = ROOT / ".github" / "workflows" / "release-android.yml"
BASH = shutil.which("bash")
ALIGNED_LOADS = """Program Header:
    LOAD off 0x0000 vaddr 0x0000 paddr 0x0000 align 2**14
         filesz 0x1000 memsz 0x1000 flags r-x
    LOAD off 0x4000 vaddr 0x4000 paddr 0x4000 align 2**14
         filesz 0x1000 memsz 0x1000 flags rw-
"""


def workflow_shell(job: str, step_name: str) -> str:
    job_text = workflow_job(WORKFLOW.read_text(encoding="utf-8"), job)
    step = job_text.split(f"      - name: {step_name}\n", 1)[1].split("\n      - ", 1)[0]
    return textwrap.dedent(step.split("        run: |\n", 1)[1])


def isolated_shell_environment(root: Path, commands: tuple[str, ...]) -> dict[str, str]:
    binary = root / "bin"
    binary.mkdir()
    for name in commands:
        executable = sys.executable if name == "python3" else shutil.which(name)
        if executable is None:
            raise unittest.SkipTest(f"workflow execution requires {name}")
        (binary / name).symlink_to(executable)
    temporary = root / "temporary"
    temporary.mkdir()
    return {"PATH": str(binary), "TMPDIR": str(temporary), "LC_ALL": "C"}


def write_local_command(path: Path, source: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(f"#!{sys.executable}\n" + textwrap.dedent(source), encoding="utf-8")
    path.chmod(0o755)


class AndroidAlignmentTests(unittest.TestCase):
    def test_both_delivered_abi_slices_must_have_16kb_load_alignment(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            aar = Path(temporary) / "selected.aar"
            with zipfile.ZipFile(aar, "w") as archive:
                for entry in verifier.ANDROID_NATIVE_LIBRARIES:
                    archive.writestr(entry, b"native library fixture")
            with mock.patch.object(
                verifier.subprocess, "run",
                return_value=subprocess.CompletedProcess([], 0, stdout=ALIGNED_LOADS),
            ):
                verifier.assert_android_16kb_alignment(aar, Path("llvm-objdump"))
            with (
                mock.patch.object(
                    verifier.subprocess, "run",
                    return_value=subprocess.CompletedProcess(
                        [], 0, stdout=ALIGNED_LOADS.replace("2**14", "2**12"),
                    ),
                ),
                self.assertRaisesRegex(RuntimeError, "16 KB.*arm64-v8a"),
            ):
                verifier.assert_android_16kb_alignment(aar, Path("llvm-objdump"))

    def test_apk_requires_both_abis_and_zip_alignment(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            apk = Path(temporary) / "consumer.apk"
            with zipfile.ZipFile(apk, "w") as archive:
                for entry in verifier.ANDROID_NATIVE_LIBRARIES:
                    archive.writestr(entry.replace("jni/", "lib/"), b"native library fixture")
            with mock.patch.object(
                verifier.subprocess, "run",
                side_effect=[
                    subprocess.CompletedProcess([], 0, stdout=ALIGNED_LOADS),
                    subprocess.CompletedProcess([], 0, stdout=ALIGNED_LOADS),
                    subprocess.CalledProcessError(1, ["zipalign"]),
                ],
            ), self.assertRaises(subprocess.CalledProcessError):
                verifier.assert_android_16kb_alignment(
                    apk, Path("llvm-objdump"), zipalign=Path("zipalign"),
                )
            with zipfile.ZipFile(apk, "w") as archive:
                archive.writestr("lib/arm64-v8a/libmerman_android_jni.so", b"one slice")
            with self.assertRaisesRegex(RuntimeError, "x86_64"):
                verifier.assert_android_16kb_alignment(
                    apk, Path("llvm-objdump"), zipalign=Path("zipalign"),
                )


class ExtractedMavenConsumerTests(unittest.TestCase):
    def test_qualifier_builds_an_external_coordinate_consumer_with_both_metadata_formats(self) -> None:
        from scripts.test_android_package import AndroidPackageTests

        fixture = AndroidPackageTests()
        fixture.setUp()
        self.addCleanup(fixture.doCleanups)
        self.assertEqual(fixture.run_package(), 0)
        spec = importlib.util.spec_from_file_location("android_package", ROOT / "scripts" / "android_package.py")
        assert spec is not None and spec.loader is not None
        package = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(package)
        work = fixture.root / "external"
        commands: list[list[str]] = []

        def run(command: list[str], **kwargs: object) -> subprocess.CompletedProcess:
            commands.append(command)
            if "assembleDebug" in command:
                apk = work / "consumer" / "build" / "outputs" / "apk" / "debug" / "merman-maven-consumer-debug.apk"
                apk.parent.mkdir(parents=True, exist_ok=True)
                apk.touch()
            return subprocess.CompletedProcess(command, 0)

        with (
            mock.patch.object(package, "verifier", verifier),
            mock.patch.object(verifier, "android_library_symbols", return_value={"JNI_OnLoad"}),
            mock.patch.object(verifier, "android_class_api", return_value="getIconPackSet();"),
            mock.patch.object(verifier, "assert_android_16kb_alignment"),
            mock.patch.object(verifier, "resolve_android_ndk_tool", return_value=Path("llvm-objdump")),
            mock.patch.object(verifier, "resolve_android_llvm_nm", return_value=Path("llvm-nm")),
            mock.patch.object(package.subprocess, "run", side_effect=run),
        ):
            self.assertEqual(package.main([
                "qualify", "--archive", str(fixture.output / "merman-android-0.9.3-maven.zip"),
                "--group-id", "org.example", "--version", "0.9.3",
                "--work-dir", str(work), "--android-ndk-home", "/selected/ndk",
                "--zipalign", "/selected/zipalign", "--connected",
            ]), 0)
        gradle_commands = [command for command in commands if "-p" in command]
        self.assertTrue(any("-PmermanMetadata=pom" in command and "verifyMermanResolution" in command for command in gradle_commands))
        self.assertTrue(any("-PmermanMetadata=gradle" in command and "verifyMermanResolution" in command for command in gradle_commands))
        self.assertTrue(any("assembleDebug" in command for command in gradle_commands))
        self.assertTrue(any("connectedDebugAndroidTest" in command for command in gradle_commands))
        self.assertTrue(all("-PmermanGroup=org.example" in command and "-PmermanVersion=0.9.3" in command for command in gradle_commands))
        self.assertFalse(any(command[0] in {"cargo", "rustup"} for command in commands))
        self.assertTrue((work / "consumer" / "gradle" / "libs.versions.toml").is_file())
        self.assertTrue((work / "consumer" / "src" / "androidTest" / "kotlin" / "io" / "merman" / "examples" / "MermanSmoke.kt").is_file())


@unittest.skipUnless(BASH, "workflow execution requires Bash")
class AndroidCandidateWorkflowTests(unittest.TestCase):
    def test_source_selection_separates_candidates_from_publication(self) -> None:
        text = WORKFLOW.read_text(encoding="utf-8")
        step = text.split("        id: release\n", 1)[1].split("\n      - ", 1)[0]
        command = textwrap.dedent(step.split("        run: |\n", 1)[1])
        commit = "a" * 40
        cases = [
            (False, False, "main", "refs/heads/main"),
            (False, False, commit, commit),
            (False, False, "v0.9.3", "refs/tags/v0.9.3"),
            (False, False, "feature/other", None),
            (True, True, "v0.9.3", "refs/tags/v0.9.3"),
            (True, True, "main", None),
            (True, True, commit, None),
            (True, True, "v0.9.2", None),
            (True, False, "v0.9.3", None),
        ]
        for publish, central, source, expected in cases:
            with self.subTest(publish=publish, source=source, central=central):
                with tempfile.TemporaryDirectory() as temporary:
                    output = Path(temporary) / "output"
                    result = subprocess.run(
                        [BASH, "-c", command], capture_output=True, text=True,
                        env=dict(
                            os.environ, DISPATCH_RELEASE_TAG="v0.9.3",
                            DISPATCH_SOURCE_REF=source, PUBLISH=str(publish).lower(),
                            PREPARE_CENTRAL_BUNDLE=str(central).lower(),
                            MAVEN_GROUP="org.example", GITHUB_OUTPUT=str(output),
                        ),
                    )
                    if expected:
                        self.assertEqual(result.returncode, 0, result.stderr)
                        self.assertIn(f"source_ref={expected}\n", output.read_text())
                        self.assertIn("maven_group=org.example\n", output.read_text())
                    else:
                        self.assertNotEqual(result.returncode, 0)
                        self.assertFalse(output.exists())

    def test_signing_requires_the_configured_verified_group(self) -> None:
        text = WORKFLOW.read_text(encoding="utf-8")
        step = text.split("      - name: Verify immutable signing source and configured namespace\n", 1)[1]
        command = textwrap.dedent(step.split("        run: |\n", 1)[1].split("\n      - ", 1)[0])
        for verified, expected in (("org.example", 0), ("io.merman", 1), ("", 1)):
            with self.subTest(verified=verified):
                result = subprocess.run(
                    [BASH, "-c", 'git() { printf "%s\\n" "$EXPECTED_SOURCE_SHA"; };\n' + command],
                    capture_output=True, text=True,
                    env=dict(
                        os.environ, EXPECTED_SOURCE_SHA="a" * 40, EXPECTED_SOURCE_TREE="a" * 40,
                        MAVEN_GROUP="org.example", VERIFIED_GROUP=verified,
                    ),
                )
                self.assertEqual(result.returncode, expected, result.stderr)

    def test_write_owner_is_explicit_and_candidate_build_qualifies_the_extracted_repository(self) -> None:
        text = WORKFLOW.read_text(encoding="utf-8")
        upload = workflow_job(text, "upload-release")
        self.assertIn("inputs.publish", upload)
        build = workflow_job(text, "build")
        self.assertNotIn("contents: write", build)
        self.assertNotIn("secrets.", build)
        self.assertIn("android_package.py qualify", build)
        self.assertIn("google_apis_ps16k", build)
        self.assertIn("getconf PAGE_SIZE", build)
        self.assertIn("16384", build)


@unittest.skipUnless(BASH, "workflow execution requires Bash")
class AndroidReleaseUploadTests(unittest.TestCase):
    def setUp(self) -> None:
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        self.environment = isolated_shell_environment(self.root, ("jq", "cmp", "mktemp", "rm"))
        self.public = self.root / "public"
        self.public.mkdir()
        self.assets = {
            "merman-android-v0.9.3.aar": b"frozen AAR\x00fixture",
            "merman-android-0.9.3-maven.zip": b"frozen signed Maven repository",
            "merman-android-0.9.3-central.zip": b"frozen Central handoff",
        }
        for name, payload in list(self.assets.items()):
            if name.endswith(".zip"):
                self.assets[name + ".sha256"] = hashlib.sha256(payload).hexdigest().encode() + b"\n"
        local = self.root / "target" / "release-assets"
        local.mkdir(parents=True)
        for name, payload in self.assets.items():
            (local / name).write_bytes(payload)
        runner = self.root / "runner"
        runner.mkdir()
        self.log = self.root / "commands.jsonl"
        self.config = self.root / "config.json"
        self.settings: dict[str, object] = {}
        self.environment.update(
            RUNNER_TEMP=str(runner), GH_TOKEN="local-fixture-token",
            GH_REPO="fixture/merman", GITHUB_REPOSITORY="fixture/merman",
            RELEASE_TAG="v0.9.3", SOURCE_SHA="a" * 40,
            ANDROID_TEST_PUBLIC=str(self.public), ANDROID_TEST_LOG=str(self.log),
            ANDROID_TEST_CONFIG=str(self.config),
        )
        # These executables only read and write the fixture. PATH exposes no network clients.
        tool = """
            import json
            import os
            import sys
            from pathlib import Path

            args = sys.argv[1:]
            name = Path(sys.argv[0]).name
            public = Path(os.environ["ANDROID_TEST_PUBLIC"])
            settings = json.loads(Path(os.environ["ANDROID_TEST_CONFIG"]).read_text())

            def record(event, **fields):
                with open(os.environ["ANDROID_TEST_LOG"], "a") as log:
                    log.write(json.dumps(dict(event=event, **fields)) + "\\n")

            record(name, args=args)
            repository = os.environ["GH_REPO"]
            tag = os.environ["RELEASE_TAG"]
            if name == "gh":
                prefix = "repos/" + repository
                if args == ["api", prefix + "/git/ref/tags/" + tag]:
                    print("{}")
                elif args == ["api", prefix + "/commits/" + tag, "--jq", ".sha"]:
                    print(settings.get("observed_sha", os.environ["SOURCE_SHA"]))
                elif args == ["api", prefix + "/releases/tags/" + tag]:
                    print(json.dumps({
                        "draft": settings.get("draft", False),
                        "assets": [{"name": path.name} for path in sorted(public.iterdir())],
                    }))
                elif args[:3] == ["release", "upload", tag]:
                    assert len(args) > 3
                    for index, source in enumerate(args[3:], 1):
                        assert not source.startswith("-"), "replacement flags are forbidden"
                        path = Path(source)
                        with (public / path.name).open("xb") as destination:
                            destination.write(path.read_bytes())
                        record("uploaded", name=path.name)
                        if index == settings.get("fail_upload_after"):
                            sys.exit(1)
                else:
                    raise AssertionError(args)
            elif name == "curl":
                assert args[0] == "--disable", "curl config must be disabled"
                assert args[-2] == "--output"
                options = args[:-3]
                flags = {"--disable", "--fail", "--location", "--silent", "--show-error"}
                values = {"--proto": "=https", "--proto-redir": "=https",
                          "--connect-timeout": "30", "--max-time": "180"}
                seen = set()
                while options:
                    option = options.pop(0)
                    seen.add(option)
                    if option in values:
                        assert options.pop(0) == values[option]
                    else:
                        assert option in flags, "authenticated or unexpected curl option: " + option
                assert seen == flags | values.keys()
                prefix = "https://github.com/" + repository + "/releases/download/" + tag + "/"
                assert args[-3].startswith(prefix)
                asset = args[-3][len(prefix):]
                assert asset and "/" not in asset
                if settings.get("download_error") == asset:
                    sys.exit(22)
                payload = (public / asset).read_bytes()
                if settings.get("download_corrupt") == asset:
                    payload = b"different anonymous response"
                Path(args[-1]).write_bytes(payload)
            else:
                raise AssertionError(name)
        """
        for name in ("gh", "curl"):
            write_local_command(self.root / "bin" / name, tool)

    def seed_public(self, names: list[str]) -> None:
        for name in names:
            (self.public / name).write_bytes(self.assets[name])

    def execute(self) -> tuple[subprocess.CompletedProcess[str], list[dict]]:
        self.config.write_text(json.dumps(self.settings), encoding="utf-8")
        before = len(self.log.read_text().splitlines()) if self.log.exists() else 0
        result = subprocess.run(
            [BASH, "-c", workflow_shell("upload-release", "Upload AAR to GitHub Release")],
            cwd=self.root, env=self.environment, capture_output=True, text=True, timeout=15,
        )
        events = [json.loads(line) for line in self.log.read_text().splitlines()]
        self.assertTrue(events, "the extracted upload shell must invoke the local tools")
        self.assertEqual(
            {event["event"] for event in events} - {"gh", "curl", "uploaded"}, set(),
        )
        for event in events[before:]:
            if event["event"] == "gh":
                arguments = event["args"]
                self.assertTrue(arguments[0] == "api" or arguments[:2] == ["release", "upload"])
                self.assertNotIn("--clobber", arguments)
        return result, events[before:]

    def assert_public_bytes(self) -> None:
        self.assertEqual(
            {path.name: path.read_bytes() for path in self.public.iterdir()}, self.assets,
        )

    def test_equal_partial_release_uploads_only_missing_assets_then_reads_back_every_asset(self) -> None:
        existing = ["merman-android-v0.9.3.aar", "merman-android-0.9.3-maven.zip.sha256"]
        self.seed_public(existing)
        result, events = self.execute()
        self.assertEqual(result.returncode, 0, result.stderr)
        uploaded = [event["name"] for event in events if event["event"] == "uploaded"]
        self.assertEqual(set(uploaded), self.assets.keys() - set(existing))
        self.assertEqual(len(uploaded), len(self.assets) - len(existing))
        upload_index = next(index for index, event in enumerate(events) if event["event"] == "uploaded")
        for section, expected in ((events[:upload_index], sorted(existing)),
                                  (events[upload_index:], sorted(self.assets))):
            downloads = [event["args"][-3].rsplit("/", 1)[1]
                         for event in section if event["event"] == "curl"]
            self.assertEqual(downloads, expected)
        self.assert_public_bytes()

    def test_complete_equal_release_performs_no_upload_and_still_reads_back_every_asset(self) -> None:
        self.seed_public(list(self.assets))
        result, events = self.execute()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertFalse(any(event["event"] == "uploaded" for event in events))
        self.assertFalse(any(event["event"] == "gh" and event["args"][:2] == ["release", "upload"]
                             for event in events))
        downloads = [event["args"][-3].rsplit("/", 1)[1]
                     for event in events if event["event"] == "curl"]
        self.assertEqual(downloads, sorted(self.assets) * 2)
        self.assert_public_bytes()

    def test_mismatched_existing_asset_fails_before_uploading_any_missing_asset(self) -> None:
        name = "merman-android-v0.9.3.aar"
        (self.public / name).write_bytes(b"previous unrelated build")
        result, events = self.execute()
        self.assertNotEqual(result.returncode, 0)
        self.assertFalse(any(event["event"] == "uploaded" for event in events))
        self.assertFalse(any(event["event"] == "gh" and event["args"][:2] == ["release", "upload"]
                             for event in events))
        self.assertEqual(list(self.public.iterdir()), [self.public / name])
        self.assertEqual((self.public / name).read_bytes(), b"previous unrelated build")

    def test_draft_release_fails_before_any_download_or_upload(self) -> None:
        self.settings["draft"] = True
        result, events = self.execute()
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual([event["event"] for event in events], ["gh", "gh", "gh"])
        self.assertEqual(list(self.public.iterdir()), [])

    def test_moved_tag_fails_before_observing_or_mutating_the_release(self) -> None:
        self.settings["observed_sha"] = "b" * 40
        result, events = self.execute()
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual([event["event"] for event in events], ["gh", "gh"])
        self.assertEqual(list(self.public.iterdir()), [])

    def test_interrupted_upload_resumes_without_replacing_the_completed_asset(self) -> None:
        self.settings["fail_upload_after"] = 1
        result, first_events = self.execute()
        self.assertNotEqual(result.returncode, 0)
        first = [event["name"] for event in first_events if event["event"] == "uploaded"]
        self.assertEqual(len(first), 1)
        self.assertEqual((self.public / first[0]).read_bytes(), self.assets[first[0]])
        self.settings.clear()
        result, resumed_events = self.execute()
        self.assertEqual(result.returncode, 0, result.stderr)
        uploads = [event["name"] for event in first_events + resumed_events
                   if event["event"] == "uploaded"]
        self.assertEqual(sorted(uploads), sorted(self.assets))
        downloads = [event["args"][-3].rsplit("/", 1)[1]
                     for event in resumed_events if event["event"] == "curl"]
        self.assertEqual(downloads, first + sorted(self.assets))
        self.assert_public_bytes()

    def test_unavailable_existing_anonymous_download_prevents_upload(self) -> None:
        name = "merman-android-v0.9.3.aar"
        self.seed_public([name])
        self.settings["download_error"] = name
        result, events = self.execute()
        self.assertNotEqual(result.returncode, 0)
        self.assertFalse(any(event["event"] == "uploaded" for event in events))
        self.assertFalse(any(event["event"] == "gh" and event["args"][:2] == ["release", "upload"]
                             for event in events))
        self.assertEqual(list(self.public.iterdir()), [self.public / name])
        self.assertEqual((self.public / name).read_bytes(), self.assets[name])

    def test_new_assets_require_matching_anonymous_readback(self) -> None:
        self.settings["download_corrupt"] = "merman-android-v0.9.3.aar"
        result, events = self.execute()
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(sum(event["event"] == "uploaded" for event in events), len(self.assets))
        self.assertEqual(sum(event["event"] == "curl" for event in events), len(self.assets))
        self.assert_public_bytes()


@unittest.skipUnless(BASH, "workflow execution requires Bash")
class AndroidTrustedSigningWorkflowTests(unittest.TestCase):
    def test_extracted_signing_steps_execute_trusted_tools_and_use_selected_facade_as_data(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            environment = isolated_shell_environment(
                root, ("python3", "sha256sum", "cut", "cat", "mkdir", "rm"),
            )
            log = root / "operators.jsonl"
            marker = root / "selected-script-executed"
            environment.update(
                ANDROID_TEST_LOG=str(log), ANDROID_TEST_MARKER=str(marker),
                ANDROID_HOME=str(root / "sdk"), GITHUB_ENV=str(root / "github-env"),
                MAVEN_GROUP="org.example", VERSION="0.9.3", SIGNING_KEY="fixture-key",
                SIGNING_KEY_ID="fixture-fingerprint", MERMAN_ANDROID_SIGNING_PASSWORD="fixture-password",
                GNUPGHOME=str(root / "gnupg"),
            )
            poison = """
                import os
                from pathlib import Path
                Path(os.environ["ANDROID_TEST_MARKER"]).write_text("selected code ran")
                raise SystemExit(97)
            """
            for prefix in (root, root / "selected-source"):
                for relative in ("scripts/android_package.py", "platforms/android/build-android.py"):
                    write_local_command(prefix / relative, poison)
            write_local_command(root / "trusted" / "platforms" / "android" / "build-android.py", """
                import json
                import os
                import sys
                assert sys.argv[1:] == ["--print-version", "ndk"]
                with open(os.environ["ANDROID_TEST_LOG"], "a") as log:
                    log.write(json.dumps({"operator": "trusted-ndk"}) + "\\n")
                print("27.2.12479018")
            """)
            write_local_command(root / "trusted" / "scripts" / "android_package.py", """
                import argparse
                import json
                import os
                from pathlib import Path
                parser = argparse.ArgumentParser()
                parser.add_argument("command", choices=["sign", "package"])
                parser.add_argument("--repository", type=Path, required=True)
                parser.add_argument("--android-root", type=Path, required=True)
                parser.add_argument("--group-id", required=True)
                parser.add_argument("--version", required=True)
                parser.add_argument("--android-ndk-home", required=True)
                parser.add_argument("--signing-key-id")
                parser.add_argument("--output-dir", type=Path)
                parser.add_argument("--central-bundle", action="store_true")
                args = parser.parse_args()
                assert args.repository.joinpath("frozen-publication").read_bytes() == b"qualified candidate"
                assert args.android_root == Path("selected-source/platforms/android")
                assert args.android_root.joinpath("facade-data").read_text() == "selected facade"
                assert args.android_ndk_home == os.environ["ANDROID_HOME"] + "/ndk/27.2.12479018"
                assert (args.group_id, args.version) == ("org.example", "0.9.3")
                if args.command == "sign":
                    assert args.signing_key_id == "fixture-fingerprint"
                    args.repository.joinpath("signed-by-trusted-fixture").touch()
                else:
                    assert args.central_bundle
                    assert args.repository.joinpath("signed-by-trusted-fixture").is_file()
                    assert args.output_dir == Path("target/signed-release-assets")
                with open(os.environ["ANDROID_TEST_LOG"], "a") as log:
                    log.write(json.dumps({"operator": "trusted-package", "command": args.command}) + "\\n")
            """)
            write_local_command(root / "bin" / "sdkmanager", """
                import json
                import os
                import sys
                assert sys.argv[1:] == ["--install", "ndk;27.2.12479018"]
                with open(os.environ["ANDROID_TEST_LOG"], "a") as log:
                    log.write(json.dumps({"operator": "fake-sdkmanager"}) + "\\n")
            """)
            write_local_command(root / "bin" / "gpg", """
                import json
                import os
                import sys
                assert sys.argv[1:] == ["--batch", "--import"]
                assert sys.stdin.read() == "fixture-key"
                with open(os.environ["ANDROID_TEST_LOG"], "a") as log:
                    log.write(json.dumps({"operator": "fake-key-import"}) + "\\n")
            """)
            (root / "selected-source" / "platforms" / "android" / "facade-data").write_text("selected facade")
            archive = root / "target" / "release-assets" / "merman-android-0.9.3-maven.zip"
            archive.parent.mkdir(parents=True)
            with zipfile.ZipFile(archive, "w") as bundle:
                bundle.writestr("frozen-publication", b"qualified candidate")
            archive.with_name(archive.name + ".sha256").write_text(hashlib.sha256(archive.read_bytes()).hexdigest())
            result = subprocess.run(
                [BASH, "-c", workflow_shell("central-bundle", "Install verification NDK")],
                cwd=root, env=environment, capture_output=True, text=True, timeout=15,
            )
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertEqual(
                Path(environment["GITHUB_ENV"]).read_text(),
                f"ANDROID_NDK_HOME={environment['ANDROID_HOME']}/ndk/27.2.12479018\n",
            )
            environment["ANDROID_NDK_HOME"] = environment["ANDROID_HOME"] + "/ndk/27.2.12479018"
            result = subprocess.run(
                [BASH, "-c", workflow_shell(
                    "central-bundle", "Sign frozen Maven publication and prepare standard Central bundle",
                )],
                cwd=root, env=environment, capture_output=True, text=True, timeout=15,
            )
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertEqual([json.loads(line) for line in log.read_text().splitlines()], [
                {"operator": "trusted-ndk"}, {"operator": "fake-sdkmanager"},
                {"operator": "fake-key-import"},
                {"operator": "trusted-package", "command": "sign"},
                {"operator": "trusted-package", "command": "package"},
            ])
            self.assertFalse(marker.exists(), "selected-source scripts must never execute")
            self.assertFalse(Path(environment["GNUPGHOME"]).exists())


if __name__ == "__main__":
    unittest.main()
