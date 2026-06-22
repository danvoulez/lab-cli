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

## Environment

```bash
export LAB_MONGO_URI='<mongo-uri>'
export LAB_MONGO_DB='santo_andre_lab'
```

Do not commit credentials. Rotate any credential that has appeared in chat or
logs.
