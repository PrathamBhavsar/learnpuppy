import { mkdirSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import type { E2EConfig } from 'e2e';
import { web } from '@e2e-dev/web';

function crc32Hex(data: Buffer): string {
  let crc = 0xffffffff;
  for (const b of data) {
    crc ^= b;
    for (let i = 0; i < 8; i += 1) {
      crc = (crc >>> 1) ^ (crc & 1 ? 0xedb88320 : 0);
    }
  }
  return ((~crc) >>> 0).toString(16).padStart(8, '0');
}

function encodeLedgerLine(value: unknown): string {
  const json = JSON.stringify(value);
  const crc = crc32Hex(Buffer.from(json, 'utf8'));
  return `${crc}\t${json}\n`;
}

function seedE2EFixtureDir(): { homeDir: string; dataDir: string } {
  const root = join(tmpdir(), 'learnpuppy-e2e-fixture');

  const homeDir = join(root, 'home');
  const dataDir = join(root, 'data');
  const graphDir = join(dataDir, 'graph');
  const claude1Dir = join(homeDir, '.claude');
  const claude2Dir = join(homeDir, '.claude-work');
  const codexDir = join(homeDir, '.codex');
  const agyDir = join(homeDir, '.gemini');

  for (const dir of [graphDir, claude1Dir, claude2Dir, codexDir, agyDir]) {
    mkdirSync(dir, { recursive: true });
  }

  // Seed multi-account CLI slots (Claude #1, Claude #2, Codex, AGY)
  writeFileSync(
    join(claude1Dir, '.claude.json'),
    JSON.stringify({
      oauthAccount: {
        accountUuid: '00000000-0000-4000-8000-000000000001',
        emailAddress: 'pratham.bhavsar501@gmail.com',
        userRateLimitTier: 'max_5x',
      },
    }),
  );
  writeFileSync(
    join(claude2Dir, '.claude.json'),
    JSON.stringify({
      oauthAccount: {
        accountUuid: '00000000-0000-4000-8000-000000000002',
        emailAddress: 'q97159745@gmail.com',
        userRateLimitTier: 'pro',
      },
    }),
  );
  writeFileSync(
    join(codexDir, 'auth.json'),
    JSON.stringify({
      tokens: {},
    }),
  );
  writeFileSync(
    join(agyDir, 'oauth_creds.json'),
    JSON.stringify({
      refresh_token: 'fixture-refresh-token',
      expiry_date: 4102444800000,
    }),
  );
  writeFileSync(
    join(agyDir, 'google_accounts.json'),
    JSON.stringify({
      active: 'pratham.bhavsar501@gmail.com',
    }),
  );

  // Seed GraphSnapshot ledgers (nodes.jsonl and chains.jsonl with <crc32 hex>\t<json>\n format)
  const nodes = [
    {
      id: 'n_wal_snapshot',
      name: 'WAL Snapshot Isolation',
      statement: 'Copying .db, -wal, and -shm into a TempDir prevents lock contention against active writers.',
      aliases: ['TempDir WAL copy'],
      created_at: '2026-10-28T00:00:00Z',
      provenance: [{ chat_id: 'sess-wal-teach', message_id: 'm1' }],
    },
    {
      id: 'n_fsrs_replay',
      name: 'Deterministic FSRS Replay',
      statement: 'Sorting ReviewEvents by (ts, id) with enable_fuzz=false guarantees identical cross-device schedules.',
      aliases: ['Order-independent FSRS fold'],
      created_at: '2026-10-28T00:01:00Z',
      provenance: [{ chat_id: 'sess-wal-teach', message_id: 'm2' }],
    },
    {
      id: 'n_position_from_calls',
      name: 'Position From Calls',
      statement: 'PlanTree node statuses are derived from quiz and explain tool calls rather than stored state.',
      aliases: ['Stateless PlanStatuses'],
      created_at: '2026-10-28T00:02:00Z',
      provenance: [{ chat_id: 'sess-wal-teach', message_id: 'm3' }],
    },
  ];

  const chains = [
    {
      id: 'arg_wal_and_fsrs_entail_calls',
      premises: ['n_wal_snapshot', 'n_fsrs_replay'],
      conclusion: 'n_position_from_calls',
      provenance: [{ chat_id: 'sess-wal-teach', message_id: 'm3' }],
      created_at: '2026-10-28T00:02:00Z',
    },
  ];

  writeFileSync(
    join(graphDir, 'nodes.jsonl'),
    nodes.map((n) => encodeLedgerLine(n)).join(''),
  );
  writeFileSync(
    join(graphDir, 'chains.jsonl'),
    chains.map((c) => encodeLedgerLine(c)).join(''),
  );

  return { homeDir, dataDir };
}

const { homeDir, dataDir } = seedE2EFixtureDir();
const cargoTargetDir = process.env.CARGO_TARGET_DIR ?? '/Users/pratham/orca/shared-target';
const appBinary = join(cargoTargetDir, 'debug', 'learnpuppy-app');

export default {
  tests: 'tests/**/*.e2e.ts',
  targets: [
    {
      name: 'chromium',
      engine: web({
        viewport: { width: 1440, height: 900 },
      }),
      app: {
        url: 'http://127.0.0.1:0',
        command: {
          executable: appBinary,
          env: {
            LP_E2E_PORT: '{port}',
            HOME: homeDir,
            LP_DATA_DIR: dataDir,
          },
          startupTimeout: 60_000,
          log: '.e2e/logs/app.log',
        },
      },
    },
  ],
  timeout: 120_000,
  actionTimeout: 15_000,
  assertionTimeout: 5_000,
  workers: 1,
  retries: 0,
  reporters: ['list'],
} satisfies E2EConfig;
