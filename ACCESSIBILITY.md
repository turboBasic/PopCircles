# Accessibility

PopCircles provides command-line tools, numerical result documents, and a static gallery of
map images. People should be able to use the tools and understand their results without relying on the
maps alone. This page describes current priorities, known barriers, and how to report one.

## Priorities

- Keep instructions and numerical answers available as text and machine-readable JSON.
- Make the purpose and important information in generated maps available in a text alternative.
- Keep repository documentation and contribution paths clear and usable.

For the static gallery, WCAG 2.2 Level AA is an aspirational target for web-facing changes, not a claim
of conformance. No accessibility evaluation has been completed. [WCAG 2.2](https://www.w3.org/TR/WCAG22/)
is the current W3C Recommendation.

## Contributor expectations

- Preserve text equivalents for information conveyed by figures, color, or other visual features.
- For a change to a public page or output, describe what was checked and name the browser, operating
  system, and assistive technology when applicable. Screenshots or recordings are optional.
- The project has no dedicated automated accessibility checks or published test matrix. Passing its
  existing checks does not establish accessibility conformance.

See [`CONTRIBUTING.md`](CONTRIBUTING.md) for the contribution process.

## Reporting accessibility issues

Report a barrier in a [GitHub issue](https://github.com/turboBasic/PopCircles/issues/new).
Include the affected task and page URL or command, what happened, and what you expected. Browser,
operating system, and assistive technology details can help reproduce the issue. You do not need to
disclose a disability, and screenshots or recordings are optional.

### Severity

There is no project severity scale or response target. Describe whether the barrier blocks the task,
slows it down, or has a workaround; the maintainer will triage its priority.

### How we respond

The maintainer reviews reports alongside other project work, may ask for clarification, and will post
status or a workaround in the issue when available. Response and fix timelines are not guaranteed.

## Ownership and maintenance

The repository maintainer owns accessibility reports and this statement. The statement is reviewed
when public surfaces or reporting routes change; there is no scheduled review cadence.

## Supported environments

The project publishes command-line binaries for macOS on Apple silicon and Linux on x86-64. The map
gallery is a static website. No browser, operating system, screen reader, or other assistive technology
combination has been systematically evaluated or is claimed as supported for accessibility.

## Known limitations

- Gallery maps are PNG images. Their current alternative text repeats the image filename stem rather
  than describing the map. The caption links to the adjacent JSON result, which provides numerical
  results and provenance but is not a full text equivalent of the visual map.
- Terminal output and the gallery have not been systematically evaluated with screen readers,
  magnification, keyboard-only use, or other assistive technologies.

These are the limitations currently documented; the absence of other reports does not establish that
no other barriers exist.

## Feedback and improvements

Use a [GitHub issue](https://github.com/turboBasic/PopCircles/issues/new) to suggest an
improvement to this statement or the project's accessibility practices. Report an active barrier using
the process above.
