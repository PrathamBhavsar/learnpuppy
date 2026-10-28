import type { E2EConfig } from 'e2e';
import { web } from '@e2e-dev/web';

// Web design-system lab (web/). Start it first:
//   cd web && bun run build && bun run start -p 3417
// Does not require lp-app.
export default {
  tests: 'tests/design-system/**/*.e2e.ts',
  targets: [
    {
      name: 'chromium',
      engine: web({ viewport: { width: 1440, height: 900 } }),
      app: { url: 'http://127.0.0.1:3417' },
    },
  ],
  timeout: 60_000,
  actionTimeout: 10_000,
  assertionTimeout: 5_000,
  workers: 1,
  retries: 0,
  reporters: ['list'],
} satisfies E2EConfig;
