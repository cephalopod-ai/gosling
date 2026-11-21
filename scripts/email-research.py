"""Import one Gosling mail paper through an explicitly configured research destination."""

import argparse
import hashlib
import html
import json
import os
from pathlib import Path
import re
import subprocess
import sys
import tempfile
import time
from urllib.error import HTTPError
from urllib.parse import urlencode, urlsplit
from urllib.request import HTTPRedirectHandler, Request, build_opener


class NoRedirect(HTTPRedirectHandler):
    def redirect_request(self, *_):
        return None


def fetch(url, headers, data=None, method=None, expect_json=True):
    request = Request(url, headers=headers, data=data, method=method)
    with build_opener(NoRedirect).open(request, timeout=15) as response:
        body = response.read(1024 * 1024 + 1)
    if len(body) > 1024 * 1024:
        raise ValueError("Research service response is too large")
    return json.loads(body) if body and expect_json else None


def stable_item_key(identity, kind):
    alphabet = "23456789ABCDEFGHIJKLMNPQRSTUVWXYZ"
    digest = hashlib.sha256(f"{identity}/{kind}".encode()).digest()
    return "".join(alphabet[byte & 31] for byte in digest[:8])


def zotero(args, paper):
    if not re.fullmatch(r"(?:users|groups)/[0-9]+", args.library):
        raise ValueError("Use a Zotero users/<id> or groups/<id> library")
    with args.key_file.open(encoding="utf-8") as source:
        key = source.read(4096).strip()
    root = f"https://api.zotero.org/{args.library}"
    headers = {"Zotero-API-Version": "3", "Zotero-API-Key": key}

    def find_or_create(identity, kind, item):
        key = stable_item_key(identity, kind)
        target = f"{root}/items/{key}"
        try:
            existing = fetch(target, headers)["data"]
            if existing.get("itemType") != item["itemType"] or existing.get("parentItem") != item.get("parentItem"):
                raise ValueError("Zotero item key belongs to a different item")
            if item.get("extra") and item["extra"] not in existing.get("extra", ""):
                raise ValueError("Zotero item key is already used")
            return key
        except HTTPError as error:
            if error.code != 404:
                raise
        result = fetch(f"{root}/items", {**headers, "Content-Type": "application/json"},
                       json.dumps([{**item, "key": key, "version": 0}]).encode())
        if result.get("successful", {}).get("0", {}).get("key") != key:
            raise ValueError("Zotero did not return the expected item key")
        return key

    identity = paper["doi"] or paper.get("arxiv") or paper["idempotencyKey"]
    item = {
        "itemType": "journalArticle", "title": paper["title"],
        "extra": f"Gosling source: {identity}", "collections": [args.collection],
        "tags": [], "relations": {},
    }
    if paper["doi"]:
        item["DOI"] = paper["doi"]
    if paper.get("arxiv"):
        item["url"] = f"https://arxiv.org/abs/{paper['arxiv']}"
    key = find_or_create(identity, "article", item)
    if paper["pdf"]:
        source = Path(paper["pdf"])
        with source.open("rb") as handle:
            pdf = handle.read(8 * 1024 * 1024 + 1)
        if len(pdf) > 8 * 1024 * 1024 or hashlib.sha256(pdf).hexdigest() != paper["pdfSha256"]:
            raise ValueError("PDF does not match its recorded identity")
        filename = paper["idempotencyKey"] + ".pdf"
        attachment = find_or_create(paper["idempotencyKey"], "pdf", {
            "itemType": "attachment", "parentItem": key, "linkMode": "imported_file",
            "title": "Full text", "contentType": "application/pdf", "filename": filename,
            "tags": [], "relations": {},
        })
        url = f"{root}/items/{attachment}/file"
        form = {**headers, "Content-Type": "application/x-www-form-urlencoded", "If-None-Match": "*"}
        md5 = hashlib.md5(pdf, usedforsecurity=False).hexdigest()
        stored = fetch(f"{root}/items/{attachment}", headers)["data"]
        if stored.get("md5") not in (None, "", md5):
            raise ValueError("Zotero already has different attachment bytes")
        upload = {"exists": 1} if stored.get("md5") == md5 else fetch(url, form, urlencode({
            "md5": md5,
            "filename": filename, "filesize": len(pdf), "mtime": int(time.time() * 1000),
        }).encode())
        if not upload.get("exists"):
            if urlsplit(upload["url"]).scheme != "https":
                raise ValueError("Zotero returned an insecure attachment transport")
            payload = upload["prefix"].encode() + pdf + upload["suffix"].encode()
            fetch(upload["url"], {"Content-Type": upload["contentType"]}, payload, expect_json=False)
            fetch(url, form, urlencode({"upload": upload["uploadKey"]}).encode())
    if paper["summary"] and not paper["summary"].startswith("Summary unavailable."):
        find_or_create(paper["idempotencyKey"], "summary", {
            "itemType": "note", "parentItem": key, "note": html.escape(paper["summary"]).replace("\n", "<br>"),
            "tags": [], "relations": {},
        })
    return key


