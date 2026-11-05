//! F24: gap pauses — a blank pause between steps, picked at random so the interval is never a
//! fixed metronome, but seeded so replay reproduces the exact same schedule. The rate is an
//! average, not a guarantee: "one per `gap_pause_every_minutes` active minutes" means each
//! active minute independently has a `1 / gap_pause_every_minutes` chance of carrying a pause,
//! which gives a geometric gap with that mean and no fixed period.
//!
//! Caller contract: call `gap_pause_due` once per completed step, with the course's seed and
//! the active-minute count at that step boundary (never mid-answer, per F24).

use rand::{rngs::SmallRng, Rng, SeedableRng};

/// Whether a gap pause fires at this active-minute boundary. `gap_pause_every_minutes == 0`
/// turns gap pauses off entirely (the plan's "setting to turn it off").
pub fn gap_pause_due(seed: u64, active_minute: u32, gap_pause_every_minutes: u32) -> bool {
    if gap_pause_every_minutes == 0 || active_minute == 0 {
        return false;
    }
    let mut rng = SmallRng::seed_from_u64(seed ^ u64::from(active_minute));
    let p = (1.0 / f64::from(gap_pause_every_minutes)).clamp(0.0, 1.0);
    rng.gen_bool(p)
}

/// The full set of active-minute marks, in `1..=total_active_minutes`, that carry a gap pause
/// (F24). A thin wrapper over `gap_pause_due` so the schedule and the live, one-at-a-time check
/// a running bout makes are always the same function of `seed`.
pub fn gap_pause_schedule(
    seed: u64,
    total_active_minutes: u32,
    gap_pause_every_minutes: u32,
) -> Vec<u32> {
    (1..=total_active_minutes)
        .filter(|&minute| gap_pause_due(seed, minute, gap_pause_every_minutes))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    #[test]
    fn turned_off_never_fires() {
        assert_eq!(gap_pause_schedule(1, 10_000, 0), Vec::<u32>::new());
    }

    #[test]
    fn minute_zero_never_fires() {
        assert!(!gap_pause_due(1, 0, 5));
    }

    proptest! {
        #[test]
        fn same_seed_gives_same_schedule(
            seed in any::<u64>(),
            total in 1u32..2000,
            every in 1u32..30,
        ) {
            let a = gap_pause_schedule(seed, total, every);
            let b = gap_pause_schedule(seed, total, every);
            prop_assert_eq!(a, b);
        }

        #[test]
        fn average_gap_stays_near_the_setting_and_is_not_fixed(
            every in 2u32..20,
        ) {
            let total_active_minutes = every * 4000;
            let mut gaps: Vec<u32> = Vec::new();
            let mut last = 0u32;
            for seed in 0u64..12 {
                let schedule = gap_pause_schedule(seed, total_active_minutes, every);
                // Enough samples to estimate the mean gap without too much noise.
                prop_assume!(schedule.len() > 50);
                for &minute in &schedule {
                    if last != 0 {
                        gaps.push(minute - last);
                    }
                    last = minute;
                }
                last = 0;
            }
            prop_assume!(gaps.len() > 100);
            let mean: f64 = gaps.iter().map(|&g| f64::from(g)).sum::<f64>() / gaps.len() as f64;
            // The geometric mean gap is `every`; allow generous slack for sampling noise.
            prop_assert!(
                (mean - f64::from(every)).abs() < f64::from(every) * 0.35,
                "mean gap {mean} too far from setting {every}"
            );
            // No fixed interval: the observed gaps must not all be identical.
            prop_assert!(gaps.iter().any(|&g| g != gaps[0]));
        }
    }
}
