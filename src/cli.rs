use std::path::PathBuf;

use clap::{ArgAction, Args, Parser, Subcommand, ValueEnum};

#[derive(Parser, Debug)]
#[command(
    name = "lab",
    version,
    about = "Pipeline local: normalizacao por LLM no computador e envio para Drive/Supabase"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Backup para Google Drive (com normalizacao local antes do upload)
    Backup(BackupArgs),
    /// Oficializa no Supabase (Storage + opcionalmente tabela)
    Officialize(OfficializeArgs),
}

#[derive(Args, Debug, Clone)]
pub struct CommonPipelineArgs {
    /// Arquivos e/ou diretorios de entrada
    #[arg(required = true)]
    pub inputs: Vec<PathBuf>,

    /// Normalizador para titulo/metadados
    #[arg(long, value_enum, default_value_t = Normalizer::LocalOllama)]
    pub normalizer: Normalizer,

    /// Host local do Ollama (quando normalizer=local-ollama)
    #[arg(long, env = "OLLAMA_HOST", default_value = "http://127.0.0.1:11434")]
    pub ollama_host: String,

    /// Modelo local no Ollama (quando normalizer=local-ollama)
    #[arg(long, env = "OLLAMA_MODEL", default_value = "qwen2.5:7b-instruct")]
    pub ollama_model: String,

    /// Modelo Gemini (quando normalizer=gemini)
    #[arg(long, default_value = "gemini-2.5-flash")]
    pub gemini_model: String,

    /// Chave da API Gemini (ou env GEMINI_API_KEY)
    #[arg(long, env = "GEMINI_API_KEY")]
    pub gemini_api_key: Option<String>,

    /// So mostra o que faria, sem publicar
    #[arg(long, default_value_t = false)]
    pub dry_run: bool,

    /// Quantidade maxima de caracteres usados do conteudo para LLM
    #[arg(long, default_value_t = 3000)]
    pub sample_chars: usize,

    /// Pular duplicados por hash no mesmo lote
    #[arg(long, default_value_t = true, action = ArgAction::Set)]
    pub skip_duplicates: bool,

    /// URL do Supabase para leitura das regras (fonte oficial)
    #[arg(long, env = "SUPABASE_URL")]
    pub rules_supabase_url: Option<String>,

    /// Service role key para leitura das regras
    #[arg(long, env = "SUPABASE_SERVICE_ROLE_KEY")]
    pub rules_supabase_key: Option<String>,

    /// Bucket oficial das regras
    #[arg(long, default_value = "lab-official")]
    pub rules_bucket: String,

    /// Objeto ponteiro para regra atual
    #[arg(long, default_value = "config/rules/current.json")]
    pub rules_pointer_object: String,
}

#[derive(Args, Debug, Clone)]
pub struct BackupArgs {
    #[command(flatten)]
    pub common: CommonPipelineArgs,

    /// ID da pasta no Google Drive (opcional)
    #[arg(long, env = "DRIVE_FOLDER_ID")]
    pub drive_folder_id: Option<String>,

    /// Compartilhar arquivo no Drive com este e-mail
    #[arg(long, default_value = "dan@danvoulez.com")]
    pub share_with: String,

    /// Token OAuth do Google (ou env GOOGLE_OAUTH_ACCESS_TOKEN)
    #[arg(long, env = "GOOGLE_OAUTH_ACCESS_TOKEN")]
    pub google_access_token: Option<String>,

    /// ID da planilha de indice de backup
    #[arg(long, env = "BACKUP_SHEET_ID")]
    pub backup_sheet_id: Option<String>,

    /// Aba da planilha de backup
    #[arg(long, default_value = "Backup", env = "BACKUP_SHEET_TAB")]
    pub backup_sheet_tab: String,
}

#[derive(Args, Debug, Clone)]
pub struct OfficializeArgs {
    #[command(flatten)]
    pub common: CommonPipelineArgs,

    /// URL do Supabase (ou env SUPABASE_URL)
    #[arg(long, env = "SUPABASE_URL")]
    pub supabase_url: Option<String>,

    /// Service role key do Supabase (ou env SUPABASE_SERVICE_ROLE_KEY)
    #[arg(long, env = "SUPABASE_SERVICE_ROLE_KEY")]
    pub supabase_key: Option<String>,

    /// Bucket no Supabase Storage
    #[arg(long, default_value = "lab-official")]
    pub supabase_bucket: String,

    /// Prefixo de path no bucket do Supabase
    #[arg(long, default_value = "ingest")]
    pub supabase_path_prefix: String,

    /// Tabela PostgREST opcional para registrar metadados (ex: official_files)
    #[arg(long, env = "SUPABASE_TABLE", default_value = "LAB-OFFICIAL-INDEX")]
    pub supabase_table: String,

    /// Tambem grava sidecar JSON de metadados no Storage
    #[arg(long, default_value_t = true, action = ArgAction::Set)]
    pub supabase_write_sidecar: bool,
}

#[derive(Clone, Debug, ValueEnum)]
pub enum Normalizer {
    LocalOllama,
    Gemini,
    None,
}
