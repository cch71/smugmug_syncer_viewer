# Security policy

## Supported versions

Only the latest release gets security fixes.

## Reporting a vulnerability

Please don't open a public issue. Report it privately through
[GitHub's private vulnerability reporting](https://github.com/cch71/smugmug_syncer_viewer/security/advisories/new).
Include the version, your platform, and steps to reproduce.

You should get a reply within a week. Once a fix is released, the advisory is published with
credit to you, unless you'd rather stay anonymous.

## What's in scope

The viewer reads archive files that may come from elsewhere. Issues such as path traversal
out of the archive through the `/media` handler, crashes or excessive memory use from crafted
`.db`, parquet, JPEG or MP4 files, or script injection through image titles and captions are
all in scope.

## Automated checks

Every push and pull request runs:

- CodeQL analysis of the Rust code and the GitHub workflows (`.github/workflows/codeql.yml`)
- `cargo deny`, which checks for RustSec advisories, disallowed licenses, and dependencies
  from unknown sources (`deny.toml`)
- Dependency review on pull requests
- OpenSSF Scorecard on `main`

Dependabot proposes dependency and action updates weekly.
