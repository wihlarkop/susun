//! Minimal Dockerignore pattern evaluation.

use std::path::Path;

/// Parsed `.dockerignore` document.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Dockerignore {
    patterns: Vec<DockerignorePattern>,
}

impl Dockerignore {
    /// Parses `.dockerignore` contents.
    pub fn parse(contents: &str) -> Self {
        let patterns = contents
            .lines()
            .filter_map(DockerignorePattern::parse)
            .collect();
        Self { patterns }
    }

    /// Returns true when `path` should be excluded from the build context.
    pub fn is_ignored(&self, path: &Path, is_dir: bool) -> bool {
        let normalized = normalize_path(path);
        let mut ignored = false;
        for pattern in &self.patterns {
            if pattern.matches(&normalized, is_dir) {
                ignored = !pattern.negated;
            }
        }
        ignored
    }
}

/// One dockerignore pattern.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DockerignorePattern {
    pattern: String,
    negated: bool,
    directory_only: bool,
}

impl DockerignorePattern {
    /// Parses one dockerignore line.
    pub fn parse(line: &str) -> Option<Self> {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            return None;
        }

        let (negated, body) = match trimmed.strip_prefix('!') {
            Some(rest) => (true, rest.trim()),
            None => (false, trimmed),
        };
        if body.is_empty() {
            return None;
        }

        let directory_only = body.ends_with('/');
        let pattern = body
            .trim_matches('/')
            .replace('\\', "/")
            .trim_end_matches('/')
            .to_owned();
        if pattern.is_empty() {
            return None;
        }

        Some(Self {
            pattern,
            negated,
            directory_only,
        })
    }

    /// Returns true when this pattern matches `normalized_path`.
    pub fn matches(&self, normalized_path: &str, is_dir: bool) -> bool {
        if self.directory_only && !is_dir {
            return false;
        }

        if self.pattern.contains('/') {
            wildcard_match(&self.pattern, normalized_path)
        } else {
            normalized_path
                .split('/')
                .any(|segment| wildcard_match(&self.pattern, segment))
        }
    }
}

fn normalize_path(path: &Path) -> String {
    path.components()
        .filter_map(|component| component.as_os_str().to_str())
        .filter(|segment| !segment.is_empty())
        .collect::<Vec<_>>()
        .join("/")
}

fn wildcard_match(pattern: &str, value: &str) -> bool {
    let pattern_chars: Vec<char> = pattern.chars().collect();
    let value_chars: Vec<char> = value.chars().collect();
    wildcard_match_inner(&pattern_chars, &value_chars)
}

fn wildcard_match_inner(pattern: &[char], value: &[char]) -> bool {
    match (pattern.split_first(), value.split_first()) {
        (None, None) => true,
        (None, Some(_)) => false,
        (Some(('*', rest)), _) => {
            wildcard_match_inner(rest, value)
                || value
                    .split_first()
                    .is_some_and(|(_, tail)| wildcard_match_inner(pattern, tail))
        }
        (Some(('?', rest)), Some((_, value_rest))) => wildcard_match_inner(rest, value_rest),
        (Some((p, rest)), Some((v, value_rest))) if p == v => {
            wildcard_match_inner(rest, value_rest)
        }
        _ => false,
    }
}
