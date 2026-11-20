# Release MCPStore

Version the **product**, not the language or distribution channel. The two stable versions need not match.

| Product | Source of truth | Delivered together | Tag |
|---|---|---|---|
| SDK | Root `Cargo.toml`, `workspace.package.version` | `mcpstore` on crates.io and PyPI | `sdk-vX.Y.Z` |
| App | `satellite/cli/Cargo.toml`, `package.version` | CLI archives, desktop, web metadata, `@ip2a/mcpstore` and its native npm dependencies | `vA.B.C` |

Use Python 3.11+. From a clean release branch run `python3 scripts/sync_version.py sdk X.Y.Z` and/or `python3 scripts/sync_version.py app A.B.C`, **only for products being released**. Run `python3 scripts/release_preflight.py`, inspect and commit the metadata changes, and test that commit. Check `satellite/cli/Cargo.toml` carefully: its package version is the App version; its `mcpstore` dependency is the SDK requirement. Updating a dependency is not releasing a product.

## What runs on GitHub

| Entry point | Trigger and responsibility |
|---|---|
| `ci.yml` | Pull request, push to main or manual dispatch: test root and independent CLI workspaces, Python, web and release metadata. A backup push to a work branch is not a release. |
| `codeql.yml` | Release tag or manual scan; no regular PR scan. |
| `release-publish-crates.yml` / `release-publish-pypi.yml` | Dispatch **both** on an exact existing stable SDK tag. Use Trusted Publishing; verify crate and wheels before claiming SDK release. The SDK does not publish a GitHub App release. |
| `release-build-cli.yml` | App tag: build and smoke-test platform binaries, archive, attest, and attach to draft. Stable tag push starts npm publishing. |
| `release-build-desktop.yml` | App tag: build desktop bundles, updater data and signatures, and attach to the same draft. |
| `release-publish-npm.yml` | Stable App only: use the matching CLI build, publish native packages before the npm wrapper, then start finalization. |
| `release-finalize.yml` | Stable App only: verify same-tag/commit prerequisites; make the draft public. |
| `.github/dependabot.yml` | Update pinned Actions and dependencies in the actual root and satellite workspaces. Issue templates do not participate in release. |

For an App RC keep the base App metadata `A.B.C` and tag `vA.B.C-rc.N`: builds and draft only, no npm/finalization. For a combined release use both tags on the **same tested commit**. Registry publication is not atomic across products or even across registries. Record the SHA, tags and run URLs; never move a tag, republish a version, or repeat a successful publish to recover a failed sibling. Don't treat workflow dispatch as delivery. Verify registries and release assets.

## One-time GitHub and registry configuration

- Use GitHub **Settings → Environments → Deployment branches and tags**. Allow **Tag `sdk-v*`** in `crates` and `pypi`; allow **Tag `v*`** in `npm`. A *branch* pattern with the same text does not admit tags. Preserve required reviewers, signing and OIDC permissions. Register the exact repository/workflow/environment in each package registry; retain the desktop signing secrets. Use tag rulesets to protect **both** `refs/tags/v*` and `refs/tags/sdk-v*` against updates/deletions. Never weaken protection as an automated workaround.
- An administrator can add the missing rule without replacing any existing policy: `gh api --method POST repos/ip2a/mcpstore/environments/crates/deployment-branch-policies -f type=tag -f 'name=sdk-v*'`. Inspect the current policies first; use the appropriate environment and pattern from the preceding bullet. These are one-time administrative changes, not steps to run on every release.
- The workflow file must be present on GitHub's default branch for `workflow_dispatch` to be registered. Execute on the explicitly chosen branch/tag, not the default branch by accident. After a failed/unknown write, check the actual run, ref or registry before retrying. Never create a second train automatically.
- Maintainers with the project-local `store-store` skill can ask for a branch backup, one domain, both domains, or an App RC in one sentence. A plain push has no release side effects. No publishing credentials belong in the checkout or skill.