def run_tool(argv):
    return subprocess.run(argv, check=True, capture_output=True, timeout=30, text=True).stdout


def physics(args, paper):
    command = [args.alfred, "--db", args.database, "ingest", "email",
               "--supplied-by", "gosling", "--content-hash", paper["idempotencyKey"]]
    if paper["doi"]:
        command.append(f"--doi={paper['doi']}")
    if paper.get("arxiv"):
        command.append(f"--arxiv-id={paper['arxiv']}")
    if paper["pdf"]:
        command.append(f"--raw-reference={Path(paper['pdf']).as_uri()}")
    if paper["summary"]:
        command.append(f"--notes=Gosling model summary: {paper['summary']}")
    output = run_tool([*command, "--", paper["title"]])
    match = re.search(r"^research_record: (\S+)", output, re.MULTILINE)
    if not match:
        raise ValueError("Alfred did not return a research record")
    return match[1]


def source_package(root, paper, pdf):
    def encoded(value):
        return json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":")).encode()

    def digest(data):
        return "sha256:" + hashlib.sha256(data).hexdigest()

    key = paper["idempotencyKey"]
    timestamp = paper["capturedAt"]
    def record_id(kind):
        return kind + "_" + hashlib.sha256(f"{key}/{kind}".encode()).hexdigest()[:32]

    actor, work, version, occurrence, artifact, run, result = map(
        record_id, ["act", "wrk", "ver", "occ", "art", "run", "res"],
    )
    if paper["doi"]:
        locator = f"https://doi.org/{paper['doi']}"
    elif paper.get("arxiv"):
        locator = f"https://arxiv.org/abs/{paper['arxiv']}"
    else:
        locator = f"urn:sha256:{key}"
    package_artifact = {
        "artifact_id": artifact, "occurrence_id": occurrence, "custody": "reference_only",
        "media_type": "application/pdf", "byte_length": 0, "digest_state": "unavailable",
    }
    payloads = []
    root.mkdir()
    if pdf is not None:
        sha = digest(pdf)
        directory = root / "payloads" / "sha256"
        directory.mkdir(parents=True)
        (directory / sha.removeprefix("sha256:")).write_bytes(pdf)
        payloads.append({"sha256": sha, "bytes": len(pdf)})
        package_artifact.update(custody="included", byte_length=len(pdf), digest_state="receiver_verified",
                                sha256=sha, payload_sha256=sha)
    records = {
        "works": [{"work_id": work, "label": paper["title"], "source_type": "journal_article",
                   "identifiers": [locator]}],
        "versions": [{"version_id": version, "work_id": work, "label": "Email source",
                      "version_kind": "unknown"}],
        "occurrences": [{"occurrence_id": occurrence, "version_id": version, "locator": locator,
                         "captured_at": timestamp, "acquiring_actor": actor}],
        "artifacts": [package_artifact],
        "processing-runs": [{"run_id": run, "actor_id": actor, "method": "gosling-email-capture",
                             "method_version": "1", "input_occurrence_ids": [occurrence],
                             "started_at": timestamp, "ended_at": timestamp, "deterministic": True}],
        "processing-results": [{"result_id": result, "run_id": run, "occurrence_id": occurrence,
                                "disposition": "deferred", "observation_ids": [], "warnings": []}],
        "observations": [],
    }
    files = {}
    for kind, rows in records.items():
        name = kind + ".jsonl"
        body = b"".join(encoded(row) + b"\n" for row in rows)
        (root / name).write_bytes(body)
        files[name] = {"bytes": len(body), "count": len(rows), "sha256": digest(body)}
    manifest = {
        "contract_version": "mathmcp.source-package/1", "created_at": timestamp,
        "origin_repository": {"repository_id": "gosling:email", "software_version": "1"},
        "contributors": [{"actor_id": actor, "kind": "automated", "identity": "Gosling email"}],
        "files": files, "capture_bundles": [], "payloads": payloads, "package_relationships": [],
        "declarations": {"access": "private operator intake", "attribution": "email source",
                         "raw_expression_redistribution": "not granted",
                         "known_limits": "Source custody only without candidate extraction"},
    }
    sha = digest(encoded(manifest))
    manifest.update(package_sha256=sha, package_id="pkg_" + sha[7:39])
    (root / "SOURCE-PACKAGE.json").write_bytes(encoded(manifest))
    return sha


