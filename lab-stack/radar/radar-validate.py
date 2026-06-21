#!/usr/bin/env python3
"""Validate a Radar v2-lite forensic export."""

from __future__ import annotations

import hashlib
import json
import pathlib
import sys
from typing import Any


SCHEMA = "radar.raw_export.v2-lite"
FORBIDDEN_FIELDS = {"action", "recommendation", "severity", "risk", "primary_action", "radar_state"}


def canonical_bytes(value: Any) -> bytes:
    return json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":")).encode("utf-8")


def hash_value(value: Any) -> str:
    return hashlib.sha256(canonical_bytes(value)).hexdigest()


def load(path: pathlib.Path) -> tuple[Any | None, str | None]:
    try:
        return json.loads(path.read_text(encoding="utf-8")), None
    except Exception as exc:
        return None, str(exc)


def walk_forbidden(value: Any, path: str, errors: list[dict[str, Any]]) -> None:
    if isinstance(value, dict):
        for key, child in value.items():
            child_path = f"{path}.{key}"
            if key in FORBIDDEN_FIELDS:
                errors.append({"kind": "forbidden_field", "path": child_path, "message": f"forbidden field {key}"})
            walk_forbidden(child, child_path, errors)
    elif isinstance(value, list):
        for i, child in enumerate(value):
            walk_forbidden(child, f"{path}[{i}]", errors)


def validate_item(section_name: str, item: dict[str, Any], seen: set[str], errors: list[dict[str, Any]]) -> None:
    required = [
        "row_key",
        "section",
        "item_index",
        "item_type",
        "observed_at",
        "observed_by",
        "source_file",
        "source_command",
        "id",
        "label",
        "state",
        "detail",
        "facts",
        "raw",
        "content_sha256",
        "observation_sha256",
        "redactions",
        "errors",
    ]
    for key in required:
        if key not in item:
            errors.append({"kind": "missing_item_field", "section": section_name, "field": key})
    row_key = item.get("row_key")
    if row_key in seen:
        errors.append({"kind": "duplicate_row_key", "section": section_name, "row_key": row_key})
    if isinstance(row_key, str):
        seen.add(row_key)
    if item.get("section") != section_name:
        errors.append({"kind": "item_section_mismatch", "section": section_name, "row_key": row_key})

    content_payload = {
        "section": item.get("section"),
        "item_type": item.get("item_type"),
        "id": item.get("id"),
        "label": item.get("label"),
        "detail": item.get("detail"),
        "facts": item.get("facts"),
        "raw": item.get("raw"),
        "redactions": item.get("redactions"),
        "errors": item.get("errors"),
    }
    if item.get("content_sha256") != hash_value(content_payload):
        errors.append({"kind": "content_hash_mismatch", "section": section_name, "row_key": row_key})
    observation_payload = dict(item)
    observation_payload["observation_sha256"] = None
    if item.get("observation_sha256") != hash_value(observation_payload):
        errors.append({"kind": "observation_hash_mismatch", "section": section_name, "row_key": row_key})

    for idx, redaction in enumerate(item.get("redactions") or []):
        if not isinstance(redaction, dict):
            errors.append({"kind": "bad_redaction", "section": section_name, "row_key": row_key, "index": idx})
            continue
        if redaction.get("value_exported") is not False:
            errors.append({"kind": "bad_redaction", "section": section_name, "row_key": row_key, "index": idx, "message": "value_exported must be false"})
        for key in ("field", "reason", "method", "match_rule"):
            if key not in redaction:
                errors.append({"kind": "bad_redaction", "section": section_name, "row_key": row_key, "index": idx, "message": f"missing {key}"})


