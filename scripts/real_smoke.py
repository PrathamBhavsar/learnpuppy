#!/usr/bin/env python3
"""Real-engine smoke run: one course, end to end, through the real server and the real CLIs.

  cargo build -p lp-server && python3 scripts/real_smoke.py [--goal TEXT] [--steps 60]
  cargo build -p lp-server && python3 scripts/real_smoke.py --textbook BOOK.pdf

Starts `learnpuppy serve --dev --real` on a free port with a throwaway data root, then:
  1. creates a workspace and a course (placement quiz is one cheap call),
  2. answers placement and waits for the Author's strong call to write the lessons,
  3. checks the server refuses a step with no open bout (bout_required),
  4. passes the ready gate, then walks the session with stored steps only,
  5. closes the session, ends the bout, and checks the gap now refuses a step,
  6. prints token usage and the number of model calls the walk itself caused.

With --textbook it runs only the textbook import: upload the PDF, poll the jobs list until the
import ends, print the chapters and two objectives per chapter, and assert at most 40 cheap calls.
It spends real cheap-model quota (up to 40 calls). Not in CI.

It spends real quota: one strong call, a few cheap ones. Exit code 0 only if every check held.
"""
import argparse
import json
import os
import subprocess
import sys
import tempfile
import time
import urllib.error
import urllib.request
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent
BIN = REPO / "target/debug/learnpuppy"


class Api:
    def __init__(self, base):
        self.base = base

    def call(self, method, path, body=None):
        data = None if body is None else json.dumps(body).encode()
        req = urllib.request.Request(
            f"{self.base}/api/v1{path}", data=data, method=method,
            headers={"Content-Type": "application/json", "X-LP-CSRF": "1", "Origin": self.base},
        )
        try:
            with urllib.request.urlopen(req, timeout=900) as r:
                raw = r.read()
                return r.status, (json.loads(raw) if raw else None)
        except urllib.error.HTTPError as e:
            raw = e.read()
            try:
                return e.code, json.loads(raw)
            except ValueError:
                return e.code, {"raw": raw.decode(errors="replace")}


def upload_textbook(api, workspace, pdf):
    """POST the PDF as multipart; returns (status, json)."""
    boundary = "lpsmokeboundary"
    data = Path(pdf).read_bytes()
    head = (f"--{boundary}\r\nContent-Disposition: form-data; name=\"file\"; "
            f"filename=\"{Path(pdf).name}\"\r\nContent-Type: application/pdf\r\n\r\n").encode()
    body = head + data + f"\r\n--{boundary}--\r\n".encode()
    req = urllib.request.Request(
        f"{api.base}/api/v1{workspace}/sources/textbook", data=body, method="POST",
        headers={"Content-Type": f"multipart/form-data; boundary={boundary}",
                 "X-LP-CSRF": "1", "Origin": api.base})
    try:
        with urllib.request.urlopen(req, timeout=900) as r:
            return r.status, json.loads(r.read())
    except urllib.error.HTTPError as e:
        return e.code, json.loads(e.read() or b"{}")


