# Android Maven Delivery

The owner is `.github/workflows/release-android.yml`. It builds the selected source once,
qualifies the extracted Maven repository in an external app, and retains the AAR and repository
ZIP as GitHub Actions artifacts. These candidates are not public releases.

## Candidate Qualification

Select the checkout's projected version and an explicit Maven group:

```sh
gh workflow run release-android.yml \
  -f release_tag=v0.9.3 -f source_ref=<40-character-source-commit> \
  -f maven_group=<selected-group> -f publish=false -f prepare_central_bundle=false
```

This command is an operator recipe, not a record of a dispatched run. `source_ref` accepts
`main`, a full commit, or the matching version tag. The owner resolves it to an immutable commit
and tree before building. `publish=false` skips the GitHub Release write job. With
`prepare_central_bundle=false`, the owner also skips protected signing and reads no signing
credentials. It never creates tags, GitHub Releases, namespaces or Central deployments.

Local staging and packaging use existing Gradle publication tasks and the existing verifier:

```sh
platforms/android/gradlew -p platforms/android \
  -PmermanMavenGroup=<selected-group> \
  publishReleasePublicationToLocalStagingRepository --max-workers=2
python3 scripts/verify-platform-bindings.py --verify-android-maven \
  --android-maven-group <selected-group> --android-maven-version 0.9.3 \
  --android-ndk-home <pinned-ndk>
python3 scripts/android_package.py package --repository platforms/android/build/repo \
  --group-id <selected-group> --version 0.9.3 --output-dir target/android-delivery
```

`package` preserves all selected-version bytes and sidecars and checks them before emitting
`merman-android-0.9.3-maven.zip` and its SHA-256. Repository `maven-metadata.xml` is narrowed to
the selected version and receives fresh MD5, SHA-1, SHA-256 and SHA-512 checksums. A missing or
wrong selected version fails; additional staged versions are not copied. The ZIP has no wrapper
directory above the Maven group path.

## External App And 16 KB Qualification

The checked-in `consumer/` is an independent Android application. The qualifier copies it,
the pinned tool catalog and public smoke into a new directory outside the source checkout;
it includes no Merman project dependency, Rust checkout or loose AAR dependency. The extracted
repository supplies only the exact Merman module; Google and Central supply normal build and
transitive dependencies. Gradle independently resolves the POM and Gradle module metadata,
including the transitive Kotlin runtime. The resulting APK packages both supported ABI slices.

With the SDK, pinned NDK and JDK already installed:

```sh
python3 scripts/android_package.py qualify \
  --archive target/android-delivery/merman-android-0.9.3-maven.zip \
  --group-id <selected-group> --version 0.9.3 \
  --work-dir <new-external-directory> --android-ndk-home <pinned-ndk> \
  --zipalign <sdk>/build-tools/35.0.0/zipalign --connected
```

Omit `--connected` only for build/ELF/APK qualification without runtime proof. The command never
invokes Cargo or rebuilds the library. `--connected` executes the shared public smoke through
the resolved AAR: ordinary SVG, outlined math with positive finite formula viewports, ASCII,
analysis, constructor services, theme errors, constrained math rejection, and missing codecs.

The same owner runs `testDebugUnitTest`, checks both AAR ABI slices with NDK `llvm-objdump -p`,
builds the external APK with uncompressed libraries, checks all its shared libraries, and invokes
`zipalign -v -c -P 16 4`. Its API 35 `google_apis_ps16k` x86-64 emulator must report `16384` from
`adb shell getconf PAGE_SIZE` before running JNI tests. This proves runtime execution on x86-64;
ARM64 receives delivered ELF and APK checks, not an ARM64 device claim. NDK r30 and AGP 9.4
are the current pinned owners of alignment defaults. Actual produced bytes are checked, rather
than inferring compatibility from those versions. See the
[Android 16 KB qualification procedure](https://developer.android.com/guide/practices/page-sizes).

The alignment-only entry point for already produced archives is:

```sh
python3 scripts/verify-platform-bindings.py --verify-android-16kb <selected.aar> \
  --android-ndk-home <pinned-ndk>
python3 scripts/verify-platform-bindings.py --verify-android-16kb <consumer.apk> \
  --android-ndk-home <pinned-ndk> --android-zipalign <sdk>/build-tools/35.0.0/zipalign
```

## Signing And Standard Central Handoff

The namespace is a deployment input. Nothing in this repository establishes ownership of
`io.merman` or any other Central namespace. Before signing for Central, its actual owner must
verify the chosen namespace in Portal and configure the protected `maven-central` environment:

- Variable `ANDROID_MAVEN_VERIFIED_GROUP`: the exact verified group authorized for this owner.
- Variable `ANDROID_MAVEN_SIGNING_KEY_ID`: the selected GPG signing key fingerprint.
- Secret `ANDROID_MAVEN_SIGNING_KEY`: the matching armored private key.
- Secret `ANDROID_MAVEN_SIGNING_PASSWORD`: its passphrase, if needed.

The signing job requires the requested group to match the configured verified group. It signs
the already qualified publication without rebuilding it. `prepare_central_bundle=true` invokes
this protected, read-only job; `publish=false` still leaves every registry and GitHub Release
unchanged. The temporary GPG home is removed after the job. The signing key's public half must
be published and discoverable according to Central's GPG requirements.

Local signing can use an existing GPG home and `MERMAN_ANDROID_SIGNING_PASSWORD` without
putting a passphrase in command arguments:

```sh
python3 scripts/android_package.py sign --repository platforms/android/build/repo \
  --group-id <verified-group> --version 0.9.3 --signing-key-id <fingerprint> \
  --android-ndk-home <pinned-ndk>
python3 scripts/android_package.py package --repository platforms/android/build/repo \
  --group-id <verified-group> --version 0.9.3 --output-dir target/android-delivery \
  --android-ndk-home <pinned-ndk> --central-bundle
```

`--central-bundle` requires GPG-verifiable detached signatures and MD5/SHA-1 sidecars for the
AAR, POM, module metadata, sources and javadoc. It emits both the complete repository ZIP and
`merman-android-0.9.3-central.zip`, using the
[official Central upload format](https://central.sonatype.org/publish/publish-portal-upload/):
Maven-relative version directories, signatures and checksums. Central's aggregate metadata is
excluded from that upload bundle. No Portal client or registry upload is implemented here.

After separate publication authorization, the operator reviews the signed bundle and submits
it through Portal's standard upload flow under the verified namespace. Portal validation and
promotion, followed by resolution of the exact coordinate through `mavenCentral()` in an external
app and comparison of downloaded checksums/signatures, are required public rollout evidence.
Those operations have not been performed by preparing this change.

## GitHub Release Assets

`publish=true` requires `source_ref` to name the matching immutable release tag and
`prepare_central_bundle=true`. Only then does the protected `github-release` job upload the
qualified AAR, signed repository ZIP, Central bundle and SHA-256 sidecars. It requires an existing
GitHub Release, rechecks the tag's remote commit at the mutation boundary, refuses to replace
existing assets, and compares GitHub's public asset digests after upload. It does not upload to
Maven Central. Keep workflow revision, selected source commit/tree and producing run/attempt in
the eventual release record; a workflow definition is not completed qualification evidence.
