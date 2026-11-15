#!/usr/bin/env python3
"""Which tutoring approach gets the most teaching out of the least model usage?

Runs real CLI calls and writes every call to results/<run>/calls.jsonl, so a report can be rebuilt
without spending quota again. See brain/cheap-tutor/benchmark.md for the method.

  python3 bench/cheap_tutor/run.py selftest
  python3 bench/cheap_tutor/run.py perstep  --run r1
  python3 bench/cheap_tutor/run.py packs    --run r1
  python3 bench/cheap_tutor/run.py judge    --run r1
  python3 bench/cheap_tutor/run.py report   --run r1
"""
import argparse
import concurrent.futures as cf
import json
import os
import random
import re
import subprocess
import sys
import tempfile
import time
from pathlib import Path

HERE = Path(__file__).resolve().parent
REPO = HERE.parent.parent
DRIVER = REPO / "target/debug/examples/bench_call"
PROMPTS = REPO / "crates/lp-agent/src/loops/prompts"
STEPS = 60  # learner-visible steps in one hour: about a minute each
NODES = 10
ITEMS_PER_NODE = 6

TOPICS = {
    "spanish": "I need to learn Spanish from the very beginning. I'm fluent in English, Gujarati and "
    "Hindi. I work for a client from Peru, so conversation with coworkers matters most. "
    "Probe result: knows hola and como estas; wrote 'Mi llamo Alex, Mi work estu'; "
    "confused manana with today.",
    "rust": "Learn Rust from zero to production. I know Python well (classes, dicts, asyncio). Never "
    "used a compiled language. Probe result: understands shared references in Python; "
    "guessed move semantics correctly with low confidence; never seen a borrow checker.",
}

STRONG = ("claude", "claude-opus-5-5")
CHEAP = {
    "haiku": ("claude", "claude-haiku-5-5"),
    "flash-low": ("agy", "gemini-3.8-flash-low"),
    "qwen": ("kiro", "qwen3-coder-next"),
}
TERSE = "You are a tutor's content writer. Output only what is asked."


# ------------------------------------------------------------------ calling the CLIs


def clean_env():
    return {k: v for k, v in os.environ.items() if not k.startswith(("CLAUDE", "ORCA"))}


def call_claude(model, prompt):
    """Mirrors the driver's isolation, plus `--system-prompt`, the overhead cut under test."""
    model, _, effort = model.partition("@")
    cmd = ["claude", "-p", "--output-format", "json", "--strict-mcp-config", "--tools", "",
           "--setting-sources", "project,local", "--model", model, "--system-prompt", TERSE]
    if effort:
        cmd += ["--effort", effort]
    r = subprocess.run(cmd, input=prompt, text=True, capture_output=True, timeout=400,
                       env=clean_env(), cwd=tempfile.gettempdir())
    d = json.loads(r.stdout)
    u = d.get("usage", {})
    tokens_in = sum(u.get(k, 0) for k in
                    ("input_tokens", "cache_read_input_tokens", "cache_creation_input_tokens"))
    return d.get("result", ""), tokens_in, u.get("output_tokens", 0), d.get("is_error") and d.get("result")


def call_kiro(model, prompt):
    # shortcut: real HOME because the driver's private-home path hung in testing, fix in the task DAG.
    cmd = ["kiro-cli", "chat", "--no-interactive", "--model", model, "--trust-tools=",
           "--output-format", "stream-json", prompt]
    r = subprocess.run(cmd, stdin=subprocess.DEVNULL, text=True, capture_output=True, timeout=400,
                       env=clean_env(), cwd=tempfile.mkdtemp())
    text = ""
    for line in r.stdout.splitlines():
        try:
            d = json.loads(line)
        except ValueError:
            continue
        if d.get("type") == "runFinished":
            text = d.get("data", {}).get("finalText", "")
    # Kiro reports credits only, so tokens are a chars/4 estimate, flagged by `estimated`.
    return text, len(prompt) // 4, len(text) // 4, None if text else (r.stderr[-200:] or "no text")


