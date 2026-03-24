use std::{
    collections::HashMap,
    fs,
    path::{Component, Path},
    process::Command,
};

use anyhow::{Context, Result, bail};
use chrono::Utc;
use reqwest::{
    Client,
    multipart::{Form, Part},
};
use serde_json::json;

use crate::{
    cli::BackupArgs,
    models::{DriveFile, DriveListResponse, DriveUploadResponse, PipelineOutput},
    pipeline::{dry_run_output, prepare_items},
};

pub async fn run_backup(args: BackupArgs) -> Result<()> {
    let items = prepare_items(&args.common, "backup").await?;
    let client = Client::new();

    if args.common.dry_run {
        for item in items {
            let out = dry_run_output(&item, Some("backup dry-run".to_string()));
            println!("{}", serde_json::to_string_pretty(&out)?);
        }
        return Ok(());
    }

    let access_token = resolve_google_access_token(args.google_access_token.as_deref())?;
    let mut folder_cache = HashMap::<String, String>::new();

    for item in items {
        let parent_folder_id = resolve_parent_folder_for_item(
            &client,
            &access_token,
            args.drive_folder_id.as_deref(),
            &item,
            &mut folder_cache,
        )
        .await?;
        let drive_file_name = if item.input_root_name.is_some() {
            item.original_name.clone()
        } else {
            item.canonical_name.clone()
        };
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
            supabase_object_path: None,
        };

        let can_remote_dedupe = item.input_root_name.is_none();
        if args.common.skip_duplicates
            && can_remote_dedupe
            && let Some(existing) = find_file_by_hash(&client, &access_token, &item.sha256).await?
        {
            output.status = "skipped_remote_duplicate".to_string();
            output.note = Some("arquivo ja existe no drive (hash_sha256)".to_string());
            output.drive_file_id = Some(existing.id);
            output.drive_web_view_link = existing.web_view_link;
            maybe_append_sheet_row(
                &client,
                &access_token,
                args.backup_sheet_id.as_deref(),
                &args.backup_sheet_tab,
                &output,
                "backup",
            )
            .await?;
            println!("{}", serde_json::to_string_pretty(&output)?);
            continue;
        }

        let uploaded = upload_to_drive(
            &client,
            &access_token,
            &item.path,
            &item.metadata,
            &item.sha256,
            &item.canonical_name,
            &drive_file_name,
            parent_folder_id.as_deref(),
        )
        .await
        .with_context(|| format!("falha no upload para {}", item.path.display()))?;

        grant_permission(&client, &access_token, &uploaded.id, &args.share_with)
            .await
            .with_context(|| {
                format!(
                    "falha ao compartilhar {} com {}",
                    uploaded.id, args.share_with
                )
            })?;

        output.drive_file_id = Some(uploaded.id);
        output.drive_web_view_link = uploaded.web_view_link;
        if item.input_root_name.is_some() {
            output.note = Some(format!(
                "estrutura_preservada: {} (drive_name={})",
                build_drive_logical_path(&item, &drive_file_name),
                drive_file_name
            ));
        }
        maybe_append_sheet_row(
            &client,
            &access_token,
            args.backup_sheet_id.as_deref(),
            &args.backup_sheet_tab,
            &output,
            "backup",
        )
        .await?;
        println!("{}", serde_json::to_string_pretty(&output)?);
    }

    Ok(())
}

async fn resolve_parent_folder_for_item(
    client: &Client,
    access_token: &str,
    base_folder_id: Option<&str>,
    item: &crate::pipeline::PreparedItem,
    folder_cache: &mut HashMap<String, String>,
) -> Result<Option<String>> {
    let Some(root_name) = item.input_root_name.as_deref() else {
        return Ok(base_folder_id.map(ToString::to_string));
    };

    let mut parent = ensure_drive_folder(
        client,
        access_token,
        root_name,
        base_folder_id,
        folder_cache,
    )
    .await?;

    if let Some(rel_parent) = item.relative_path.parent() {
        for component in rel_parent.components() {
            let Component::Normal(segment) = component else {
                continue;
            };
            let name = segment.to_string_lossy().trim().to_string();
            if name.is_empty() {
                continue;
            }
            parent = ensure_drive_folder(client, access_token, &name, Some(&parent), folder_cache)
                .await?;
        }
    }

    Ok(Some(parent))
}

