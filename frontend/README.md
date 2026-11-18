# Gallery frontend

See the frontend subject document for [intended behaviour](../docs/ai/frontend.md#behaviour),
[verification](../docs/ai/frontend.md#finding-verification), and
[contribution guidance](../docs/ai/frontend.md#contributing).

## Development

Run `mise run test:frontend` to build the static Svelte app into `frontend/dist/`.

`npm test` runs unit and component tests. Use `npm run test:unit` or
`npm run test:component` to select one Vitest project, and `npm run test:e2e` for Playwright
against the production preview. `npm run test:watch` and `npm run test:e2e:ui` support local
iteration.

After `mise run deps` installs dependencies and `mise run gallery` generates the data and maps, run
`mise run gallery:dev` to rebuild the frontend and serve the existing gallery. This does not
regenerate the data or maps. To refresh frontend edits while the server is already running, run
`mise run gallery:frontend`, then reload the page.