def call_driver(provider, model, prompt):
    req = {"provider": provider, "model": model, "prompt": prompt}
    r = subprocess.run([str(DRIVER)], input=json.dumps(req), text=True, capture_output=True, timeout=400)
    d = json.loads(r.stdout.strip().splitlines()[-1])
    return d["text"], d["in"], d["out"], d["error"]


class Log:
    def __init__(self, run):
        self.dir = HERE / "results" / run
        self.dir.mkdir(parents=True, exist_ok=True)
        self.path = self.dir / "calls.jsonl"

    def rows(self):
        if not self.path.exists():
            return []
        return [json.loads(x) for x in self.path.read_text().splitlines()]

    def add(self, row):
        with self.path.open("a") as f:
            f.write(json.dumps(row, ensure_ascii=False) + "\n")


def call(log, key, provider, model, prompt, **tags):
    """One model call, skipped when `key` is already logged, so a rerun spends nothing twice."""
    for row in log.rows():
        if row["key"] == key and not row.get("error"):
            return row
    t = time.time()
    try:
        if provider == "claude":
            text, i, o, err = call_claude(model, prompt)
        elif provider == "kiro":
            text, i, o, err = call_kiro(model, prompt)
        else:
            text, i, o, err = call_driver(provider, model, prompt)
    except Exception as e:  # a failed call is a result: it stays in the log
        text, i, o, err = "", 0, 0, str(e)[:300]
    row = dict(key=key, provider=provider, model=model, tokens_in=i, tokens_out=o, text=text,
               error=err or None, secs=round(time.time() - t, 1), prompt_chars=len(prompt),
               estimated=provider == "kiro", **tags)
    log.add(row)
    print(f"{key}: in={i} out={o} {row['secs']}s" + (f" ERROR {str(err)[:120]}" if err else ""), flush=True)
    return row


# ------------------------------------------------------------------ prompts

SKELETON = """Plan a course for this learner. Output JSON only, no code fence:
{{"learner":"<3 lines: level, goal, known gaps>","nodes":[{{"id":"slug","title":"...",
"objective":"what they can do after","teach_notes":"<80 words max: how to explain it to THIS learner>",
"key_facts":["up to 8 exact facts, words or rules to teach, each correct"],
"misconceptions":["up to 3 likely mistakes"]}}]}}
Exactly {nodes} nodes in prerequisite order, each teachable in about 6 minutes.
Learner: {topic}"""

PACK = """Write one lesson step set. Output JSON only, no code fence:
{{"explain":"<under 120 words, teach before testing, with one concrete example>",
"items":[{{"type":"mcq","q":"...","options":["a","b","c","d"],"answer":"<exact option text>",
"why":"<one sentence>"}},{{"type":"recall","q":"...","answer":"<model answer>","why":"..."}}]}}
Exactly {n} items, 4 mcq then 2 recall. Options similar in length and all plausible. Test only what the
explanation taught. Never repeat a question from the list of used questions.
Learner: {learner}
Lesson: {node}
Used questions: {used}"""

STRONG_PACKS = """Plan a course and write its first {k} lessons. Output JSON only, no code fence:
{{"learner":"...","lessons":[{{"id":"slug","title":"...","explain":"<under 120 words with one example>",
"items":[{{"type":"mcq","q":"...","options":["a","b","c","d"],"answer":"<exact option text>","why":"..."}},
{{"type":"recall","q":"...","answer":"...","why":"..."}}]}}]}}
Each lesson has exactly {n} items, 4 mcq then 2 recall. Each lesson takes about 6 minutes.
Learner: {topic}"""

GRADE = """Grade each answer. Output JSON only: [{{"i":0,"correct":true,"note":"<one short sentence>"}}].
{answers}"""

STATE_STEP = """Rules: you are a tutor. Output one JSON object only, one question at a time:
{{"type":"mcq","q":"...","options":["a","b","c","d"],"answer":"<exact option text>","why":"..."}}
Learner: {topic}
Plan: {plan}
Mastered: {mastered}. Weak: {weak}. Current node: {node}.
Last exchange: {last}
Write the next question for the current node. Do not repeat earlier questions."""


