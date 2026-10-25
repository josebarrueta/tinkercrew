"""Secret-free dependency evidence tooling. Inventory is not compliance approval."""
import argparse
import datetime
import hashlib
import json
import os
import stat
from pathlib import Path
import sys
import tempfile
import tomllib
from urllib.parse import urlsplit


class AuditError(Exception):
    pass


def canonical(value):
    return (json.dumps(value, sort_keys=True, indent=2, ensure_ascii=True) + "\n").encode()


def read(path, maximum=8 * 1024 * 1024):
    path = Path(path).absolute()
    if any(part.is_symlink() for part in (path, *path.parents)):
        raise AuditError("Symlinked audit inputs are refused.")
    descriptor = os.open(path, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK)
    with os.fdopen(descriptor,"rb") as source:
        details = os.fstat(source.fileno())
        if not stat.S_ISREG(details.st_mode) or details.st_size > maximum:
            raise AuditError("Audit input must be a bounded regular file.")
        data = source.read(maximum + 1)
        if len(data) > maximum:
            raise AuditError("Audit input exceeds its safe bound.")
        return data


def write(path, data):
    path = Path(path).absolute()
    if any(part.is_symlink() for part in (path,*path.parents)):
        raise AuditError("Symlinked audit outputs are refused.")
    descriptor,temporary = tempfile.mkstemp(prefix=".tinkercrew-notices-",dir=path.parent)
    try:
        with os.fdopen(descriptor,"wb") as target:
            os.fchmod(target.fileno(),0o644)
            target.write(data)
            target.flush()
            os.fsync(target.fileno())
        os.replace(temporary,path)
        directory = os.open(path.parent,os.O_RDONLY | os.O_DIRECTORY)
        try:
            os.fsync(directory)
        finally:
            os.close(directory)
    finally:
        if os.path.exists(temporary):
            os.unlink(temporary)


def unique(pairs):
    result = {}
    for key,value in pairs:
        if key in result:
            raise AuditError("Duplicate audit JSON key.")
        result[key] = value
    return result


def decode(raw):
    return json.loads(raw,object_pairs_hook=unique)


def inventory(root):
    root = Path(root).resolve()
    cargo = read(root / "app/src-tauri/Cargo.lock")
    npm = read(root / "app/package-lock.json")
    components = []
    seen = set()
    cargo_lock = tomllib.loads(cargo.decode())
    if type(cargo_lock.get("version")) is not int or cargo_lock["version"] not in {3,4}:
        raise AuditError("Unsupported Cargo lock schema.")
    for package in cargo_lock["package"]:
        source = package.get("source", "workspace")
        if not isinstance(source,str):
            raise AuditError("Unsupported package source declaration.")
        if source != "workspace":
            address = urlsplit(source.removeprefix("registry+").removeprefix("git+"))
            if address.scheme != "https" or not address.hostname or address.username or address.password or address.query:
                raise AuditError("Unsupported or credential-bearing package source.")
        identity = (package["name"], package["version"], source)
        if identity in seen:
            raise AuditError("Duplicate package identity.")
        seen.add(identity)
        components.append({"ecosystem": "cargo", "name": package["name"], "version": package["version"], "source": package.get("source", "workspace"), "checksum": package.get("checksum"), "licenseExpression": None, "scope": "unclassified-build-runtime-or-optional"})
    lock = decode(npm)
    if lock["lockfileVersion"] != 3:
        raise AuditError("Unsupported npm lock schema.")
    for location, package in lock["packages"].items():
        if not location:
            continue
        if package.get("link") or not location.startswith("node_modules/") or "\\" in location or any(part in {".","..",""} for part in location.split("/")):
            raise AuditError("Unclassified npm workspace/link entry; review it explicitly.")
        components.append({"ecosystem": "npm", "name": package.get("name", location.rsplit("node_modules/", 1)[1]), "version": package["version"], "location": location, "integrity": package.get("integrity"), "licenseExpression": package.get("license"), "scope": "unclassified-build-runtime-or-optional"})
    for component in components:
        if any(not isinstance(component[key],str) or not component[key] for key in ("name","version")):
            raise AuditError("Missing package identity.")
        for key in ("name","version","source","location","integrity","checksum","licenseExpression"):
            value = component.get(key)
            if value is not None and (not isinstance(value,str) or not 0 < len(value) <= 1024 or any(ord(char) < 32 or ord(char) > 126 for char in value)):
                raise AuditError("Unsupported package metadata; only bounded printable declarations are accepted.")
    components.sort(key=lambda entry: (entry["ecosystem"], entry["name"], entry["version"], entry.get("source", ""), entry.get("location", "")))
    return {"schemaVersion": 1, "assessment": "inventory-only-not-legal-approval", "inputs": {"cargoLockSha256": hashlib.sha256(cargo).hexdigest(), "npmLockSha256": hashlib.sha256(npm).hexdigest()}, "components": components}


