# Build and check Husker

This page is for contributors who need to build Husker and verify a change.

## Run the checks

Run these commands from the repository directory:

```text
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test --all-targets
cargo build --release --locked
openspec validate --all --strict
```

The test suite uses temporary profiles. It does not read or change your
`~/.husker` profile. Keep a separate profile for interactive checks too:

```text
cargo run -- --profile ./target/manual-profile
```

Close the running app before rebuilding it on Windows. Windows locks an
executable while it runs. A rebuild of that executable fails with
`Access is denied (os error 5)` until the process exits.

## Checks for version 0.1.0

The first working version passed these checks on November 8, 2026:

| Check | Result |
| --- | --- |
| Unit tests | 45 passed. Covers storage, planning, search, configuration, localization, and terminal actions. |
| CLI integration tests | 6 passed. Runs commands against isolated profiles. |
| Formatting | `cargo fmt --check` passed. |
| Clippy | All targets passed with warnings denied. |
| Release build | `cargo build --release --locked` passed. |
| Local installation | `cargo install --path . --locked --offline --root .scratch/install` passed. |
| Interactive terminal | English launch and quit passed. Danish task creation, multiline text, due date, description focus, discard, and help scrolling passed at 80 by 24 cells. |
| Persistence | The CLI returned the exact Danish title, two description lines, and date saved through the terminal. |
| OpenSpec | All five capability specs passed strict validation. |

The checks ran on Windows build 26300 with Rust 1.91.1. Linux and macOS
were not tested in this session. AltGr handling has a regression test based
on Crossterm's Windows key events. A physical Danish keyboard was not tested.

## Performance sample

The release executable searched 10,000 tasks and printed all matches as
JSON in a median 73.5 ms across five warm runs. This is a local measurement,
not a latency guarantee.

The fixture used one list and 10,000 open tasks outside Trash. Every task
had the `shopping` tag and the description `Fixture description with Danish: Køb æbler`.
Titles were `Task 1` through `Task 10000`. Two thirds of tasks had dates
from five days before today through 24 days after today. The rest were undated.

The measured command was:

```text
husker --profile FIXTURE_DIRECTORY --json tasks "#shopping æbler"
```

Timing included process startup, database reads, Unicode search, sorting,
and JSON output captured in memory. Terminal drawing and fixture creation
were outside the measurement.

| Run | Elapsed time |
| --- | --- |
| First run | 71.5 ms |
| Median of five later runs | 73.5 ms |
| Fastest later run | 62.5 ms |
| Slowest later run | 75.3 ms |

The machine reported `Intel64 Family 6 Model 189 Stepping 1, GenuineIntel`
and `Microsoft Windows NT 10.0.26300.0`. The benchmark used a local SQLite
database and a release build with thin link-time optimization.