def strip_json(text):
    text = text.strip()
    m = re.search(r"```(?:json)?\s*(.*?)```", text, re.S)
    if m:
        text = m.group(1).strip()
    for op, cl in (("{", "}"), ("[", "]")):
        a, b = text.find(op), text.rfind(cl)
        if a == 0 or (a >= 0 and text[:a].strip() == ""):
            return json.loads(text[a : b + 1])
    a, b = text.find("{"), text.rfind("}")
    return json.loads(text[a : b + 1])


# ------------------------------------------------------------------ rule gates


def gate_item(item):
    """Item-writing flaws a rule can catch; an empty list means the item ships."""
    flaws = []
    q, ans = str(item.get("q", "")), str(item.get("answer", ""))
    if not q or not ans:
        return ["missing q or answer"]
    if item.get("type") == "mcq":
        opts = [str(o) for o in item.get("options", [])]
        if len(opts) != 4 or len(set(o.strip().lower() for o in opts)) != 4:
            flaws.append("needs 4 distinct options")
        if ans not in opts:
            flaws.append("answer not among options")
        joined = " ".join(opts).lower()
        if "all of the above" in joined or "none of the above" in joined:
            flaws.append("all/none of the above")
        if opts and ans in opts:
            longest = max(len(o) for o in opts)
            others = [len(o) for o in opts if o != ans] or [0]
            if len(ans) == longest and len(ans) > 1.5 * max(others):
                flaws.append("answer is conspicuously longest")
        if ans and len(ans) > 3 and ans.lower() in q.lower():
            flaws.append("question reveals answer")
    elif item.get("type") != "recall":
        flaws.append("unknown type")
    return flaws


def selftest():
    good = {"type": "mcq", "q": "2+2?", "options": ["3", "4", "5", "6"], "answer": "4", "why": "sum"}
    assert gate_item(good) == []
    assert "answer not among options" in gate_item({**good, "answer": "7"})
    assert "needs 4 distinct options" in gate_item({**good, "options": ["3", "3", "5", "4"]})
    assert "answer is conspicuously longest" in gate_item(
        {**good, "options": ["a", "b", "c", "the long right one"], "answer": "the long right one"})
    assert "question reveals answer" in gate_item(
        {**good, "q": "Is banana a fruit: banana?", "options": ["banana", "x", "y", "z"], "answer": "banana"})
    assert gate_item({"type": "recall", "q": "x", "answer": "y"}) == []
    assert strip_json('```json\n{"a":1}\n```') == {"a": 1}
    assert strip_json('[{"i":0}]') == [{"i": 0}]
    print("selftest ok")


# ------------------------------------------------------------------ per-step approaches (10 real steps)


def simulated_answer(rng, q):
    if q.get("options"):
        right = rng.random() < 0.7
        pick = q.get("answer") if right else rng.choice(q["options"])
        return f"selected: {pick}; confidence: {rng.randint(1, 5)}/5"
    return "answer: not sure; confidence: 2/5"


def run_baseline(log, topic, model, steps=10):
    """Today's shape: shared + teacher prompt + tool list + the whole transcript, every step."""
    rng = random.Random(7)
    head = (PROMPTS / "shared.md").read_text() + (PROMPTS / "teacher.md").read_text()
    tools = "Tools: quiz(question, options, correctAnswer, explanation, type, nodeId), explain(...), " \
            "plan_init(mermaid). Reply with one JSON tool call."
    transcript = f"[learner] {TOPICS[topic]}\n"
    for i in range(steps):
        prompt = f"{head}\n{tools}\n\nTranscript:\n{transcript}"
        r = call(log, f"baseline/{topic}/{model}/{i}", *CHEAP[model], prompt, approach="baseline", topic=topic, cheap=model)
        transcript += f"[assistant] {r['text']}\n[learner answer] {simulated_answer(rng, {})}\n"


