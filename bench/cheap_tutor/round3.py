#!/usr/bin/env python3
"""Gate G1: does one Opus call hold up on more topics, with the final schema, and at 20 lessons?

Pass (brain/cheap-tutor/task-dag.md): every run parses, late lessons within 1 point of early ones,
under about 15,000 output for 10 lessons. Fail: default to cheap skeleton + Haiku.
"""
import concurrent.futures as cf
import json
import random
import sys

import run as R

LOG = R.Log("r3")
TOPICS = dict(R.TOPICS)
TOPICS["stats"] = ("I run A/B tests at work and want to really understand the statistics. I know basic algebra "
                   "and averages. Probe result: thinks a p-value is the chance the variant is better; has never "
                   "computed a confidence interval; stops tests as soon as they look significant.")
RUNS = [("rust", 10, 0), ("rust", 10, 1), ("stats", 10, 0), ("stats", 10, 1), ("spanish", 10, 0), ("spanish", 20, 0)]

AUTHOR = """Plan a course and write its first {k} lessons. Output JSON only, no code fence:
{{"learner":"<3 to 5 lines: level, goal, known gaps>","lessons":[{{"id":"slug","title":"...",
"objective":"what they can do after","explain":"<under 120 words, with one concrete example>",
"key_facts":["each fact the lesson teaches, stated exactly; all must appear in explain"],
"fixes":["placement mistakes this lesson corrects, may be empty"],
"items":[{{"type":"mcq","q":"...","options":["a","b","c","d"],"answer":"<exact option text>","why":"<one sentence>","tests":0}},
{{"type":"recall","q":"...","answer":"<model answer>","accept":["other correct answers"],"why":"...","tests":1}}]}}]}}
Rules:
1. Order lessons by prerequisite, starting at the edge of what the learner knows.
2. Teach everything you test: `tests` is the index of the key fact the item tests, and that fact must be in explain.
3. Every placement mistake must appear in some lesson's fixes, and be tested there.
4. Each lesson has exactly {n} items, 4 mcq then 2 recall. Options similar in length and all plausible.
5. Use the learner's other languages and background only where you are sure the comparison is correct.
Learner: {topic}"""


def gen(topic, k, i):
    p = AUTHOR.format(k=k, n=R.ITEMS_PER_NODE, topic=TOPICS[topic])
    R.call(LOG, f"author/{topic}/{k}/{i}", *R.STRONG, p, approach="author", topic=topic, k=k, run=i)


def lessons(row):
    return R.strip_json(row["text"]).get("lessons", [])


def judge():
    rng = random.Random(31)
    for topic in TOPICS:
        variants = {}
        for row in LOG.rows():
            if row.get("approach") != "author" or row["topic"] != topic or row.get("error"):
                continue
            try:
                ls = lessons(row)
            except ValueError:
                continue
            tag = f"{row['k']}L-run{row['run']}"
            variants[f"{tag}-early"] = ls[:2]
            variants[f"{tag}-late"] = ls[-2:]
        names = list(variants)
        rng.shuffle(names)
        blind = {f"V{i}": n for i, n in enumerate(names)}
        (LOG.dir / f"blind-{topic}.json").write_text(json.dumps(blind, indent=1))
        lines = []
        for label, n in blind.items():
            for li, lesson in enumerate(variants[n]):
                keep = {k: lesson.get(k) for k in ("title", "explain", "items")}
                lines.append(f"\n## {label} lesson {li}\n" + json.dumps(keep, ensure_ascii=False))
        prompt = ("You are auditing tutor content blind. Lessons labelled late come from later in a course, so "
                  "assume earlier lessons covered prerequisites. Learner: " + TOPICS[topic] + "\n"
                  "For each variant score 1-5: accuracy (facts and answer keys are correct; list every wrong one), "
                  "teaching (the explanation really teaches what the items test), level, variety. Also count "
                  "untaught: items testing something the explanation did not teach. "
                  'Output JSON only: {"V0":{"accuracy":n,"teaching":n,"level":n,"variety":n,"untaught":n,'
                  '"errors":["..."],"note":"one sentence"},...}\n' + "\n".join(lines))
        R.call(LOG, f"judge/{topic}", *R.STRONG, prompt, approach="judge", topic=topic)


def report():
    rows = LOG.rows()
    print("| run | in | out | secs | parsed | lessons | items | gate pass | bad tests idx | error |")
    print("|---|---|---|---|---|---|---|---|---|---|")
    for r in rows:
        if r["approach"] != "author":
            continue
        parsed, nl, ni, ok, badt = "NO", 0, 0, 0, 0
        try:
            ls = lessons(r)
            parsed, nl = "yes", len(ls)
            for l in ls:
                for it in l.get("items", []):
                    ni += 1
                    ok += not R.gate_item(it)
                    t = it.get("tests")
                    badt += not (isinstance(t, int) and 0 <= t < len(l.get("key_facts", [])))
        except ValueError:
            pass
        print(f"| {r['key']} | {r['tokens_in']} | {r['tokens_out']} | {r['secs']} | {parsed} | {nl} | {ni} | {ok} | "
              f"{badt} | {str(r.get('error') or '-')[:60]} |")
    for topic in TOPICS:
        j = [r for r in rows if r["approach"] == "judge" and r["topic"] == topic and not r.get("error")]
        if not j:
            continue
        blind = json.loads((LOG.dir / f"blind-{topic}.json").read_text())
        scores = R.strip_json(j[0]["text"])
        print(f"\n### {topic}\n| variant | acc | teach | level | variety | untaught | errors |\n|---|---|---|---|---|---|---|")
        for label, name in sorted(blind.items(), key=lambda kv: kv[1]):
            s = scores.get(label, {})
            print(f"| {name} | {s.get('accuracy')} | {s.get('teaching')} | {s.get('level')} | {s.get('variety')} | "
                  f"{s.get('untaught')} | {'; '.join(s.get('errors', []))[:240].replace('|', '/') or '-'} |")


def main():
    cmd = sys.argv[1]
    if cmd == "gen":
        with cf.ThreadPoolExecutor(3) as ex:
            [j.result() for j in [ex.submit(gen, *r) for r in RUNS]]
    elif cmd == "judge":
        judge()
    else:
        report()


if __name__ == "__main__":
    main()
