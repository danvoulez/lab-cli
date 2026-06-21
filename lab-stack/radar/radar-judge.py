#!/usr/bin/env python3
"""RADAR judgment engine.

Reads ~/.radar/success.json (the success list = SLOs), and for each item that
applies to THIS box, runs EVERY defined method (whitebox + blackbox), with
per-method timeouts so nothing can hang the whole pass. It then aggregates by
liveness vs readiness into a verdict + a proposed action, and writes a report
with the full evidence trail.

Principles (the "kill the thieves" rules):
  - Never green without a passing check. If nothing could run -> UNKNOWN.
  - Liveness fail  -> the process is dead -> propose RESTART.
  - Readiness fail but liveness ok -> up-but-not-serving -> propose INVESTIGATE
    (a dependency/config problem; do NOT blindly restart).
  - This engine only JUDGES. It proposes actions; it never claims it did them.

Read-only. No deps beyond the Python 3 stdlib. Survives the cloud being gone.
"""
import json
import os
import socket
import subprocess
import sys
import time
from datetime import datetime, timezone

RADAR = os.path.expanduser("~/.radar")


def box_id():
    try:
        out = subprocess.run(["scutil", "--get", "LocalHostName"],
                             capture_output=True, text=True, timeout=4)
        if out.returncode == 0 and out.stdout.strip():
            return out.stdout.strip()
    except Exception:
        pass
    return socket.gethostname().split(".")[0]


def pub_for(box):
    # lab-256 -> 256, lab-8gb -> 8gb, lab-512 -> 512
    return box.replace("lab-", "")


# --- check primitives. each returns (state, evidence) -----------------------
# state in: "pass", "fail", "unknown"

def chk_launchd(c, ctx):
    label = c["label"]
    try:
        out = subprocess.run(["launchctl", "list"], capture_output=True,
                             text=True, timeout=5).stdout
    except Exception as e:
        return "unknown", f"launchctl error: {e}"
    for line in out.splitlines():
        parts = line.split("\t")
        if len(parts) >= 3 and parts[2] == label:
            code = parts[1]
            if code in ("0", "-"):
                return "pass", f"loaded, last exit {code} (pid {parts[0]})"
            return "fail", f"loaded but last exit {code}"
    return "fail", "not loaded"


def chk_http(c, ctx):
    url = c["url"].replace("{PUB}", ctx["pub"])
    expect = set(c.get("expect", [200]))
    try:
        out = subprocess.run(
            ["curl", "-s", "-m", "5", "-o", "/dev/null", "-w", "%{http_code}", url],
            capture_output=True, text=True, timeout=8)
        code = out.stdout.strip() or "000"
    except Exception as e:
        return "unknown", f"curl error: {e}"
    if code == "000":
        return "fail", f"no HTTP response ({url})"
    try:
        ok = int(code) in expect
    except ValueError:
        ok = False
    return ("pass" if ok else "fail"), f"HTTP {code} (expected {sorted(expect)}) <- {url}"


def chk_port(c, ctx):
    host, port = c["host"], int(c["port"])
    try:
        with socket.create_connection((host, port), timeout=3):
            return "pass", f"tcp {host}:{port} open"
    except OSError as e:
        return "fail", f"tcp {host}:{port} closed ({e.__class__.__name__})"
    except Exception as e:
        return "unknown", f"socket error: {e}"


def chk_file_fresh(c, ctx):
    path = os.path.expanduser(c["path"])
    if not os.path.exists(path):
        return "fail", f"missing: {path}"
    age_min = (time.time() - os.path.getmtime(path)) / 60.0
    limit = c.get("max_age_min", 30)
    if age_min <= limit:
        return "pass", f"updated {age_min:.0f} min ago (limit {limit})"
    return "fail", f"stale: {age_min:.0f} min old (limit {limit})"


