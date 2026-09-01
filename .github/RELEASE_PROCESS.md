# Release process

`main` is release source. Every change reaches it through a pull request with
required CI. Do not publish from a local build or from a mutable branch.

## Prepare release

1. Open a release PR from `release/vX.Y.Z` (or another short-lived branch).
2. Run `python scripts/sync_version.py X.Y.Z`; include release notes and all
   necessary fixes.
3. Merge only after CI is green.
4. From clean, current `main`, create and push a signed annotated tag:

   ```bash
   git tag -s vX.Y.Z -m "vX.Y.Z"
   git push origin vX.Y.Z
   ```

   Tags are immutable after push. A failed release is corrected by a new tag,
   never by moving the published tag.

## Publish in GitHub Actions

Pushing tag starts `release-build-cli`, which builds assets, attests them, and
creates a **draft** GitHub Release. Run these workflows from same tag:

```bash
gh workflow run release-publish-crates.yml --ref vX.Y.Z
gh workflow run release-publish-npm.yml --ref vX.Y.Z
gh workflow run release-publish-pypi.yml --ref vX.Y.Z
```

After all succeed, publish public release only through final gate:

```bash
gh workflow run release-finalize.yml --ref vX.Y.Z
```

`release-finalize` checks successful CLI, crates.io, npm, and PyPI workflows
for exact tag commit before changing GitHub Release from draft to public.
