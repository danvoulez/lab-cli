#!/usr/bin/env python3
"""Export Radar's local scan slices as forensic raw evidence.

This is the equilibrium exporter: the shell scanner stays boring and phased,
while this exporter normalizes its slices into a stricter custody format.
It does not judge, recommend, rank, or mutate anything.
"""

from __future__ import annotations

import datetime as _dt
import getpass
import hashlib
import json
import os
import pathlib
import platform
import re
import shutil
import socket
import subprocess
import sys
import tempfile
import time
from typing import Any


SCHEMA = "radar.raw_export.v2-lite"
SCANNER_VERSION = "2-lite"
DEFAULT_SECTIONS = [
    "system",
    "storage",
    "files",
    "services",
    "processes",
    "schedules",
    "network",
    "apps",
    "packages",
    "repos",
    "secrets",
    "config",
    "cloud",
    "accounts",
    "runtimes",
    "junk",
    "unknown",
]
FORBIDDEN_FIELDS = {"action", "recommendation", "severity", "risk", "primary_action", "radar_state"}

RADAR_DIR = pathlib.Path(os.environ.get("RADAR_DIR", pathlib.Path.home() / ".radar"))
EXPORT_DIR = RADAR_DIR / "exports"


SECRET_RULES: list[tuple[str, re.Pattern[str]]] = [
    ("openai_key", re.compile(r"\bsk-[A-Za-z0-9_\-]{16,}\b")),
    ("bearer_token", re.compile(r"\bBearer\s+[A-Za-z0-9._~+/=\-]{12,}", re.IGNORECASE)),
    (
        "flag_value",
        re.compile(
            r"(?i)(--?(?:api[-_]?key|token|password|passwd|secret|client[-_]?secret|access[-_]?token)"
            r"(?:=|\s+))([^ \t\r\n\"']+)"
        ),
    ),
    (
        "key_value",
        re.compile(
            r"(?i)\b((?:api[-_]?key|token|password|passwd|secret|client[-_]?secret|access[-_]?token)"
            r"\s*[:=]\s*)([^ \t\r\n\"',}]+)"
        ),
    ),
    ("url_credentials", re.compile(r"([a-z][a-z0-9+.-]*://)([^:/@\s]+):([^/@\s]+)@", re.IGNORECASE)),
]


def utc_now() -> str:
    return _dt.datetime.now(_dt.timezone.utc).replace(microsecond=0).isoformat().replace("+00:00", "Z")


def file_stamp() -> str:
    return _dt.datetime.now(_dt.timezone.utc).strftime("%Y%m%dT%H%M%SZ")


def iso_mtime(path: pathlib.Path) -> str | None:
    try:
        return _dt.datetime.fromtimestamp(path.stat().st_mtime, _dt.timezone.utc).replace(microsecond=0).isoformat().replace("+00:00", "Z")
    except OSError:
        return None


def host() -> str:
    try:
        out = subprocess.check_output(["scutil", "--get", "LocalHostName"], text=True, stderr=subprocess.DEVNULL).strip()
        if out:
            return out
    except Exception:
        pass
    return socket.gethostname().split(".")[0]


def sha256_bytes(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def canonical_bytes(value: Any) -> bytes:
    return json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":")).encode("utf-8")


def hash_value(value: Any) -> str:
    return sha256_bytes(canonical_bytes(value))


