import { beforeEach, describe, test } from '@e2e-dev/web';
import { expect } from 'e2e';

describe('shell navigation, Cmd+A accounts modal, and Cmd+K command palette', () => {
  beforeEach(async ({ app, screen }) => {
    await app.open('/');
    await expect(screen.getByTestId('app-shell')).toBeVisible();
  });

  test('navigates across all 5 left icon rail views and quick action buttons', async ({ screen }) => {
    await expect(screen.getByTestId('navigation-rail')).toBeVisible();
    await expect(screen.getByTestId('view-graph')).toBeVisible();
    await expect(screen.getByTestId('nav-graph')).toHaveAttribute('aria-current', 'page');

    await screen.getByTestId('nav-classroom').tap();
    await expect(screen.getByTestId('view-classroom')).toBeVisible();
    await expect(screen.getByTestId('nav-classroom')).toHaveAttribute('aria-current', 'page');

    await screen.getByTestId('nav-plan').tap();
    await expect(screen.getByTestId('view-plan')).toBeVisible();
    await expect(screen.getByTestId('nav-plan')).toHaveAttribute('aria-current', 'page');

    await screen.getByTestId('nav-review').tap();
    await expect(screen.getByTestId('view-review')).toBeVisible();
    await expect(screen.getByTestId('nav-review')).toHaveAttribute('aria-current', 'page');

    await screen.getByTestId('nav-digest').tap();
    await expect(screen.getByTestId('view-digest')).toBeVisible();
    await expect(screen.getByTestId('nav-digest')).toHaveAttribute('aria-current', 'page');

    // Quick action toolbar routes to Classroom or Review with the selected action
    await screen.getByTestId('action-practice').tap();
    await expect(screen.getByTestId('view-classroom')).toBeVisible();
    await expect(screen.getByTestId('classroom-action')).toContainText('Practice session');

    await screen.getByTestId('action-review').tap();
    await expect(screen.getByTestId('view-review')).toBeVisible();
  });

  test('opens and refreshes the Cmd+A CLI accounts modal showing multi-account slots', async ({ screen }) => {
    await screen.getByTestId('open-accounts').tap();
    await expect(screen.getByTestId('accounts-modal')).toBeVisible();
    await expect(screen.getByTestId('accounts-list')).toBeVisible();
    await expect(screen.getByTestId('account-slot')).toHaveCount(4);
    await expect(screen.getByTestId('accounts-list')).toContainText('pratham.bhavsar501@gmail.com');
    await expect(screen.getByTestId('accounts-list')).toContainText('q97159745@gmail.com');

    await screen.getByTestId('accounts-refresh').tap();
    await expect(screen.getByTestId('account-slot')).toHaveCount(4);

    // Dismiss via backdrop and reopen via Meta+A keyboard shortcut on app-shell
    await screen.getByTestId('accounts-modal-backdrop').tap({ position: { x: 10, y: 10 } });
    await expect(screen.getByTestId('accounts-modal')).toBeHidden();

    await screen.getByTestId('app-shell').press('Control+a');
    await expect(screen.getByTestId('accounts-modal')).toBeVisible();
  });

  test('opens the Cmd+K command palette, filters commands, and navigates on selection', async ({ screen }) => {
    await screen.getByTestId('open-command-palette').tap();
    await expect(screen.getByTestId('command-palette')).toBeVisible();

    const input = screen.getByTestId('command-palette-input');
    await input.fill('Digest');
    await expect(screen.getByTestId('command-palette-item-digest')).toBeVisible();

    await screen.getByTestId('command-palette-item-digest').tap();
    await expect(screen.getByTestId('command-palette')).toBeHidden();
    await expect(screen.getByTestId('view-digest')).toBeVisible();

    // Reopen via keyboard shortcut, filter for CLI accounts, and open modal via command palette
    await screen.getByTestId('app-shell').press('Control+k');
    await expect(screen.getByTestId('command-palette')).toBeVisible();
    await input.fill('CLI accounts');
    await expect(screen.getByTestId('command-palette-item-accounts')).toBeVisible();
    await screen.getByTestId('command-palette-item-accounts').tap();
    await expect(screen.getByTestId('accounts-modal')).toBeVisible();
  });
});
