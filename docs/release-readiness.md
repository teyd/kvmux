# Signed release readiness

Status: **not ready to publish**. This checklist tracks issue #15; it does not
implement packaging or authorize a release. The embedded publisher key is a
fail-closed placeholder. No artifact can currently be signed for that key.

## Maintainer decisions and access

- Review the pinned fastframe dependency source and PR #9 before merging.
- Provision the real Ed25519 publisher key **outside the repository and agent
  session**. Keep the private key out of commits, logs, issues and PR comments.
  Verify a securely retained backup and document recovery/rotation ownership.
- Share only the public key for a reviewed replacement of the placeholder.
- Configure a GitHub Environment with required maintainer approval for releases
  and protect the signing credential. Neither untrusted PR code nor workflows
  running on PR-controlled refs may obtain it.
- Review changes to `.github/`, release scripts, dependencies, `Cargo.lock`, and
  updater code before authorizing implementation. These paths are protected by
  `AGENTS.md`; this document intentionally does not change them.
- Define the supported portable installation location and retention policy for
  immutable releases. Authenticode is separate: do not call Windows artifacts
  Authenticode-signed merely because the update manifest uses Ed25519.

## Artifact contract

For version `X.Y.Z`, use the pinned fastframe target/name contract:

- `kvmux-vX.Y.Z-x86_64-pc-windows-msvc.zip`
- `kvmux-vX.Y.Z-x86_64-unknown-linux-gnu.tar.gz`

Validate the archive root/layout against the pinned extractor, not just the
asset name. Ship the stable `kvmux.exe` or `kvmux` executable and the marker
`kvmux-portable.txt` with exact contents `kvmux-portable-v1\n`. Include
dependency license texts and applicable notices; the application's MIT license
does not cover all bundled dependencies.

Publish `checksums.txt` and the raw Ed25519 manifest signature in the format
required by the pinned verifier. Sign the exact manifest bytes after computing
hashes of the final archives. Verify the public-key/manifest/signature/artifact
combination independently before publishing. Confirm the signature asset name
and manifest syntax directly from the pinned source when implementing the
workflow; never silently fall back to unsigned checksums.

## Required verification matrix

Run these checks on both Windows x86_64 and Linux x86_64, using isolated
**installed copies** with disposable configuration and a test publisher key.
Test keys must never become production publisher credentials.

| Case | Required evidence |
| --- | --- |
| Signed N to N+1 | Selected version and target remain pinned; signature and hash verification precede any installation/execution. |
| Explicit consent | Check/download cannot install or restart; installation/restart requires a clearly labeled user action. |
| Restart success | New binary starts, renders the expected application, acknowledges startup, and preserves config. |
| Restart failure | A real failed/unacknowledged startup rolls back, leaving the previous binary launchable. |
| Invalid signature/hash | Rejected before installation; original binary/config unchanged. |
| Truncated download | Rejected; original installation stays launchable. |
| Offline check | Actionable error; application continues operating. |
| Permission failure | Visible error without a false success state or damaged installation. |
| Interrupted restart | Recovery completes or preserves a launchable previous installation. |
| Downgrade/unsigned payload | Refused without executing the payload. |
| Package-manager-owned copy | Does not replace itself; points to package-manager updates. |
| Optional automatic checks/downloads | Configurable; disabled setting is honored and never implies consent to install. |

Record OS versions, release versions, artifact hashes and test results separately
from CI results. Unit tests of the dependency are not proof that kvmux's
packaged handoff, startup acknowledgement or rollback works.

## Completion gate

Issue #15 stays open until reviewed workflows, real publisher-key provisioning,
license-complete artifacts and the full runtime matrix are complete. This
readiness document does not satisfy those criteria. Do not publish a release
from the current placeholder-key build. macOS distribution, additional
architectures, DEB/RPM packaging and Windows Authenticode remain separate work.