async fn ensure_drive_folder(
    client: &Client,
    access_token: &str,
    folder_name: &str,
    parent_id: Option<&str>,
    folder_cache: &mut HashMap<String, String>,
) -> Result<String> {
    let cache_key = format!("{}::{}", parent_id.unwrap_or("root"), folder_name);
    if let Some(existing) = folder_cache.get(&cache_key) {
        return Ok(existing.clone());
    }

    if let Some(found) = find_folder(client, access_token, folder_name, parent_id).await? {
        folder_cache.insert(cache_key, found.clone());
        return Ok(found);
    }

    let created = create_folder(client, access_token, folder_name, parent_id).await?;
    folder_cache.insert(cache_key, created.clone());
    Ok(created)
}

async fn find_folder(
    client: &Client,
    access_token: &str,
    folder_name: &str,
    parent_id: Option<&str>,
) -> Result<Option<String>> {
    let folder_name_escaped = escape_drive_query_literal(folder_name);
    let parent_clause = match parent_id {
        Some(parent) => format!("'{parent}' in parents"),
        None => "'root' in parents".to_string(),
    };
    let query = format!(
        "trashed = false and mimeType = 'application/vnd.google-apps.folder' and name = '{folder_name_escaped}' and {parent_clause}"
    );
    let resp = client
        .get("https://www.googleapis.com/drive/v3/files")
        .bearer_auth(access_token)
        .query(&[
            ("q", query.as_str()),
            ("pageSize", "1"),
            ("fields", "files(id)"),
        ])
        .send()
        .await?;
    let status = resp.status();
    let body = resp.text().await?;
    if !status.is_success() {
        bail!("erro ao buscar pasta no Drive {}: {}", status, body);
    }
    let parsed: DriveListResponse = serde_json::from_str(&body)
        .with_context(|| format!("resposta invalida no find_folder: {body}"))?;
    Ok(parsed.files.and_then(|mut files| files.pop()).map(|f| f.id))
}

async fn create_folder(
    client: &Client,
    access_token: &str,
    folder_name: &str,
    parent_id: Option<&str>,
) -> Result<String> {
    let mut payload = json!({
        "name": folder_name,
        "mimeType": "application/vnd.google-apps.folder"
    });
    if let Some(parent) = parent_id {
        payload["parents"] = json!([parent]);
    }

    let resp = client
        .post("https://www.googleapis.com/drive/v3/files?fields=id")
        .bearer_auth(access_token)
        .json(&payload)
        .send()
        .await?;
    let status = resp.status();
    let body = resp.text().await?;
    if !status.is_success() {
        bail!("erro ao criar pasta no Drive {}: {}", status, body);
    }
    let id = serde_json::from_str::<serde_json::Value>(&body)
        .ok()
        .and_then(|v| v.get("id").and_then(|id| id.as_str()).map(str::to_string))
        .ok_or_else(|| anyhow::anyhow!("resposta sem id ao criar pasta: {body}"))?;
    Ok(id)
}

fn escape_drive_query_literal(s: &str) -> String {
    s.replace('\\', "\\\\").replace('\'', "\\'")
}

fn build_drive_logical_path(item: &crate::pipeline::PreparedItem, drive_file_name: &str) -> String {
    let mut parts = Vec::<String>::new();
    if let Some(root) = item.input_root_name.as_ref() {
        parts.push(root.clone());
    }
    if let Some(parent) = item.relative_path.parent() {
        for comp in parent.components() {
            if let Component::Normal(v) = comp {
                parts.push(v.to_string_lossy().to_string());
            }
        }
    }
    parts.push(drive_file_name.to_string());
    parts.join("/")
}

async fn maybe_append_sheet_row(
    client: &Client,
    access_token: &str,
    sheet_id: Option<&str>,
    tab: &str,
    out: &PipelineOutput,
    pipeline: &str,
) -> Result<()> {
    let Some(sheet_id) = sheet_id else {
        return Ok(());
    };
    let ts = Utc::now().to_rfc3339();
    let row = serde_json::json!({
        "values": [[
            ts,
            pipeline,
            out.file,
            out.source_relative_path.clone().unwrap_or_default(),
            out.canonical_name,
            out.sha256,
            out.drive_file_id.clone().unwrap_or_default(),
            out.drive_web_view_link.clone().unwrap_or_default(),
            out.status,
            out.note.clone().unwrap_or_default()
        ]]
    });
    let range = format!("{tab}!A:J");
    let url = format!(
        "https://sheets.googleapis.com/v4/spreadsheets/{}/values/{}:append?valueInputOption=RAW",
        sheet_id,
        urlencoding::encode(&range)
    );
    let resp = client
        .post(url)
        .bearer_auth(access_token)
        .json(&row)
        .send()
        .await?;
    let status = resp.status();
    let body = resp.text().await?;
    if !status.is_success() {
        bail!("falha ao registrar backup no sheets {}: {}", status, body);
    }
    Ok(())
}

