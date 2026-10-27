//! `learnpuppy review [--limit 15]`: interactive FSRS flashcard session.

use std::io::{BufRead, Write};

use clap::Args as ClapArgs;
use lp_core::fsrs::Rating;
use lp_core::ports::TrackerStore;

use crate::review::{due_cards, grade};
use crate::state::now_ms;

#[derive(ClapArgs)]
pub struct Args {
    #[arg(long, default_value_t = 15)]
    limit: usize,
}

/// Prompts until a number in `1..=4` arrives; `None` on EOF or `q`.
fn choose(
    input: &mut dyn BufRead,
    out: &mut dyn Write,
    prompt: &str,
) -> anyhow::Result<Option<usize>> {
    loop {
        write!(out, "{prompt} ")?;
        out.flush()?;
        let mut line = String::new();
        if input.read_line(&mut line)? == 0 || line.trim() == "q" {
            return Ok(None);
        }
        if let Ok(n @ 1..=4) = line.trim().parse::<usize>() {
            return Ok(Some(n));
        }
    }
}

pub fn run(
    tracker: &dyn TrackerStore,
    args: Args,
    input: &mut dyn BufRead,
    out: &mut dyn Write,
) -> anyhow::Result<()> {
    let due = due_cards(tracker, now_ms())?;
    if due.is_empty() {
        writeln!(out, "nothing due")?;
    }
    let mut done = 0;
    for card in due.into_iter().take(args.limit) {
        writeln!(out, "\n{}", card.front)?;
        for (i, o) in card.options.iter().enumerate() {
            writeln!(out, "  {}) {o}", i + 1)?;
        }
        let Some(pick) = choose(input, out, "answer [1-4, q quits]:")? else {
            break;
        };
        let right = usize::from(card.correct_index) + 1;
        let verdict = if pick == right { "correct" } else { "wrong" };
        writeln!(out, "{verdict}: {right}) — {}", card.rationale)?;
        let Some(r) = choose(input, out, "grade [1 again, 2 hard, 3 good, 4 easy]:")? else {
            break;
        };
        let rating = [Rating::Again, Rating::Hard, Rating::Good, Rating::Easy][r - 1];
        grade(tracker, &card.id, rating, now_ms())?;
        done += 1;
    }
    writeln!(out, "\nreviewed {done}")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::review::tests::card;
    use lp_store::SqliteTrackerStore;

    #[test]
    fn reviews_due_cards_and_stops_at_quit() {
        let d = tempfile::tempdir().unwrap();
        let t = SqliteTrackerStore::open(d.path()).unwrap();
        t.upsert_card(&card("c1", 0)).unwrap();
        t.upsert_card(&card("c2", 1)).unwrap();
        let mut out = Vec::new();
        // card 1: pick 3 (right), grade good; card 2: quit at the answer prompt.
        run(
            &t,
            Args { limit: 15 },
            &mut "3\n3\nq\n".as_bytes(),
            &mut out,
        )
        .unwrap();
        let text = String::from_utf8(out).unwrap();
        assert!(text.contains("correct: 3)") && text.contains("reviewed 1"));
        assert_eq!(t.reviews().unwrap().len(), 1);
        assert_eq!(due_cards(&t, now_ms()).unwrap().len(), 1);
    }
}