def run_state(log, topic, model, steps=10):
    """One call per step, but a fixed-size prompt: deterministic state plus the last exchange."""
    rng = random.Random(7)
    last, mastered, weak = "none", [], []
    for i in range(steps):
        node = f"lesson-{i // ITEMS_PER_NODE + 1}"
        prompt = STATE_STEP.format(topic=TOPICS[topic], plan="10 lessons in order", mastered=mastered,
                                   weak=weak, node=node, last=last)
        r = call(log, f"state/{topic}/{model}/{i}", *CHEAP[model], prompt, approach="state", topic=topic, cheap=model)
        try:
            q = strip_json(r["text"])
        except ValueError:
            q = {}
        ans = simulated_answer(rng, q)
        (mastered if "selected: " + str(q.get("answer")) in ans else weak).append(str(q.get("q", ""))[:40])
        mastered, weak = mastered[-5:], weak[-5:]
        last = f"Q: {q.get('q')} A: {ans}"


# ------------------------------------------------------------------ pack approaches


def run_packs_for(log, topic, skel_src, model, nodes=3):
    """A skeleton from `skel_src`, then a cheap model expands a few lessons, one call each."""
    who = STRONG if skel_src == "strong" else CHEAP[model]
    key = f"skeleton/{topic}/{skel_src}" + ("" if skel_src == "strong" else f"/{model}")
    s = call(log, key, *who, SKELETON.format(nodes=NODES, topic=TOPICS[topic]),
             approach="skeleton", topic=topic, skel=skel_src, cheap=None if skel_src == "strong" else model)
    if s.get("error"):
        return
    try:
        skel = strip_json(s["text"])
    except ValueError as e:
        print("bad skeleton", key, e)
        return
    used = []
    for i, node in enumerate(skel["nodes"][:nodes]):
        p = PACK.format(n=ITEMS_PER_NODE, learner=skel["learner"], node=json.dumps(node, ensure_ascii=False),
                        used=used[-12:])
        r = call(log, f"pack/{topic}/{skel_src}/{model}/{i}", *CHEAP[model], p, approach="pack",
                 topic=topic, skel=skel_src, cheap=model, node=i)
        try:
            used += [x.get("q", "") for x in strip_json(r["text"]).get("items", [])]
        except (ValueError, AttributeError):
            pass


def run_strong_packs(log, topic, k=3):
    call(log, f"strongpack/{topic}", *STRONG, STRONG_PACKS.format(k=k, n=ITEMS_PER_NODE, topic=TOPICS[topic]),
         approach="strongpack", topic=topic, k=k)


def run_grade(log, model):
    answers = "\n".join(
        f"{i}: Q: {q} | model answer: {m} | learner wrote: {a}"
        for i, (q, m, a) in enumerate([
            ("Write 'My name is Alex'", "Me llamo Alex", "Mi llamo Alex"),
            ("Write 'See you tomorrow'", "Hasta manana", "hasta mañana"),
            ("What does a move do in Rust?", "Transfers ownership; the old name is invalid", "copies it"),
            ("Write 'I work here'", "Trabajo aqui", "yo trabajo aquí"),
        ]))
    call(log, f"grade/{model}", *CHEAP[model], GRADE.format(answers=answers), approach="grade", cheap=model)


# ------------------------------------------------------------------ judging and reporting


def collect_items(rows):
    """{variant: [items]} from every pack call in the log."""
    out = {}
    for r in rows:
        if r.get("error"):
            continue
        try:
            if r["approach"] == "pack":
                d = strip_json(r["text"])
                v = f"{r['skel']}-skeleton+{r['cheap']}"
                out.setdefault((r["topic"], v), []).append(d)
            elif r["approach"] == "strongpack":
                d = strip_json(r["text"])
                out.setdefault((r["topic"], "strong-writes-all"), []).extend(d.get("lessons", []))
        except (ValueError, AttributeError, KeyError):
            out.setdefault((r["topic"], r.get("cheap") or "strong"), [])
    return out


