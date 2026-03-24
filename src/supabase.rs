use std::{
    fs,
    path::{Component, Path},
};

use anyhow::{Context, Result, anyhow, bail};
use reqwest::Client;
use serde_json::json;

use crate::{
    cli::OfficializeArgs,
    fileops::sanitize_filename,
    models::PipelineOutput,
    pipeline::{dry_run_output, prepare_items},
};

pub async fn run_officialize(args: OfficializeArgs) -> Result<()> {
    let items = prepare_items(&args.common, "officialize").await?;
    let client = Client::new();

    if args.common.dry_run {
        for item in items {
            let out = dry_run_output(&item, Some("officialize dry-run".to_string()));
            println!("{}", serde_json::to_string_pretty(&out)?);
        }
        return Ok(());
    }

    let base_url = args
        .supabase_url
        .as_deref()
        .ok_or_else(|| anyhow!("faltou --supabase-url ou SUPABASE_URL"))?
        .trim_end_matches('/')
        .to_string();
    let key = args
        .supabase_key
        .as_deref()
        .ok_or_else(|| anyhow!("faltou --supabase-key ou SUPABASE_SERVICE_ROLE_KEY"))?
        .to_string();

    for item in items {
        let object_path = build_object_path(&args.supabase_path_prefix, &item);
        let mut output = PipelineOutput {
            file: item.path.display().to_string(),
            source_relative_path: Some(item.source_relative_path()),
            sha256: item.sha256.clone(),
            canonical_name: item.canonical_name.clone(),
            normalized_title: item.metadata.title.clone(),
            status: "processed".to_string(),
            note: None,
            drive_file_id: None,
            drive_web_view_link: None,
            supabase_object_path: Some(object_path.clone()),
        };
        if item.input_root_name.is_some() {
            output.note = Some(format!(
                "estrutura_preservada: {}",
                build_storage_logical_path(&args.supabase_path_prefix, &item)
            ));
        }

        if args.common.skip_duplicates
            && object_exists(
                &client,
                &base_url,
                &key,
                &args.supabase_bucket,
                &object_path,
            )
            .await?
        {
            output.status = "skipped_remote_duplicate".to_string();
            output.note = Some("objeto ja existe no supabase storage".to_string());
            println!("{}", serde_json::to_string_pretty(&output)?);
            continue;
        }

        let bytes = fs::read(&item.path)?;
        upload_object(
            &client,
            &base_url,
            &key,
            &args.supabase_bucket,
            &object_path,
            &bytes,
            mime_guess::from_path(&item.path)
                .first_or_octet_stream()
                .essence_str(),
        )
        .await
        .with_context(|| format!("falha upload supabase para {}", item.path.display()))?;

        if args.supabase_write_sidecar {
            let sidecar_path = format!("{object_path}.metadata.json");
            let sidecar = json!({
                "sha256": item.sha256,
                "source_path": item.path.display().to_string(),
                "source_relative_path": item.source_relative_path(),
                "original_name": item.original_name,
                "canonical_name": item.canonical_name,
                "normalized": item.metadata,
            });
            upload_object(
                &client,
                &base_url,
                &key,
                &args.supabase_bucket,
                &sidecar_path,
                sidecar.to_string().as_bytes(),
                "application/json",
            )
            .await
            .with_context(|| format!("falha upload sidecar: {}", sidecar_path))?;
        }

        upsert_row(
            &client,
            &base_url,
            &key,
            &args.supabase_table,
            &item.path.display().to_string(),
            &item.source_relative_path(),
            &object_path,
            &args.supabase_bucket,
            &item.sha256,
            &item.canonical_name,
            &item.metadata,
        )
        .await
        .with_context(|| format!("falha upsert na tabela {}", args.supabase_table))?;

        println!("{}", serde_json::to_string_pretty(&output)?);
    }

    Ok(())
}

fn build_object_path(prefix: &str, item: &crate::pipeline::PreparedItem) -> String {
    let mut segments = prefix_segments(prefix);

    if let Some(root) = item.input_root_name.as_ref() {
        segments.push(sanitize_filename(root));
        if let Some(parent) = item.relative_path.parent() {
            segments.extend(path_parent_segments(parent));
        }
        segments.push(sanitize_filename(&item.original_name));
    } else {
        segments.push(sanitize_filename(&item.canonical_name));
    }

    join_segments(&segments)
}