def textbook_smoke(api, w, pdf):
    t0 = time.time()
    st, up = upload_textbook(api, w, pdf)
    check(st in (200, 202), f"textbook uploaded ({st})")
    book, job = up["bookId"], up["jobId"]
    print(f"book {book}, job {job}", flush=True)
    last = None
    while True:
        st, jobs = api.call("GET", f"{w}/jobs")
        mine = next((j for j in jobs["items"] if j["id"] == job), None)
        check(mine is not None, "the import job is on the jobs route")
        line = (mine["status"], mine.get("pagesRead"), mine.get("chaptersDone"), mine.get("chaptersTotal"))
        if line != last:
            print(f"  job {line}", flush=True)
            last = line
        if mine["status"] != "running":
            break
        if time.time() - t0 > 1800:
            check(False, "import finished within 30 minutes")
        time.sleep(3)
    print(f"import ended in {time.time() - t0:.0f}s: {mine['status']}", flush=True)
    st, book_json = api.call("GET", f"{w}/textbooks/{book}")
    check(st == 200, "book readable")
    for c in book_json["chapters"]:
        print(f"{c['index']:>3} {c['state']:<9} pp{c['startPage']}-{c['endPage']} {c['title'][:60]}", flush=True)
        if c["state"] == "ready":
            st, d = api.call("GET", f"{w}/textbooks/{book}/chapters/{c['index']}")
            for o in d["objectives"][:2]:
                print(f"       - {o['text'][:100]} (pp{o['startPage']}-{o['endPage']}, {o['items']} items)", flush=True)
    st, usage = api.call("GET", f"{w}/textbooks/{book}/usage")
    calls = usage["autoCalls"] + usage["requestedCalls"]
    check(usage["autoCalls"] <= 40 and calls <= usage["callsCap"], f"calls {calls} within the lifetime cap of {usage['callsCap']}")
    ready = [c for c in book_json["chapters"] if c["state"] == "ready"]
    if ready:
        before = calls
        st, chk = api.call("POST", f"{w}/textbooks/{book}/chapters/{ready[0]['index']}/check")
        check(st == 200 and chk["items"], "a reading check comes back from stored items")
        st, usage = api.call("GET", f"{w}/textbooks/{book}/usage")
        check(usage["autoCalls"] + usage["requestedCalls"] == before, "the reading check made zero model calls")
    print(json.dumps({"ok": True, "book": book, "chapters": len(book_json["chapters"]),
                      "ready": len(ready), "usage": usage}))


def check(ok, what):
    print(("ok   " if ok else "FAIL ") + what, flush=True)
    if not ok:
        raise SystemExit(f"check failed: {what}")


