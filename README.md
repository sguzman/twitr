# twitr

`twitr` chunks plain text into tweet-sized posts. It accepts input from a plain-text file or from an interactive REPL that works well with `rlwrap`.

## Features

- Chunks text under a configurable character limit.
- Reserves space for optional `current/total` numbering.
- Preserves paragraphs when possible and falls back to sentence, word, or hard token splitting.
- Treats an isolated `---` line as a forced new tweet boundary.
- Loads a comprehensive TOML config.
- Emits detailed logs with `tracing`.
- Includes unit tests for chunking behavior and config parsing.

## Requirements

- Rust toolchain with Cargo.
- Optional: `rlwrap` if you want command history in REPL mode.

## Usage

Chunk a file:

```bash
cargo run -- input.txt
```

Use a custom config:

```bash
cargo run -- --config twitr.toml input.txt
```

Start the REPL:

```bash
rlwrap cargo run
```

REPL commands:

- `/done`: chunk the current buffer and print the result
- `/clear`: clear the current buffer
- `/stats`: show the current buffer character count
- `/help`: show commands
- `/quit` or `/exit`: leave the REPL without printing chunks

Print the effective config:

```bash
cargo run -- --print-config
```

Force a new tweet manually from a file or the REPL by putting `---` on its own line:

```text
This will be one tweet.
---
This starts a new tweet even if the first one had room left.
```

## Config

The repo includes a ready-to-edit config at [`twitr.toml`](/win/linux/Code/rust/twitr/twitr.toml).

Important settings:

- `chunking.max_chars`: hard per-post limit
- `chunking.numbering`: prefix each chunk with sequence numbers
- `chunking.numbering_format`: prefix template with `{current}` and `{total}`
- `chunking.suffix`: static suffix appended to every chunk
- `chunking.preserve_paragraphs`: keep paragraphs together when possible
- `chunking.preserve_line_breaks`: preserve single line breaks instead of normalizing them
- `chunking.collapse_whitespace`: condense repeated spaces
- `chunking.split_sentences`: prefer sentence-aware chunking before word fallback
- `logging.filter`: default `tracing` filter, overridden by `RUST_LOG`

## Development

Run tests:

```bash
cargo test
```

Verify the build:

```bash
cargo build
```

Format:

```bash
cargo fmt
```