def validate(path: pathlib.Path, sidecar: pathlib.Path | None = None) -> dict[str, Any]:
    errors: list[dict[str, Any]] = []
    warnings: list[dict[str, Any]] = []
    data, parse_error = load(path)
    if parse_error:
        return {
            "valid": False,
            "schema": None,
            "sections_checked": 0,
            "items_checked": 0,
            "export_sha256_verified": False,
            "errors": [{"kind": "parse_error", "message": parse_error}],
            "warnings": [],
        }
    if not isinstance(data, dict):
        errors.append({"kind": "bad_top_level", "message": "export must be a JSON object"})
        data = {}
    if data.get("schema") != SCHEMA:
        errors.append({"kind": "bad_schema", "expected": SCHEMA, "actual": data.get("schema")})

    matrix = data.get("collection_matrix")
    sections = data.get("sections")
    if not isinstance(matrix, dict):
        errors.append({"kind": "bad_collection_matrix"})
        matrix = {}
    if not isinstance(sections, dict):
        errors.append({"kind": "bad_sections"})
        sections = {}

    seen: set[str] = set()
    items_checked = 0
    for name, matrix_row in matrix.items():
        if name not in sections:
            errors.append({"kind": "missing_section", "section": name})
            continue
        section = sections[name]
        for key in ("subject", "collector", "coverage", "granularity", "items", "errors", "redactions", "raw"):
            if key not in section:
                errors.append({"kind": "missing_section_field", "section": name, "field": key})
        items = section.get("items") if isinstance(section.get("items"), list) else []
        coverage = section.get("coverage") if isinstance(section.get("coverage"), dict) else {}
        if coverage.get("emitted_count") != len(items):
            errors.append({"kind": "section_count_mismatch", "section": name, "emitted_count": coverage.get("emitted_count"), "actual": len(items)})
        if isinstance(matrix_row, dict) and matrix_row.get("emitted_count") != len(items):
            errors.append({"kind": "matrix_count_mismatch", "section": name, "emitted_count": matrix_row.get("emitted_count"), "actual": len(items)})
        cap = coverage.get("cap")
        observed = coverage.get("observed_count")
        if isinstance(cap, int) and isinstance(observed, int) and observed > cap:
            if coverage.get("cap_hit") is not True or coverage.get("complete") is not False:
                errors.append({"kind": "bad_cap_metadata", "section": name})
        for item in items:
            if isinstance(item, dict):
                validate_item(name, item, seen, errors)
                items_checked += 1
            else:
                errors.append({"kind": "bad_item", "section": name})

    for name in sections:
        if name not in matrix:
            errors.append({"kind": "section_not_in_matrix", "section": name})

    totals = data.get("totals") if isinstance(data.get("totals"), dict) else {}
    if totals.get("items_total") != items_checked:
        errors.append({"kind": "total_items_mismatch", "declared": totals.get("items_total"), "actual": items_checked})

    hashes = data.get("hashes") if isinstance(data.get("hashes"), dict) else {}
    section_hashes = hashes.get("sections_sha256") if isinstance(hashes.get("sections_sha256"), dict) else {}
    for name, section in sections.items():
        if section_hashes.get(name) != hash_value(section):
            errors.append({"kind": "section_hash_mismatch", "section": name})
    source_files = data.get("source_files") if isinstance(data.get("source_files"), list) else []
    if hashes.get("source_files_sha256") != hash_value(source_files):
        errors.append({"kind": "source_files_hash_mismatch"})

    manifest_sections = {m.get("section") for m in source_files if isinstance(m, dict)}
    for name in matrix:
        if name not in manifest_sections:
            errors.append({"kind": "manifest_missing_section", "section": name})

    walk_forbidden(data, "$", errors)

    export_sha256_verified = False
    if sidecar:
        expected = sidecar.read_text(encoding="utf-8").split()[0]
        actual = hashlib.sha256(path.read_bytes()).hexdigest()
        export_sha256_verified = expected == actual
        if not export_sha256_verified:
            errors.append({"kind": "sidecar_hash_mismatch", "expected": expected, "actual": actual})
    else:
        warnings.append({"kind": "sidecar_not_checked"})

    return {
        "valid": not errors,
        "schema": data.get("schema"),
        "sections_checked": len(sections),
        "items_checked": items_checked,
        "export_sha256_verified": export_sha256_verified,
        "errors": errors,
        "warnings": warnings,
    }


def main(argv: list[str]) -> int:
    if not argv:
        print("usage: radar-validate.py <export.json> [sidecar.sha256]", file=sys.stderr)
        return 2
    path = pathlib.Path(argv[0])
    sidecar = pathlib.Path(argv[1]) if len(argv) > 1 else None
    result = validate(path, sidecar)
    print(json.dumps(result, ensure_ascii=False, indent=2, sort_keys=True))
    return 0 if result["valid"] else 1


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
