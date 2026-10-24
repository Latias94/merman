# Merman SDK archives

SDKs are packaged from existing artifacts; the packager never builds or installs tools.
The capability descriptor owns the six exact recipes: `sdk-native-standard`,
`sdk-native-math`, `sdk-native-full`, and their `sdk-wasi-*` counterparts.
Standard omits math and ELK, math adds formula rendering without ELK, and full includes
ELK. Native and WASI have different descriptor-owned output surfaces. The artifact's
runtime catalog, not its Cargo feature names, proves the surface.

## Native C SDK

Extract the archive and configure the included consumer against its installed prefix:

```text
cmake -S /path/to/sdk -B consumer-build -DCMAKE_PREFIX_PATH=/path/to/sdk
cmake --build consumer-build --config Release
ctest --test-dir consumer-build -C Release --output-on-failure
```

`Merman::Merman` imports the shared library; `Merman::Static` imports the static
library and platform system dependencies. `lib/pkgconfig/merman.pc` supports
`pkg-config --cflags --libs merman` and `--static`. On Windows, add the SDK's `bin`
directory to `PATH` before running a consumer; `merman` selects the DLL import
library, while `pkg-config --static --libs merman-static` selects the separate
MSVC static library. Compile and execute on the actual
target host; cross-compilation alone is not consumer evidence.

The native owner runs both shared and static installed consumers. Before catalog
capture or hashing, its macOS staging step rewrites the dylib identity to
`@rpath/libmerman_ffi.dylib` and applies an ad-hoc signature. The archive therefore
does not retain a producer's absolute build path; consumers choose their own runpath.

The GNU Linux owner currently builds and tests on Ubuntu 24.04 (glibc), not musl;
older glibc installations are not qualified by that host smoke. Windows libraries
use the MSVC ABI and require a compatible x64 Microsoft Visual C++ runtime for
shared linking. macOS qualification uses the declared macOS runners; inspect the
dylib's Mach-O minimum OS version before selecting an older deployment target.
Static consumers still need their platform toolchain and the system libraries
listed by `Merman::Static` / pkg-config. WASI is the platform-neutral option when
those native runtime assumptions do not fit the host.

The example discovers C ABI 3, queries the runtime catalog and renders SVG using
the appended memory-font constructor with bundled fonts. It can
write the **raw** artifact catalog with `merman-render --catalog-out catalog.json`.
Font bytes and their OFL license are in `fonts/`. The `merman-probe` target exercises
caller-supplied memory fonts and the shared SVG/PNG/math fixtures used by WASI.
The small `merman-render` example does not claim font-parity or formula qualification
by itself.

Compiled math capability does not override resource policy. Native C keeps the
existing conservative option defaults: pass
`{"resources":{"profile":"interactive"}}` when constructing a math engine and
impose host execution/memory limits. WASI selects interactive for omitted or empty
constructor options. An explicit constrained profile still rejects mathematical
rendering in either transport; selecting a math/full archive is not a policy bypass.

## WASI SDK

`lib/merman_wasi.wasm` uses the independently versioned WASI ABI 1 in
`contracts/abi/merman-wasi-v1.json`, not the C function-pointer table. The host must
implement the documented ABI and resource policy; `docs/WASI_ABI.md` in the WASI
archive contains allocation, ownership, request and descriptor examples. Execute the module with the
owner harness `platforms/wasi/smoke.py` before packaging. That harness owns the
WASI-standard-import-only check, formula SVG/PNG execution and constrained-resource
rejection; the packager does not parse WASM imports or claim execution evidence.

## Build and packaging handoff

Build and qualify one SDK locally with the same entry point used by CI:

```text
python3 scripts/sdk_qualify.py --profile sdk-wasi-math --target wasm32-wasip1 --work-dir target/sdk-check/wasi-math --output-dir target/sdk-check/archives
python3 scripts/sdk_qualify.py --profile sdk-native-math --target aarch64-apple-darwin --work-dir target/sdk-check/native-math --output-dir target/sdk-check/archives --wasi-dir target/sdk-check/wasi-math/wasi-smoke
```

