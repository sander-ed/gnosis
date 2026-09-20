use std::collections::{BTreeMap, BTreeSet};

use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};

use crate::contributors::Contributors;
use crate::tree::{MAX_FILE, Tree};

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Source {
    pub repository: String,
    #[serde(rename = "ref")]
    pub reference: String,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    pub format: u32,
    #[serde(default)]
    pub packages: BTreeSet<String>,
    #[serde(default)]
    pub sources: BTreeMap<String, Source>,
    #[serde(default)]
    pub dependencies: BTreeMap<String, String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Package {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub contributors: Option<Contributors>,
    pub name: String,
    pub owner: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub dependencies: BTreeSet<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LockedPackage {
    pub source: String,
    pub location: Source,
    pub commit: String,
    pub dependencies: BTreeSet<String>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Lock {
    pub format: u32,
    #[serde(default)]
    pub requirements: BTreeMap<String, String>,
    #[serde(default)]
    pub sources: BTreeMap<String, Source>,
    #[serde(default)]
    pub packages: BTreeMap<String, LockedPackage>,
}

pub fn decode<T: for<'de> Deserialize<'de>>(bytes: &[u8]) -> Result<T> {
    ensure!(bytes.len() <= MAX_FILE, "metadata exceeds 16 MiB");
    Ok(toml::from_str(std::str::from_utf8(bytes)?)?)
}

pub fn encode(value: &impl Serialize) -> Result<Vec<u8>> {
    Ok(toml::to_string_pretty(value)?.into_bytes())
}

pub fn name(value: &str) -> Result<()> {
    ensure!(
        !value.is_empty()
            && value.len() <= 64
            && value.split('-').all(|part| !part.is_empty()
                && part
                    .bytes()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit())),
        "invalid name {value:?}: use lowercase letters/digits separated by single hyphens"
    );
    Ok(())
}

pub fn validate_source(source: &Source) -> Result<()> {
    ensure!(!source.repository.is_empty(), "empty source repository");
    ensure!(
        !source.repository.starts_with('-') && !source.repository.contains("::"),
        "unsupported Git repository syntax"
    );
    if source.repository.starts_with("https://") || source.repository.starts_with("http://") {
        let authority = source
            .repository
            .split("://")
            .nth(1)
            .unwrap_or("")
            .split('/')
            .next()
            .unwrap_or("");
        ensure!(
            !authority.contains('@'),
            "do not embed credentials in source URLs; use Git authentication"
        );
    }
    ensure!(
        !source.reference.is_empty() && !source.reference.starts_with('-'),
        "invalid Git ref"
    );
    Ok(())
}

pub fn validate_manifest(manifest: &Manifest) -> Result<()> {
    ensure!(
        manifest.format == 1,
        "unsupported manifest format {}",
        manifest.format
    );
    for package in &manifest.packages {
        name(package)?;
        ensure!(
            !manifest.dependencies.contains_key(package),
            "{package} is both local and imported"
        );
    }
    for (alias, source) in &manifest.sources {
        name(alias)?;
        validate_source(source)?;
    }
    for (package, source) in &manifest.dependencies {
        name(package)?;
        ensure!(
            manifest.sources.contains_key(source),
            "unknown source {source} for {package}"
        );
    }
    Ok(())
}

pub fn package(tree: &Tree, expected: &str) -> Result<Package> {
    let manifest: Package = decode(tree.get("package.toml").context("missing package.toml")?)?;
    if let Some(policy) = &manifest.contributors {
        policy.validate()?;
    }
    name(&manifest.name)?;
    ensure!(
        manifest.name == expected,
        "package {expected} declares name {}",
        manifest.name
    );
    ensure!(
        !manifest.owner.trim().is_empty(),
        "package {expected} needs an owner"
    );
    for dependency in &manifest.dependencies {
        name(dependency)?;
    }
    Ok(manifest)
}
