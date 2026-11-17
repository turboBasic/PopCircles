# Playwright screenshot baselines across platforms

Research note, 2026-11-17.

## Finding

Playwright warns that screenshot rendering varies with host operating system, operating-system
version and settings, hardware, power source, and headless mode. Its recommendation is to compare
screenshots in the same environment that created the baseline. It also notes that different
platforms and browsers need different snapshots. [Visual comparisons](https://playwright.dev/docs/test-snapshots)

This repository overrides Playwright's snapshot path with
`{testDir}/{testFilePath}-snapshots/{arg}{ext}` in
[`frontend/playwright.config.js`](../../frontend/playwright.config.js). That omits the platform
identifier. The local full CI run passed on macOS; GitHub's Ubuntu 26.04 run failed only on
`gallery-mobile-dark.png`, with 17,048 changed pixels (6%) against a 5% limit. The evidence points
to a macOS/Linux baseline mismatch. The mobile-dark baseline generated in the Playwright 1.64.1
Ubuntu 26.04 image differs visibly in system-font rendering from the Mac baseline; the layout and
content remain intact. GitHub's actual/diff images were not available, so this is a close
reproduction rather than a byte-for-byte capture of the hosted runner.

Playwright's `snapshotPathTemplate` supports a `{platform}` token whose value is `process.platform`.
The custom template can therefore keep separate `darwin` and `linux` snapshots. The older
`testInfo.snapshotSuffix` property can do this too, but Playwright discourages it in favor of the
path template. [TestProject.snapshotPathTemplate](https://playwright.dev/docs/api/class-testproject)

The platform token distinguishes Darwin from Linux, but it does not distinguish Linux releases,
font packages, browser revisions, or rendering settings. Playwright recommends pinning its Docker
image to the same Playwright release as the project; its CI guide says containers provide a
consistent screenshot environment across operating systems. [Docker](https://playwright.dev/docs/docker),
[Continuous integration](https://playwright.dev/docs/ci)

## Recommendation

Use platform-specific baselines for this repository: add `-{platform}` to the custom snapshot
filename, suffix the existing macOS baselines with `-darwin`, and generate `-linux` baselines in the
pinned Ubuntu 26.04 Playwright image. The baselines have now been visually reviewed in that image.
Keep the 5% threshold; increasing it would hide differences without identifying their cause.

For stronger long-term reproducibility, run screenshot tests both locally and in CI inside the same
version-pinned Playwright Linux image. That gives one canonical baseline but requires making the
container environment part of the local test workflow and the reusable CI job. The current CI job
uses an external reusable workflow, so this is a broader infrastructure change than adding
platform-specific baselines.

## Sources

- [Playwright visual comparisons](https://playwright.dev/docs/test-snapshots)
- [Playwright `snapshotPathTemplate` API](https://playwright.dev/docs/api/class-testproject)
- [Playwright Docker images](https://playwright.dev/docs/docker)
- [Playwright continuous integration](https://playwright.dev/docs/ci)