def evidence_file(root, relative, maximum=1024 * 1024):
    if not isinstance(relative, str) or "\\" in relative or relative.startswith("/") or any(part in {"", ".", ".."} for part in relative.split("/")):
        raise AuditError("Invalid evidence path.")
    path = root / relative
    if not path.resolve().is_relative_to(root) or any(part.is_symlink() for part in (path, *path.parents) if part.is_relative_to(root)):
        raise AuditError("Evidence escapes its package tree.")
    if not path.is_file() or path.stat().st_size > maximum:
        raise AuditError("Missing or oversized license evidence.")
    return read(path,maximum)


def notices(root, inventory_path, evidence_path):
    root = Path(root).resolve()
    current = inventory(root)
    raw = read(inventory_path)
    if raw != canonical(current):
        raise AuditError("Inventory is stale; do not render stale evidence.")
    evidence_raw = read(evidence_path)
    evidence = decode(evidence_raw)
    if set(evidence) != {"schemaVersion", "entries"} or type(evidence["schemaVersion"]) is not int or evidence["schemaVersion"] != 1:
        raise AuditError("Unsupported evidence schema.")
    available = {canonical(component): component for component in current["components"]}
    entries = {}
    for entry in evidence["entries"]:
        key = canonical(entry["component"])
        if key not in available or key in entries or entry["status"] not in {"included", "excluded", "pending"}:
            raise AuditError("Unknown or duplicate evidence component.")
        if entry["status"] == "pending" and not set(entry) <= {"component","status","reason"}:
            raise AuditError("Unsupported pending evidence fields.")
        entries[key] = entry
    lines = ["TINKEROO DEPENDENCY DISCLOSURE — NOT RELEASE APPROVAL", "Conservative lock inventory; inclusion/scope and legal obligations require review.", "Inventory SHA-256: " + hashlib.sha256(raw).hexdigest(), "Evidence SHA-256: " + hashlib.sha256(evidence_raw).hexdigest(), ""]
    total_license_bytes = 0
    for key, component in available.items():
        label = f"{component['ecosystem']} {component['name']} {component['version']}"
        entry = entries.get(key)
        if not entry or entry["status"] == "pending":
            lines.extend(["PENDING: " + label, "Scope/license/source-availability evidence has not been reviewed.", ""])
            continue
        if entry["status"] == "excluded":
            if set(entry) != {"component", "status", "reason"} or not isinstance(entry["reason"], str) or not entry["reason"].strip():
                raise AuditError("An exclusion requires an explicit reviewed reason.")
            lines.extend(["EXCLUDED: " + label, entry["reason"], ""])
            continue
        if set(entry) != {"component", "status", "licenseExpression", "sourceAvailability", "files"} or not entry["licenseExpression"] or not entry["files"]:
            raise AuditError("Included components require license and source evidence.")
        source = urlsplit(entry["sourceAvailability"])
        if source.scheme != "https" or not source.hostname or source.username or source.password or source.query:
            raise AuditError("Unsupported source-availability reference.")
        lines.extend(["INCLUDED: " + label, "License expression: " + entry["licenseExpression"], "Source availability: " + entry["sourceAvailability"]])
        for item in entry["files"]:
            if set(item) != {"path", "sha256"}:
                raise AuditError("Unsupported license evidence fields.")
            if not isinstance(item["path"],str) or not item["path"].startswith("docs/release/third-party/licenses/"):
                raise AuditError("License evidence must be in the reviewed public-license directory.")
            text = evidence_file(root, item["path"])
            if not text.strip():
                raise AuditError("Empty license evidence is not a notice.")
            total_license_bytes += len(text)
            if total_license_bytes > 16 * 1024 * 1024:
                raise AuditError("Combined license evidence exceeds its safe bound.")
            if hashlib.sha256(text).hexdigest() != item["sha256"]:
                raise AuditError("License evidence changed; review before regeneration.")
            lines.extend(["Evidence: " + item["path"], text.decode("utf-8"), ""])
    lines.append("END OF DEPENDENCY DISCLOSURE")
    result = ("\n".join(lines) + "\n").encode("utf-8")
    if len(result) > 32 * 1024 * 1024:
        raise AuditError("Combined disclosure exceeds its safe bound.")
    return result


