// LearnPuppy: all site copy and config. Every claim here is traceable to the repo
// (README.md, docs/web/*.md, crates/*, release docs). Pre-release: no numbers that were not measured.
export default {
  name: "LearnPuppy",
  markImg: "favicon.svg",
  heroNote: "The repository is private until the first release, so the clone above needs access. Nothing is downloadable yet.",
  wordmark: "learn<b>puppy</b>",
  repo: "PrathamBhavsar/learnpuppy",
  scene: "graph",
  license: "MIT",
  stack: "Rust · React",
  status: "pre-release",
  headline: "Learn it once.<br><em>Remember</em><br>it for good.",
  sub: "A tutor that probes, plans and teaches. A <strong>Knowledge Graph</strong> that keeps what you learn. <strong>Spaced repetition</strong> that brings it back before it fades. All on your machine, driving the AI CLIs you already use.",
  heroCmd: "git clone https://github.com/PrathamBhavsar/learnpuppy && cd learnpuppy && make install",

  liveTitle: "Facts, from the source.",
  liveNote: "GitHub stats appear here once the repo is public. Until then these are facts read from the source: the code, `README.md` and `docs/web/`.",
  facts: [
    ["Listens on", "`127.0.0.1` only"],
    ["Your data", "`~/.learnpuppy`, plain files"],
    ["Ships as", "one binary, web app embedded"],
    ["Drives", "`claude`, `codex`, `agy`"],
    ["Scheduler", "FSRS v5, 0.9 target retention"],
    ["Version", "0.1.0, nothing tagged yet"],
    ["License", "MIT, from `Cargo.toml`"],
  ],

  featuresTitle: "Everything it does, listed.",
  featuresLede: "One binary, a local web app, and the AI CLIs you are already logged into. What was checked against real CLIs, and what was not, is in the FAQ.",
  features: [
    { title: "Learn", blurb: "Four phases in one chat, with the plan visible while you work.", items: [
      ["chat", "Probe", "Up to 8 questions find what you already know before anything is taught, as free text or choices."],
      ["tree", "Plan", "The plan appears beside the chat as a live graph of steps: current, visited, upcoming. Click a step to jump to where it was taught."],
      ["help", "Teach with questions", "Quiz (options A to D, optional confidence 1 to 5), recall (with an I don't know button), explain in your own words, and problems with a hint ladder."],
      ["checklist", "Practice", "Pick nodes in the graph and start a Practice chat on them. Problems come in recall, discrimination and composition modes."],
      ["pdf", "Workbench jobs", "Research, visual and PDF jobs run beside the chat. Artifacts open in a viewer; HTML renders in an inert sandboxed iframe with no scripts."],
      ["repeat", "Stop, steer, resume", "Esc stops a turn and keeps the partial answer. Steer mid-turn. A turn cut off by a restart offers Resume."],
    ]},
    { title: "Remember", blurb: "What you learn lands in one graph, then comes back on a schedule.", items: [
      ["share", "Knowledge Graph", "Learning chats and ingest add propositions and chains to one cross-session graph. Pan, zoom, multi-select, keyboard navigation, a minimap."],
      ["search", "Search and filters", "Full-text search dims everything else. Filter by source, cluster or minimum degree. A chain table sorts, scopes to a selection and exports CSV."],
      ["eye", "Provenance", "Open any node to read where it came from, its chains and its linked cards, then jump to the source chat."],
      ["flash", "Practice from selection", "Select nodes, press Practice, and a chat on exactly those ideas opens."],
      ["calendar", "FSRS spaced repetition", "Review sessions with Again, Hard, Good, Easy and the next interval previewed on each. Keys 1 to 4, Space accepts the suggestion."],
      ["fire", "Stats and review-all", "Due now, new today, a 14-day forecast and your streak. A card browser shows each card's history. Review all walks every workspace."],
    ]},
    { title: "Feed it", blurb: "Learn from the work you already did with AI.", items: [
      ["inbox", "Ingest", "Scans claude, codex and agy session transcripts and git commits in the folders you allow, over 7, 30, 90 days or all time."],
      ["lock", "Secrets scrubbed", "Ingested transcripts, agy artifacts and git diffs pass through secret redaction before anything is distilled."],
      ["book", "Digest", "Decisions grouped by day, with the basis, alternatives considered, related commits and the cards generated from them."],
      ["refresh", "Quiz me on this", "Review exactly the decisions in view. Regenerate a decision's cards if you want different ones."],
    ]},
    { title: "Workspaces and accounts", blurb: "Separate subjects, shared model access.", items: [
      ["folder", "Isolated workspaces", "Each has its own graph, chats, jobs, cards, decisions and search index. Archive, trash and restore, export and import as a zip, switch with Cmd 1 to 9."],
      ["users", "Account pool", "Several Claude profiles, plus codex and agy. Each profile is its own rate-limit bucket. kiro-cli and opencode are picked up too when installed."],
      ["sync", "Failover", "A rate-limited account cools down until its reset time while the others keep serving. A banner counts down when everything is busy."],
      ["settings", "Role slots", "Choose provider, model, effort and fallback per role: prober, teacher, practice, extractor, auditor and the workbench roles. Override per workspace."],
    ]},
    { title: "Local only", blurb: "No hosted service in the loop.", items: [
      ["shield", "Loopback and a launch token", "The server binds 127.0.0.1. `serve` prints a one-time URL; the token is traded for an HttpOnly, SameSite=Strict cookie and removed from the URL."],
      ["shield2", "Host and Origin checks", "A foreign Host is refused with 421. Writes need your own Origin and a CSRF header."],
      ["disk", "Plain files you own", "Append-only JSONL ledgers plus SQLite with full-text search, under `~/.learnpuppy`. Set `LEARNPUPPY_HOME` to move it."],
      ["alert", "Tutor turns run caged, with a caveat", "Each CLI runs in an empty private directory, with tools off where the CLI allows it. codex and agy can still read files you can read."],
    ]},
    { title: "At your keyboard", blurb: "Built to be driven without a mouse.", items: [
      ["terminal", "Command palette", "Cmd K: navigate, switch workspace, start a chat, run ingest, start a review, search nodes in this workspace or all of them."],
      ["keyboard", "Chords and a shortcut sheet", "g then g, l, r, d or i to jump between screens. `[` folds the sidebar, `/` focuses search, `?` lists every binding."],
      ["globe", "RTL, themes, reduced motion", "Right-to-left layout, light, dark or system theme, compact density and a reduce-motion override."],
      ["support", "Help built in", "A first-run tour, an About this screen guide on each screen, and short tips beside terms like ingest and cooldown."],
      ["activity", "learnpuppy doctor", "Checks the data folder, port, disk space and which CLI logins were found, and exits non-zero on a failure."],
    ]},
  ],

  howTitle: "One topic, from fuzzy to remembered.",
  howLede: "The learning loop is enforced by the server, not left to a prompt. The graph and the cards are written as a side effect of studying.",
  steps: [
    ["You name a topic", "Start a Teach chat in a workspace. If an agent CLI is not logged in, the app tells you the command to run."],
    ["Probe finds the edges", "Up to eight questions show what you already know and what you only think you know."],
    ["Plan lays out the route", "A graph of steps appears beside the chat. The plan decides what is taught and in what order."],
    ["Teach asks, then explains", "Quizzes, recall, explain-it-yourself and problems, with hints when you ask for them."],
    ["The graph and cards update", "Three stages extract what was learned, audit it, and merge it into the Knowledge Graph. Cards for it enter the FSRS queue."],
    ["Reviews bring it back", "Cards come due on their own schedule. You rate each one and FSRS sets the next interval."],
  ],

  installTitle: "Build it today.",
  installLede: "Release downloads work once the repo is public. Until then, build from source: one command builds the web app and installs one binary.",
  install: [
    { id: "source", label: "Source", cmds: [
      "git clone https://github.com/PrathamBhavsar/learnpuppy && cd learnpuppy",
      "make install",
      "learnpuppy doctor",
      "learnpuppy serve",
    ], note: "Needs Rust (rustup.rs) and bun (bun.sh), macOS or Linux,. `make release` builds `target/release/learnpuppy` without installing." },
    { id: "binary", label: "Binary", cmds: [
      "curl --proto '=https' --tlsv1.2 -LsSf https://github.com/PrathamBhavsar/learnpuppy/releases/latest/download/lp-server-installer.sh | sh",
      "learnpuppy doctor",
      "learnpuppy serve",
    ], note: "Not live yet: the repo is private and no release is published. Once public this installs to `~/.cargo/bin` for macOS and Linux, arm64 and x86_64." },
  ],

  faq: [
    ["Is it ready to use?", "It is pre-release: version 0.1.0 with no tagged release, and the repository is currently private. Build from source today. The one-line installer and release tarballs start working when the repo is public."],
    ["What leaves my machine?", "Nothing, except the calls your own `claude`, `codex` and `agy` CLIs make, as they would anyway. LearnPuppy itself has no server and no account. It listens on `127.0.0.1` only."],
    ["Do the tutor CLIs get access to my files?", "They are started with a cleared environment, in an empty private directory. claude runs with tools off. codex and agy run in a read-only sandbox, and read-only still means they can read files you can read, so treat those turns accordingly. This is written down in `docs/web/12-cli-isolation-and-failures.md`."],
    ["Which AI do I need?", "At least one logged-in CLI: `claude`, `codex` or `agy`. Several Claude profiles work as separate rate-limit buckets and are failed over automatically. LearnPuppy has no login of its own."],
    ["What is verified, and what is not?", "Probe, Plan, Teach, Practice, workbench jobs (research, visual, PDF), ingest and Digest were checked against real CLIs. Review-all was only checked against the scripted engine and mocks. agy was out of quota during verification, so agy as the teacher is untested; failover away from it is tested. The graph legend's claim, concept, procedure and decision kinds are cosmetic."],
    ["What is the animation in the hero?", "A synthetic illustration: a made-up forgetting curve that drops, then jumps back up at each review, with intervals that grow. It is not measured data and not a result from LearnPuppy."],
    ["Which scheduling algorithm?", "FSRS v5, ported from the ts-fsrs build with default weights, short-term steps on, fuzz off and a 0.9 target retention."],
    ["Which platforms?", "macOS and Linux. A release workflow for arm64 and x86_64 exists on a branch; it has not run on a tag yet."],
  ],

  footBlurb: "A local learning harness: tutor, Knowledge Graph and spaced repetition.",
  footNote: "Pre-release. Nothing here is a benchmark.",
  footDocs: [["Install", "install"], ["Quickstart", "quickstart"], ["Security", "security"], ["FAQ", "faq"]],
  footProject: [["GitHub", "https://github.com/PrathamBhavsar/learnpuppy"], ["Issues", "https://github.com/PrathamBhavsar/learnpuppy/issues"]],
};
