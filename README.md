# twitr

`twitr` chunks plain text into tweet-sized posts from either files or an interactive REPL workflow.

## Intent

Reduce the friction of turning longer writing into postable threaded chunks while staying in a simple terminal workflow.

## Ambition

The REPL and docs suggest a practical authoring helper for social posting rather than a general social-media automation platform.

## Current Status

The codebase is compact but complete enough to expose a library, binary, examples, config, and usage documentation.

## Core Capabilities Or Focus Areas

- Chunk long text into tweet-sized segments.
- Accept input from files or interactive sessions.
- Use config-driven behavior.
- Support example-driven experimentation.
- Expose a small reusable Rust library alongside the CLI.

## Project Layout

- `docs/`: project documentation, reference material, and roadmap notes.
- `examples/`: sample inputs, example configs, or demonstration workflows.
- `src/`: Rust source for the main crate or application entrypoint.
- `Cargo.toml`: crate or workspace manifest and the first place to check for package structure.

## Setup And Requirements

- Rust toolchain.
- Text input to chunk.
- Optional config via `twitr.toml`.

## Build / Run / Test Commands

```bash
cargo build
cargo test
cargo run -- --help
```

## Notes, Limitations, Or Known Gaps

- This tool is about text segmentation, not posting or account automation.
- Good chunking behavior depends on content style as much as raw character count.

## Next Steps Or Roadmap Hints

- Add more regression examples for difficult prose shapes.
- Clarify how the project should handle URLs, hashtags, and thread-specific formatting edge cases.
