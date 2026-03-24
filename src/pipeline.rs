use std::{collections::HashSet, path::PathBuf};

use anyhow::{Result, anyhow};
use reqwest::Client;

use crate::{
    cli::CommonPipelineArgs,
    fileops::{collect_files_with_context, file_sha256, read_text_sample},
    models::{NormalizedMetadata, PipelineOutput},
    normalize::{NormalizerConfig, normalize_metadata},
    rules::{build_canonical_name, load_official_rules},
};

#[derive(Debug, Clone)]
pub struct PreparedItem {
    pub path: PathBuf,
    pub original_name: String,
    pub input_root_name: Option<String>,
    pub relative_path: PathBuf,
    pub sha256: String,
    pub canonical_name: String,
    pub metadata: NormalizedMetadata,
}

impl PreparedItem {
    pub fn source_relative_path(&self) -> String {
        if let Some(root) = self.input_root_name.as_ref() {
            format!("{}/{}", root, self.relative_path.display())
        } else {
            self.relative_path.display().to_string()
        }
    }
}

pub async fn prepare_items(
    common: &CommonPipelineArgs,
    pipeline_name: &str,
) -> Result<Vec<PreparedItem>> {
    let files = collect_files_with_context(&common.inputs)?;
    if files.is_empty() {
        return Err(anyhow!("nenhum arquivo encontrado nos inputs"));
    }

    eprintln!("Arquivos encontrados: {}", files.len());

    let client = Client::new();
    let rules_url = common
        .rules_supabase_url
        .as_deref()
        .ok_or_else(|| anyhow!("faltou SUPABASE_URL para carregar regras oficiais"))?;
    let rules_key = common
        .rules_supabase_key
        .as_deref()
        .ok_or_else(|| anyhow!("faltou SUPABASE_SERVICE_ROLE_KEY para carregar regras oficiais"))?;
    let rules = load_official_rules(
        &client,
        rules_url,
        rules_key,
        &common.rules_bucket,
        &common.rules_pointer_object,
    )
    .await?;
    let rules_json = serde_json::to_string_pretty(&rules)?;

    let mut seen_keys: HashSet<String> = HashSet::new();
    let mut items = Vec::new();

    for file in files {
        let original_name = file
            .path
            .file_name()
            .and_then(|v| v.to_str())
            .ok_or_else(|| anyhow!("nome de arquivo invalido: {}", file.path.display()))?
            .to_string();

        let sha256 = file_sha256(&file.path)?;
        let dedupe_key = if let Some(root) = file.input_root_name.as_ref() {
            format!(
                "folder:{}:{}:{}",
                root,
                file.relative_path.display(),
                sha256
            )
        } else {
            format!("single:{}", sha256)
        };
        if common.skip_duplicates && !seen_keys.insert(dedupe_key) {
            eprintln!("Pulando duplicado local: {}", file.path.display());
            continue;
        }

        let sample = read_text_sample(&file.path, common.sample_chars)?;
        let metadata = normalize_metadata(
            &client,
            NormalizerConfig {
                normalizer: &common.normalizer,
                ollama_host: &common.ollama_host,
                ollama_model: &common.ollama_model,
                gemini_api_key: common.gemini_api_key.as_deref(),
                gemini_model: &common.gemini_model,
                rules_json: &rules_json,
            },
            &original_name,
            sample.as_deref(),
        )
        .await?;

        let extension = file
            .path
            .extension()
            .and_then(|v| v.to_str())
            .unwrap_or("bin");
        let canonical_name = build_canonical_name(
            &rules,
            pipeline_name,
            &metadata.title,
            metadata.document_type.as_deref(),
            &sha256,
            extension,
        )?;

        items.push(PreparedItem {
            path: file.path,
            original_name,
            input_root_name: file.input_root_name,
            relative_path: file.relative_path,
            sha256,
            canonical_name,
            metadata,
        });
    }

    Ok(items)
}

pub fn dry_run_output(item: &PreparedItem, note: Option<String>) -> PipelineOutput {
    PipelineOutput {
        file: item.path.display().to_string(),
        source_relative_path: Some(item.source_relative_path()),
        sha256: item.sha256.clone(),
        canonical_name: item.canonical_name.clone(),
        normalized_title: item.metadata.title.clone(),
        status: "dry_run".to_string(),
        note,
        drive_file_id: None,
        drive_web_view_link: None,
        supabase_object_path: None,
    }
}