async fn find_file_by_hash(
    client: &Client,
    access_token: &str,
    hash: &str,
) -> Result<Option<DriveFile>> {
    let query =
        format!("trashed = false and appProperties has {{ key='hash_sha256' and value='{hash}' }}");
    let url = "https://www.googleapis.com/drive/v3/files";
    let resp = client
        .get(url)
        .bearer_auth(access_token)
        .query(&[
            ("q", query.as_str()),
            ("pageSize", "1"),
            ("fields", "files(id,webViewLink)"),
        ])
        .send()
        .await?;

    let status = resp.status();
    let body = resp.text().await?;
    if !status.is_success() {
        bail!("erro ao buscar duplicado no Drive {}: {}", status, body);
    }

    let parsed: DriveListResponse = serde_json::from_str(&body)
        .with_context(|| format!("resposta invalida drive list: {body}"))?;
    Ok(parsed.files.and_then(|mut f| f.pop()))
}

async fn upload_to_drive(
    client: &Client,
    access_token: &str,
    path: &Path,
    meta: &crate::models::NormalizedMetadata,
    sha256: &str,
    canonical_name: &str,
    drive_file_name: &str,
    folder_id: Option<&str>,
) -> Result<DriveUploadResponse> {
    let bytes = fs::read(path)?;
    let mime = mime_guess::from_path(path).first_or_octet_stream();

    let mut metadata = json!({
        "name": drive_file_name,
        "description": meta.description.clone().unwrap_or_default(),
        "appProperties": {
            "hash_sha256": sha256,
            "canonical_name": canonical_name,
            "document_type": meta.document_type.clone().unwrap_or_default(),
            "language": meta.language.clone().unwrap_or_default(),
            "tags_csv": meta.tags.clone().unwrap_or_default().join(",")
        }
    });
    if let Some(folder) = folder_id {
        metadata["parents"] = json!([folder]);
    }

    let metadata_part = Part::text(metadata.to_string())
        .mime_str("application/json; charset=UTF-8")
        .context("falha ao montar metadata multipart")?;
    let media_part = Part::bytes(bytes)
        .file_name(drive_file_name.to_string())
        .mime_str(mime.as_ref())
        .context("falha ao montar media multipart")?;

    let form = Form::new()
        .part("metadata", metadata_part)
        .part("file", media_part);
    let url = "https://www.googleapis.com/upload/drive/v3/files?uploadType=multipart&fields=id,webViewLink";

    let resp = client
        .post(url)
        .bearer_auth(access_token)
        .multipart(form)
        .send()
        .await?;
    let status = resp.status();
    let body = resp.text().await?;
    if !status.is_success() {
        bail!("drive upload error {}: {}", status, body);
    }

    let parsed: DriveUploadResponse = serde_json::from_str(&body)
        .with_context(|| format!("resposta invalida do drive upload: {body}"))?;
    Ok(parsed)
}

async fn grant_permission(
    client: &Client,
    access_token: &str,
    file_id: &str,
    email: &str,
) -> Result<()> {
    let url = format!(
        "https://www.googleapis.com/drive/v3/files/{file_id}/permissions?sendNotificationEmail=false"
    );
    let body = json!({
        "role": "writer",
        "type": "user",
        "emailAddress": email
    });
    let resp = client
        .post(url)
        .bearer_auth(access_token)
        .json(&body)
        .send()
        .await?;
    let status = resp.status();
    let raw = resp.text().await?;
    if !status.is_success() {
        bail!("drive permission error {}: {}", status, raw);
    }
    Ok(())
}

fn resolve_google_access_token(input: Option<&str>) -> Result<String> {
    if let Some(token) = input {
        return Ok(token.trim().to_string());
    }

    if let Ok(token) = std::env::var("GOOGLE_OAUTH_ACCESS_TOKEN") {
        let t = token.trim().to_string();
        if !t.is_empty() {
            return Ok(t);
        }
    }

    let candidates: [(&str, &[&str]); 2] = [
        (
            "gcloud",
            &["auth", "application-default", "print-access-token"],
        ),
        ("gcloud", &["auth", "print-access-token"]),
    ];

    for (bin, args) in candidates {
        let output = Command::new(bin).args(args).output();
        if let Ok(output) = output
            && output.status.success()
        {
            let token = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !token.is_empty() {
                return Ok(token);
            }
        }
    }

    bail!(
        "nao consegui resolver token oauth do google; use --google-access-token ou autentique no gcloud"
    )
}
