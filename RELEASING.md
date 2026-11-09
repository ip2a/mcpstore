# Release process

`main` is the only long-lived development and release branch. Every change
reaches it through a pull request with required CI. Do not publish from a
local build or from a mutable branch.

## Everyday changes

1. Create a short-lived branch from the latest `main`:

   ```bash
   git switch main
   git pull --ff-only
   git switch -c fix/short-name
   ```

2. Make and commit the change.
3. Push the branch and open a pull request to `main`.
4. Merge only after required CI is green.
5. Delete the short-lived branch after merge.

Documentation, dependency, CI, and small fixes follow the same flow. They do
not need a version bump or a tag unless they are part of a release.

## Candidate builds

Candidate builds use the version already declared on `main` and an immutable
pre-release tag. No release branch is required:

```text
main commit
  ├─ v2.3.1-alpha.1
  ├─ v2.3.1-beta.1
  ├─ v2.3.1-rc.1
  └─ v2.3.1-rc.2
```

Pushing a candidate tag starts the CLI/Desktop build workflows. They create a
draft GitHub pre-release. Candidate builds do not publish to PyPI, npm, or
crates.io, and Desktop candidates do not upload `latest.json` to the updater
channel.

Tags are immutable. If a candidate fails, use the next sequence number rather
than moving or deleting the tag.

## Formal release

MCPStore releases two independent version domains:

| Domain | Ships | Canonical version | Tag |
|---|---|---|---|
| SDK | crates.io `mcpstore` + PyPI `mcpstore` | `[workspace.package]` version in the root `Cargo.toml` | `sdk-vX.Y.Z` (never a prerelease) |
| App | CLI/TUI, desktop bundles, web panel, npm packages | `version` in `satellite/cli/Cargo.toml` | `vX.Y.Z` / `vX.Y.Z-rc.N` |

The CLI satellite holds the single cross-domain contract: it depends on the
SDK crate with `path = "../.."` plus a minor-tracked version requirement.
`sync_version.py app` bumps that requirement to the current SDK minor, and
the `release-metadata` CI job fails when the two drift apart.

Release the domains independently, in whichever order; the habitual order is
SDK first so the App release picks up the newest SDK.

### SDK release

1. From the latest `main`, bump the SDK version and open a pull request:

   ```bash
   python scripts/sync_version.py sdk X.Y.Z
   ```

2. Merge after required CI is green.
3. From the merged `main` commit, create and push a signed annotated tag:

   ```bash
   git tag -s sdk-vX.Y.Z -m "sdk-vX.Y.Z"
   git push origin sdk-vX.Y.Z
   ```

4. Dispatch the publish workflows from the exact tag:

   ```bash
   gh workflow run release-publish-crates.yml --ref sdk-vX.Y.Z
   gh workflow run release-publish-pypi.yml --ref sdk-vX.Y.Z
   ```

SDK releases create no GitHub Release and have no finalize gate. A stale
lock version fails the publish builds (`--locked`); `sync_version.py sdk`
keeps every downstream lock current.

### App release

1. From the latest `main`, bump the App version and open a pull request:

   ```bash
   python scripts/sync_version.py app X.Y.Z
   ```

   This also moves the CLI satellite's SDK version requirement to the
   current SDK minor.

2. Merge after required CI is green.
3. From the merged `main` commit, create and push a signed annotated tag:

   ```bash
   git tag -s vX.Y.Z -m "vX.Y.Z"
   git push origin vX.Y.Z
   ```

   The merge and the formal tag are one release action. Pushing the tag
   starts the CLI and Desktop build workflows automatically.

4. After the build workflow succeeds, run the npm publishing workflow from
   the exact formal tag:

   ```bash
   gh workflow run release-publish-npm.yml --ref vX.Y.Z
   ```

5. After npm publishing succeeds, run the final gate:

   ```bash
   gh workflow run release-finalize.yml --ref vX.Y.Z
   ```

`release-finalize` verifies the exact tag commit and the App-domain build
and publishing workflows (build-cli, publish-npm) before making the GitHub
Release public. SDK publishing never blocks an App release.

## Branch protection

`main` is protected. Direct pushes and force pushes are disabled. All changes
must use a pull request and pass the required checks:

- `rust`
- `python`
- `web`
- `release-metadata`

The repository may contain short-lived feature, fix, documentation, or
candidate branches, but none is a required permanent development branch.
