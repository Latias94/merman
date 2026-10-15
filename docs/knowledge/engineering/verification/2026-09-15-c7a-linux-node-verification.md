# C7a installed Linux Node candidates

Date: 2026-09-15. Source: `a6b9f9ae57b627a92fe2940454a2ac6b6de5a022`.

Both targets were built from independent clean clones using the repository's GNU and musl
Dockerfiles, `build-candidate.mjs --candidate napi`, package assembly and verification, and the
installed-package smoke from Release Preflight. Cargo builds ran serially with one build job.
This local x86-64 Linux execution used Rosetta inside a dedicated Colima VM on macOS ARM64.
It is an installed-artifact observation for the named source, not a current-HEAD all-platform
preflight or a release. The package version remains the unpublished checkout's `0.8.0-alpha.6`;
these archives do not replace registry artifacts with that version.

## Environment and execution

| Target | Runtime | Toolchain | Native build duration | Container peak memory |
| --- | --- | --- | --- | --- |
| `linux-x64-gnu` | Debian Bullseye, actual glibc 2.31 | Rust 1.95.0, Node 24.13.1, npm 11.8.0 | 11m 15s | 3,091,771,392 bytes |
| `linux-x64-musl` | Alpine 3.22.3, musl 1.2.5 | Rust 1.95.0, Node 24.13.1, npm 11.8.0 | 15m 11s | 4,056,772,608 bytes |

These are container cgroup peaks and observed build times, not a renderer memory or latency
benchmark. The VM had 2 CPUs and 8 GiB RAM; the task containers had a 7 GiB limit. GNU execution
checked the actual runtime glibc version against 2.31. Both source clones remained clean after
building and smoking the installed packages.

The pinned container lanes deliberately support npm 11 array metadata through the existing
explicit adapter. The host Node/Web preflight lanes need npm 12.0.2 and are a separate toolchain
contract; these container results do not validate that host setup.

## Installed consumer results

For each target, the loader and platform tarballs were installed with `npm install --ignore-scripts`
into a fresh project before running `smoke-installed-package.mjs` against its public entrypoint.
Both passed with 10,492 SVG bytes and identical theme-authoring checks:

- 2 shared materialization vectors and 2 catalog checks;
- 3 family-isolation checks, 1 rule override and 1 cold complete-spec check;
- 3 preset exports and 44 support queries;
- 6 authoring diagnostics and 2 resource-limit checks;
- 23 JSON operations and 23 SVG renders.

The GNU installed runtime also passed the compiled-theme example added to the Node README after
this source revision. This verifies the example against that runtime; it does not imply that the
new README was present in these older tarballs.

## Artifact identity

| Artifact | Bytes | SHA-256 |
| --- | ---: | --- |
| Common loader tarball | 112,781 | `3932ac23ffe280fc1d3730f654165d01cd02fc15c8a37a2cca195c9e81244b95` |
| GNU platform tarball | 13,305,652 | `c8907e4ecff6cd9765c70049ffe63a651bf6cdc0e4f26bb5b1b7716505e88aca` |
| GNU `merman.node` | 35,172,592 | `0f97e8d482956b74a43f785f72892640c544e6e9276f4b68031a5adb2e08af35` |
| musl platform tarball | 13,459,834 | `2c136fe4a73bfdf065df72aad2423f07a42cdd8ec54857538f557cb4b234ec65` |
| musl `merman.node` | 34,545,176 | `418d54b3ebeb7ad87191745ec0334f8514765a58504f3eb9ff4ce48cb6c3420d` |

Local artifacts and owner build receipts are retained under
`/tmp/merman-c7a-a6b9f9ae5-node-{gnu,musl}-artifacts/`. Build and installed-consumer logs use
`/tmp/merman-c7a-a6b9f9ae5-node-{gnu,musl}-{build,installed}.log`.

## Remaining gate

Changes after the named source, including packaged documentation and host npm setup, require
fresh owner-generated receipts and package assembly before final candidate acceptance. No public
catalog cell was promoted by these smokes. Other hosts, the full candidate profile matrix and the
formal release/version/contract freeze remain governed by the C7a replan.