def judge(log):
    rows = log.rows()
    rng = random.Random(11)
    for topic in TOPICS:
        variants = {v: ls for (t, v), ls in collect_items(rows).items() if t == topic and ls}
        names = list(variants)
        rng.shuffle(names)
        blind = {f"V{i}": n for i, n in enumerate(names)}
        (log.dir / f"blind-{topic}.json").write_text(json.dumps(blind, indent=1))
        lines = []
        for label, n in blind.items():
            # Same lesson slots for each variant, so the judge compares like with like.
            for li, lesson in enumerate(variants[n][:2]):
                lines.append(f"\n## {label} lesson {li}\nEXPLAIN: {lesson.get('explain')}\n"
                             + json.dumps(lesson.get("items", [])[:6], ensure_ascii=False))
        prompt = (
            "You are auditing tutor content blind. Learner: " + TOPICS[topic] + "\n"
            "For each variant score 1-5: accuracy (facts and answer keys are correct; list every wrong one), "
            "teaching (the explanation really teaches this learner what the items test), "
            "level (fits this learner), variety (items are not near-duplicates). "
            'Output JSON only: {"V0":{"accuracy":n,"teaching":n,"level":n,"variety":n,"errors":["..."],'
            '"note":"one sentence"},...}\n' + "\n".join(lines)
        )
        call(log, f"judge/{topic}", *STRONG, prompt, approach="judge", topic=topic)


