use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Result, bail};
use sha2::{Digest, Sha256};
use walkdir::WalkDir;

#[derive(Debug, Clone)]
pub struct CollectedFile {
    pub path: PathBuf,
    pub input_root_name: Option<String>,
    pub relative_path: PathBuf,
}

pub fn collect_files_with_context(inputs: &[PathBuf]) -> Result<Vec<CollectedFile>> {
    let mut files = Vec::new();
    for input in inputs {
        if input.is_file() {
            let file_name = input
                .file_name()
                .and_then(|v| v.to_str())
                .unwrap_or("file")
                .to_string();
            files.push(CollectedFile {
                path: input.clone(),
                input_root_name: None,
                relative_path: PathBuf::from(file_name),
            });
            continue;
        }

        if input.is_dir() {
            let root_name = input
                .file_name()
                .and_then(|v| v.to_str())
                .unwrap_or("folder")
                .to_string();
            for entry in WalkDir::new(input) {
                let entry = entry?;
                if entry.path().is_file() {
                    let relative_path = entry
                        .path()
                        .strip_prefix(input)
                        .map(|p| p.to_path_buf())
                        .unwrap_or_else(|_| PathBuf::from(entry.file_name()));
                    files.push(CollectedFile {
                        path: entry.path().to_path_buf(),
                        input_root_name: Some(root_name.clone()),
                        relative_path,
                    });
                }
            }
            continue;
        }

        bail!("input invalido: {}", input.display());
    }

    files.sort_by(|a, b| a.path.cmp(&b.path));
    files.dedup_by(|a, b| a.path == b.path);
    Ok(files)
}

pub fn read_text_sample(path: &Path, sample_chars: usize) -> Result<Option<String>> {
    let ext = path
        .extension()
        .and_then(|v| v.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();

    let text_like = matches!(
        ext.as_str(),
        "txt" | "md" | "markdown" | "json" | "csv" | "xml" | "html" | "htm" | "yaml" | "yml"
    );
    if !text_like {
        return Ok(None);
    }

    let bytes = fs::read(path)?;
    let text = String::from_utf8_lossy(&bytes);
    let sample: String = text.chars().take(sample_chars).collect();
    Ok(Some(sample))
}

pub fn file_sha256(path: &Path) -> Result<String> {
    let bytes = fs::read(path)?;
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    Ok(format!("{:x}", hasher.finalize()))
}

pub fn sanitize_filename(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    for c in name.chars() {
        let keep = c.is_ascii_alphanumeric() || c == '.' || c == '_' || c == '-';
        if keep {
            out.push(c);
        } else {
            out.push('_');
        }
    }
    if out.is_empty() {
        "file".to_string()
    } else {
        out
    }
}