def math(args, paper):
    path = Path(paper["pdf"]) if paper["pdf"] else None
    pdf = path.read_bytes() if path else None
    if pdf is not None and hashlib.sha256(pdf).hexdigest() != paper["pdfSha256"]:
        raise ValueError("PDF does not match its recorded identity")
    with tempfile.TemporaryDirectory(prefix="gosling-math-") as scratch:
        package = Path(scratch) / "source"
        sha = source_package(package, paper, pdf)
        run_tool([args.mathmcp, "source-package", "retain", "--store", args.store,
                  "--package", package, "--receipt", Path(scratch) / "source-receipt.json"])
        if path is None:
            return f"{sha} (citation retained; full text unavailable)"

        from sampo.adapters.sources.pdf import PDFSourceAdapter
        from sampo.core.capture_store import FileCaptureStore
        from sampo.math_capture.acquisition import ingest_source_to_math_bundle

        source = PDFSourceAdapter(path=path, source_origin="gosling:email",
                                  origin_timestamp=paper["capturedAt"], preview_pages=40).capture()
        bundle = Path(scratch) / "bundle"
        ingest_source_to_math_bundle(
            bundle, source, FileCaptureStore(args.capture_store), project_scope=args.project,
            ingest_actor="gosling", destinations={}, license_note=args.license_note,
            created_at=paper["capturedAt"],
        )
        output = run_tool([args.mathmcp, "capture", "import", "--store", args.store,
                           "--bundle", bundle, "--receipt", Path(scratch) / "capture-receipt.json"])
        result = json.loads(output)
    if not result["declared_count"]:
        return f"{sha} (source retained; no equation candidates)"
    return f"{sha} ({json.dumps(result['counts'], sort_keys=True)})"


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    targets = parser.add_subparsers(dest="destination", required=True)
    target = targets.add_parser("zotero")
    target.add_argument("--library", required=True)
    target.add_argument("--collection", required=True)
    target.add_argument("--key-file", type=Path, required=True)
    target = targets.add_parser("physics")
    target.add_argument("--alfred", type=Path, required=True)
    target.add_argument("--database", type=Path, required=True)
    target = targets.add_parser("math")
    target.add_argument("--mathmcp", type=Path, required=True)
    target.add_argument("--store", type=Path, required=True)
    target.add_argument("--capture-store", type=Path, required=True)
    target.add_argument("--project", required=True)
    target.add_argument("--license-note", required=True)
    args = parser.parse_args()
    os.umask(0o077)
    paper = json.loads(sys.stdin.buffer.read(64 * 1024))
    if paper["version"] != 1 or paper["destination"] != args.destination:
        raise ValueError("Unsupported research intake request")
    if not re.fullmatch(r"[0-9a-f]{64}", paper["idempotencyKey"]):
        raise ValueError("Invalid research identity")
    handler = {"zotero": zotero, "physics": physics, "math": math}[args.destination]
    print(json.dumps({"id": handler(args, paper)}))


if __name__ == "__main__":
    main()
