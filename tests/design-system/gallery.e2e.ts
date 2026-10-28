import { copyFileSync, mkdirSync } from 'node:fs';
import { join } from 'node:path';
import { beforeEach, describe, test } from '@e2e-dev/web';
import { expect } from 'e2e';

const SHOTS = join(process.cwd(), 'tests/design-system/screenshots');

const SWATCHES = [
  'background', 'foreground', 'primary', 'accent', 'muted-foreground', 'destructive',
  'success', 'warning', 'info', 'chart-3', 'chart-4', 'border',
];

// Parses any computed CSS color (rgb(), color(srgb ...), oklch) to sRGB 0-255 via canvas.
const COLOR_PROBE = `(sel) => {
  const el = document.querySelector(sel);
  const c = document.createElement('canvas'); c.width = c.height = 1;
  const ctx = c.getContext('2d');
  ctx.fillStyle = '#000';
  ctx.fillStyle = getComputedStyle(el).backgroundColor;
  ctx.fillRect(0, 0, 1, 1);
  const d = ctx.getImageData(0, 0, 1, 1).data;
  return [d[0], d[1], d[2]];
}`;

describe('design system lab (web)', () => {
  beforeEach(async ({ app, screen }) => {
    await app.open('/');
    await expect(screen.getByTestId('gallery')).toBeVisible();
  });

  test('renders title and every gallery section', async ({ app, screen }) => {
    await expect(screen.getByTestId('gallery-title')).toContainText('design system lab');
    for (const id of ['palette', 'icons', 'button', 'badge', 'card', 'tabs', 'dialog', 'progress', 'slider', 'switch', 'command']) {
      await expect(screen.getByTestId(`section-${id}`)).toBeAttached();
    }
    for (const id of [
      'button-default', 'button-secondary', 'button-outline', 'button-ghost', 'button-destructive', 'button-link', 'button-icon',
      'badge-default', 'badge-secondary', 'badge-outline', 'badge-destructive', 'badge-ghost', 'chip-graph-node',
      'card-demo', 'tabs-demo', 'dialog-trigger', 'tooltip-trigger',
      'progress-novice', 'progress-learning', 'progress-mastered', 'slider-demo', 'switch-demo', 'command-demo',
    ]) {
      await expect(screen.getByTestId(id)).toBeAttached();
    }
    mkdirSync(SHOTS, { recursive: true });
    copyFileSync(await app.screenshot('gallery'), join(SHOTS, 'gallery.png'));
  });

  test('torlink palette swatches render and primary is near #a78bfa', async ({ app, screen, browser }) => {
    for (const t of SWATCHES) {
      await expect(screen.getByTestId(`swatch-${t}`)).toBeVisible();
    }
    const rgb = (await browser.evaluate(`(${COLOR_PROBE})('[data-testid="swatch-primary"] > div')`)) as number[];
    const target = [0xa7, 0x8b, 0xfa];
    const dist = Math.max(...rgb.map((v, i) => Math.abs(v - target[i])));
    expect(dist).toBeLessThanOrEqual(6);
    const bg = (await browser.evaluate(`(${COLOR_PROBE})('[data-testid="swatch-background"] > div')`)) as number[];
    expect(Math.max(...bg.map((v, i) => Math.abs(v - [0x0a, 0x08, 0x10][i])))).toBeLessThanOrEqual(6);
    copyFileSync(await app.screenshot('palette'), join(SHOTS, 'palette.png'));
  });

  test('dialog opens and closes', async ({ app, screen, browser }) => {
    await expect(screen.getByTestId('dialog-content')).toBeHidden();
    await screen.getByTestId('dialog-trigger').tap();
    await expect(screen.getByTestId('dialog-content')).toBeVisible();
    await expect(screen.getByTestId('dialog-content')).toContainText('Accounts and slots');
    copyFileSync(await app.screenshot('dialog-open'), join(SHOTS, 'dialog-open.png'));
    await browser.keyboard.press('Escape');
    await expect(screen.getByTestId('dialog-content')).toBeHidden();
  });

  test('tabs switch panels', async ({ screen }) => {
    await expect(screen.getByText('Knowledge graph canvas.')).toBeVisible();
    await screen.getByTestId('tab-plan').tap();
    await expect(screen.getByText('Plan DAG viewport.')).toBeVisible();
    await screen.getByTestId('tab-digest').tap();
    await expect(screen.getByText('FSRS digest.')).toBeVisible();
    await expect(screen.getByText('Plan DAG viewport.')).toBeHidden();
  });

  test('switch toggles', async ({ screen }) => {
    await expect(screen.getByText('Sync on')).toBeVisible();
    await screen.getByTestId('switch-demo').tap();
    await expect(screen.getByText('Sync off')).toBeVisible();
    await screen.getByTestId('switch-demo').tap();
    await expect(screen.getByText('Sync on')).toBeVisible();
  });

  test('command palette filters items', async ({ app, screen }) => {
    await expect(screen.getByTestId('command-item-graph')).toBeVisible();
    await expect(screen.getByTestId('command-item-home')).toBeVisible();
    await screen.getByTestId('command-input').fill('graph');
    await expect(screen.getByTestId('command-item-graph')).toBeVisible();
    await expect(screen.getByTestId('command-item-home')).toBeHidden();
    await screen.getByTestId('command-input').fill('zzzz');
    await expect(screen.getByText('No results.')).toBeVisible();
    await screen.getByTestId('command-input').fill('');
    await expect(screen.getByTestId('command-item-home')).toBeVisible();
    copyFileSync(await app.screenshot('command'), join(SHOTS, 'command.png'));
  });

  test('icons are Hugeicons svgs; no lucide, no emoji', async ({ screen, browser }) => {
    for (const n of ['Home01', 'Search01', 'Brain02', 'Settings01', 'Star', 'Flash', 'Tick02']) {
      await expect(screen.getByTestId(`icon-${n}`)).toBeVisible();
    }
    const r = (await browser.evaluate(`(() => {
      const root = document.querySelector('[data-testid="gallery"]');
      const svgs = root.querySelectorAll('svg');
      const html = document.documentElement.outerHTML;
      const emoji = /\\p{Extended_Pictographic}/u.test(root.innerText);
      const iconSvgs = document.querySelectorAll('[data-testid^="icon-"] svg').length;
      return { svgs: svgs.length, iconSvgs, lucide: /lucide/i.test(html), emoji };
    })()`)) as { svgs: number; iconSvgs: number; lucide: boolean; emoji: boolean };
    expect(r.iconSvgs).toBe(7);
    expect(r.svgs).toBeGreaterThanOrEqual(10);
    expect(r.lucide).toBe(false);
    expect(r.emoji).toBe(false);
  });

  test('computed font-family is Geist Mono', async ({ browser }) => {
    const ff = (await browser.evaluate(
      `getComputedStyle(document.querySelector('[data-testid="gallery-title"]')).fontFamily + '|' + getComputedStyle(document.body).fontFamily`,
    )) as string;
    expect(ff).toMatch(/geist mono/i);
  });
});
