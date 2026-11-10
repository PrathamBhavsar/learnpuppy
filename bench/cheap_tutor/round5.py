#!/usr/bin/env python3
"""G3: does the richer Author schema (learning-method plan, M5) still hold up on real calls?

Uses the shipped prompt, `crates/lp-agent/src/course/prompts/author.md`, unchanged, so what is
measured is what the app sends. Three topics, two runs each: Spanish and Rust (as in G1) and music
theory, a topic that is neither a language nor programming. Every call is logged to
`results/<G3_RUN>/calls.jsonl` (default `r5`), so a re-run, `report` and `check` cost nothing once the calls exist.

  python3 bench/cheap_tutor/round5.py gen      # 6 strong calls
  python3 bench/cheap_tutor/round5.py check    # real Rust parser and gate over every output
  python3 bench/cheap_tutor/round5.py judge    # 3 blind judge calls: early vs late lessons
  python3 bench/cheap_tutor/round5.py report

G3 passes when, per brain/learning-method/task-dag.md: every run parses, output stays under
about 28,000 tokens, late lessons score within 1 point of early ones, and no angle states an answer.
"""
import concurrent.futures as cf
import json
import random
import subprocess
import sys
from pathlib import Path

import run as R
import round4 as R4

HERE = Path(__file__).resolve().parent
REPO = HERE.parent.parent
LOG = R.Log(R.os.environ.get("G3_RUN", "r5"))
R4.LOG = LOG  # `call` caches and appends through the module's log
PROMPT = (REPO / "crates/lp-agent/src/course/prompts/author.md").read_text()
MODEL = "claude-opus-5-5@low"  # the default Author, decided at G1 (benchmark.md section 9)
RUNS = (1, 2)
MAX_OUT_TOKENS = 28_000
MAX_RAW_ANGLE_LEAKS = 2  # per 10-lesson run, before the gate drops them

TOPICS = {
    "spanish": R.TOPICS["spanish"],
    "rust": R.TOPICS["rust"],
    "music": "Learn music theory well enough to write my own chord progressions. I play guitar by ear, "
    "know open chords and a few barre chords, and can't read notation. Probe result: names the notes "
    "of a C major scale, guessed that a key has seven chords, did not know what makes a chord minor.",
}


def render(topic):
    return PROMPT.replace("{k}", "10").replace("{n}", str(R.ITEMS_PER_NODE)).replace("{topic}", topic)


def gen():
    jobs = [(t, text, run) for t, text in TOPICS.items() for run in RUNS]
    with cf.ThreadPoolExecutor(max_workers=3) as ex:
        for f in [ex.submit(R4.call, f"author/{t}/{run}", MODEL, render(text), approach="oneshot", topic=t, run=run)
                  for t, text, run in jobs]:
            f.result()


def outputs():
    out = {}
    for row in LOG.rows():
        if row.get("approach") == "oneshot" and not row.get("error"):
            out[(row["topic"], row["run"])] = row
    return out


def cargo_check(files):
    cmd = ["cargo", "run", "-q", "-p", "lp-core", "--example", "gate_check", "--", *map(str, files)]
    r = subprocess.run(cmd, cwd=REPO, capture_output=True, text=True, env=R.clean_env() | {"PATH": R.os.environ["PATH"]})
    if r.returncode != 0:
        sys.exit(r.stderr)
    return [json.loads(line) for line in r.stdout.splitlines()]


def check():
    rows = outputs()
    d = LOG.dir / "outputs"
    d.mkdir(exist_ok=True)
    files = []
    for (topic, run), row in sorted(rows.items()):
        f = d / f"{topic}-{run}.json"
        f.write_text(row["text"])
        files.append(f)
    results = cargo_check(files)
    (LOG.dir / "gate.json").write_text(json.dumps(results, indent=1))
    for r in results:
        print(json.dumps(r))


def variants(topic):
    v = {}
    for (t, run), row in sorted(outputs().items()):
        if t != topic:
            continue
        ls = R.strip_json(row["text"]).get("lessons", [])
        v[f"run{run}-early"], v[f"run{run}-late"] = ls[:2], ls[-2:]
    return v


