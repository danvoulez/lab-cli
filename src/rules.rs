use anyhow::{Context, Result, anyhow, bail};
use chrono::Utc;
use reqwest::Client;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NamingRules {
    pub version: String,
    pub canonical_naming: CanonicalNaming,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CanonicalNaming {
    pub pattern: String,
    pub title_slug_max_len: usize,
    pub default_doc_type: String,
}

#[derive(Debug, Deserialize)]
struct PointerFile {
    current: String,
}

pub async fn load_official_rules(
    client: &Client,
    supabase_url: &str,
    supabase_key: &str,
    bucket: &str,
    pointer_object: &str,
) -> Result<NamingRules> {
    let pointer_raw =
        get_storage_object(client, supabase_url, supabase_key, bucket, pointer_object).await?;
    let pointer: PointerFile = serde_json::from_str(&pointer_raw)
        .with_context(|| format!("ponteiro de regras invalido: {}", pointer_object))?;

    if pointer.current.trim().is_empty() {
        bail!("ponteiro de regras sem campo current");
    }

    let rules_raw =
        get_storage_object(client, supabase_url, supabase_key, bucket, &pointer.current).await?;
    let rules: NamingRules = serde_json::from_str(&rules_raw)
        .with_context(|| format!("arquivo de regras invalido: {}", pointer.current))?;
    Ok(rules)
}

async fn get_storage_object(
    client: &Client,
    supabase_url: &str,
    supabase_key: &str,
    bucket: &str,
    object_path: &str,
) -> Result<String> {
    let encoded = encode_path(object_path);
    let url = format!(
        "{}/storage/v1/object/{}/{}",
        supabase_url.trim_end_matches('/'),
        bucket,
        encoded
    );
    let resp = client
        .get(url)
        .header("apikey", supabase_key)
        .header("Authorization", format!("Bearer {supabase_key}"))
        .send()
        .await?;

    let status = resp.status();
    let body = resp.text().await?;
    if !status.is_success() {
        bail!(
            "falha ao ler regras oficiais no storage {} {}: {}",
            bucket,
            object_path,
            body
        );
    }
    Ok(body)
}

pub fn build_canonical_name(
    rules: &NamingRules,
    pipeline: &str,
    normalized_title: &str,
    document_type: Option<&str>,
    sha256: &str,
    extension: &str,
) -> Result<String> {
    let now = Utc::now();
    let yyyy = now.format("%Y").to_string();
    let mm = now.format("%m").to_string();
    let dd = now.format("%d").to_string();

    let ext = extension.trim_start_matches('.').to_ascii_lowercase();
    let ext = if ext.is_empty() {
        "bin".to_string()
    } else {
        ext
    };

    let doc_type = document_type
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .unwrap_or(&rules.canonical_naming.default_doc_type);

    let title_slug = slugify(normalized_title, rules.canonical_naming.title_slug_max_len);
    let sha8 = sha256
        .get(0..8)
        .ok_or_else(|| anyhow!("sha256 invalido para sha8"))?;

    let mut out = rules.canonical_naming.pattern.clone();
    out = out.replace("{yyyy}", &yyyy);
    out = out.replace("{mm}", &mm);
    out = out.replace("{dd}", &dd);
    out = out.replace("{pipeline}", &slugify(pipeline, 24));
    out = out.replace("{doc_type}", &slugify(doc_type, 24));
    out = out.replace("{title_slug}", &title_slug);
    out = out.replace("{sha8}", sha8);
    out = out.replace("{ext}", &ext);
    Ok(out)
}

fn slugify(input: &str, max_len: usize) -> String {
    let mut out = String::with_capacity(input.len());
    let mut prev_dash = false;
    for c in input.chars() {
        let lc = c.to_ascii_lowercase();
        if lc.is_ascii_alphanumeric() {
            out.push(lc);
            prev_dash = false;
        } else if !prev_dash {
            out.push('-');
            prev_dash = true;
        }
        if out.len() >= max_len {
            break;
        }
    }
    let trimmed = out.trim_matches('-');
    if trimmed.is_empty() {
        "untitled".to_string()
    } else {
        trimmed.to_string()
    }
}

fn encode_path(path: &str) -> String {
    path.split('/')
        .map(urlencoding::encode)
        .map(|v| v.into_owned())
        .collect::<Vec<_>>()
        .join("/")
}