def release_check(root, inventory_path, evidence_path, notice_path, review_path):
    expected = notices(root,inventory_path,evidence_path)
    if read(notice_path,32 * 1024 * 1024) != expected:
        raise AuditError("Notices are stale or modified.")
    data = decode(read(inventory_path))
    entries = decode(read(evidence_path))["entries"]
    if len(entries) != len(data["components"]) or any(entry["status"] == "pending" for entry in entries):
        raise AuditError("Pending component evidence or scope decisions block release.")
    review = decode(read(review_path))
    fields = {"schemaVersion","decision","reviewer","reviewedAt","inventorySha256","evidenceSha256","noticesSha256"}
    if set(review) != fields or type(review["schemaVersion"]) is not int or review["schemaVersion"] != 1 or review["decision"] != "approved-for-release" or not isinstance(review["reviewer"],str) or not review["reviewer"].strip() or not isinstance(review["reviewedAt"],str) or not review["reviewedAt"].endswith("Z"):
        raise AuditError("Explicit owner review record is missing or pending.")
    datetime.datetime.strptime(review["reviewedAt"],"%Y-%m-%dT%H:%M:%SZ")
    for key,path in (("inventorySha256",inventory_path),("evidenceSha256",evidence_path),("noticesSha256",notice_path)):
        if review[key] != hashlib.sha256(read(path,32 * 1024 * 1024)).hexdigest():
            raise AuditError("Owner review is stale; no approval was renewed automatically.")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    create = commands.add_parser("inventory", help="Write a conservative lockfile inventory; never legal approval.")
    create.add_argument("--root", type=Path, required=True)
    create.add_argument("--output", type=Path, required=True)
    check = commands.add_parser("check", help="Check lock inventory freshness without rewriting it.")
    check.add_argument("--root", type=Path, required=True)
    check.add_argument("--inventory", type=Path, required=True)
    check.add_argument("--evidence", type=Path)
    check.add_argument("--notices", type=Path)
    render = commands.add_parser("notices", help="Render verified evidence; missing components stay explicitly pending.")
    render.add_argument("--root", type=Path, required=True)
    render.add_argument("--inventory", type=Path, required=True)
    render.add_argument("--evidence", type=Path, required=True)
    render.add_argument("--output", type=Path, required=True)
    bundle = commands.add_parser("bundle-check", help="Verify actual app resources; not legal approval or a signature check.")
    bundle.add_argument("--root", type=Path, required=True)
    bundle.add_argument("--inventory", type=Path, required=True)
    bundle.add_argument("--evidence", type=Path, required=True)
    bundle.add_argument("--notices", type=Path, required=True)
    bundle.add_argument("--app", type=Path, required=True)
    release = commands.add_parser("release-check", help="Require complete evidence and an existing hash-bound owner review record.")
    release.add_argument("--root", type=Path, required=True)
    release.add_argument("--inventory", type=Path, required=True)
    release.add_argument("--evidence", type=Path, required=True)
    release.add_argument("--notices", type=Path, required=True)
    release.add_argument("--review", type=Path, required=True)
    args = parser.parse_args()
    try:
        if args.command == "release-check":
            release_check(args.root,args.inventory,args.evidence,args.notices,args.review)
            print("Review record matches evidence. Protected owner publication approval is still required; reviewer identity is not authenticated by this CLI.")
            return 0
        if args.command == "bundle-check":
            expected = notices(args.root,args.inventory,args.evidence)
            if args.app.is_symlink():
                raise AuditError("An app symlink cannot substitute for shipped bundle contents.")
            app = args.app.resolve()
            if read(args.notices,32 * 1024 * 1024) != expected or evidence_file(app,"Contents/Resources/third-party/DEPENDENCY_NOTICES.txt",32 * 1024 * 1024) != expected or evidence_file(app,"Contents/Resources/third-party/DEPENDENCY_INVENTORY.json",8 * 1024 * 1024) != read(args.inventory):
                raise AuditError("Packaged dependency evidence is missing, stale or modified.")
            print("Packaged dependency evidence matches. This is not legal approval or signature verification.")
            return 0
        protected = {(args.root / name).resolve() for name in ("app/src-tauri/Cargo.lock", "app/package-lock.json")}
        protected.update(path.resolve() for path in (getattr(args,"inventory",None),getattr(args,"evidence",None)) if path)
        if getattr(args,"output",None) and args.output.resolve() in protected:
            raise AuditError("Output cannot replace its inputs.")
        result = notices(args.root,args.inventory,args.evidence) if args.command == "notices" else canonical(inventory(args.root))
        if args.command == "check":
            if read(args.inventory) != result:
                print("Dependency inventory is stale or modified; regenerate and review explicitly.", file=sys.stderr)
                return 1
            if bool(args.evidence) != bool(args.notices):
                raise AuditError("Supply evidence and source notices together.")
            if args.evidence and read(args.notices,32 * 1024 * 1024) != notices(args.root,args.inventory,args.evidence):
                raise AuditError("Source dependency notices are stale or modified.")
        else:
            write(args.output,result)
        print("Dependency inventory matches inputs. Scope and license evidence remain unreviewed.")
        return 0
    except AuditError as error:
        print("Dependency audit failed: " + str(error), file=sys.stderr)
        return 1
    except (OSError, ValueError, KeyError, TypeError, AttributeError, RecursionError):
        print("Dependency audit failed: unsupported, missing or invalid input; no approval was created.", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
