# Projection and Law Navigation

Projection tooling for Santo Andre Laboratory lives in:

`Santo-Andre-Laboratory/PROJECTIONS`

The CLI owns only thin navigation and invocation surfaces. It must not copy the
projection/reconciliation logic back into the runtime.

## Authority Model

- `public.logline_acts` remains the current custody ledger.
- `public.lab_log` is legacy historical material.
- MongoDB stores non-authoritative, rebuildable maps.
- Reports are derived maps, not law.
- Sealed zip packages are analyzed as doctrine/procedure sources, not automatic
  admitted rules.

## CLI Wrappers

Set the projections repo path if it is not at `~/PROJECTIONS`:

```bash
export LAB_PROJECTIONS_REPO=/Users/ubl-ops/PROJECTIONS
```

Projection rebuild wrappers:

```bash
lab project all
lab project law
lab project reconcile --zip /Users/ubl-ops/Meu-Lab-clean.zip
```

Read-only law navigation commands:

```bash
lab law current
lab law gaps
lab law graph
lab law check activate.route_to_devin.v1
```

`lab law ...` reads projection documents from Mongo and always reports that the
output is non-authoritative.

Candidate legislative writes use a constrained command, not generic `lab act`:

```bash
lab law propose rule:constitution/current-custody-surface \
  --title 'Current custody surface' \
  --text 'public.logline_acts is the current custody ledger; public.lab_log is legacy historical material.' \
  --superior act:<constitutional-root-hash> \
  --source post-reconciliation-2026-06-22 \
  --reason 'Needed to close the lab_log/logline_acts split before further lawmaking.'
```

`lab law propose` always writes `status=candidate` and does not admit active
law. It requires a superior content hash and refuses to write if the superior is
not present in `public.logline_acts`.

## Environment

```bash
export LAB_MONGO_URI='<mongo-uri>'
export LAB_MONGO_DB='santo_andre_lab'
```

Do not commit credentials. Rotate any credential that has appeared in chat or
logs.
