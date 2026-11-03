# Node 0.9.3 Publication Incident

Status: complete. The approved Windows recovery and all seven original Node packages are
published and independently reconciled at `0.9.3`.
Observed on 2026-11-03 (Asia/Shanghai).

The stable Web group, Python wheels, Flutter package and Android AAR are delivered. The original
Node attempt-2 report retains its uncertain Windows outcome unchanged; the approved later
Windows send is reconciled below. See the [publication ledger](PUBLISH_ORDER.md#093-binding-backfill).

## Retained Evidence

- Source: `3b915f69c02d55b873d252cf6c8a6048e7f3427d`, tree
  `fc57c459736f1857b5bdc6dd148d9bb0ad645e9c`, immutable `v0.9.3`.
- Original [Node run 41503526369](https://github.com/Latias94/merman/actions/runs/41503526369).
  Artifact `13173510468`, `merman-node-npm-package-group`, retains all seven tarballs;
  GitHub ZIP digest `sha256:6520beb7e91a29e2e94aab24592329a7ea5b8239f3d34e1fcd2f541e2db452e0`.
  The canonical manifest SHA-256 is
  `3dd94dbe9444c23dc42436cbb18029f37beb2316c9e9039bf7a67e44b286afc8`.
- Attempt 1 accepted macOS ARM64, then timed out observing registry visibility. Its exact
  integrity became visible before the failed publish job was rerun. Attempt 2 retained the
  original group, skipped that package and published macOS x64, GNU Linux x64 and musl Linux x64.
- Attempt 2's Windows command failed with `TLOG_CREATE_ENTRY_ERROR` and a Rekor HTTP 409.
  Its original reconciliation report has SHA-256
  `af9657804c7c1c3560913574b73a27dd1f002a838b9760abe0e81b875d0d7bb3`.
  That report records Windows as `upload_outcome_unknown`, with WASM and the loader `not_attempted`.
  Preserve this report unchanged.

The pinned npm `12.2.1` awaits provenance generation before its registry PUT. The error is
consistent with the known [npm provenance conflict issue](https://github.com/npm/cli/issues/10078).
This source-backed explanation does not satisfy the current recovery policy's requirement for
complete job history proving that the actual upload was skipped. It does not automatically
clear the recorded uncertainty. See [registry recovery](RELEASING.md#registry-propagation-and-recovery).

## Existing Windows Provenance

The [public Rekor entry](https://rekor.sigstore.dev/api/v1/log/entries/108e9186e8c5677a0ee1960a069afc42bdf8551009e0812affff1f2604fd81520001bbc37fa18f3b)
contains the original certificate, signature, payload hash and inclusion evidence. It was
integrated at `2026-11-02T19:54:15Z`, log index `3648312221`.

The npm SLSA statement is deterministic for a given subject and producing invocation. Replacing
the subject of the same attempt's published musl provenance with the original Windows name and
SHA-512 reconstructs payload bytes whose SHA-256 exactly matches Rekor:
`8cf7bf69371a2828a504c802f90d700efdbae9bc88b6ab695d4c6f81a1429be8`.
The retained [Sigstore bundle](incidents/node-093-windows.sigstore.json) combines that payload
with the original public certificate, DSSE signature and Rekor evidence. It is not a new
signature or publication receipt and contains no private key or credential.

The pinned npm installation's official `sigstore.verify` passes certificate chain, DSSE,
transparency-log and identity verification for this exact bundle:

- Subject: `pkg:npm/%40mermanjs/node-win32-x64-msvc@0.9.3`.
- Issuer: `https://token.actions.githubusercontent.com`.
- Identity: `https://github.com/Latias94/merman/.github/workflows/release-node.yml@refs/tags/v0.9.3`.
- Source: the immutable stable commit above.
- Invocation: `https://github.com/Latias94/merman/actions/runs/41503526369/attempts/2`.
- Bundle file SHA-256: `7d377383905b41aac589109843230609e00e46a89ecfdbafe4b5abfc242957ec`.

An additional in-process check exercises the actual pinned `libnpmpublish` with a mocked npm
registry. It verifies and attaches the original tarball and bundle, never calls the provenance
generator, and rejects a mismatched subject digest before any PUT. That check uses the official
trust-root cache from the independent verification above and performs no real registry writes.

The original Windows tarball is 9,797,506 bytes:

```text
mermanjs-node-win32-x64-msvc-0.9.3.tgz
SHA-256: 1429c00b9b5529514ac0cc4316acafca391e642908e6097158e8faa2d3c0652d
Integrity: sha512-UsUD3zbPv0cy4usMFMFFfQFhPY7An7meHj1YZLPEqLnx6HBG1BJ/ONvy5nxSZ9Ai/Oj9JozKI0VS3SYTl8geSQ==
```

## Approved One-Time Recovery

The maintainer explicitly approved this incident recovery with “可以重试” on 2026-11-03.
The original report remains unchanged; approval is limited to
this exact package, version, artifact and bundle. It is not an automatic retry rule for TLOG
errors or other unknown outcomes.

1. Download the original group, verify it with the stable descriptor and verifier, and retain
   both original attempt reports. Recheck all seven registry versions and tags. Keep every
   accepted matching version. Stop on any conflicting bytes or tags.
2. Verify the existing bundle and its exact subject, source and invocation again. If Windows
   is already public with matching bytes and provenance, observe it rather than sending.
3. If the approved Windows member is still absent, use a reviewed Actions execution of the
   existing `release-node.yml` publisher identity and its `npm` environment. Record upload
   intent before the one allowed send. Publish the original Windows tarball with the original
   verified provenance file:

   ```text
   npm publish mermanjs-node-win32-x64-msvc-0.9.3.tgz \
     --ignore-scripts --access public --tag latest \
     --registry=https://registry.npmjs.org \
     --provenance-file=docs/release/incidents/node-093-windows.sigstore.json
   ```

   npm `12.2.1` gives an explicit provenance file precedence over OIDC's automatic provenance
   generation. Its publisher validates the subject/digest and calls the official Sigstore
   verifier before attaching the bundle. This retains provenance without creating another
   Rekor entry. Do not add an explicit `--provenance` flag alongside the file.
4. Independently confirm Windows integrity, `latest` and the original invocation's registry
   provenance. If the new send has an uncertain result, observe only; the single-send decision
   does not authorize another send.
5. Resume the original failed publishing job with its latest untouched recovery report. Its
   ordinary reconciliation can then observe the matching Windows member and publish the
   still-unattempted WASM and loader from the original group. Reconcile all seven public
   tarballs, provenance and complete tag maps before declaring the group delivered.

No product rebuild, new version, tag move, accepted-package replacement, provenance suppression
or generic uncertainty override is part of this recovery. A workflow-only execution revision
must be recorded separately from the stable product source and the original proof's attempt.

The separately reviewed [incident execution](https://github.com/Latias94/merman/actions/runs/41558175355)
uses workflow revision `3a5179d92bd75aa4ad82b47bc806a68ebf53fe3e` on
`release/node-093-windows-recovery-20261103`; that temporary sender is not intended for merging
into the normal release workflow. It pins the original seven-member artifact, stable owner,
report and proof. A repository-visible send-intent artifact is uploaded before the npm command.
A prior intent, including one from a different run, or a rerun selects observation only.
The normal group publisher is excluded during this incident dispatch. Independent review,
103 owner protocol/security checks, seven offline incident-preparation checks, actionlint,
zizmor and the official Sigstore identity verification passed before dispatch.

## Execution And Public Reconciliation

[Incident run 41558175355](https://github.com/Latias94/merman/actions/runs/41558175355), attempt 1,
completed successfully. It uploaded `merman-node-093-windows-single-send-intent` before the single
npm command; the original report and package-group artifact were not changed or rebuilt.

At `2026-11-03T01:20:25Z`, independent public-registry reconciliation confirmed the Windows
tarball is byte-identical to the original artifact, its integrity is the value above, and
`latest=0.9.3`. The old `alpha` and staging tags are unchanged. The public provenance retains
the original statement, certificate, DSSE signature, Rekor inclusion evidence and invocation
`41503526369/attempts/2`; npm's response adds only an empty optional
`timestampVerificationData: null`. The official Sigstore verifier passes against the public
bundle with the exact original workflow identity and issuer.

Only after that reconciliation was the original failed group-publisher job resumed as attempt 3.
It restored the untouched attempt-2 report, observed all five accepted native members and
published only the originally unattempted WASM and loader. The original run completed successfully
with `status=released`. Final report artifact `13196367052` has ZIP digest
`sha256:6a90be693677a048a34569ab3c1d1f1039a5691ad88fd425f4febaf9ed2ab059`;
the extracted report SHA-256 is
`c49483765e025a8b5e133083a970708daf3df4d1fe5f5a7d79044d14e6e0daa9`.
The original manifest identity is unchanged.

Independent reconciliation confirms all seven public tarballs are byte-identical to the original
group, their integrities and complete tag maps match, and each registry provenance binds the
stable source and the corresponding original invocation:

| Members | Provenance attempt in run `41503526369` |
| --- | --- |
| macOS ARM64 | 1 |
| macOS x64, GNU Linux x64, musl Linux x64, Windows x64 | 2 |
| Node-WASM and loader | 3 |

The official npm `12.2.1` consumer audit verifies all seven registry signatures and attestations.
A clean ordinary install runs the native macOS ARM64 and explicit Node-WASM transports through
the stable installed-package SVG and Iconify lifecycle smoke. No additional Windows send,
product rebuild, source change or tag move was needed.
