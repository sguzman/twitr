use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use tracing::{debug, info, instrument, trace};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AppConfig {
    pub chunking: ChunkingConfig,
    pub repl: ReplConfig,
    pub output: OutputConfig,
    pub logging: LoggingConfig,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            chunking: ChunkingConfig::default(),
            repl: ReplConfig::default(),
            output: OutputConfig::default(),
            logging: LoggingConfig::default(),
        }
    }
}

impl AppConfig {
    #[instrument]
    pub fn load(path: Option<&Path>) -> Result<Self> {
        let resolved_path = path
            .map(Path::to_path_buf)
            .or_else(|| default_config_path().filter(|candidate| candidate.is_file()));

        match resolved_path.as_deref() {
            Some(path) => {
                let config_text = fs::read_to_string(path)
                    .with_context(|| format!("failed to read config file {}", path.display()))?;
                let config = toml::from_str::<AppConfig>(&config_text)
                    .with_context(|| format!("failed to parse config file {}", path.display()))?;
                info!(config_path = %path.display(), "loaded config file");
                Ok(config)
            }
            None => {
                trace!("using built-in default configuration");
                Ok(Self::default())
            }
        }
    }
}

fn default_config_path() -> Option<PathBuf> {
    std::env::current_dir()
        .ok()
        .map(|dir| dir.join("twitr.toml"))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ChunkingConfig {
    pub max_chars: usize,
    pub numbering: bool,
    pub numbering_format: String,
    pub suffix: String,
    pub preserve_paragraphs: bool,
    pub preserve_line_breaks: bool,
    pub collapse_whitespace: bool,
    pub split_sentences: bool,
    pub paragraph_separator: String,
}

impl Default for ChunkingConfig {
    fn default() -> Self {
        Self {
            max_chars: 280,
            numbering: true,
            numbering_format: "{current}/{total} ".to_string(),
            suffix: String::new(),
            preserve_paragraphs: true,
            preserve_line_breaks: false,
            collapse_whitespace: true,
            split_sentences: true,
            paragraph_separator: "\n\n".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ReplConfig {
    pub prompt: String,
    pub done_command: String,
    pub clear_command: String,
    pub help_command: String,
    pub quit_commands: Vec<String>,
    pub show_stats_command: String,
}

impl Default for ReplConfig {
    fn default() -> Self {
        Self {
            prompt: "twitr> ".to_string(),
            done_command: "/done".to_string(),
            clear_command: "/clear".to_string(),
            help_command: "/help".to_string(),
            quit_commands: vec!["/quit".to_string(), "/exit".to_string()],
            show_stats_command: "/stats".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct OutputConfig {
    pub chunk_separator: String,
    pub emit_trailing_newline: bool,
}

impl Default for OutputConfig {
    fn default() -> Self {
        Self {
            chunk_separator: "\n---\n".to_string(),
            emit_trailing_newline: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct LoggingConfig {
    pub filter: String,
    pub ansi: bool,
    pub with_target: bool,
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            filter: "twitr=trace,info".to_string(),
            ansi: true,
            with_target: false,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ChunkedOutput {
    pub source: InputSource,
    pub original_length: usize,
    pub chunks: Vec<String>,
}

#[derive(Debug, Clone)]
pub enum InputSource {
    File(PathBuf),
    Repl,
}

impl InputSource {
    pub fn label(&self) -> String {
        match self {
            Self::File(path) => format!("file:{}", path.display()),
            Self::Repl => "repl".to_string(),
        }
    }
}

#[instrument(skip_all, fields(path = %path.display()))]
pub fn read_input_file(path: &Path) -> Result<String> {
    let contents =
        fs::read_to_string(path).with_context(|| format!("failed to read {}", path.display()))?;
    debug!(bytes = contents.len(), "loaded input file");
    Ok(contents)
}

#[instrument(skip_all)]
pub fn chunk_text(text: &str, config: &ChunkingConfig) -> Result<Vec<String>> {
    if config.max_chars == 0 {
        bail!("chunking.max_chars must be greater than zero");
    }

    let sections = split_manual_sections(text)
        .into_iter()
        .map(|section| normalize_text(&section, config))
        .filter(|section| !section.trim().is_empty())
        .collect::<Vec<_>>();

    debug!(
        original_chars = text.chars().count(),
        section_count = sections.len(),
        "prepared text for chunking"
    );

    if sections.is_empty() {
        return Ok(Vec::new());
    }

    let mut expected_total = 1usize;

    loop {
        let prefix_width = if config.numbering {
            render_prefix(&config.numbering_format, expected_total, expected_total)
                .chars()
                .count()
        } else {
            0
        };
        let suffix_width = config.suffix.chars().count();

        if prefix_width + suffix_width >= config.max_chars {
            bail!(
                "prefix and suffix consume {} chars, leaving no room under the {} char limit",
                prefix_width + suffix_width,
                config.max_chars
            );
        }

        let body_limit = config.max_chars - prefix_width - suffix_width;
        trace!(
            body_limit,
            expected_total, "chunking with reserved numbering width"
        );

        let bodies = sections
            .iter()
            .flat_map(|section| chunk_bodies(section, config, body_limit))
            .collect::<Vec<_>>();
        let actual_total = bodies.len();

        if actual_total == expected_total {
            let chunks = bodies
                .into_iter()
                .enumerate()
                .map(|(index, body)| {
                    let prefix = if config.numbering {
                        render_prefix(&config.numbering_format, index + 1, actual_total)
                    } else {
                        String::new()
                    };

                    format!("{prefix}{body}{}", config.suffix)
                })
                .collect::<Vec<_>>();

            debug!(chunk_count = chunks.len(), "finished chunking");
            return Ok(chunks);
        }

        expected_total = actual_total;
    }
}

fn split_manual_sections(text: &str) -> Vec<String> {
    let normalized_newlines = text.replace("\r\n", "\n").replace('\r', "\n");
    let mut sections = Vec::new();
    let mut current = Vec::new();

    for line in normalized_newlines.lines() {
        if line.trim() == "---" {
            trace!("encountered manual tweet boundary");
            sections.push(current.join("\n"));
            current.clear();
            continue;
        }

        current.push(line);
    }

    sections.push(current.join("\n"));
    sections
}

fn normalize_text(text: &str, config: &ChunkingConfig) -> String {
    let normalized_newlines = text.replace("\r\n", "\n").replace('\r', "\n");

    if config.preserve_line_breaks {
        return normalized_newlines
            .lines()
            .map(|line| collapse_inline_whitespace(line, config.collapse_whitespace))
            .collect::<Vec<_>>()
            .join("\n")
            .trim()
            .to_string();
    }

    let paragraph_separator = config.paragraph_separator.as_str();
    split_paragraphs(&normalized_newlines)
        .into_iter()
        .map(|paragraph| collapse_inline_whitespace(paragraph, config.collapse_whitespace))
        .filter(|paragraph| !paragraph.is_empty())
        .collect::<Vec<_>>()
        .join(paragraph_separator)
}

fn split_paragraphs(text: &str) -> Vec<&str> {
    text.split("\n\n")
        .map(str::trim)
        .filter(|paragraph| !paragraph.is_empty())
        .collect()
}

fn collapse_inline_whitespace(input: &str, collapse_whitespace: bool) -> String {
    if !collapse_whitespace {
        return input.trim().to_string();
    }

    input.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn chunk_bodies(text: &str, config: &ChunkingConfig, body_limit: usize) -> Vec<String> {
    if config.preserve_paragraphs && !config.preserve_line_breaks {
        chunk_paragraphs(text, config, body_limit)
    } else {
        chunk_segments(text, config, body_limit)
    }
}

fn chunk_paragraphs(text: &str, config: &ChunkingConfig, body_limit: usize) -> Vec<String> {
    let separator = config.paragraph_separator.as_str();
    let paragraphs = text
        .split(separator)
        .filter(|paragraph| !paragraph.is_empty())
        .collect::<Vec<_>>();

    let mut chunks = Vec::new();
    let mut current = String::new();

    for paragraph in paragraphs {
        if paragraph.chars().count() > body_limit {
            flush_current(&mut current, &mut chunks);
            chunks.extend(chunk_segments(paragraph, config, body_limit));
            continue;
        }

        if current.is_empty() {
            current.push_str(paragraph);
            continue;
        }

        let candidate = format!("{current}{separator}{paragraph}");
        if candidate.chars().count() <= body_limit {
            current = candidate;
        } else {
            chunks.push(current);
            current = paragraph.to_string();
        }
    }

    flush_current(&mut current, &mut chunks);
    chunks
}

fn chunk_segments(text: &str, config: &ChunkingConfig, body_limit: usize) -> Vec<String> {
    let segments = if config.split_sentences {
        split_sentences(text)
    } else if config.preserve_line_breaks {
        text.lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .map(str::to_string)
            .collect::<Vec<_>>()
    } else {
        vec![text.trim().to_string()]
    };

    let mut chunks = Vec::new();
    let mut current = String::new();

    for segment in segments {
        if segment.chars().count() > body_limit {
            flush_current(&mut current, &mut chunks);
            chunks.extend(chunk_words(&segment, body_limit));
            continue;
        }

        if current.is_empty() {
            current.push_str(&segment);
            continue;
        }

        let joiner = if config.preserve_line_breaks {
            "\n"
        } else {
            " "
        };
        let candidate = format!("{current}{joiner}{segment}");

        if candidate.chars().count() <= body_limit {
            current = candidate;
        } else {
            chunks.push(current);
            current = segment;
        }
    }

    flush_current(&mut current, &mut chunks);
    chunks
}

fn split_sentences(text: &str) -> Vec<String> {
    let mut sentences = Vec::new();
    let mut current = String::new();

    for ch in text.chars() {
        current.push(ch);
        if matches!(ch, '.' | '!' | '?' | '\n') {
            let trimmed = current.trim();
            if !trimmed.is_empty() {
                sentences.push(trimmed.to_string());
            }
            current.clear();
        }
    }

    let trimmed = current.trim();
    if !trimmed.is_empty() {
        sentences.push(trimmed.to_string());
    }

    sentences
}

fn chunk_words(text: &str, body_limit: usize) -> Vec<String> {
    let mut chunks = Vec::new();
    let mut current = String::new();

    for word in text.split_whitespace() {
        let word_len = word.chars().count();

        if word_len > body_limit {
            flush_current(&mut current, &mut chunks);
            chunks.extend(split_long_token(word, body_limit));
            continue;
        }

        if current.is_empty() {
            current.push_str(word);
            continue;
        }

        let candidate = format!("{current} {word}");
        if candidate.chars().count() <= body_limit {
            current = candidate;
        } else {
            chunks.push(current);
            current = word.to_string();
        }
    }

    flush_current(&mut current, &mut chunks);
    chunks
}

fn split_long_token(token: &str, body_limit: usize) -> Vec<String> {
    let mut chunks = Vec::new();
    let mut current = String::new();

    for ch in token.chars() {
        current.push(ch);
        if current.chars().count() == body_limit {
            chunks.push(std::mem::take(&mut current));
        }
    }

    if !current.is_empty() {
        chunks.push(current);
    }

    chunks
}

fn flush_current(current: &mut String, chunks: &mut Vec<String>) {
    if !current.is_empty() {
        chunks.push(std::mem::take(current));
    }
}

fn render_prefix(format: &str, current: usize, total: usize) -> String {
    format
        .replace("{current}", &current.to_string())
        .replace("{total}", &total.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chunks_plain_text_under_the_limit() {
        let config = ChunkingConfig {
            max_chars: 40,
            numbering: false,
            split_sentences: false,
            preserve_paragraphs: false,
            ..ChunkingConfig::default()
        };

        let chunks =
            chunk_text("one two three four five six seven eight nine ten", &config).unwrap();

        assert_eq!(chunks.len(), 2);
        assert!(chunks.iter().all(|chunk| chunk.chars().count() <= 40));
    }

    #[test]
    fn reserves_space_for_numbering_prefix() {
        let config = ChunkingConfig {
            max_chars: 18,
            numbering: true,
            numbering_format: "({current}/{total}) ".to_string(),
            split_sentences: false,
            preserve_paragraphs: false,
            ..ChunkingConfig::default()
        };

        let chunks = chunk_text("alpha beta gamma delta epsilon zeta eta theta", &config).unwrap();

        assert!(chunks.len() > 1);
        assert!(chunks.iter().all(|chunk| chunk.chars().count() <= 18));
        assert!(chunks[0].starts_with("(1/"));
    }

    #[test]
    fn preserves_paragraph_boundaries_when_possible() {
        let config = ChunkingConfig {
            max_chars: 80,
            numbering: false,
            preserve_paragraphs: true,
            paragraph_separator: "\n\n".to_string(),
            ..ChunkingConfig::default()
        };

        let chunks = chunk_text("first paragraph\n\nsecond paragraph", &config).unwrap();

        assert_eq!(chunks, vec!["first paragraph\n\nsecond paragraph"]);
    }

    #[test]
    fn splits_long_tokens_when_needed() {
        let config = ChunkingConfig {
            max_chars: 10,
            numbering: false,
            split_sentences: false,
            preserve_paragraphs: false,
            ..ChunkingConfig::default()
        };

        let chunks = chunk_text("supercalifragilisticexpialidocious", &config).unwrap();
        assert!(chunks.iter().all(|chunk| chunk.chars().count() <= 10));
        assert_eq!(chunks.concat(), "supercalifragilisticexpialidocious");
    }

    #[test]
    fn parses_partial_config_with_defaults() {
        let config: AppConfig = toml::from_str(
            r#"
            [chunking]
            max_chars = 240
            "#,
        )
        .unwrap();

        assert_eq!(config.chunking.max_chars, 240);
        assert_eq!(config.repl.done_command, "/done");
    }

    #[test]
    fn default_config_path_points_to_repo_filename() {
        let path = default_config_path().unwrap();
        assert_eq!(path.file_name().unwrap(), "twitr.toml");
    }

    #[test]
    fn isolated_marker_forces_a_new_chunk_boundary() {
        let config = ChunkingConfig {
            max_chars: 280,
            numbering: false,
            ..ChunkingConfig::default()
        };

        let chunks = chunk_text("first tweet\n---\nsecond tweet", &config).unwrap();

        assert_eq!(chunks, vec!["first tweet", "second tweet"]);
    }

    #[test]
    fn marker_with_surrounding_spaces_still_counts_as_manual_boundary() {
        let config = ChunkingConfig {
            max_chars: 280,
            numbering: false,
            ..ChunkingConfig::default()
        };

        let chunks = chunk_text("alpha\n  ---  \nbeta", &config).unwrap();

        assert_eq!(chunks, vec!["alpha", "beta"]);
    }
}