def atomic_write(path: pathlib.Path, text: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    fd, tmp_name = tempfile.mkstemp(prefix=f".{path.name}.", dir=str(path.parent))
    with os.fdopen(fd, "w", encoding="utf-8") as tmp:
        tmp.write(text)
    os.replace(tmp_name, path)


def read_json(path: pathlib.Path) -> tuple[Any | None, str | None]:
    try:
        return json.loads(path.read_text(encoding="utf-8")), None
    except Exception as exc:
        return None, str(exc)


def source_file_manifest(section: str) -> dict[str, Any]:
    path = RADAR_DIR / f"{section}.json"
    exists = path.exists()
    data, parse_error = read_json(path) if exists else (None, "missing_file")
    items = data.get("items") if isinstance(data, dict) else None
    raw = path.read_bytes() if exists else b""
    return {
        "section": section,
        "path": str(path),
        "basename": path.name,
        "loaded": exists and parse_error is None,
        "exists": exists,
        "size_bytes": len(raw) if exists else 0,
        "sha256": sha256_bytes(raw) if exists else None,
        "modified_at": iso_mtime(path) if exists else None,
        "parse_ok": exists and parse_error is None,
        "parse_error": parse_error,
        "items_count": len(items) if isinstance(items, list) else 0,
    }


def intended_sections() -> list[str]:
    status_path = RADAR_DIR / "status.json"
    data, _ = read_json(status_path) if status_path.exists() else (None, None)
    subjects = data.get("subjects") if isinstance(data, dict) else None
    if isinstance(subjects, list) and all(isinstance(x, str) for x in subjects):
        return subjects
    return DEFAULT_SECTIONS


def redact_string(text: str, field: str) -> tuple[str, list[dict[str, Any]]]:
    redactions: list[dict[str, Any]] = []
    out = text
    for rule_name, pattern in SECRET_RULES:
        def replace(match: re.Match[str]) -> str:
            redactions.append({
                "field": field,
                "reason": "secret_value",
                "method": "masked",
                "match_rule": rule_name,
                "value_exported": False,
            })
            if rule_name in {"flag_value", "key_value"} and match.lastindex and match.lastindex >= 2:
                return f"{match.group(1)}<REDACTED:secret_value>"
            if rule_name == "url_credentials" and match.lastindex and match.lastindex >= 3:
                return f"{match.group(1)}<REDACTED:user>:<REDACTED:secret_value>@"
            return "<REDACTED:secret_value>"

        out = pattern.sub(replace, out)
    return out, redactions


def redact_value(value: Any, field: str = "$") -> tuple[Any, list[dict[str, Any]]]:
    if isinstance(value, str):
        return redact_string(value, field)
    if isinstance(value, list):
        items: list[Any] = []
        redactions: list[dict[str, Any]] = []
        for i, item in enumerate(value):
            redacted, records = redact_value(item, f"{field}[{i}]")
            items.append(redacted)
            redactions.extend(records)
        return items, redactions
    if isinstance(value, dict):
        out: dict[str, Any] = {}
        redactions: list[dict[str, Any]] = []
        for key, item in value.items():
            redacted, records = redact_value(item, f"{field}.{key}")
            out[key] = redacted
            redactions.extend(records)
        return out, redactions
    return value, []


def item_type_for(section: str) -> str:
    return {
        "accounts": "account",
        "apps": "application",
        "cloud": "cloud_object",
        "config": "config_object",
        "files": "filesystem_object",
        "junk": "junk_indicator",
        "network": "network_listener",
        "packages": "package",
        "processes": "process",
        "repos": "repository",
        "runtimes": "runtime",
        "schedules": "schedule",
        "secrets": "secret_indicator",
        "services": "service",
        "storage": "storage_object",
        "system": "system_property",
        "unknown": "unknown_object",
    }.get(section, "observation")


def facts_from_legacy(section: str, legacy: dict[str, Any]) -> dict[str, Any]:
    facts: dict[str, Any] = {
        "legacy_id": legacy.get("id"),
        "legacy_label": legacy.get("label"),
        "legacy_detail": legacy.get("detail"),
        "legacy_state": legacy.get("state"),
        "size_bytes": legacy.get("size_bytes"),
    }
    ident = legacy.get("id")
    detail = legacy.get("detail")
    label = legacy.get("label")
    state = legacy.get("state")
    if isinstance(ident, str) and ":" in ident:
        prefix, tail = ident.split(":", 1)
        facts["legacy_id_prefix"] = prefix
        if tail.startswith("/"):
            facts["path"] = tail
    if section in {"files", "storage", "cloud", "config", "junk", "unknown"} and isinstance(detail, str) and detail.startswith("/"):
        facts.setdefault("path", detail)
    if section == "accounts" and isinstance(detail, str):
        m = re.search(r"uid\s+(\d+)\s+·\s+home\s+(.+?)\s+·\s+shell\s+(.+)$", detail)
        if m:
            facts.update({"uid": int(m.group(1)), "home": m.group(2), "shell": m.group(3), "username": label})
    elif section == "processes":
        if isinstance(detail, str):
            m = re.search(r"cpu\s+([0-9.]+)%\s+mem\s+([0-9.]+)%", detail)
            if m:
                facts["cpu_percent"] = float(m.group(1))
                facts["memory_percent"] = float(m.group(2))
        if isinstance(ident, str) and ident.startswith("processes:"):
            command = ident.split(":", 1)[1]
            facts["command"] = command
            facts["executable_path"] = command
    elif section == "services":
        facts["service_label"] = label
        if isinstance(detail, str):
            m = re.search(r"exit\s+([^\s]+)", detail)
            if m:
                facts["exit_code"] = None if m.group(1) == "-" else m.group(1)
    elif section == "network":
        if isinstance(label, str):
            facts["socket"] = label
            m = re.search(r"(.+):(\d+)(?:\s+\(LISTEN\))?", label)
            if m:
                facts["local_address"] = m.group(1)
                facts["local_port"] = int(m.group(2))
        if isinstance(detail, str):
            m = re.search(r"(.+)\s+\(pid\s+(\d+)\)", detail)
            if m:
                facts["process_name"] = m.group(1)
                facts["pid"] = int(m.group(2))
    elif section == "repos" and isinstance(detail, str):
        m = re.search(r"branch\s+(.+?)\s+·\s+(\d+)\s+uncommitted\s+·\s*(.*)$", detail)
        if m:
            facts["branch"] = m.group(1)
            facts["uncommitted_count"] = int(m.group(2))
            facts["remote_url"] = m.group(3) or None
    elif section == "runtimes" and isinstance(detail, str):
        parts = [p.strip() for p in detail.split("·", 1)]
        facts["runtime_kind"] = label
        if parts:
            facts["executable_path"] = parts[0]
        if len(parts) > 1:
            facts["version"] = parts[1]
    elif section == "packages":
        facts["package_name"] = label
        facts["manager"] = "homebrew" if detail == "homebrew leaf" else None
        facts["install_path"] = detail if isinstance(detail, str) and detail.startswith("/") else None
    elif section == "apps":
        facts["app_name"] = label
        facts["install_root"] = detail
    elif section == "system":
        facts["property_name"] = ident.split(":", 1)[1] if isinstance(ident, str) and ":" in ident else label
        facts["observed_value"] = detail
        if ident == "system:disk" and isinstance(detail, str):
            m = re.search(r"([0-9.]+)GB free of ([0-9.]+)GB", detail)
            if m:
                facts["disk_free_gb"] = float(m.group(1))
                facts["disk_total_gb"] = float(m.group(2))
    elif section in {"files", "storage", "cloud", "config", "junk", "unknown"}:
        path = facts.get("path") if isinstance(facts.get("path"), str) else detail
        if isinstance(path, str):
            facts["basename"] = pathlib.Path(path).name
            facts["extension"] = pathlib.Path(path).suffix or None
        if section == "unknown":
            facts["unknown_kind"] = state
            facts["why_unknown"] = "top-level home entry outside Radar known buckets"
    elif section == "secrets":
        facts["indicator_kind"] = "secret_indicator"
        facts["path"] = detail if isinstance(detail, str) and detail.startswith("/") else facts.get("path")
        facts["matched_key_name_or_pattern"] = label
        facts["value_present"] = "unknown"
        facts["value_exported"] = False
    return facts


def omitted_secret_redaction(field: str) -> dict[str, Any]:
    return {
        "field": field,
        "reason": "secret_value_not_collected",
        "method": "omitted",
        "match_rule": "secret_indicator",
        "value_exported": False,
    }


def normalize_item(section: str, index: int, source_file: str, source_command: str | None, scanned_at: str | None, legacy: Any) -> dict[str, Any]:
    legacy_obj = legacy if isinstance(legacy, dict) else {"value": legacy}
    observed_at = scanned_at or utc_now()
    raw_legacy, raw_redactions = redact_value(legacy_obj, "raw.legacy")
    facts, fact_redactions = redact_value(facts_from_legacy(section, legacy_obj), "facts")
    label, label_redactions = redact_value(legacy_obj.get("label"), "label")
    detail, detail_redactions = redact_value(legacy_obj.get("detail"), "detail")
    item_id, id_redactions = redact_value(legacy_obj.get("id"), "id")
    redactions = raw_redactions + fact_redactions + label_redactions + detail_redactions + id_redactions
    if section == "secrets":
        redactions.append(omitted_secret_redaction("facts.value"))

    content_payload = {
        "section": section,
        "item_type": item_type_for(section),
        "id": item_id,
        "label": label,
        "detail": detail,
        "facts": facts,
        "raw": {"legacy": raw_legacy},
        "redactions": redactions,
        "errors": [],
    }
    content_hash = hash_value(content_payload)
    row_key = f"{section}:{index:06d}:{content_hash[:16]}"
    item = {
        "row_key": row_key,
        "section": section,
        "item_index": index,
        "item_type": item_type_for(section),
        "observed_at": observed_at,
        "observed_by": f"radar.{section}",
        "source_file": source_file,
        "source_command": source_command,
        "id": item_id,
        "label": label,
        "state": None,
        "detail": detail,
        "facts": facts,
        "raw": {"legacy": raw_legacy},
        "content_sha256": content_hash,
        "observation_sha256": None,
        "redactions": redactions,
        "errors": [],
    }
    observation_payload = dict(item)
    observation_payload["observation_sha256"] = None
    item["observation_sha256"] = hash_value(observation_payload)
    return item


def method_from_source(source: str | None) -> str:
    if not source:
        return "unknown"
    if "find" in source or "du" in source or "ps" in source or "lsof" in source or "launchctl" in source:
        return "command"
    return "parse"


def scope_kind_for(section: str, source: str | None) -> str:
    return {
        "accounts": "system/accounts",
        "apps": "filesystem/applications",
        "cloud": "filesystem/cloud",
        "config": "filesystem/config",
        "files": "filesystem",
        "junk": "filesystem/junk",
        "network": "command/network",
        "packages": "command/packages",
        "processes": "command/process_table",
        "repos": "repository",
        "runtimes": "command/runtime_resolution",
        "schedules": "command/scheduler",
        "secrets": "filesystem/secret_indicators",
        "services": "command/launchd",
        "storage": "filesystem/storage",
        "system": "command/system",
        "unknown": "filesystem/home_root",
    }.get(section, "unknown" if not source else "command")


def argv_from_source(source: str | None) -> list[str]:
    if not source:
        return []
    # This is provenance for the legacy collector source string, not a shell-safe
    # replay recipe. Keep it exact and unsplit when it contains comma-joined tools.
    return [source]


def build_section(section: str, manifest: dict[str, Any], box: str) -> dict[str, Any]:
    data, parse_error = read_json(pathlib.Path(manifest["path"])) if manifest["exists"] else (None, "missing_file")
    meta = data.get("_meta") if isinstance(data, dict) and isinstance(data.get("_meta"), dict) else {}
    started_at = meta.get("started_at") or (data.get("scanned_at") if isinstance(data, dict) else None)
    ended_at = meta.get("ended_at") or started_at
    coverage_in = data.get("coverage") if isinstance(data, dict) and isinstance(data.get("coverage"), dict) else {}
    legacy_items = data.get("items") if isinstance(data, dict) and isinstance(data.get("items"), list) else []
    source = meta.get("source_command") if isinstance(meta.get("source_command"), str) else coverage_in.get("source") if isinstance(coverage_in.get("source"), str) else None
    argv = meta.get("argv") if isinstance(meta.get("argv"), list) else argv_from_source(source)
    cap = meta.get("cap") if "cap" in meta else None
    cap_hit = bool(meta.get("cap_hit")) if "cap_hit" in meta else False
    duration_ms = meta.get("duration_ms") if isinstance(meta.get("duration_ms"), int) else None
    duration_measured = duration_ms is not None
    observed_count = coverage_in.get("count") if isinstance(coverage_in.get("count"), int) else len(legacy_items)
    emitted_count = len(legacy_items)
    items = [
        normalize_item(section, i, manifest["basename"], source, started_at, legacy)
        for i, legacy in enumerate(legacy_items)
    ]
    errors = []
    if parse_error:
        errors.append({
            "phase": "load",
            "kind": "missing_file" if parse_error == "missing_file" else "parse_error",
            "message": parse_error,
            "path": manifest["path"],
            "command": None,
            "exit_code": None,
            "errno": None,
        })
    redactions = [r for item in items for r in item["redactions"]]
    timing = {
        "duration_ms": duration_ms,
        "measured": duration_measured,
        "reason": None if duration_measured else "legacy_slice_has_no_duration",
    }
    section_obj: dict[str, Any] = {
        "subject": section,
        "box": box,
        "collector": {
            "name": f"radar.{section}",
            "version": SCANNER_VERSION,
            "source": source,
            "method": method_from_source(source),
            "argv": argv,
            "started_at": started_at,
            "ended_at": ended_at,
            "duration_ms": duration_ms,
            "duration_measured": duration_measured,
            "duration_unavailable_reason": None if duration_measured else "legacy_slice_has_no_duration",
            "exit_code": 0 if manifest["loaded"] else None,
            "stdout_bytes": manifest["size_bytes"] if manifest["exists"] else 0,
            "stderr_bytes": 0,
            "stderr_sample": "",
            "truncated": False,
            "timeout": False,
            "permission_denied": False,
        },
        "coverage": {
            "scope": source,
            "scope_kind": scope_kind_for(section, source),
            "observed_count": observed_count,
            "emitted_count": emitted_count,
            "cap": cap,
            "cap_hit": cap_hit,
            "complete": manifest["loaded"] and parse_error is None,
            "recursive": False,
            "depth": None,
            "selected_roots": [],
            "excluded_roots": [],
            "filters": [],
            "sort": None,
        },
        "timing": timing,
        "granularity": {
            "level": "structured",
            "includes_contents": False,
            "includes_content_hashes": False,
            "includes_metadata": True,
            "includes_permissions": False,
            "includes_ownership": False,
            "includes_timestamps": False,
            "includes_extended_attributes": False,
        },
        "items": items,
        "errors": errors,
        "redactions": redactions,
        "raw": {
            "format": "json",
            "sha256": manifest["sha256"],
            "sample": None,
            "stored_inline": False,
            "external_ref": manifest["path"] if manifest["exists"] else None,
        },
    }
    section_hash_payload = dict(section_obj)
    section_hash_payload["raw"] = dict(section_obj["raw"])
    section_hash_payload["raw"]["sha256"] = None
    section_obj["raw"]["sha256"] = hash_value(section_hash_payload)
    return section_obj


def build_matrix(sections: dict[str, Any], manifests: list[dict[str, Any]]) -> dict[str, Any]:
    manifest_by_section = {m["section"]: m for m in manifests}
    matrix: dict[str, Any] = {}
    for name, section in sections.items():
        manifest = manifest_by_section[name]
        errors_count = len(section["errors"])
        redactions_count = len(section["redactions"])
        matrix[name] = {
            "intended": True,
            "loaded": manifest["loaded"],
            "complete": section["coverage"]["complete"],
            "supported_on_platform": True,
            "skipped_reason": None,
            "observed_count": section["coverage"]["observed_count"],
            "emitted_count": section["coverage"]["emitted_count"],
            "source_file": manifest["basename"],
            "collector": section["collector"]["name"],
            "started_at": section["collector"]["started_at"],
            "ended_at": section["collector"]["ended_at"],
            "duration_ms": section["collector"]["duration_ms"],
            "cap_hit": section["coverage"]["cap_hit"],
            "cap": section["coverage"]["cap"],
            "errors_count": errors_count,
            "redactions_count": redactions_count,
        }
    return matrix


def markdown_for(export: dict[str, Any]) -> str:
    lines = [
        "# Radar Raw Export",
        "",
        f"Schema: `{export['schema']}`  ",
        f"Box: `{export['box']}`  ",
        f"Generated: `{export['generated_at']}`  ",
        f"Run: `{export['run']['run_id']}`  ",
        "",
        "This is a forensic export. It is not a judgment, recommendation, dashboard, or remediation plan.",
        "",
        "## Collection Matrix",
        "",
        "| Section | Loaded | Complete | Observed | Emitted | Errors | Redactions |",
        "|---|---:|---:|---:|---:|---:|---:|",
    ]
    for name, row in export["collection_matrix"].items():
        lines.append(
            f"| `{name}` | {row['loaded']} | {row['complete']} | {row['observed_count']} | "
            f"{row['emitted_count']} | {row['errors_count']} | {row['redactions_count']} |"
        )
    lines.extend(["", "## Sections", ""])
    for name, section in export["sections"].items():
        lines.extend([
            f"### {name}",
            "",
            f"Source: `{section['collector']['source']}`  ",
            f"Items: `{section['coverage']['emitted_count']}`  ",
            f"Complete: `{section['coverage']['complete']}`  ",
            "",
            "| # | row_key | id | label | detail |",
            "|---:|---|---|---|---|",
        ])
        for item in section["items"]:
            detail = "" if item.get("detail") is None else str(item.get("detail")).replace("\n", " ").replace("|", "\\|")
            label = "" if item.get("label") is None else str(item.get("label")).replace("|", "\\|")
            item_id = "" if item.get("id") is None else str(item.get("id")).replace("|", "\\|")
            lines.append(f"| {item['item_index']} | `{item['row_key']}` | `{item_id}` | {label} | {detail} |")
        lines.append("")
    lines.append(f"JSON twin: `{export['paths']['json']}`")
    lines.append(f"SHA-256 sidecar: `{export['paths']['sha256_sidecar']}`")
    return "\n".join(lines) + "\n"


def build_export() -> tuple[dict[str, Any], str]:
    box = host()
    generated_at = utc_now()
    run_id = f"{box}-{file_stamp()}-{os.getpid()}"
    sections_list = intended_sections()
    manifests = [source_file_manifest(section) for section in sections_list]
    sections = {m["section"]: build_section(m["section"], m, box) for m in manifests}
    collection_matrix = build_matrix(sections, manifests)
    sections_sha256 = {name: hash_value(section) for name, section in sections.items()}
    item_total = sum(section["coverage"]["emitted_count"] for section in sections.values())
    errors_total = sum(len(section["errors"]) for section in sections.values())
    redactions_total = sum(len(section["redactions"]) for section in sections.values())
    stamp = file_stamp()
    json_path = EXPORT_DIR / f"radar-full-{box}-{stamp}.json"
    export = {
        "schema": SCHEMA,
        "box": box,
        "generated_at": generated_at,
        "scanner": {
            "name": "radar",
            "version": SCANNER_VERSION,
            "mode": "full",
            "host_user": getpass.getuser(),
            "host_uid": os.getuid() if hasattr(os, "getuid") else None,
            "working_directory": os.getcwd(),
            "argv": sys.argv,
            "environment_redactions": [],
        },
        "run": {
            "run_id": run_id,
            "clock_source": "system",
            "clock_skew_known": False,
            "timezone": time.tzname[0] if time.tzname else None,
            "platform": sys.platform,
            "privilege_level": "root" if hasattr(os, "geteuid") and os.geteuid() == 0 else "user",
            "partial_run": False,
            "interrupted": False,
            "python": platform.python_version(),
        },
        "collection_matrix": collection_matrix,
        "source_files": manifests,
        "sections": sections,
        "totals": {
            "sections_intended": len(sections_list),
            "sections_loaded": sum(1 for m in manifests if m["loaded"]),
            "sections_complete": sum(1 for s in sections.values() if s["coverage"]["complete"]),
            "sections_failed": sum(1 for s in sections.values() if not s["coverage"]["complete"]),
            "items_total": item_total,
            "errors_total": errors_total,
            "redactions_total": redactions_total,
        },
        "hashes": {
            "algorithm": "sha256",
            "canonicalization": "sorted-compact-utf8",
            "source_files_sha256": hash_value(manifests),
            "sections_sha256": sections_sha256,
            "export_sha256": None,
        },
        "paths": {
            "json": str(json_path),
            "markdown": str(json_path.with_suffix(".md")),
            "text": str(json_path.with_suffix(".txt")),
            "sha256_sidecar": str(pathlib.Path(str(json_path) + ".sha256")),
        },
    }
    return export, stamp


def main() -> int:
    EXPORT_DIR.mkdir(parents=True, exist_ok=True)
    export, _stamp = build_export()
    json_path = pathlib.Path(export["paths"]["json"])
    md_path = pathlib.Path(export["paths"]["markdown"])
    txt_path = pathlib.Path(export["paths"]["text"])
    sidecar_path = pathlib.Path(export["paths"]["sha256_sidecar"])

    payload = json.dumps(export, ensure_ascii=False, indent=2, sort_keys=True) + "\n"
    atomic_write(json_path, payload)
    export_sha256 = sha256_bytes(payload.encode("utf-8"))
    atomic_write(sidecar_path, f"{export_sha256}  {json_path.name}\n")
    md = markdown_for(export)
    atomic_write(md_path, md)
    atomic_write(txt_path, md)

    latest_pairs = (
        ("radar-full-latest.json", json_path),
        ("radar-full-latest.md", md_path),
        ("radar-full-latest.txt", txt_path),
        ("radar-full-latest.json.sha256", sidecar_path),
    )
    for latest_name, source in latest_pairs:
        latest = EXPORT_DIR / latest_name
        tmp = latest.with_name(f".{latest.name}.tmp")
        shutil.copyfile(source, tmp)
        os.replace(tmp, latest)

    print(json.dumps({
        "schema": SCHEMA,
        "json_path": str(json_path),
        "markdown_path": str(md_path),
        "text_path": str(txt_path),
        "sidecar_path": str(sidecar_path),
        "export_sha256": export_sha256,
        "items_total": export["totals"]["items_total"],
        "source_files": len(export["source_files"]),
        "subjects": {
            name: section["coverage"]["emitted_count"]
            for name, section in export["sections"].items()
        },
    }, sort_keys=True))
    return 0


if __name__ == "__main__":
    sys.exit(main())
