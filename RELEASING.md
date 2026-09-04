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

1. From the latest `main`, run the version synchronization script:

   ```bash
   python scripts/sync_version.py X.Y.Z
   ```

2. Include the version metadata and release notes in a short-lived branch,
   then open a pull request to `main`.
3. Merge after required CI is green.
4. From the merged `main` commit, create and push a signed annotated tag:

   ```bash
   git tag -s vX.Y.Z -m "vX.Y.Z"
   git push origin vX.Y.Z
   ```

   The merge and the formal tag are one release action. Do not leave `main`
   at a new release version without its corresponding formal tag.

5. After the build workflow succeeds, run the package publishing workflows
   from the exact formal tag:

   ```bash
   gh workflow run release-publish-crates.yml --ref vX.Y.Z
   gh workflow run release-publish-pypi.yml --ref vX.Y.Z
   gh workflow run release-publish-npm.yml --ref vX.Y.Z
   ```

6. After all required publishing workflows succeed, run the final gate:

   ```bash
   gh workflow run release-finalize.yml --ref vX.Y.Z
   ```

`release-finalize` verifies the exact tag commit and the required build and
publishing workflows before making the GitHub Release public.

## Branch protection

`main` is protected. Direct pushes and force pushes are disabled. All changes
must use a pull request and pass the required checks:

- `rust`
- `python`
- `web`
- `release-metadata`

The repository may contain short-lived feature, fix, documentation, or
candidate branches, but none is a required permanent development branch.
