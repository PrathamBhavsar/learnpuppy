#!/usr/bin/env python3
"""G1 failed on output size, so decide with the final schema and real cost weights.

Compares, on two topics: Opus one shot at default effort (from r3), Opus one shot at low effort, and
the two-stage Haiku path writing all 10 lessons. Logs the CLI's total_cost_usd as the only published
weighting between models. Subscription quota weighting stays unknown.
"""
import concurrent.futures as cf
import json
import random
import subprocess
import sys
import tempfile
import time

import run as R
import round3 as R3

LOG = R.Log("r4")
TOPICS = ("spanish", "stats")

OUTLINE = """Plan a course for this learner. Output JSON only, no code fence:
{{"learner":"<3 to 5 lines: level, goal, known gaps>","must_fix":["each placement mistake"],"lessons":[{{"id":"slug",
"title":"...","objective":"what they can do after","teach_notes":"<40 words: how to explain it to THIS learner>",
"key_facts":["up to 6 exact facts this lesson teaches, each correct"],"fixes":["placement mistakes it corrects"]}}]}}
Exactly 10 lessons in prerequisite order, starting at the edge of what the learner knows. Every placement
mistake must be in some lesson's fixes.
Learner: {topic}"""

LESSON = """Write one lesson. Output JSON only, no code fence:
{{"explain":"<under 120 words, with one concrete example>","items":[{{"type":"mcq","q":"...","options":["a","b","c","d"],
"answer":"<exact option text>","why":"<one sentence>","tests":0}},{{"type":"recall","q":"...","answer":"<model answer>",
"accept":["other correct answers"],"why":"...","tests":1}}]}}
Exactly 6 items, 4 mcq then 2 recall. `tests` is the index of the key fact the item tests. Explain must teach every
key fact. Options similar in length and all plausible. Do not repeat any used question. Assume earlier lessons were
studied. Only make comparisons with the learner's background that you are sure are correct.
Learner: {learner}
Lesson: {lesson}
Used questions: {used}"""


def call(key, model, prompt, **tags):
    for row in LOG.rows():
        if row["key"] == key and not row.get("error"):
            return row
    name, _, effort = model.partition("@")
    cmd = ["claude", "-p", "--output-format", "json", "--strict-mcp-config", "--tools", "",
           "--setting-sources", "project,local", "--model", name, "--system-prompt", R.TERSE]
    if effort:
        cmd += ["--effort", effort]
    t = time.time()
    r = subprocess.run(cmd, input=prompt, text=True, capture_output=True, timeout=500,
                       env=R.clean_env(), cwd=tempfile.gettempdir())
    d = json.loads(r.stdout)
    u = d.get("usage", {})
    row = dict(key=key, model=model, text=d.get("result", ""), usd=d.get("total_cost_usd", 0),
               tokens_in=sum(u.get(k, 0) for k in ("input_tokens", "cache_read_input_tokens", "cache_creation_input_tokens")),
               tokens_out=u.get("output_tokens", 0), secs=round(time.time() - t, 1),
               error=d.get("result") if d.get("is_error") else None, **tags)
    LOG.add(row)
    print(f"{key}: in={row['tokens_in']} out={row['tokens_out']} ${row['usd']:.4f} {row['secs']}s", flush=True)
    return row


def opus_low(topic):
    p = R3.AUTHOR.format(k=10, n=R.ITEMS_PER_NODE, topic=R3.TOPICS[topic])
    call(f"opus-low/{topic}", "claude-opus-5-5@low", p, approach="oneshot", topic=topic, path="opus-low")


def opus_default_cost(topic):
    """One more default-effort run, because r3 did not log cost."""
    p = R3.AUTHOR.format(k=10, n=R.ITEMS_PER_NODE, topic=R3.TOPICS[topic])
    call(f"opus-default/{topic}", "claude-opus-5-5", p, approach="oneshot", topic=topic, path="opus-default")


