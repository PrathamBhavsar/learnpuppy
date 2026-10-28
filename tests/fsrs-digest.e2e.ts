import { beforeEach, describe, test } from '@e2e-dev/web';
import { expect } from 'e2e';

describe('FSRS 4-choice flashcard review grading and CLI digest Fold into Graph', () => {
  beforeEach(async ({ app, screen }) => {
    await app.open('/');
  });

  test('selects 4-choice FSRS option, reveals rationale, and grades recall with FSRS v5', async ({ screen }) => {
    await screen.getByTestId('nav-review').tap();
    await expect(screen.getByTestId('view-review')).toBeVisible();
    await expect(screen.getByTestId('fsrs-completed-badge')).toHaveText('Reviewed: 0');
    await expect(screen.getByTestId('fsrs-card-front')).toContainText('conversation_summaries.db');

    // Select option A (correct), verify rationale banner reveals
    await screen.getByTestId('fsrs-option-A').tap();
    await expect(screen.getByTestId('fsrs-rationale-banner')).toBeVisible();
    await expect(screen.getByTestId('fsrs-verdict-badge')).toContainText('Correct ([A])');

    // Grade recall with Good and verify deck advances and logs the review
    await screen.getByTestId('fsrs-grade-good').tap();
    await expect(screen.getByTestId('fsrs-completed-badge')).toHaveText('Reviewed: 1');
    await expect(screen.getByTestId('fsrs-review-log')).toBeVisible();
    await expect(screen.getByTestId('fsrs-log-entry-0')).toContainText('c_agy_snapshot: Good');
    await expect(screen.getByTestId('fsrs-card-front')).toContainText('fsrs_reviews');
  });

  test('folds distilled CLI transcript decisions into the Knowledge Graph and jumps to Graph view', async ({ screen }) => {
    await screen.getByTestId('nav-digest').tap();
    await expect(screen.getByTestId('view-digest')).toBeVisible();
    await expect(screen.getByTestId('digest-folded-badge')).toHaveText('Graph Folded: 4');
    await expect(screen.getByTestId('digest-decision-card-d_fsrs_deterministic_replay')).toBeVisible();

    // Fold entire session into Graph
    await screen.getByTestId('digest-fold-into-graph').tap();
    await expect(screen.getByTestId('digest-folded-badge')).toHaveText('Graph Folded: 6');
    await expect(screen.getByTestId('digest-fold-status')).toContainText(
      'Folded 2 DecisionRecord(s) from ses_tracker_fsrs_01 into Knowledge Graph',
    );
    await expect(screen.getByTestId('digest-session-folded-ses_tracker_fsrs_01')).toHaveText('Folded');

    // Fold a single decision card and jump to Graph view
    await screen.getByTestId('digest-fold-single-d_fsrs_deterministic_replay').tap();
    await expect(screen.getByTestId('digest-folded-badge')).toHaveText('Graph Folded: 7');

    await screen.getByTestId('digest-jump-to-graph').tap();
    await expect(screen.getByTestId('view-graph')).toBeVisible();
  });
});
