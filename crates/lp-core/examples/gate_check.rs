//! Runs the real course parser and gate over saved Author output, for the G3 benchmark
//! (`bench/cheap_tutor/round5.py`): one JSON summary line per file given on the command line.
//!
//!   cargo run -q -p lp-core --example gate_check -- out1.json out2.json
//!
//! `angle_leaks` counts alternate explanations that contain a quiz answer verbatim. An angle exists
//! to re-teach after a miss without revealing the answer, so any leak is a failure of the prompt.

use lp_core::course::{gate_course, parse_course};

fn main() {
    for path in std::env::args().skip(1) {
        let text = std::fs::read_to_string(&path).unwrap_or_default();
        let line = match parse_course(&text, "bench", Vec::new()) {
            Err(e) => serde_json::json!({ "file": path, "parsed": false, "error": e.to_string() }),
            Ok(mut course) => {
                let raw = course.clone();
                let report = gate_course(&mut course);
                let mut leaks = 0usize;
                for lesson in &raw.lessons {
                    for angle in &lesson.angles {
                        let hay = angle.to_lowercase();
                        for item in &lesson.items {
                            let a = item.answer.trim().to_lowercase();
                            if a.chars().count() >= 6 && hay.contains(&a) {
                                leaks += 1;
                                break;
                            }
                        }
                    }
                }
                serde_json::json!({
                    "file": path,
                    "parsed": true,
                    "lessons": raw.lessons.len(),
                    "items": raw.lessons.iter().map(|l| l.items.len()).sum::<usize>(),
                    "with_predict": raw.lessons.iter().filter(|l| l.predict.is_some()).count(),
                    "with_angles": raw.lessons.iter().filter(|l| !l.angles.is_empty()).count(),
                    "procedural_with_rungs": raw.lessons.iter().filter(|l| !l.rungs.is_empty()).count(),
                    "dropped": report.dropped.len(),
                    "warnings": report.warnings.len(),
                    "needs_repair": report.needs_repair.len(),
                    "angle_leaks": leaks,
                })
            }
        };
        println!("{line}");
    }
}
