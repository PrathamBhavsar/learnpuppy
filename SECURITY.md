# Security

LearnPuppy is a local app: it binds `127.0.0.1` only, keeps data in `~/.learnpuppy`, and drives the agent CLIs you already have installed. It never reads or stores your CLI credentials.

## Reporting a vulnerability

Use GitHub's private vulnerability reporting on this repository (Security tab, "Report a vulnerability"). Please do not open a public issue for anything exploitable. Include the version (`learnpuppy --version`), what you did, and what happened.

## What counts

- Anything reachable without the session cookie, or from another origin, host or process.
- Secret or token exposure in responses, logs, exports or the diagnostics copy.
- A fetch, file or subprocess path that escapes its limits: the SSRF-guarded web adapter, the artifact sandbox, the CLI sandbox directory, the PDF reader.
- Data loss or corruption in the workspace store.

## Supported versions

Pre-release: only the latest commit on `main` is supported.
