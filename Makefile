SHELL := /bin/bash

PROJECT ?= labs-ecosystem
CONFIG ?= dev_personal
FILES ?=
FOLDER_ID ?=
SHARE_WITH ?= dan@danvoulez.com
LLM ?= local-ollama
OLLAMA_MODEL ?= qwen2.5:7b-instruct
SUPABASE_BUCKET ?= lab-official
SUPABASE_PREFIX ?= ingest

DRIVE_ARG := $(if $(strip $(FOLDER_ID)),--drive-folder-id "$(FOLDER_ID)",)
SUPABASE_TABLE_ARG := $(if $(strip $(SUPABASE_TABLE)),--supabase-table "$(SUPABASE_TABLE)",)

.PHONY: help backup backup-dry-run officialize officialize-dry-run pwa check-files

help:
	@echo "Targets:"
	@echo "  make backup FILES='\"/abs/path/file.pdf\"' [FOLDER_ID='id']"
	@echo "  make backup FILES='\"/abs/path/project-folder\"' [FOLDER_ID='id']"
	@echo "  make backup-dry-run FILES='\"/abs/path/file-ou-pasta\"'"
	@echo "  make officialize FILES='\"/abs/path/file-ou-pasta\"' [SUPABASE_TABLE='official_files']"
	@echo "  make officialize-dry-run FILES='\"/abs/path/file-ou-pasta\"'"
	@echo "  make pwa"
	@echo ""
	@echo "Variaveis:"
	@echo "  PROJECT=$(PROJECT)"
	@echo "  CONFIG=$(CONFIG)"
	@echo "  SHARE_WITH=$(SHARE_WITH)"
	@echo "  LLM=$(LLM)"
	@echo "  OLLAMA_MODEL=$(OLLAMA_MODEL)"
	@echo ""
	@echo "Backup de pasta preserva arvore de diretorios no Drive."
	@echo "Officialize de pasta preserva arvore de diretorios no Supabase Storage."

check-files:
	@if [[ -z "$(strip $(FILES))" ]]; then \
		echo "Erro: defina FILES com pelo menos um arquivo/pasta."; \
		echo "Exemplo: make backup FILES='\"/tmp/a.txt\" \"/tmp/b.pdf\"' FOLDER_ID='abc123'"; \
		exit 1; \
	fi

backup: check-files
	@doppler run --project "$(PROJECT)" --config "$(CONFIG)" -- \
		cargo run -- backup $(FILES) $(DRIVE_ARG) \
		--share-with "$(SHARE_WITH)" \
		--normalizer "$(LLM)" \
		--ollama-model "$(OLLAMA_MODEL)"

backup-dry-run: check-files
	@doppler run --project "$(PROJECT)" --config "$(CONFIG)" -- \
		cargo run -- backup $(FILES) $(DRIVE_ARG) \
		--share-with "$(SHARE_WITH)" \
		--normalizer "$(LLM)" \
		--ollama-model "$(OLLAMA_MODEL)" \
		--dry-run

officialize: check-files
	@doppler run --project "$(PROJECT)" --config "$(CONFIG)" -- \
		cargo run -- officialize $(FILES) \
		--normalizer "$(LLM)" \
		--ollama-model "$(OLLAMA_MODEL)" \
		--supabase-bucket "$(SUPABASE_BUCKET)" \
		--supabase-path-prefix "$(SUPABASE_PREFIX)" \
		$(SUPABASE_TABLE_ARG)

officialize-dry-run: check-files
	@doppler run --project "$(PROJECT)" --config "$(CONFIG)" -- \
		cargo run -- officialize $(FILES) \
		--normalizer "$(LLM)" \
		--ollama-model "$(OLLAMA_MODEL)" \
		--supabase-bucket "$(SUPABASE_BUCKET)" \
		--supabase-path-prefix "$(SUPABASE_PREFIX)" \
		$(SUPABASE_TABLE_ARG) \
		--dry-run

pwa:
	@doppler run --project "$(PROJECT)" --config "$(CONFIG)" -- \
		sh -lc 'cd pwa && npm start'
