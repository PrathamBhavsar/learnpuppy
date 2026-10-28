import { beforeEach, describe, test } from '@e2e-dev/web';
import { expect } from 'e2e';

describe('knowledge graph canvas zoom, pan, node provenance drawer, and chain table', () => {
  beforeEach(async ({ app, screen }) => {
    await app.open('/');
    await expect(screen.getByTestId('view-graph')).toBeVisible();
    await expect(screen.getByTestId('graph-canvas')).toBeVisible();
  });

  test('zooms, pans, and resets the SVG graph camera', async ({ screen, browser }) => {
    await expect(screen.getByTestId('graph-zoom-level')).toHaveText('100%');

    // Zoom in and out via toolbar controls
    await screen.getByTestId('graph-zoom-in').tap();
    await expect(screen.getByTestId('graph-zoom-level')).toHaveText('120%');

    await screen.getByTestId('graph-zoom-out').tap();
    await expect(screen.getByTestId('graph-zoom-level')).toHaveText('100%');

    // Pan via keyboard arrow keys on the SVG canvas and verify transform updates
    const canvas = screen.getByTestId('graph-canvas');
    await canvas.focus();
    await canvas.press('+');
    await expect(screen.getByTestId('graph-zoom-level')).toHaveText('120%');

    const beforeTransform = await browser.locator('[data-testid="graph-canvas"] > g').getAttribute('transform');
    await canvas.press('ArrowRight');
    const afterTransform = await browser.locator('[data-testid="graph-canvas"] > g').getAttribute('transform');
    expect(afterTransform).not.toBe(beforeTransform);

    // Reset camera to default fit
    await screen.getByTestId('graph-camera-reset').tap();
    await expect(screen.getByTestId('graph-zoom-level')).toHaveText('100%');
  });

  test('opens node provenance drawer, selects propositions for practice, and expands 3-level chain table', async ({ screen }) => {
    await expect(screen.getByTestId('graph-cluster-hull')).toBeVisible();
    await expect(screen.getByTestId('graph-chain-junction')).toBeVisible();

    const walNode = screen.getByTestId('graph-node-n_wal_snapshot');
    await expect(walNode).toBeVisible();
    await walNode.tap();

    // Provenance drawer opens with proposition details and birth session source
    const drawer = screen.getByTestId('graph-provenance-drawer');
    await expect(drawer).toBeVisible();
    await expect(drawer).toContainText('WAL Snapshot Isolation');
    await expect(drawer).toContainText('n_wal_snapshot');
    await expect(screen.getByTestId('graph-provenance-source')).toContainText('Session sess-wal-teach · Message m1');

    // Select proposition for adaptive practice from drawer checkbox
    await screen.getByTestId('graph-drawer-select').tap();
    await expect(screen.getByTestId('graph-practice')).toContainText('Practice selected (1)');

    // Close provenance drawer
    await screen.getByTestId('graph-close-provenance').tap();
    await expect(drawer).toBeHidden();

    // Open 3-level collapsible chain table and expand cluster + implication chain
    await screen.getByTestId('graph-toggle-table').tap();
    await expect(screen.getByTestId('graph-chain-table')).toBeVisible();

    await screen.getByTestId('graph-cluster-toggle-Session sess-wal-teach').tap();
    await screen.getByTestId('graph-chain-toggle-arg_wal_and_fsrs_entail_calls').tap();
    await expect(
      screen.getByTestId('graph-statement-arg_wal_and_fsrs_entail_calls-n_position_from_calls'),
    ).toBeVisible();
  });
});