def judge():
    rng = random.Random(53)
    for topic, text in TOPICS.items():
        v = variants(topic)
        names = list(v)
        rng.shuffle(names)
        blind = {f"V{i}": n for i, n in enumerate(names)}
        (LOG.dir / f"blind-{topic}.json").write_text(json.dumps(blind, indent=1))
        lines = []
        for label, n in blind.items():
            for li, lesson in enumerate(v[n]):
                keep = {k: lesson.get(k) for k in ("explain", "items")}
                lines.append(f"\n## {label} lesson {li}\n" + json.dumps(keep, ensure_ascii=False))
        prompt = ("You are auditing tutor content blind. Variants named only V0, V1 and so on. Some are from the start "
                  "of a 10-lesson course and some from its end. A later lesson's level is right if it is a sensible "
                  "next step after earlier lessons, even when it would be too hard on day one. Learner at the start: "
                  + text + "\nFor each variant score 1-5: accuracy (facts and answer keys are correct; list "
                  "every wrong one), teaching (the explanation really teaches what the items test), level, variety. "
                  "Also count untaught: items testing something the explanation did not teach. "
                  'Output JSON only: {"V0":{"accuracy":n,"teaching":n,"level":n,"variety":n,"untaught":n,'
                  '"errors":["..."]},...}\n' + "\n".join(lines))
        R4.call(f"judge/{topic}", "claude-opus-5-5", prompt, approach="judge", topic=topic)


def report():
    rows = outputs()
    gate = {(Path(g["file"]).stem): g for g in json.loads((LOG.dir / "gate.json").read_text())} if (LOG.dir / "gate.json").exists() else {}
    verdicts = []
    print("| topic | run | out tokens | parsed | lessons | items | repair | dropped | raw angle leaks | usd |\n|---|---|---|---|---|---|---|---|---|---|")
    for (topic, run), row in sorted(rows.items()):
        g = gate.get(f"{topic}-{run}", {})
        # `angle_leaks` counts raw output; `gate_course` drops those angles, so none reach the learner.
        # The raw count is reported because it measures the prompt, and a high one means a weak prompt.
        ok = g.get("parsed") and row["tokens_out"] < MAX_OUT_TOKENS and g.get("angle_leaks", 99) <= MAX_RAW_ANGLE_LEAKS
        verdicts.append(bool(ok))
        print(f"| {topic} | {run} | {row['tokens_out']} | {g.get('parsed')} | {g.get('lessons')} | {g.get('items')} | "
              f"{g.get('needs_repair')} | {g.get('dropped')} | {g.get('angle_leaks')} | {row['usd']:.3f} |")
    print("\nlate vs early (judge, 1-5; passes when late is within 1 of early):")
    worst = 0.0
    for topic in TOPICS:
        jr = next((r for r in LOG.rows() if r.get("approach") == "judge" and r["topic"] == topic and not r.get("error")), None)
        bf = LOG.dir / f"blind-{topic}.json"
        if not jr or not bf.exists():
            continue
        blind = json.loads(bf.read_text())
        scores = R.strip_json(jr["text"])
        for run in RUNS:
            early = next(k for k, n in blind.items() if n == f"run{run}-early")
            late = next(k for k, n in blind.items() if n == f"run{run}-late")
            for dim in ("accuracy", "teaching", "level", "variety"):
                drop = scores[early][dim] - scores[late][dim]
                worst = max(worst, drop)
            print(f"- {topic} run{run}: early {scores[early]} | late {scores[late]}")
    print(f"\nlargest early-to-late drop on any dimension: {worst:.1f}")
    print("G3:", "PASS" if verdicts and all(verdicts) and len(verdicts) == 6 and worst <= 1 else "FAIL (read the table)")


def main():
    cmd = sys.argv[1] if len(sys.argv) > 1 else "report"
    {"gen": gen, "check": check, "judge": judge, "report": report}[cmd]()


if __name__ == "__main__":
    main()
