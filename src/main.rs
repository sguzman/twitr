use std::io::{self, BufRead, Write};
use std::path::PathBuf;

use anyhow::Result;
use clap::Parser;
use tracing::{debug, info, instrument};
use tracing_subscriber::EnvFilter;
use tracing_subscriber::fmt;

use twitr::{AppConfig, ChunkedOutput, InputSource, chunk_text, read_input_file};

#[derive(Debug, Parser)]
#[command(author, version, about = "Chunk plain text into tweet-sized posts")]
struct Cli {
    /// Plain-text file to chunk. If omitted, interactive REPL mode starts.
    input: Option<PathBuf>,

    /// Path to a TOML config file.
    #[arg(short, long)]
    config: Option<PathBuf>,

    /// Print the effective configuration as TOML and exit.
    #[arg(long)]
    print_config: bool,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let config = AppConfig::load(cli.config.as_deref())?;
    init_tracing(&config)?;

    if cli.print_config {
        print!("{}", toml::to_string_pretty(&config)?);
        return Ok(());
    }

    let output = if let Some(path) = cli.input.as_deref() {
        chunk_file(path, &config)?
    } else {
        run_repl(&config)?
    };

    emit_chunks(&output, &config);
    Ok(())
}

fn init_tracing(config: &AppConfig) -> Result<()> {
    let filter = std::env::var("RUST_LOG").unwrap_or_else(|_| config.logging.filter.clone());
    let env_filter = EnvFilter::try_new(filter)?;

    fmt()
        .with_env_filter(env_filter)
        .with_ansi(config.logging.ansi)
        .with_target(config.logging.with_target)
        .compact()
        .init();

    Ok(())
}

#[instrument(skip_all, fields(path = %path.display()))]
fn chunk_file(path: &std::path::Path, config: &AppConfig) -> Result<ChunkedOutput> {
    let text = read_input_file(path)?;
    let chunks = chunk_text(&text, &config.chunking)?;

    info!(chunk_count = chunks.len(), "chunked file input");

    Ok(ChunkedOutput {
        source: InputSource::File(path.to_path_buf()),
        original_length: text.chars().count(),
        chunks,
    })
}

#[instrument(skip_all)]
fn run_repl(config: &AppConfig) -> Result<ChunkedOutput> {
    let stdin = io::stdin();
    let mut stdout = io::stdout();
    let mut buffer = String::new();
    let mut line = String::new();

    writeln!(
        stdout,
        "REPL mode. Paste text, then run {} to chunk it. Use {} for commands. rlwrap works fine here.",
        config.repl.done_command, config.repl.help_command
    )?;
    let mut handle = stdin.lock();

    loop {
        write!(stdout, "{}", config.repl.prompt)?;
        stdout.flush()?;
        line.clear();

        if handle.read_line(&mut line)? == 0 {
            break;
        }

        let trimmed = line.trim();

        if trimmed == config.repl.help_command {
            writeln!(
                stdout,
                "commands: {} {} {} {} {}",
                config.repl.done_command,
                config.repl.clear_command,
                config.repl.show_stats_command,
                config.repl.quit_commands.join(" "),
                config.repl.help_command
            )?;
            continue;
        }

        if trimmed == config.repl.clear_command {
            buffer.clear();
            info!("cleared REPL buffer");
            writeln!(stdout, "buffer cleared")?;
            continue;
        }

        if trimmed == config.repl.show_stats_command {
            writeln!(stdout, "buffer chars: {}", buffer.chars().count())?;
            continue;
        }

        if config
            .repl
            .quit_commands
            .iter()
            .any(|command| command == trimmed)
        {
            info!("user exited REPL without chunking");
            return Ok(ChunkedOutput {
                source: InputSource::Repl,
                original_length: 0,
                chunks: Vec::new(),
            });
        }

        if trimmed == config.repl.done_command {
            let chunks = chunk_text(&buffer, &config.chunking)?;
            info!(chunk_count = chunks.len(), "chunked REPL input");
            return Ok(ChunkedOutput {
                source: InputSource::Repl,
                original_length: buffer.chars().count(),
                chunks,
            });
        }

        if !buffer.is_empty() {
            buffer.push('\n');
        }
        buffer.push_str(line.trim_end_matches(['\r', '\n']));
        debug!(
            buffer_chars = buffer.chars().count(),
            "appended line to REPL buffer"
        );
    }

    let chunks = chunk_text(&buffer, &config.chunking)?;
    Ok(ChunkedOutput {
        source: InputSource::Repl,
        original_length: buffer.chars().count(),
        chunks,
    })
}

fn emit_chunks(output: &ChunkedOutput, config: &AppConfig) {
    info!(
        source = output.source.label(),
        original_length = output.original_length,
        chunk_count = output.chunks.len(),
        "emitting chunked output"
    );

    if output.chunks.is_empty() {
        if config.output.emit_trailing_newline {
            println!();
        }
        return;
    }

    let rendered = output.chunks.join(&config.output.chunk_separator);
    if config.output.emit_trailing_newline {
        println!("{rendered}");
    } else {
        print!("{rendered}");
    }
}