def chk_disk_free(c, ctx):
    path = c.get("path", "/")
    try:
        st = os.statvfs(path)
        free_gb = st.f_bavail * st.f_frsize / (1024 ** 3)
    except Exception as e:
        return "unknown", f"statvfs error: {e}"
    minimum = c.get("min_gb", 10)
    if free_gb >= minimum:
        return "pass", f"{free_gb:.0f} GB free (min {minimum})"
    return "fail", f"only {free_gb:.0f} GB free (min {minimum})"


def chk_command(c, ctx):
    argv = [str(part).replace("{BOX}", ctx["box"]).replace("{PUB}", ctx["pub"]) for part in c["argv"]]
    expect = set(c.get("expect_codes", [0]))
    timeout = int(c.get("timeout", 10))
    try:
        out = subprocess.run(argv, capture_output=True, text=True, timeout=timeout)
    except FileNotFoundError:
        return "fail", f"command not found: {argv[0]}"
    except subprocess.TimeoutExpired:
        return "fail", f"command timed out after {timeout}s: {' '.join(argv)}"
    except Exception as e:
        return "unknown", f"command error: {e}"
    detail = (out.stdout or out.stderr or "").strip().splitlines()
    suffix = f": {detail[0][:160]}" if detail else ""
    state = "pass" if out.returncode in expect else "fail"
    return state, f"exit {out.returncode} (expected {sorted(expect)}){suffix}"


CHECKS = {
    "launchd": chk_launchd,
    "http": chk_http,
    "port": chk_port,
    "file_fresh": chk_file_fresh,
    "disk_free": chk_disk_free,
    "command": chk_command,
}


def run_check(c, ctx):
    fn = CHECKS.get(c["kind"])
    if not fn:
        return "unknown", f"unknown check kind: {c['kind']}"
    try:
        return fn(c, ctx)
    except Exception as e:  # a single check can never crash the pass
        return "unknown", f"check raised: {e}"


def verdict_for(results):
    """results: list of dicts with role + state. Returns (verdict, action)."""
    live = [r for r in results if r["role"] == "liveness"]
    ready = [r for r in results if r["role"] == "readiness"]

    def any_pass(rs):
        return any(r["state"] == "pass" for r in rs)

    def all_unknown(rs):
        return rs and all(r["state"] == "unknown" for r in rs)

    if not results:
        return "UNDEFINED", "no success criteria defined"
    if all_unknown(results):
        return "UNKNOWN", "no method could run (offline/unreachable) -- not claiming OK"

    live_ok = any_pass(live) if live else None
    ready_ok = any_pass(ready) if ready else None

    # readiness is the strongest signal of "actually working"
    if ready_ok is True:
        return "OK", "serving -- verified by at least one readiness method"
    if ready_ok is False:
        if live_ok is True:
            return "DEGRADED", "process up but NOT serving -> investigate dependency/config (do NOT blindly restart)"
        if live_ok is False:
            return "DOWN", "dead and not serving -> RESTART (liveness recovery)"
        # no liveness defined, readiness failed
        return "DOWN", "not serving -> investigate / restart"
    # no readiness checks; judge on liveness
    if live_ok is True:
        return "OK", "process alive (liveness only)"
    if live_ok is False:
        return "DOWN", "process not alive -> RESTART"
    return "UNKNOWN", "indeterminate"


def human(n):
    n = n or 0
    for u in ("B", "K", "M", "G", "T"):
        if n < 1024:
            return f"{n:.0f}{u}"
        n /= 1024
    return f"{n:.0f}P"


def inventory_section():
    lines = []

    def load(name):
        try:
            with open(os.path.join(RADAR, name)) as f:
                return json.load(f)
        except Exception:
            return None

    for subj, title in (("storage", "Storage hogs"),
                        ("junk", "Junk / corpses (safe to delete on your order)")):
        j = load(subj + ".json")
        if not j or "items" not in j:
            continue
        items = [i for i in j["items"] if i.get("size_bytes")]
        items.sort(key=lambda i: i.get("size_bytes") or 0, reverse=True)
        cov = j.get("coverage", {})
        lines.append(f"\n### {title}\n_source: {cov.get('source','')} ({cov.get('count','')} items)_\n")
        for i in items[:8]:
            d = f" — {i['detail']}" if i.get("detail") else ""
            lines.append(f"- {human(i.get('size_bytes'))}  `{i.get('label','')}`{d}")
    return "\n".join(lines) if lines else "_no scan inventory found (~/.radar/*.json)_"


