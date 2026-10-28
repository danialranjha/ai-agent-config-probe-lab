# AI-agent configuration probe lab

A status check can have a configuration-writing side effect. This experiment runs an unchanged Asyar registration helper on invented temporary files, then compares that with inspection that never invokes registration.

**Observed result:** when the server entry was absent, registration during checking added it. After a simulated user removal, the next registration-bearing check reinstated it. Status-only inspection preserved the exact removed-state bytes. A separate explicit-registration action still added the entry. No agent, coding CLI, Asyar application or MCP server ran.

## Hypothesis and threat model

Checking whether an integration is available should be distinguishable from changing its configuration. The hypothesis is that separating those operations preserves existing configuration during inspection without removing the ability to register explicitly.

Assume an application already has write access and a user wants status information. The relevant trust error is assigning configuration mutation to that read-oriented operation. An attacker or model compromise is not needed to demonstrate it. The helper, observer, filesystem and test driver remain trusted. All configuration and command strings are synthetic. The configured command is `INERT_ASYAR_COMMAND`; it is never executed.

This is a component experiment, not a demonstrated prompt injection, a safe-registration library, or a reproduction of every finding in the issue.

## Source and scope

[Asyar issue959](https://github.com/Xoshbin/asyar/issues/959) reports several integration problems. Its [maintainer reply](https://github.com/Xoshbin/asyar/issues/959#issuecomment-6452697101) confirms reproduction. This lab isolates configuration registration, not the separate permission flags or credential-storage findings.

At [pinned commit334c8fa](https://github.com/Xoshbin/asyar/blob/334c8fa6717c7a09ac60e075395d20eba3e70f5b/asyar-launcher/src-tauri/src/ai/cli.rs), `check_cli_status` first resolves a CLI binary. Only after successful resolution does its Claude branch invoke registration, before checking the version. `ensure_mcp_registered_for_claude` selects the real home file in the application. **Neither complete function is executed here.** Instead, the harness passes an owned temporary path directly to the exact registration helper, copied from lines510–565. No user's home configuration is accessed.

The helper body is byte-identical to the pinned source. [source.json](source.json) records its hash, Git blob identity and attribution. The original status observer and fixture harness are an engineering comparison, not a claim that Asyar has shipped this fix. The extraction is GPLv3; this lab is provided under the included [LICENSE](LICENSE), without warranty.

## Setup and exact commands

Requirements: Rust/Cargo and Python3. The recorded run used rustc1.97.1. Cargo.lock pins all Rust packages; Python uses only its standard library. On a new machine, fetch these public dependencies once:

```sh
cargo fetch --locked
```

Then run locally and offline:

```sh
cargo test --offline --locked
python3 run.py
```

`run.py` runs the normal suite, emits the experiment output, and runs two functional faults in separate temporary source copies. It overwrites only the three files under `artifacts/`. The Rust binary constructs and removes its own temporary fixtures. It accepts no target paths and launches no external command. Cargo compilation and test execution are the only subprocesses launched by the Python runner.

To emit the exact result without the fault checks:

```sh
cargo run --offline --locked --quiet
```

## Technical walkthrough

Three methods receive identical input bytes in fresh directories:

- **Registration during check:** Call the original registration helper, then inspect the file.
- **Status only:** Read and classify the file without calling registration.
- **Explicit registration:** Call the same original helper as a distinct requested action, then inspect the file.

The third method is a positive control. The program does not implement an approval UI or authenticate intent. It demonstrates that removing registration from inspection need not remove the separate registration operation.

The main input represents an already-removed server entry:

```json
{"mcpServers":{},"theme":"light"}
```

Registration during check adds an `asyar` entry with the inert command and `mcp-server` argument. Status only leaves those input bytes, including the trailing newline, unchanged and reports `registration_absent`. The sequential trace first registers, removes that entry, and checks again. It produces the same distinction.

The oracle reads actual filesystem bytes before and after each action. It records existence, parent existence, byte hex, parsed content and a descriptive status. File mutation is determined from existence and byte differences, independently of the original helper's Boolean return. Read errors other than absent-file errors stop the run. Invalid bytes are preserved in hex instead of being coerced into text. Matching status describes the command and argument fields only; it does not mean consent, executable availability or permission to run.

## Results

There are 11 selected fixtures, three methods and 33 rows, plus two sequential removal traces. Eleven tests pass. Deliberately registering during the status-only path causes four assertion failures; disabling explicit registration causes three. These are finite designed cases, not model trials or an attack-success rate.

| Case family | Registration during check | Status only | Explicit registration |
|---|---|---|---|
| Missing file, empty object, unrelated settings, removed entry, changed command (5 fixtures) | File changes | Exact state preserved | File changes |
| Already matching, extra arguments (2 fixtures) | No file change | Exact state preserved | No file change |
| Malformed JSON, non-object, malformed server map, non-UTF8 (4 fixtures) | File changes | Exact state preserved, invalid state reported | File changes |

The already-matching benign twin stays byte-identical under every method. Valid unrelated settings and server entries survive ordinary registration. In the extra-arguments case, the original helper accepts any list containing `mcp-server`, whereas the observer expects exactly that argument. The helper therefore returns no write but the observer still reports different configuration. The observer is not being presented as an exact replacement for the helper's matching rule.

For malformed/non-object/non-UTF8 input, the upstream helper constructs a new object and overwrites the file. Explicit registration retains this behavior. Separating inspection from registration does not repair error handling in registration.

- [run.json](artifacts/run.json): all exact input/output byte sequences and removal traces.
- [evidence.md](artifacts/evidence.md): per-fixture table and readable paired trace.
- [checks.json](artifacts/checks.json): completed tests, failed functional faults and build-input hashes.

## Limits and engineering decision

No installation flow, CLI resolution, UI, account probe, server startup, permission enforcement or model ran. No real home file, credentials, endpoint or attack payload is used. The component result does not prove the full application behavior on other revisions. The fixture suite is not exhaustive. Concurrent writers, symlink races, filesystem-denial recovery, durability and atomic replacement are untested; the copied writer is not production-hardened. We do not infer successful execution from a configuration entry.

Keep integration inspection separate from registration. Test that inspection preserves both absent files and exact existing bytes, including removal and malformed-data cases. Invoke configuration writes only through a distinct explicit action, and review the writer's error handling separately.
