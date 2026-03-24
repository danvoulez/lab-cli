use std::path::Path;

use anyhow::{Context, Result, anyhow, bail};
use reqwest::Client;
use serde::Deserialize;
use serde_json::json;

use crate::{cli::Normalizer, models::NormalizedMetadata};

pub struct NormalizerConfig<'a> {
    pub normalizer: &'a Normalizer,
    pub ollama_host: &'a str,
    pub ollama_model: &'a str,
    pub gemini_api_key: Option<&'a str>,
    pub gemini_model: &'a str,
    pub rules_json: &'a str,
}

pub async fn normalize_metadata(
    client: &Client,
    cfg: NormalizerConfig<'_>,
    file_name: &str,
    text_sample: Option<&str>,
) -> Result<NormalizedMetadata> {
    match cfg.normalizer {
        Normalizer::LocalOllama => {
            normalize_with_ollama(
                client,
                cfg.ollama_host,
                cfg.ollama_model,
                file_name,
                text_sample,
                cfg.rules_json,
            )
            .await
        }
        Normalizer::Gemini => {
            normalize_with_gemini(
                client,
                cfg.gemini_api_key
                    .ok_or_else(|| anyhow!("faltou GEMINI_API_KEY para normalizer=gemini"))?,
                cfg.gemini_model,
                file_name,
                text_sample,
                cfg.rules_json,
            )
            .await
        }
        Normalizer::None => Ok(fallback_metadata(file_name)),
    }
}

fn fallback_metadata(original_name: &str) -> NormalizedMetadata {
    let stem = Path::new(original_name)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or(original_name)
        .replace(['_', '-'], " ");

    let title = stem
        .split_whitespace()
        .map(capitalize_word)
        .collect::<Vec<_>>()
        .join(" ");

    NormalizedMetadata {
        title,
        description: Some("metadata fallback sem llm".to_string()),
        tags: Some(vec!["fallback".to_string()]),
        document_type: None,
        language: None,
    }
}

fn capitalize_word(word: &str) -> String {
    let mut chars = word.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}

fn build_prompt(file_name: &str, text_sample: Option<&str>, rules_json: &str) -> String {
    format!(
        "Voce e um normalizador de metadados para documentos.\n\
Retorne SOMENTE JSON valido com schema:\n\
{{\"title\":\"string\",\"description\":\"string\",\"tags\":[\"string\"],\"document_type\":\"string\",\"language\":\"string\"}}\n\
Regras oficiais de nomeacao (obrigatorio respeitar para sugerir title/document_type):\n\
{rules_json}\n\
\n\
Regras:\n\
- title curto (maximo 80 chars)\n\
- description em portugues\n\
- tags com 3 a 8 itens\n\
- language em BCP-47 quando possivel (pt-BR, en)\n\
- se faltar contexto, inferir sem inventar fatos sensiveis\n\
\n\
Entrada:\n\
file_name: {file_name}\n\
text_sample: {}\n",
        text_sample.unwrap_or("<sem amostra de texto>")
    )
}

#[derive(Deserialize)]
struct OllamaGenerateResponse {
    response: String,
}

async fn normalize_with_ollama(
    client: &Client,
    ollama_host: &str,
    model: &str,
    file_name: &str,
    text_sample: Option<&str>,
    rules_json: &str,
) -> Result<NormalizedMetadata> {
    let endpoint = format!(
        "{}/api/generate",
        normalize_ollama_host(ollama_host).trim_end_matches('/')
    );
    let body = json!({
        "model": model,
        "prompt": build_prompt(file_name, text_sample, rules_json),
        "stream": false,
        "format": "json",
        "options": {
            "temperature": 0.1
        }
    });

    let resp = client.post(endpoint).json(&body).send().await?;
    let status = resp.status();
    let raw = resp.text().await?;
    if !status.is_success() {
        bail!("ollama erro {}: {}", status, raw);
    }

    let parsed: OllamaGenerateResponse =
        serde_json::from_str(&raw).with_context(|| format!("resposta ollama invalida: {raw}"))?;

    parse_metadata_json(file_name, &parsed.response)
}

fn normalize_ollama_host(input: &str) -> String {
    let mut trimmed = input.trim().to_string();
    if trimmed == "0.0.0.0" {
        trimmed = "127.0.0.1:11434".to_string();
    }
    if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
        return trimmed;
    }
    format!("http://{trimmed}")
}

async fn normalize_with_gemini(
    client: &Client,
    api_key: &str,
    model: &str,
    file_name: &str,
    text_sample: Option<&str>,
    rules_json: &str,
) -> Result<NormalizedMetadata> {
    let url = format!(
        "https://generativelanguage.googleapis.com/v1beta/models/{model}:generateContent?key={api_key}"
    );
    let body = json!({
        "contents": [{ "role": "user", "parts": [{ "text": build_prompt(file_name, text_sample, rules_json) }] }],
        "generationConfig": {
            "temperature": 0.1,
            "responseMimeType": "application/json"
        }
    });

    let resp = client.post(url).json(&body).send().await?;
    let status = resp.status();
    let raw = resp.text().await?;
    if !status.is_success() {
        bail!("gemini erro {}: {}", status, raw);
    }

    let value: serde_json::Value =
        serde_json::from_str(&raw).with_context(|| format!("resposta gemini invalida: {raw}"))?;
    let text = value["candidates"][0]["content"]["parts"][0]["text"]
        .as_str()
        .ok_or_else(|| anyhow!("nao consegui extrair texto da resposta do gemini"))?;

    parse_metadata_json(file_name, text)
}

fn parse_metadata_json(file_name: &str, content: &str) -> Result<NormalizedMetadata> {
    let stripped = strip_markdown_json_fence(content);
    let json_text = stripped.trim();
    let mut parsed: NormalizedMetadata = serde_json::from_str(json_text)
        .with_context(|| format!("json invalido de metadados: {json_text}"))?;

    if parsed.title.trim().is_empty() {
        parsed.title = fallback_metadata(file_name).title;
    }

    Ok(parsed)
}

fn strip_markdown_json_fence(text: &str) -> String {
    let trimmed = text.trim();
    if let Some(stripped) = trimmed.strip_prefix("```json") {
        return stripped.trim().trim_end_matches("```").trim().to_string();
    }
    if let Some(stripped) = trimmed.strip_prefix("```") {
        return stripped.trim().trim_end_matches("```").trim().to_string();
    }
    trimmed.to_string()
}