def haiku_path(topic):
    o = call(f"haiku-outline/{topic}", "claude-haiku-5-5", OUTLINE.format(topic=R3.TOPICS[topic]),
             approach="outline", topic=topic, path="haiku")
    outline = R.strip_json(o["text"])
    used = []
    for i, lesson in enumerate(outline["lessons"][:10]):
        p = LESSON.format(learner=outline["learner"], lesson=json.dumps(lesson, ensure_ascii=False), used=used[-20:])
        r = call(f"haiku-lesson/{topic}/{i}", "claude-haiku-5-5", p, approach="lesson", topic=topic, path="haiku", i=i)
        try:
            used += [x.get("q", "") for x in R.strip_json(r["text"]).get("items", [])]
        except (ValueError, AttributeError):
            pass


def variants(topic):
    out, lessons = {}, {}
    for row in LOG.rows():
        if row.get("topic") != topic or row.get("error"):
            continue
        if row["approach"] == "oneshot":
            ls = R.strip_json(row["text"]).get("lessons", [])
            out[f"{row['path']}-early"], out[f"{row['path']}-late"] = ls[:2], ls[-2:]
        elif row["approach"] == "lesson":
            try:
                lessons[row["i"]] = R.strip_json(row["text"])
            except ValueError:
                pass
    if lessons:
        order = [lessons[i] for i in sorted(lessons)]
        out["haiku-early"], out["haiku-late"] = order[:2], order[-2:]
    return out


def judge():
    rng = random.Random(41)
    for topic in TOPICS:
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
                  + R3.TOPICS[topic] + "\nFor each variant score 1-5: accuracy (facts and answer keys are correct; list "
                  "every wrong one), teaching (the explanation really teaches what the items test), level, variety. "
                  "Also count untaught: items testing something the explanation did not teach. "
                  'Output JSON only: {"V0":{"accuracy":n,"teaching":n,"level":n,"variety":n,"untaught":n,'
                  '"errors":["..."]},...}\n' + "\n".join(lines))
        call(f"judge/{topic}", "claude-opus-5-5", prompt, approach="judge", topic=topic)


def report():
    rows = LOG.rows()
    print("| path | topic | calls | in | out | usd | secs total | lessons parsed |\n|---|---|---|---|---|---|---|---|")
    for topic in TOPICS:
        for path in ("opus-default", "opus-low", "haiku"):
            rs = [r for r in rows if r.get("path") == path and r["topic"] == topic]
            if not rs:
                continue
            parsed = 0
            for r in rs:
                try:
                    d = R.strip_json(r["text"])
                    parsed += len(d.get("lessons", [])) if r["approach"] == "oneshot" else r["approach"] == "lesson"
                except ValueError:
                    pass
            print(f"| {path} | {topic} | {len(rs)} | {sum(r['tokens_in'] for r in rs)} | {sum(r['tokens_out'] for r in rs)} | "
                  f"{sum(r['usd'] for r in rs):.4f} | {sum(r['secs'] for r in rs):.0f} | {parsed} |")
    for topic in TOPICS:
        j = [r for r in rows if r["approach"] == "judge" and r["topic"] == topic]
        if not j:
            continue
        blind = json.loads((LOG.dir / f"blind-{topic}.json").read_text())
        scores = R.strip_json(j[0]["text"])
        print(f"\n### {topic}\n| variant | acc | teach | level | variety | untaught | errors |\n|---|---|---|---|---|---|---|")
        for label, name in sorted(blind.items(), key=lambda kv: kv[1]):
            s = scores.get(label, {})
            print(f"| {name} | {s.get('accuracy')} | {s.get('teaching')} | {s.get('level')} | {s.get('variety')} | "
                  f"{s.get('untaught')} | {'; '.join(s.get('errors', []))[:220].replace('|', '/') or '-'} |")


def main():
    if sys.argv[1] == "gen":
        with cf.ThreadPoolExecutor(4) as ex:
            jobs = [ex.submit(f, t) for t in TOPICS for f in (opus_low, opus_default_cost, haiku_path)]
            [j.result() for j in jobs]
    elif sys.argv[1] == "judge":
        judge()
    else:
        report()


if __name__ == "__main__":
    main()
