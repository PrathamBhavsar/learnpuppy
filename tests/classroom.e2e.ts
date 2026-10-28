import { beforeEach, describe, test } from '@e2e-dev/web';
import { expect } from 'e2e';

describe('classroom question dock answer, confidence slider, and graded verdict', () => {
  beforeEach(async ({ app, screen }) => {
    await app.open('/');
    await screen.getByTestId('nav-classroom').tap();
    await expect(screen.getByTestId('view-classroom')).toBeVisible();
  });

  test('submits answer options with confidence levels and renders graded verdicts', async ({ screen }) => {
    await expect(screen.getByTestId('question-dock')).toBeVisible();
    await expect(screen.getByTestId('graded-verdict')).toHaveAttribute('data-verdict', 'correct');

    // Select an incorrect option [B], set confidence to 2 (Low), enter reasoning, and grade
    await screen.getByTestId('option-B').tap();
    await expect(screen.getByTestId('option-B')).toHaveAttribute('aria-checked', 'true');

    await screen.getByTestId('confidence-slider-step-2').tap();
    await expect(screen.getByTestId('confidence-slider-value')).toHaveText('2 · Low');

    await screen.getByTestId('tab-trigger-reasoning').tap();
    await expect(screen.getByTestId('tab-trigger-reasoning')).toHaveAttribute('aria-selected', 'true');

    await screen.getByTestId('submit-answer-btn').tap();
    await expect(screen.getByTestId('graded-verdict')).toHaveAttribute('data-verdict', 'incorrect');
    await expect(screen.getByTestId('verdict-headline')).toContainText('Incorrect — review the derivation below');
    await expect(screen.getByTestId('verdict-badge')).toContainText('Choice [B] · Conf 2/5');
    await expect(screen.getByTestId('verdict-envelope')).toContainText(
      'selected: opt_mermaid_limit; confidence: 2;',
    );

    // Select [I] I don't know, set confidence to 1 (Guess), and grade
    await screen.getByTestId('option-I').tap();
    await screen.getByTestId('confidence-slider-step-1').tap();
    await screen.getByTestId('submit-answer-btn').tap();
    await expect(screen.getByTestId('graded-verdict')).toHaveAttribute('data-verdict', 'idk');
    await expect(screen.getByTestId('verdict-headline')).toContainText("Marked 'I don't know' — explanation unlocked");

    // Select the correct option [A], set confidence to 5 (Certain), and grade
    await screen.getByTestId('option-A').tap();
    await screen.getByTestId('confidence-slider-step-5').tap();
    await expect(screen.getByTestId('confidence-slider-value')).toHaveText('5 · Certain');
    await screen.getByTestId('submit-answer-btn').tap();
    await expect(screen.getByTestId('graded-verdict')).toHaveAttribute('data-verdict', 'correct');
    await expect(screen.getByTestId('verdict-headline')).toContainText('Correct — proposition verified');
    await expect(screen.getByTestId('verdict-badge')).toContainText('Choice [A] · Conf 5/5');
  });

  test('advances Plan DAG node statuses when teaching a node via quiz tool call', async ({ screen }) => {
    await expect(screen.getByTestId('plan-dag-pane')).toBeVisible();
    await expect(screen.getByTestId('dag-node-row-snapshot-isolation')).toHaveAttribute('data-status', 'current');
    await expect(screen.getByTestId('dag-node-row-fts5-sync')).toHaveAttribute('data-status', 'upcoming');

    await screen.getByTestId('dag-focus-node-fts5-sync').tap();
    await expect(screen.getByTestId('dag-node-row-fts5-sync')).toHaveAttribute('data-status', 'current');
    await expect(screen.getByTestId('dag-node-row-snapshot-isolation')).toHaveAttribute('data-status', 'visited');
  });
});