def report(log):
    rows = [r for r in log.rows()]
    ok = [r for r in rows if not r.get("error")]
    out = ["# Results", "", f"{len(rows)} calls, {len(rows) - len(ok)} failed."]

    def mean(xs):
        xs = list(xs)
        return sum(xs) / len(xs) if xs else 0

    out += ["", "## Per-step approaches, projected to a full session", "",
            f"| approach | topic | model | real steps | tokens in, step 1 | tokens in, last | "
            f"projected {STEPS}-step input | failed |", "|---|---|---|---|---|---|---|---|"]
    for approach in ("baseline", "state"):
        for topic in TOPICS:
            for model in CHEAP:
                rs = [r for r in ok if r["approach"] == approach and r["topic"] == topic and r.get("cheap") == model]
                bad = sum(1 for r in rows if r["approach"] == approach and r.get("topic") == topic
                          and r.get("cheap") == model and r.get("error"))
                if not rs:
                    continue
                first, last = rs[0]["tokens_in"], rs[-1]["tokens_in"]
                slope = (last - first) / max(len(rs) - 1, 1)
                projected = int(sum(first + slope * i for i in range(STEPS)))
                out.append(f"| {approach} | {topic} | {model} | {len(rs)} | {first} | {last} | {projected} | {bad} |")

    out += ["", "## Pack approaches, projected to a full session", "",
            "Input = skeleton + one pack call per lesson + one grading call per lesson. "
            "Strong calls are counted separately.", "",
            "| variant | topic | strong in | cheap in | cheap out | total in | packs parsed | "
            "items passing gates | flaws |", "|---|---|---|---|---|---|---|---|---|"]
    grade_cost = {m: mean(r["tokens_in"] for r in ok if r["approach"] == "grade" and r["cheap"] == m) for m in CHEAP}
    for topic in TOPICS:
        for skel in ("strong", "cheap"):
            for model in CHEAP:
                ps = [r for r in rows if r["approach"] == "pack" and r["topic"] == topic
                      and r["skel"] == skel and r["cheap"] == model]
                if not ps:
                    continue
                sk = [r for r in ok if r["approach"] == "skeleton" and r["topic"] == topic and r["skel"] == skel
                      and (skel == "strong" or r.get("cheap") == model)]
                sk_in = sk[0]["tokens_in"] if sk else 0
                each = mean(r["tokens_in"] for r in ps if not r.get("error"))
                each_out = mean(r["tokens_out"] for r in ps if not r.get("error"))
                parsed, total, passed, flaws = 0, 0, 0, {}
                for r in ps:
                    try:
                        items = strip_json(r["text"])["items"]
                    except (ValueError, KeyError, TypeError, AttributeError):
                        continue
                    parsed += 1
                    for it in items:
                        total += 1
                        fl = gate_item(it)
                        passed += not fl
                        for f in fl:
                            flaws[f] = flaws.get(f, 0) + 1
                strong_in = sk_in if skel == "strong" else 0
                cheap_in = int((0 if skel == "strong" else sk_in) + NODES * (each + grade_cost[model]))
                out.append(f"| {skel}-skeleton+{model} | {topic} | {strong_in} | {cheap_in} | "
                           f"{int(NODES * each_out)} | {strong_in + cheap_in} | {parsed}/{len(ps)} | "
                           f"{passed}/{total} | {flaws or '-'} |")
        sp = [r for r in ok if r["approach"] == "strongpack" and r["topic"] == topic]
        if sp:
            k = sp[0].get("k", 3)
            out.append(f"| strong-writes-all | {topic} | {int(sp[0]['tokens_in'] * NODES / k)} | 0 | 0 | "
                       f"{int(sp[0]['tokens_in'] * NODES / k)} | - | - | out for {k} lessons: {sp[0]['tokens_out']} |")

    out += ["", "## Blind judge (1 to 5)", "", "| topic | variant | accuracy | teaching | level | variety | errors | note |",
            "|---|---|---|---|---|---|---|---|"]
    for topic in TOPICS:
        j = [r for r in ok if r["approach"] == "judge" and r["topic"] == topic]
        p = log.dir / f"blind-{topic}.json"
        if not j or not p.exists():
            continue
        blind = json.loads(p.read_text())
        try:
            scores = strip_json(j[0]["text"])
        except ValueError:
            out.append(f"| {topic} | judge output unparsable | | | | | | |")
            continue
        for label, name in sorted(blind.items(), key=lambda kv: kv[1]):
            s = scores.get(label, {})
            errs = "; ".join(s.get("errors", []))[:300].replace("|", "/")
            out.append(f"| {topic} | {name} | {s.get('accuracy')} | {s.get('teaching')} | {s.get('level')} | "
                       f"{s.get('variety')} | {errs or '-'} | {str(s.get('note')).replace('|', '/')} |")

    out += ["", "## Call time", "", "| provider/model | calls | mean seconds | failed |", "|---|---|---|---|"]
    for pm in sorted({(r["provider"], r["model"]) for r in rows}):
        rs = [r for r in rows if (r["provider"], r["model"]) == pm]
        out.append(f"| {pm[0]} {pm[1]} | {len(rs)} | {mean(r['secs'] for r in rs):.1f} | "
                   f"{sum(1 for r in rs if r.get('error'))} |")
    text = "\n".join(out) + "\n"
    (log.dir / "report.md").write_text(text)
    print(text)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("cmd", choices=["selftest", "perstep", "packs", "judge", "report"])
    ap.add_argument("--run", default="r1")
    ap.add_argument("--workers", type=int, default=3)
    a = ap.parse_args()
    if a.cmd == "selftest":
        return selftest()
    log = Log(a.run)
    if a.cmd == "perstep":
        with cf.ThreadPoolExecutor(a.workers) as ex:
            jobs = [ex.submit(fn, log, "spanish", m) for fn in (run_baseline, run_state) for m in ("haiku", "flash-low")]
            [j.result() for j in jobs]
    elif a.cmd == "packs":
        with cf.ThreadPoolExecutor(a.workers) as ex:
            jobs = [ex.submit(run_grade, log, m) for m in CHEAP]
            jobs += [ex.submit(run_strong_packs, log, t) for t in TOPICS]
            [j.result() for j in jobs]
            # The strong skeleton is written once and shared by every cheap model.
            jobs = [ex.submit(run_packs_for, log, t, "strong", m) for t in TOPICS for m in CHEAP]
            jobs += [ex.submit(run_packs_for, log, t, "cheap", m) for t in TOPICS for m in CHEAP]
            [j.result() for j in jobs]
    elif a.cmd == "judge":
        judge(log)
    else:
        report(log)


if __name__ == "__main__":
    sys.exit(main())
