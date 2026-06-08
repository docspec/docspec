# Releasing DocSpec

This document is the maintainer runbook for releasing DocSpec. It covers the full release lifecycle: from conventional commit on `main` to published crates, signed binaries, and Docker images.

## Overview

DocSpec uses a unified ecosystem version across all 12 crates. Every release is tagged `vX.Y.Z` at the workspace level, and all publishable crates carry that same version. [release-plz](https://release-plz.dev) is the canonical release driver: it reads Conventional Commits, opens a release PR, and on merge handles tagging and publishing.

Crates publish to crates.io via Trusted Publishing (OIDC). No long-lived API tokens are stored in repository secrets. Binaries for `docspec-cli` are built and distributed by [cargo-dist](https://opensource.axo.dev/cargo-dist/). Docker images are built and pushed by `release.yml`. All artifacts carry SLSA L2 build provenance attestations and cosign keyless signatures.

The release process is designed to be boring. Most releases require a maintainer to review and merge a single PR. The automation handles the rest.

## Versioning Policy

DocSpec uses one ecosystem version across all 12 crates. The version lives in `[workspace.package].version` in the root `Cargo.toml`, and every crate inherits it via `version.workspace = true`. Internal crate-to-crate dependencies are declared in `[workspace.dependencies]` and track the workspace version exactly.

SemVer applies at the ecosystem level. A breaking change in any single crate triggers a major bump for the entire ecosystem. This is intentional: users who depend on the `docspec` facade crate get a clear signal when anything in the stack changed incompatibly, even if the specific sub-crate they use was untouched.

The tradeoff is that ecosystem coherence takes priority over per-crate precision. A breaking change in `docspec-docx-reader` bumps the major version even for users who only use `docspec-markdown-reader`. We accept this cost because the alternative, independent per-crate versioning, creates a combinatorial compatibility matrix that nobody can reason about.

In practice: `feat` commits trigger a minor bump, `fix` commits trigger a patch bump, and any commit with `BREAKING CHANGE` in the footer triggers a major bump. release-plz reads these signals and proposes the correct version in the release PR.

## The Release Cycle (Happy Path)

The typical release flows like this:

1. A developer lands a commit on `main` with a conventional commit message.
2. release-plz detects the new commit and opens a release PR. The PR contains the proposed version bump and an updated `CHANGELOG.md`.
3. A maintainer reviews the release PR (see the next section for what to check).
4. The maintainer merges the release PR.
5. The `release-plz-release.yml` workflow runs release-plz, which tags `vX.Y.Z` and publishes all 11 publishable crates to crates.io in topological order, with a 30-minute retry wrapper to handle index propagation lag.
6. The `release.yml` workflow fires on the new tag, builds `docspec-cli` binaries via cargo-dist, builds and pushes the Docker image, attaches everything to the GitHub Release, and generates SLSA attestations and cosign signatures.

From merge to fully published artifacts typically takes 30 to 60 minutes, depending on crates.io index propagation.

## The Release PR: What to Review

Before merging a release PR, check:

- **Version bump matches commit content.** A `feat` commit should produce a minor bump. A `fix` should produce a patch. If the bump looks wrong, check whether release-plz picked up a commit it shouldn't have, or missed one.
- **CHANGELOG diff is reasonable.** Skim the generated entries. They should match what actually landed. Entries that look garbled usually mean a commit message didn't follow the conventional format.
- **No unexpected `Cargo.toml` mass-edits.** release-plz should only touch `version` fields and `[workspace.dependencies]` version pins. If you see other fields changing, investigate before merging.
- **No `[workspace.dependencies]` drift.** All internal dependency version pins should match the new workspace version exactly.
- **`docspec-wasm` shows a version bump but no publish line.** The wasm crate is `publish = false`. release-plz will still bump its version in `Cargo.toml` to keep it in sync, but it won't attempt to publish it. Confirm this is the case.

If anything looks off, close the release PR, fix the underlying issue (usually a malformed commit message or a release-plz config problem), and let release-plz open a fresh PR.

## Trusted Publishing Setup (One-Time Per Crate)

Trusted Publishing lets crates.io verify the publish request came from a specific GitHub Actions workflow, with no stored API token. You configure it once per crate on crates.io, and then the workflow just works.

For each of the 11 publishable crates, add two Trusted Publishing entries:

1. Log in to [crates.io](https://crates.io) and navigate to the crate's page.
2. Go to **Settings** > **Trusted Publishing**.
3. Click **Add a publisher**.
4. Fill in the form for the automated release workflow:
   - **Repository owner**: `docspec`
   - **Repository name**: `docspec`
   - **Workflow filename**: `release-plz-release.yml`
   - **Environment**: `release`
5. Save.
6. Click **Add a publisher** again.
7. Fill in the form for the manual emergency publish workflow:
   - **Repository owner**: `docspec`
   - **Repository name**: `docspec`
   - **Workflow filename**: `publish-crate-manual.yml`
   - **Environment**: `release`
8. Save.

The 11 publishable crates are: `docspec-core`, `docspec-json`, `docspec-markdown-reader`, `docspec-html-reader`, `docspec-docx-reader`, `docspec-blocknote-writer`, `docspec-oxa-writer`, `docspec-html-writer`, `docspec`, `docspec-cli`, and `docspec-http`.

`docspec-wasm` is **not** configured for Trusted Publishing. It has `publish = false` and never reaches crates.io.

## Trusted Publishing Migration from publish-crates.yml

If Trusted Publishing was previously configured pointing to the old `publish-crates.yml` workflow filename, you need to update each crate's TP config to trust both current publishing workflows.

On crates.io, go to each crate's **Settings** > **Trusted Publishing**, delete the old entry pointing to `publish-crates.yml`, and add entries for `release-plz-release.yml` and `publish-crate-manual.yml` as workflow filenames. The environment name (`release`) stays the same for both entries.

Failing to update means the OIDC token exchange will fail silently when the new workflow runs. The publish step will error with an authentication failure. There is no fallback: the legacy `CARGO_REGISTRY_TOKEN` path was removed when we migrated to Trusted Publishing. Fix the TP config on crates.io and re-run the workflow.

## Supply Chain Guarantees

Every release artifact carries two layers of supply chain verification.

**SLSA L2 build provenance** is generated by `actions/attest-build-provenance@v4` for all binary tarballs and the Docker image. To verify a binary:

```bash
gh attestation verify docspec-cli-x86_64-unknown-linux-gnu.tar.gz \
  --owner docspec \
  --repo docspec
```

**cosign keyless signatures** are applied to binary tarballs and the Docker image using Fulcio (certificate authority) and Rekor (transparency log). No private key is stored anywhere. To verify a binary tarball:

```bash
cosign verify-blob \
  --certificate docspec-cli-x86_64-unknown-linux-gnu.tar.gz.pem \
  --signature docspec-cli-x86_64-unknown-linux-gnu.tar.gz.sig \
  --certificate-identity-regexp "https://github.com/docspec/docspec/.github/workflows/release.yml" \
  --certificate-oidc-issuer "https://token.actions.githubusercontent.com" \
  docspec-cli-x86_64-unknown-linux-gnu.tar.gz
```

To verify the Docker image:

```bash
cosign verify \
  --certificate-identity-regexp "https://github.com/docspec/docspec/.github/workflows/release.yml" \
  --certificate-oidc-issuer "https://token.actions.githubusercontent.com" \
  ghcr.io/docspec/docspec:v1.5.0
```

The `.pem` and `.sig` files are attached to the GitHub Release alongside the tarballs.

## Publishing Order and Index Propagation

release-plz publishes crates in topological dependency order so that each crate's dependencies are already on crates.io before it publishes. The order is:

1. `docspec-core` (leaf, no internal dependencies)
2. `docspec-json`, `docspec-markdown-reader`, `docspec-html-reader`, `docspec-docx-reader` (depend only on `docspec-core`)
3. `docspec-blocknote-writer`, `docspec-oxa-writer`, `docspec-html-writer` (depend on `docspec-core` and `docspec-json`)
4. `docspec` (the facade, depends on readers and writers)
5. `docspec-cli` and `docspec-http` (depend on the facade and core crates)

After each publish, crates.io needs time to propagate the new version to its index before the next crate can resolve it as a dependency. `release-plz` waits for newly published crates according to `publish_timeout = "30m"` in `release-plz.toml`, so normal propagation lag should not need manual intervention.

If propagation takes longer than the configured timeout, the workflow will fail. In that case, wait for the index to catch up and re-run the workflow manually. release-plz skips crates that are already published at the target version, so re-running is safe.

## Manual Operations

**Single-crate emergency publish.** If you need to publish one crate outside the normal release cycle (for example, to push a security fix to a single crate before the full release machinery runs), use the `publish-crate-manual.yml` workflow. Trigger it from the Actions tab, select the crate name, and confirm. This workflow also uses Trusted Publishing, so no token is needed.

**Re-running a failed release.** release-plz is idempotent: it checks whether each crate is already published at the target version before attempting to publish. Re-running `release-plz-release.yml` after a partial failure is safe. Go to the Actions tab, find the failed run, and click "Re-run all jobs".

**Deleting a stuck release PR.** If a release PR is stale or was opened in error, close it without merging. release-plz will open a fresh one the next time it runs (triggered by the next commit to `main`, or manually via workflow dispatch).

## Yank Policy

Under unified ecosystem versioning, yanking one crate at a release version without yanking the others breaks the ecosystem version story. A user who depends on `docspec = "1.5.0"` expects all 11 crates at `1.5.0` to be coherent.

If a bad release slips through, the policy is: yank all 11 publishable crates at the bad version simultaneously, then rush a `1.5.x+1` patch release with the fix. Do not yank selectively.

To yank all crates at a version:

```bash
for crate in docspec-core docspec-json docspec-markdown-reader docspec-html-reader \
  docspec-docx-reader docspec-blocknote-writer docspec-oxa-writer \
  docspec-html-writer docspec docspec-cli docspec-http; do
  cargo yank --version 1.5.0 "$crate"
done
```

Announce the yank in the GitHub Release notes and open a tracking issue.

## Recovery Procedures

**Tag pushed but workflow failed.** The tag is sticky; pushing it again won't help. Instead, go to the Actions tab and re-run the failed workflow. The workflow is idempotent: it skips already-published crates and already-built artifacts.

**Crate publish failed halfway.** Re-run `release-plz-release.yml`. It will skip the crates already published and retry the ones that failed.

**Wrong tag pushed by mistake.** Delete the release and the tag, then push the correct tag:

```bash
gh release delete v1.5.0 --yes
git push --delete origin v1.5.0
git tag -d v1.5.0
# Fix whatever was wrong, then:
git tag v1.5.0 <correct-commit-sha>
git push origin v1.5.0
```

**release-plz opened a release PR with the wrong version.** Close the PR without merging. Fix the commit messages that caused the wrong version calculation (usually a `BREAKING CHANGE` footer that shouldn't be there, or a missing one). Push a fixup commit to `main` and let release-plz open a fresh PR.

**Trusted Publishing OIDC failure.** Check that the workflow filename in the crates.io TP config matches the failing workflow exactly (`release-plz-release.yml` for automated releases, or `publish-crate-manual.yml` for emergency single-crate publishes), the environment name is `release`, and the repository owner and name are correct. A single character mismatch causes a silent OIDC failure.

## WASM and Future npm Distribution

`docspec-wasm` is `publish = false`. The crate is tagged at the ecosystem version with every release, so it stays in sync with the rest of the workspace, but it never reaches crates.io.

Today, there is no npm distribution. The wasm crate exists as a foundation for future browser and Node.js integration. When npm distribution is added, the team will need to decide whether to ship raw wasm-pack output or introduce a hand-written TypeScript wrapper that provides a more ergonomic API. Either way, the npm publish step will be a separate workflow, not part of `release-plz-release.yml`.

The ecosystem version tag (`vX.Y.Z`) will continue to apply to `docspec-wasm` even after npm distribution is added. The npm package version may or may not track the ecosystem version exactly, depending on how the TypeScript API evolves independently.

## Historical Notes

Before v1.5.0, DocSpec used independent per-crate versioning. Each crate had its own version, and releases were tagged with component-specific tags like `docspec-core-v0.3.1` or `docspec-cli-v0.2.0`. These tags are retained in git history and will not be rewritten.

The very early tags `v0.2.0` and `v0.3.0` predate the monorepo split. They refer to a single-crate era before the workspace was restructured. They are also retained as-is.

Starting with `v1.5.0`, all releases use the unified ecosystem version. The tag format is `vX.Y.Z` with no component prefix. Anything before `v1.5.0` predates the unified-version policy and should be treated as historical context, not as a model for current practice.

If you're looking at an old tag and wondering why the version numbers don't add up, that's why.
