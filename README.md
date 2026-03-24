# lab-cli

CLI e PWA local para dois fluxos:

- `lab backup`: normaliza metadados/título por LLM, preserva estrutura de pasta no Google Drive e indexa no Google Sheets.
- `lab officialize`: normaliza metadados/título por LLM, preserva estrutura de pasta no Supabase Storage e indexa no Postgres.

## Instalação (outro computador)

Pré-requisitos:

- Rust (toolchain estável)
- `cargo`
- `gcloud` (para token OAuth do Google no backup)
- `doppler` (opcional, recomendado para variáveis de ambiente)
- `ollama` (opcional, se usar normalizador local)

Instalar direto do GitHub:

```bash
cargo install --git https://github.com/danvoulez/lab-cli.git lab
```

Depois disso, o binário `lab` estará disponível no PATH do Cargo.

## Uso rápido

Backup (arquivo ou pasta):

```bash
lab backup "/abs/path/arquivo-ou-pasta"
```

Officialize (arquivo ou pasta):

```bash
lab officialize "/abs/path/arquivo-ou-pasta"
```

Com `Makefile` + Doppler:

```bash
make backup FILES='"/abs/path/pasta"'
make officialize FILES='"/abs/path/pasta"'
make pwa
```

## Contrato de comportamento

- Entrada arquivo único: fluxo normal.
- Entrada pasta: preserva árvore no destino (`pasta-raiz/subpastas/arquivo`).
- `canonical_name`: usado para índice/metadados (não para destruir hierarquia).
- Dedupe em modo pasta: por identidade de caminho (`root_rel_path + hash`), não por hash global.
- Paridade: mesma regra em `backup` e `officialize`.

## Variáveis de ambiente principais

- `SUPABASE_URL`
- `SUPABASE_SERVICE_ROLE_KEY`
- `SUPABASE_TABLE` (default `LAB-OFFICIAL-INDEX`)
- `SUPABASE_BUCKET` (default `lab-official`)
- `SUPABASE_PATH_PREFIX` (default `ingest`)
- `DRIVE_FOLDER_ID`
- `BACKUP_SHEET_ID`
- `BACKUP_SHEET_TAB` (default `Backup`)
- `GEMINI_API_KEY`
- `OLLAMA_HOST`
- `OLLAMA_MODEL`

## PWA

Ver [pwa/README.md](./pwa/README.md).
