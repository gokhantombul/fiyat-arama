# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Commands

```bash
cargo build --release      # production build
cargo check                # fast syntax/type check (run after every Rust change)
cargo run                  # launch interactive menu
cargo run -- <command>     # run a specific subcommand, e.g.: cargo run -- doviz
cargo clippy               # linter
```

There are no automated tests yet.

## Architecture

All code lives in a single file: `src/main.rs`.

**Entry points:**
- `main()` — parses args via clap. If a subcommand is given, calls `execute_command()` directly; otherwise launches `run_interactive_menu()`.
- `run_interactive_menu()` — `inquire::Select` loop that maps menu choices to `execute_command()` calls.
- `run_repl()` — rustyline REPL that re-parses each line as `Cli::try_parse_from(argv)` and dispatches to `execute_command()`.

**Commands (`Commands` enum):**
| Variant | Status |
|---|---|
| `Help` | prints usage summary |
| `Doviz` | demo table output (no real HTTP) |
| `Benzin` | demo output (no real HTTP) |
| `Ara` | demo output (no real HTTP) |
| `Incele` | **real HTTP** — fetches URL, parses HTML with `scraper`, extracts metadata and JSON-LD |

`#[command(disable_help_subcommand = true)]` is set on `Cli` to prevent a panic from clap's built-in `help` subcommand conflicting with the custom `Help` variant.

**`incele` pipeline:** `inspect_url()` → validates URL, fetches body, extracts `<title>`, `<meta>` (description + OpenGraph), and `<script type="application/ld+json">` nodes → `detect_product_from_jsonld()` / `detect_article_from_jsonld()` → `render_inspection()` prints comfy-table output.

**Output:** all tables use `comfy_table` with `UTF8_FULL` preset and `UTF8_ROUND_CORNERS` modifier.

## Rules

- All user-facing text must be in **Turkish**.
- When adding a new command, update the command list in `README.md`.