fn build_storage_logical_path(prefix: &str, item: &crate::pipeline::PreparedItem) -> String {
    let mut parts = Vec::<String>::new();
    let cleaned_prefix = prefix.trim_matches('/');
    if !cleaned_prefix.is_empty() {
        parts.push(cleaned_prefix.to_string());
    }
    if let Some(root) = item.input_root_name.as_ref() {
        parts.push(root.clone());
        if let Some(parent) = item.relative_path.parent() {
            for comp in parent.components() {
                if let Component::Normal(v) = comp {
                    parts.push(v.to_string_lossy().to_string());
                }
            }
        }
        parts.push(item.original_name.clone());
    } else {
        parts.push(item.canonical_name.clone());
    }
    parts.join("/")
}

fn prefix_segments(prefix: &str) -> Vec<String> {
    prefix
        .trim_matches('/')
        .split('/')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(sanitize_filename)
        .collect()
}

fn path_parent_segments(parent: &Path) -> Vec<String> {
    parent
        .components()
        .filter_map(|comp| match comp {
            Component::Normal(v) => Some(sanitize_filename(&v.to_string_lossy())),
            _ => None,
        })
        .filter(|s| !s.is_empty())
        .collect()
}

fn join_segments(segments: &[String]) -> String {
    segments.join("/")
}

async fn object_exists(
    client: &Client,
    base_url: &str,
    key: &str,
    bucket: &str,
    object_path: &str,
) -> Result<bool> {
    let encoded = encode_path(object_path);
    let url = format!("{base_url}/storage/v1/object/{bucket}/{encoded}");
    let resp = client
        .head(url)
        .header("apikey", key)
        .header("Authorization", format!("Bearer {key}"))
        .send()
        .await?;

    if resp.status().is_success() {
        return Ok(true);
    }
    if resp.status().as_u16() == 404 || resp.status().as_u16() == 400 {
        return Ok(false);
    }
    let status = resp.status();
    let body = resp.text().await.unwrap_or_default();
    bail!("erro ao verificar objeto no supabase {}: {}", status, body);
}

async fn upload_object(
    client: &Client,
    base_url: &str,
    key: &str,
    bucket: &str,
    object_path: &str,
    content: &[u8],
    content_type: &str,
) -> Result<()> {
    let encoded = encode_path(object_path);
    let url = format!("{base_url}/storage/v1/object/{bucket}/{encoded}");
    let resp = client
        .post(url)
        .header("apikey", key)
        .header("Authorization", format!("Bearer {key}"))
        .header("x-upsert", "false")
        .header("content-type", content_type)
        .body(content.to_vec())
        .send()
        .await?;

    let status = resp.status();
    let body = resp.text().await.unwrap_or_default();
    if !status.is_success() {
        bail!("supabase storage upload error {}: {}", status, body);
    }
    Ok(())
}

async fn upsert_row(
    client: &Client,
    base_url: &str,
    key: &str,
    table: &str,
    source_path: &str,
    source_relative_path: &str,
    storage_path: &str,
    storage_bucket: &str,
    sha256: &str,
    canonical_name: &str,
    meta: &crate::models::NormalizedMetadata,
) -> Result<()> {
    let url = format!("{base_url}/rest/v1/{table}?on_conflict=storage_path");
    let row = json!({
        "sha256": sha256,
        "source_path": source_path,
        "metadata": {
            "source_relative_path": source_relative_path,
            "storage_path": storage_path
        },
        "storage_bucket": storage_bucket,
        "storage_path": storage_path,
        "canonical_name": canonical_name,
        "normalized_title": meta.title,
        "description": meta.description.clone().unwrap_or_default(),
        "tags": meta.tags.clone().unwrap_or_default(),
        "document_type": meta.document_type.clone().unwrap_or_default(),
        "language": meta.language.clone().unwrap_or_default()
    });

    let resp = client
        .post(url)
        .header("apikey", key)
        .header("Authorization", format!("Bearer {key}"))
        .header("Prefer", "resolution=merge-duplicates,return=minimal")
        .json(&vec![row])
        .send()
        .await?;

    let status = resp.status();
    let body = resp.text().await.unwrap_or_default();
    if !status.is_success() {
        bail!("supabase upsert error {}: {}", status, body);
    }
    Ok(())
}

fn encode_path(path: &str) -> String {
    path.split('/')
        .map(urlencoding::encode)
        .map(|v| v.into_owned())
        .collect::<Vec<_>>()
        .join("/")
}
