# Frontend

## Web page architecture

- PopCircles web page is a Svelte application for exploring geographic maps and associated data.
- Keep UI components modular, semantic, and reusable.
- Separate presentation logic from data processing and map generation.

## UI/UX

- Use a viewport-contained application shell (`100dvh`).
- The document must not scroll vertically during normal application use.
- The main workspace owns primary vertical scrolling.
- Country navigation may scroll independently.
- Use native scrolling and browser controls.
- Support Focused and Expand All map modes.
- Preserve map aspect ratios and avoid layout shifts.
- Use responsive layouts based on available content width.
- Preserve light and dark theme support.

## Development Principles

- Follow existing Svelte conventions and project tooling.
- Prefer semantic HTML and native browser behavior.
- Avoid unnecessary dependencies and custom abstractions.
- Keep changes focused on the requested scope.
- Validate UI changes in the rendered application.
- Run relevant existing checks after modifications.