Choose a native target matching `rustc -vV` on the machine running the command.
Use `standard`, `math`, or `full` in either profile name. Run different profiles
serially when reusing `CARGO_TARGET_DIR`; each invocation stages its libraries and
finishes qualification and packaging before returning. The optional native
`--wasi-dir` compares against the existing two-runtime outputs for that same flavor.
The independent CI parity job performs that comparison across matrix jobs.

The command needs Python 3.11+, the repository Rust toolchain with the selected
target installed, cargo-about 0.9.2 on `PATH`, and the pinned host dependencies in
`platforms/wasi/requirements.txt`. WASI additionally needs the Go version in
`platforms/wasi/README.md`; native qualification needs CMake and the platform's C
toolchain. macOS uses its `install_name_tool`, `codesign` and `otool`. The command
does not install tools. Run it with the Python environment containing those host
dependencies; it invokes that same interpreter for all Python helpers.

`--work-dir` must not already exist. Failed runs retain their files for diagnosis;
retry with a new directory. Existing archives are never replaced. An optional
`--source-commit` must equal the checked-out HEAD. A successful run writes
`qualification.json`, exact catalogs and legal report beside the staged libraries;
`evidence/` contains only the host-smoke files for CI upload. Native consumers use
the same generated headers, CMake/pkg-config files and installed library layout as
the archive. The extracted archive's shared/static consumers must pass before the
command reports success. No successful local run publishes an archive.

Print the descriptor-owned build command without executing it:

```text
python3 scripts/sdk_archive.py build-command --profile sdk-native-standard --target x86_64-unknown-linux-gnu
python3 scripts/sdk_archive.py catalog-sidecar --profile sdk-native-standard --target x86_64-unknown-linux-gnu --artifact-dir target/sdk-input --catalog-in target/sdk-input/raw-catalog.json --output target/sdk-input/runtime-catalog.json
python3 scripts/sdk_archive.py package --profile sdk-native-standard --target x86_64-unknown-linux-gnu --artifact-dir target/sdk-input --output-dir target/sdk-output --runtime-catalog target/sdk-input/runtime-catalog.json
```

Use Python 3.11 or newer. `--descriptor` and `--repo-root` are available for fixture
tests. An explicit `--source-commit` must equal the checked-out Git HEAD. Release
trust is established by the existing workflow's immutable-source and release-tag
checks, not by a new receipt or source-code inference engine.

The build/host-smoke owner supplies `runtime-catalog.json` as this small sidecar:

```json
{
  "schema_version": 1,
  "profile": "sdk-native-standard",
  "target": "x86_64-unknown-linux-gnu",
  "source_commit": "<exact 40-character HEAD commit>",
  "artifacts": {
    "libmerman_ffi.a": "<sha256>",
    "libmerman_ffi.so": "<sha256>"
  },
  "runtime_catalog": "<replace with the actual JSON object returned by the artifact>"
}
```

Include exactly the target's libraries in `artifacts`: Linux `.a`/`.so`, macOS
`.a`/`.dylib`, Windows `merman_ffi.lib`/`merman_ffi.dll`/`merman_ffi.dll.lib`, or
WASI `merman_wasi.wasm`. Digests bind the catalog to those packaged bytes; they do
not replace the trusted host smoke. Native legal scopes are the profile IDs
`sdk-native-standard`, `sdk-native-math`, and `sdk-native-full`, including the
native-only ASCII notices. WASI selects `sdk-standard`, `sdk-math`, or `sdk-full` in
`docs/release/THIRD_PARTY_COMPONENTS.json`, reusing the existing legal generator's
scope resolver and notice formatter. Canonical license bytes come only from those
selected components. Supply `--rust-license-report` (default:
`<artifact-dir>/rust-cargo-dependencies.json`) with the exact profile report produced
by `generate_report_for_profile` in `scripts/generate-rust-license-report.py`.
The complete report, including normal dependency license texts such as MPL, is
retained. Packaging never invokes cargo-about and never falls back to the broad
repository legal bundle. The font manifest's listed files and checksums are verified.

