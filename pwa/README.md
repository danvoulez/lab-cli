# LAB PWA (macOS)

Interface PWA com duas telas:

- `backup`: chama `lab backup` e move arquivo/pasta para `/Users/ubl-ops/BACKUP-FEITO`
- `officialize`: chama `lab officialize` e move arquivo/pasta para `/Users/ubl-ops/officialize`

## Rodar

```bash
cd "/Users/ubl-ops/lab cli"
make pwa
```

Abra:

```text
http://127.0.0.1:4319
```

## Variaveis uteis

- `DRIVE_FOLDER_ID`
- `SUPABASE_URL`
- `SUPABASE_SERVICE_ROLE_KEY`
- `SUPABASE_BUCKET`
- `SUPABASE_TABLE`

## Observacao sobre drag-and-drop

Para mover o item real de origem, o frontend precisa capturar caminho absoluto no drop (normalmente via `text/uri-list` em Chromium/macOS). Se o navegador nao fornecer caminho absoluto, o app vai avisar no log.

## Comportamento de pasta no backup

Se voce arrastar uma pasta inteira no `backup`, a CLI preserva a estrutura no Drive:

- cria a pasta raiz (nome da pasta local) no destino do Drive;
- cria subpastas recursivamente;
- envia os arquivos dentro da arvore, mantendo a hierarquia;
- preserva o nome original de cada arquivo no Drive;
- ainda calcula o `canonical_name` para indexacao/metadados.

## Comportamento de pasta no officialize

Se voce arrastar uma pasta inteira no `officialize`, a CLI preserva a estrutura no Supabase Storage:

- usa path do tipo `prefixo/pasta-raiz/subpastas/arquivo-original`;
- mantem `canonical_name` no indice/tabela para rastreabilidade;
- grava sidecar `.metadata.json` no mesmo path do arquivo.