EMOJI = {"OK": "🟢", "DEGRADED": "🟡", "DOWN": "🔴", "UNKNOWN": "⚪", "UNDEFINED": "⚫"}


def main():
    box = box_id()
    ctx = {"box": box, "pub": pub_for(box)}
    stamp = datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")
    fstamp = datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%SZ")

    with open(os.path.join(RADAR, "success.json")) as f:
        spec = json.load(f)

    judged = []
    for item in spec["items"]:
        if box not in item.get("applies_to", []):
            continue
        results = []
        for c in item["checks"]:
            state, evidence = run_check(c, ctx)
            results.append({"role": c.get("role", "readiness"),
                            "view": c.get("view", "whitebox"),
                            "desc": c.get("desc", c["kind"]),
                            "state": state, "evidence": evidence})
        verdict, action = verdict_for(results)
        judged.append({"item": item, "verdict": verdict, "action": action,
                       "results": results})

    # --- render ---
    out = []
    out.append("# RADAR — Situation Report (pro)")
    out.append("")
    out.append(f"**Box:** `{box}`  ")
    out.append(f"**Generated:** {stamp} (UTC)  ")
    out.append("**How:** every system is judged against the success list (SLOs) by "
               "MULTIPLE independent methods — whitebox (innards) + blackbox (from "
               "outside). Verdict is never green without a passing check; if no method "
               "could run it says UNKNOWN, not OK. Generated BY Radar, not by a chat.")
    out.append("")
    out.append("---\n\n## Headline\n")
    out.append("| | System | Verdict | Proposed action |")
    out.append("|---|---|---|---|")
    for j in judged:
        e = EMOJI.get(j["verdict"], "")
        out.append(f"| {e} | {j['item']['name']} | **{j['verdict']}** | {j['action']} |")
    out.append("")
    out.append("---\n\n## Evidence trail (the many ways)\n")
    for j in judged:
        e = EMOJI.get(j["verdict"], "")
        out.append(f"### {e} {j['item']['name']} — {j['verdict']}")
        out.append(f"_{j['action']}_\n")
        out.append("| method | view | role | result | evidence |")
        out.append("|---|---|---|---|---|")
        for r in j["results"]:
            mark = {"pass": "✓", "fail": "✗", "unknown": "?"}[r["state"]]
            out.append(f"| {r['desc']} | {r['view']} | {r['role']} | {mark} {r['state']} | {r['evidence']} |")
        out.append("")
    out.append("---\n\n## Inventory — what's eating the box")
    out.append(inventory_section())
    out.append("")

    # machine-readable verdicts (for the UI / ledger)
    summary = {}
    for j in judged:
        summary[j["verdict"]] = summary.get(j["verdict"], 0) + 1
    vjson = {
        "box": box,
        "generated_at": stamp,
        "summary": summary,
        "items": [{"id": j["item"]["id"], "name": j["item"]["name"],
                   "verdict": j["verdict"], "action": j["action"],
                   "results": j["results"]} for j in judged],
    }

    report = "\n".join(out)

    def write(name, data, as_json=False):
        with open(os.path.join(RADAR, name), "w") as f:
            if as_json:
                json.dump(data, f, indent=2)
            else:
                f.write(data)

    # stable "latest" the UI/handoff reads; timestamped only on demand
    write("report-latest.md", report)
    write("report-latest.txt", report)
    write("verdict.json", vjson, as_json=True)
    if "--snapshot" in sys.argv:
        write(f"report-{box}-{fstamp}.md", report)
        write(f"report-{box}-{fstamp}.txt", report)

    print(report)
    print(f"\n[radar-judge] verdicts: {summary}", file=sys.stderr)


if __name__ == "__main__":
    main()