Unix archives use a single named wrapper and `.tar.xz`; Windows archives are flat
`.zip`, matching the existing release convention. For example,
`merman-sdk-native-standard-x86_64-unknown-linux-gnu.tar.xz` carries its version in
the manifest and release tag, like other cargo-dist assets. Each archive has a `.sha256`
sidecar. `manifest.json` records version, commit, target, profile, recipe, actual
capabilities, options schemas and transport ABI. Its file hashes cover every other
payload file; the archive checksum covers the manifest itself without self-reference.
Only explicit public inputs are copied; build directories and test fixtures are not.
`SOURCE_AVAILABILITY.md` records the exact commit and corresponding source paths,
including the Rust ELK implementation for full profiles.

`release-sdk.yml` reuses the existing release trust and artifact conventions; it
does not add another release framework. Its reusable and manual interfaces take
only `source_ref` and `release_tag`. Preflight passes its
pinned source SHA as `source_ref` and `v<version>` as `release_tag`; no manually
uploaded artifacts or external producer are required.

The 18 matrix jobs each own one profile/target-specific `CARGO_TARGET_DIR` and stage
only that just-built library set. They call `scripts/sdk_qualify.py`, which executes
the exact locked descriptor command, runs real host smokes, writes the actual catalog
sidecar, generates the exact cargo-about report, packages and verifies the same
bytes, then runs extracted native consumers. No job rebuilds another profile before
catalog capture and archiving have finished. The packager itself never runs Cargo;
the workflow owns source trust, tool installation, scheduling and artifact upload.

The native lanes compile and run the shared memory-font probe on five actual
target hosts and build the installed consumer against the extracted archive.
The WASI lanes call `platforms/wasi/smoke.py --module PATH --flavor FLAVOR
--catalog-out PATH --output-dir PATH`, which serially runs both qualified real
runtimes against the same module and owns its standard-import-only and resource
checks. Runtime dependencies are pinned by that host directory. The sidecar helper
validates the raw catalog before binding it to HEAD and the product SHA-256 values;
it does not synthesize a capability list or substitute for host execution.

The producer has only read permissions and never publishes. Its success requires
all native/WASI host qualifications and decoded PNG parity; its outputs are
`source_sha` and `version`. It does not request a write-permission envelope from
the calling preflight workflow.

## Explicit publication

The separate, manual-only `release-sdk-publish.yml` takes `build_run_id` and
`release_tag`. The selected run must have succeeded in this repository under
`release-sdk.yml` or `release-preflight.yml`, with its GitHub run HEAD equal to the
existing release tag's exact commit. A qualification run dispatched at a different
workflow HEAD cannot be used for publication, even if it selected an older source
SHA; rerun qualification at the tag commit instead.

The publisher never rebuilds. It downloads only that run's `sdk-package-*` assets,
requires the exact 18 archives and 18 checksums, and validates every manifest's
source/version/profile/target, catalog, payload file set and per-file SHA-256. The
`verify` command reuses the existing bounded archive extractor and persists only
checksum-bound snapshots with `--verified-output-dir`; these exact bytes are
attested and uploaded. Only its publication job has write permissions, behind the
existing `github-release` environment. It rechecks the tag and successful run,
uses official build attestations, and uploads only to an existing release, without
creating releases or replacing assets. No release is triggered by packaging.

```text
python3 scripts/sdk_archive.py verify --profile sdk-native-standard --target x86_64-unknown-linux-gnu --archive merman-sdk-native-standard-x86_64-unknown-linux-gnu.tar.xz --checksum merman-sdk-native-standard-x86_64-unknown-linux-gnu.tar.xz.sha256 --source-commit <tag-commit> --version <version> --verified-output-dir <existing-empty-directory>
```
