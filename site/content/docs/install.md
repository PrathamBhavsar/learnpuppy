---
title: Install
group: Start
order: 2
description: Build from source today, or use the release installer once the repo is public.
---

# Install

## Requirements

- macOS or Linux.
- [Rust](https://rustup.rs) and [bun](https://bun.sh) to build from source.
- At least one logged-in agent CLI: `claude`, `codex` or `agy`.
- About 3 GB of free disk. The build refuses to start below that.

## Build from source

This works today.

```bash
git clone https://github.com/PrathamBhavsar/learnpuppy && cd learnpuppy
make install
```

`make install` builds the web app with bun, then runs `cargo install --path crates/lp-server --locked`, which puts `learnpuppy` in `~/.cargo/bin`. The web app is compiled into the binary, so there is one file to run.

To build without installing:

```bash
make release
./target/release/learnpuppy --help
```

## Install a release binary

> Not available yet. The repository is private and no release is published. These commands start working when the repo is public.

```bash
curl --proto '=https' --tlsv1.2 -LsSf \
  https://github.com/PrathamBhavsar/learnpuppy/releases/latest/download/lp-server-installer.sh | sh
```

Releases are built for `aarch64-apple-darwin`, `x86_64-apple-darwin`, `x86_64-unknown-linux-gnu` and `aarch64-unknown-linux-gnu`, as `lp-server-<target>.tar.xz` containing the `learnpuppy` binary, with checksums. You can also download a tarball and put `learnpuppy` on your `PATH`. The installer puts it in `~/.cargo/bin`.

With access to the private repo, `gh` works now:

```bash
gh release download --repo PrathamBhavsar/learnpuppy --pattern 'lp-server-installer.sh' -O - | sh
```

## Check it

```bash
learnpuppy doctor
```

See [CLI](#/cli) for what it checks. Then follow the [quickstart](#/quickstart).
