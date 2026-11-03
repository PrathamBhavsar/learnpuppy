#!/usr/bin/env python3
"""Second round, on the questions round one left open. Same log format; reuses r1's strong skeletons.

1. Does low effort cut Haiku's hidden thinking without hurting the lessons?
2. Does one strong call really hold up across 10 lessons, or only the 3 that round one projected from?
3. Is Sonnet enough as the strong writer?
4. How does today's per-step output score on the same blind rubric?
"""
import concurrent.futures as cf
import json
import random
import sys

import run as R

R1 = R.Log("r1")
LOG = R.Log("r2")
HAIKU_LOW = ("claude", "claude-haiku-5-5@low")
SONNET = ("claude", "claude-sonnet-5-5@medium")


def r1_skeleton(topic):
    for row in R1.rows():
        if row["key"] == f"skeleton/{topic}/strong":
            return R.strip_json(row["text"])
    raise SystemExit(f"no r1 skeleton for {topic}")


def haiku_low(topic):
    skel = r1_skeleton(topic)
    used = []
    for i, node in enumerate(skel["nodes"][:3]):
        p = R.PACK.format(n=R.ITEMS_PER_NODE, learner=skel["learner"],
                          node=json.dumps(node, ensure_ascii=False), used=used[-12:])
        r = R.call(LOG, f"pack/{topic}/haiku-low/{i}", *HAIKU_LOW, p, approach="pack", topic=topic,
                   skel="strong", cheap="haiku-low", node=i)
        try:
            used += [x.get("q", "") for x in R.strip_json(r["text"]).get("items", [])]
        except (ValueError, AttributeError):
            pass


def one_shot(topic, name, who):
    p = R.STRONG_PACKS.format(k=10, n=R.ITEMS_PER_NODE, topic=R.TOPICS[topic])
    R.call(LOG, f"oneshot/{topic}/{name}", *who, p, approach="oneshot", topic=topic, writer=name)


def lessons(row):
    return R.strip_json(row["text"]).get("lessons", [])


def judge():
    rng = random.Random(23)
    for topic in R.TOPICS:
        variants = {}
        for row in LOG.rows():
            if row.get("topic") != topic or row.get("error"):
                continue
            try:
                if row["approach"] == "oneshot":
                    ls = lessons(row)
                    # Early and late lessons, judged apart, because decay over a long reply is the question.
                    variants[f"{row['writer']}-10-early"] = ls[:2]
                    variants[f"{row['writer']}-10-late"] = ls[-2:]
                elif row["approach"] == "pack":
                    variants.setdefault("strong-skeleton+haiku-low", []).append(R.strip_json(row["text"]))
            except ValueError:
                pass
        # Anchors from round one, so the two rounds' scores can be compared.
        for row in R1.rows():
            if row["key"] == f"pack/{topic}/strong/haiku/0" or row["key"] == f"pack/{topic}/strong/haiku/1":
                variants.setdefault("strong-skeleton+haiku", []).append(R.strip_json(row["text"]))
        # Today's shape, on Spanish only, where round one logged it. No explanation exists to judge.
        steps = [r for r in R1.rows() if r["approach"] == "baseline" and r.get("topic") == topic
                 and r.get("cheap") == "haiku"]
        if steps:
            variants["today-per-step"] = [{"explain": "(none: this approach writes one question per call)",
                                           "items": [r["text"][:900] for r in steps[:6]]}]
        names = list(variants)
        rng.shuffle(names)
        blind = {f"V{i}": n for i, n in enumerate(names)}
        (LOG.dir / f"blind-{topic}.json").write_text(json.dumps(blind, indent=1))
        lines = []
        for label, n in blind.items():
            for li, lesson in enumerate(variants[n][:2]):
                lines.append(f"\n## {label} lesson {li}\nEXPLAIN: {lesson.get('explain')}\n"
                             + json.dumps(lesson.get("items", [])[:6], ensure_ascii=False))
        prompt = ("You are auditing tutor content blind. Learner: " + R.TOPICS[topic] + "\n"
                  "For each variant score 1-5: accuracy (facts and answer keys are correct; list every wrong one), "
                  "teaching (the explanation really teaches this learner what the items test), "
                  "level (fits this learner), variety (items are not near-duplicates). "
                  'Output JSON only: {"V0":{"accuracy":n,"teaching":n,"level":n,"variety":n,"errors":["..."],'
                  '"note":"one sentence"},...}\n' + "\n".join(lines))
        R.call(LOG, f"judge/{topic}", *R.STRONG, prompt, approach="judge", topic=topic)


def report():
    rows = LOG.rows()
    print(f"{len(rows)} calls, {sum(1 for r in rows if r.get('error'))} failed\n")
    print("| call | in | out | visible chars | secs | parsed | lessons | error |\n|---|---|---|---|---|---|---|---|")
    for r in rows:
        if r["approach"] == "judge":
            continue
        n, ok = "-", "yes"
        try:
            d = R.strip_json(r["text"])
            n = len(d.get("lessons", [])) if r["approach"] == "oneshot" else "-"
        except ValueError:
            ok = "NO"
        print(f"| {r['key']} | {r['tokens_in']} | {r['tokens_out']} | {len(r['text'])} | {r['secs']} | {ok} | {n} | "
              f"{str(r.get('error') or '-')[:80]} |")
    for topic in R.TOPICS:
        j = [r for r in rows if r["approach"] == "judge" and r["topic"] == topic and not r.get("error")]
        if not j:
            continue
        blind = json.loads((LOG.dir / f"blind-{topic}.json").read_text())
        scores = R.strip_json(j[0]["text"])
        print(f"\n### {topic}\n| variant | acc | teach | level | variety | errors | note |\n|---|---|---|---|---|---|---|")
        for label, name in sorted(blind.items(), key=lambda kv: kv[1]):
            s = scores.get(label, {})
            errs = "; ".join(s.get("errors", []))[:260].replace("|", "/")
            print(f"| {name} | {s.get('accuracy')} | {s.get('teaching')} | {s.get('level')} | {s.get('variety')} | "
                  f"{errs or '-'} | {str(s.get('note')).replace('|', '/')[:220]} |")


def main():
    cmd = sys.argv[1]
    if cmd == "gen":
        with cf.ThreadPoolExecutor(4) as ex:
            jobs = [ex.submit(haiku_low, t) for t in R.TOPICS]
            jobs += [ex.submit(one_shot, t, "sonnet", SONNET) for t in R.TOPICS]
            jobs += [ex.submit(one_shot, "spanish", "opus", R.STRONG)]
            [j.result() for j in jobs]
    elif cmd == "judge":
        judge()
    else:
        report()


if __name__ == "__main__":
    main()