def gate(body):
    return ((body or {}).get("error") or {}).get("details", {}).get("gate")


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--goal", default="Read Rust code that uses Option and Result with confidence")
    ap.add_argument("--steps", type=int, default=60)
    ap.add_argument("--textbook", metavar="PDF", help="only run the textbook import on this PDF")
    args = ap.parse_args()
    if not BIN.exists():
        sys.exit("build first: cargo build -p lp-server")

    home = tempfile.mkdtemp(prefix="lp-real-smoke-")
    env = {**os.environ, "LEARNPUPPY_HOME": home}
    proc = subprocess.Popen([str(BIN), "serve", "--dev", "--real", "--port", "0", "--no-open"],
                            env=env, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, text=True)
    try:
        launch = json.loads(proc.stdout.readline())
        api = Api(launch["url"].split("/#")[0].rstrip("/"))
        print(f"serving {api.base}, data root {home}", flush=True)

        st, ws = api.call("POST", "/workspaces", {"name": "real smoke"})
        check(st in (200, 201), f"workspace created ({st})")
        w = f"/workspaces/{ws['id']}"
        if args.textbook:
            textbook_smoke(api, w, args.textbook)
            return

        t0 = time.time()
        st, created = api.call("POST", f"{w}/courses", {"goal": args.goal, "why": "I read Rust at work and it slows me down"})
        check(st in (200, 201), f"course created, strand probe written in {time.time() - t0:.0f}s ({st})")
        cid = created["id"]
        st, bad = api.call("POST", f"{w}/courses", {"goal": args.goal, "why": "  "})
        check(st == 400, f"a course without a why is refused ({st})")

        # The probe: one question per strand, then follow-ups for strands not settled. At most six.
        items, asked, rounds = created["items"], 0, 0
        while items:
            rounds += 1
            check(rounds <= 3, "the probe ends within three rounds")
            answers = [{"item": i["id"], "text": (i["options"][0] if i["options"] else "I do not know"),
                        "guessed": False, "blocked": False} for i in items]
            asked += len(answers)
            st, res = api.call("POST", f"{w}/courses/{cid}/placement", {"answers": answers})
            check(st == 200, f"probe round {rounds} answered ({len(answers)} items)")
            items = res["next"]
        check(3 <= asked <= 6, f"the probe asked {asked} questions (3 to 6)")

        t0 = time.time()
        while True:
            st, status = api.call("GET", f"{w}/courses/{cid}/status")
            if status.get("state") in ("ready", "refilling"):
                break
            check(status.get("state") != "failed", f"author did not fail ({status.get('reason')})")
            if time.time() - t0 > 1500:
                check(False, "author finished within 25 minutes")
            time.sleep(5)
        print(f"author done in {time.time() - t0:.0f}s", flush=True)

        st, plan = api.call("GET", f"{w}/courses/{cid}/plan")
        check(st == 200 and len(plan["lessons"]) >= 3, f"plan has {len(plan.get('lessons', []))} lessons")
        check(plan.get("why", "").startswith("I read Rust"), "the plan carries the learner's why")
        labels = [x["label"] for x in plan.get("strands", [])]
        check(len(labels) == 3 and all(l in ("known", "edge", "unknown", "blocked") for l in labels),
              f"the plan carries three strand labels ({labels})")

        st, body = api.call("POST", f"{w}/courses/{cid}/step")
        check(st == 409 and gate(body) == "bout_required", "step refused without an open bout")

        st, body = api.call("POST", f"{w}/learning/bout/start",
                            {"course": cid, "ready": {"phoneAway": False, "tabsClosed": True, "focusSeconds": 30}})
        check(st == 409 and gate(body) == "not_ready", "ready gate refuses an unchecked box")
        st, clock = api.call("POST", f"{w}/learning/bout/start",
                             {"course": cid, "ready": {"phoneAway": True, "tabsClosed": True, "focusSeconds": 30}})
        check(st == 200 and clock["bout"], "bout started through the ready gate")
        bout = clock["bout"]

        st, before = api.call("GET", f"{w}/courses/{cid}/usage")
        seen = {}
        closed = False
        for n in range(args.steps):
            st, step = api.call("POST", f"{w}/courses/{cid}/step")
            check(st == 200, f"step {n} ok ({st})")
            kind = step["step"]
            seen[kind] = seen.get(kind, 0) + 1
            api.call("POST", f"{w}/learning/bout/heartbeat", {"bout": bout})
            if kind == "confirm_plan":
                api.call("POST", f"{w}/courses/{cid}/plan/confirm")
            elif kind == "predict":
                api.call("POST", f"{w}/courses/{cid}/predict", {"lesson": step["lesson"], "guess": (step["options"] or ["?"])[0]})
            elif kind == "explain":
                api.call("POST", f"{w}/courses/{cid}/explain-seen", {"lesson": step["id"]})
            elif kind == "notes":
                api.call("POST", f"{w}/courses/{cid}/note", {"lesson": step["lesson"], "text": "All clear"})
            elif kind == "ask":
                text = step["options"][0] if step["options"] else "I am not sure"
                st, res = api.call("POST", f"{w}/courses/{cid}/quiz-answer", {
                    "item": step["id"], "lesson": step["lesson"], "text": text,
                    "confidence": 2, "ms": 4000, "reasoning": "guess", "gap": False})
                check(st == 200, f"answer accepted ({st})")
                leaked = {"answer", "why", "accept", "must_name", "mustName", "misconceptions"} & set(step)
                check(not leaked, f"step carries no answer key ({sorted(leaked)})")
            elif kind == "break":
                api.call("POST", f"{w}/courses/{cid}/break", {"lesson": step["lesson"]})
            elif kind == "feynman":
                api.call("POST", f"{w}/courses/{cid}/feynman", {"lesson": step["lesson"], "explanation": "It is a way to handle a value that may be missing."})
            elif kind == "close":
                st, report = api.call("POST", f"{w}/courses/{cid}/close")
                check(st == 200, f"close report written ({st})")
                closed = True
                break
            else:
                check(False, f"known step kind: {kind}")
        print("steps seen:", seen, flush=True)
        if not closed:
            print("note: session did not reach Close within the step cap; ending the bout anyway", flush=True)

        st, after = api.call("GET", f"{w}/courses/{cid}/usage")
        walk = {k: after[k] - before[k] for k in ("strongIn", "strongOut", "cheapIn", "cheapOut")}
        print("model usage caused by the walk:", walk, flush=True)

        st, _ = api.call("POST", f"{w}/learning/bout/end", {"bout": bout})
        check(st == 200, "bout ended")
        st, body = api.call("POST", f"{w}/courses/{cid}/step")
        check(st == 409 and gate(body) == "gap", "step refused inside the gap after the bout")
        print(json.dumps({"ok": True, "closed": closed, "seen": seen, "walk": walk, "totals": after}))
    finally:
        proc.terminate()
        try:
            proc.wait(timeout=10)
        except subprocess.TimeoutExpired:
            proc.kill()


if __name__ == "__main__":
    main()
