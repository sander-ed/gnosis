use std::collections::BTreeMap;
use std::path::Path;

use anyhow::{Context, Result, bail, ensure};
use serde_yaml_ng::{Mapping, Value};

use crate::tree::{MAX_FILE, Tree, safe_path};

#[derive(Default)]
pub struct ConceptMetadata {
    pub kind: Option<String>,
    pub title: Option<String>,
    pub description: Option<String>,
    pub resource: Option<String>,
    pub tags: Vec<String>,
    pub sources: Vec<String>,
    pub fields: Vec<String>,
}

impl ConceptMetadata {
    fn values(&self, name: &str) -> Result<Mapping> {
        let mut metadata = Mapping::new();
        for (key, value) in [
            ("type", &self.kind),
            ("title", &self.title),
            ("description", &self.description),
            ("resource", &self.resource),
        ] {
            if let Some(value) = value {
                metadata.insert(Value::from(key), Value::from(value.clone()));
            }
        }
        if !self.tags.is_empty() {
            metadata.insert(Value::from("tags"), serde_yaml_ng::to_value(&self.tags)?);
        }
        if !self.sources.is_empty() {
            let sources: Vec<_> = self
                .sources
                .iter()
                .map(|source| BTreeMap::from([("resource", source)]))
                .collect();
            metadata.insert(Value::from("sources"), serde_yaml_ng::to_value(sources)?);
        }
        for field in &self.fields {
            let (key, yaml) = field.split_once('=').context("--field requires KEY=YAML")?;
            ensure!(
                !key.trim().is_empty() && key.trim() == key,
                "metadata field needs a nonempty key without surrounding whitespace"
            );
            ensure!(
                !metadata.contains_key(key),
                "metadata field {key:?} was supplied more than once"
            );
            let value: Value = serde_yaml_ng::from_str(yaml)
                .with_context(|| format!("invalid YAML for field {key}"))?;
            metadata.insert(Value::from(key), value);
        }
        if !metadata.contains_key("type") {
            metadata.insert(Value::from("type"), Value::from("Reference"));
        }
        if !metadata.contains_key("title") {
            let title = name.replace(['-', '_'], " ");
            let mut chars = title.chars();
            let title = match chars.next() {
                Some(first) => format!("{}{}", first.to_uppercase(), chars.as_str()),
                None => bail!("name cannot be empty"),
            };
            metadata.insert(Value::from("title"), Value::from(title));
        }
        let title = metadata
            .get("title")
            .and_then(Value::as_str)
            .context("title must be a string")?;
        ensure!(
            !title.trim().is_empty() && !title.contains(['\n', '\r']),
            "title must be a nonempty single line"
        );
        ensure!(
            metadata
                .get("type")
                .and_then(Value::as_str)
                .is_some_and(|kind| !kind.trim().is_empty()),
            "concept needs a nonempty string type"
        );
        Ok(metadata)
    }

    pub fn render_directory(&self, path: &str) -> Result<Vec<u8>> {
        let name = Path::new(path)
            .file_name()
            .and_then(|name| name.to_str())
            .context("invalid directory name")?;
        let bytes = serde_yaml_ng::to_string(&self.values(name)?)?.into_bytes();
        ensure!(bytes.len() <= MAX_FILE, "directory metadata exceeds 16 MiB");
        Ok(bytes)
    }

    pub fn render(&self, path: &str, body: Option<&str>) -> Result<Vec<u8>> {
        let stem = Path::new(path)
            .file_stem()
            .and_then(|stem| stem.to_str())
            .context("invalid concept filename")?;
        let metadata = self.values(stem)?;
        let title = metadata
            .get("title")
            .and_then(Value::as_str)
            .context("title must be a string")?;
        let default_body = format!("# {}\n", escape(title));
        let document = format!(
            "---\n{}---\n{}",
            serde_yaml_ng::to_string(&metadata)?,
            body.unwrap_or(&default_body)
        );
        ensure!(document.len() <= MAX_FILE, "new concept exceeds 16 MiB");
        let bytes = document.into_bytes();
        validate_okf(&Tree::from([(path.to_owned(), bytes.clone())]))?;
        Ok(bytes)
    }
}

pub(crate) fn directory_metadata(path: &str, bytes: &[u8]) -> Result<Mapping> {
    let parse = || -> Result<Mapping> {
        let value: Value = serde_yaml_ng::from_slice(bytes)?;
        let mapping = value
            .as_mapping()
            .context("directory metadata must be a YAML mapping")?;
        for key in ["type", "title", "description"] {
            if let Some(value) = mapping.get(key) {
                let text = value
                    .as_str()
                    .with_context(|| format!("{key} must be a string"))?;
                if key != "description" {
                    ensure!(!text.trim().is_empty(), "{key} must not be empty");
                }
                if key == "title" {
                    ensure!(!text.contains(['\n', '\r']), "title must be a single line");
                }
            }
        }
        Ok(mapping.clone())
    };
    parse().with_context(|| format!("invalid directory metadata {path}"))
}

pub(crate) fn frontmatter(text: &str) -> Result<Option<Value>> {
    let normalized = text.replace("\r\n", "\n");
    let Some(rest) = normalized.strip_prefix("---\n") else {
        return Ok(None);
    };
    let mut yaml = String::new();
    for line in rest.lines() {
        if line == "---" {
            return Ok(Some(serde_yaml_ng::from_str(&yaml)?));
        }
        yaml.push_str(line);
        yaml.push('\n');
    }
    bail!("unterminated YAML frontmatter")
}

pub fn validate_okf(tree: &Tree) -> Result<()> {
    for (path, bytes) in tree {
        safe_path(path)?;
        if path.rsplit('/').next() == Some("index.yml") {
            directory_metadata(path, bytes)?;
        }
        if !path.ends_with(".md") {
            continue;
        }
        let check = || -> Result<()> {
            let text = std::str::from_utf8(bytes)?;
            let metadata = frontmatter(text)?;
            match path.rsplit('/').next() {
                Some("index.md") => {
                    if let Some(metadata) = metadata {
                        ensure!(path == "index.md", "nested indexes cannot have frontmatter");
                        let mapping = metadata
                            .as_mapping()
                            .context("index frontmatter must be a mapping")?;
                        ensure!(
                            mapping.len() == 1
                                && mapping.get("okf_version").and_then(Value::as_str)
                                    == Some("0.2"),
                            "root index frontmatter must contain only okf_version: \"0.2\""
                        );
                    }
                }
                Some("log.md") => ensure!(metadata.is_none(), "log.md cannot have frontmatter"),
                _ => {
                    let metadata = metadata.context("concept needs YAML frontmatter")?;
                    let mapping = metadata
                        .as_mapping()
                        .context("frontmatter must be a mapping")?;
                    ensure!(
                        mapping
                            .get("type")
                            .and_then(Value::as_str)
                            .is_some_and(|s| !s.trim().is_empty()),
                        "concept needs a nonempty string type"
                    );
                }
            }
            Ok(())
        };
        check().with_context(|| format!("invalid OKF document {path}"))?;
    }
    Ok(())
}

pub fn escape(text: &str) -> String {
    text.replace(['\n', '\r'], " ")
        .replace('\\', "\\\\")
        .replace('[', "\\[")
        .replace(']', "\\]")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}
