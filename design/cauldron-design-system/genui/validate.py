#!/usr/bin/env python3
"""Validate generative-UI messages from an agent.

Usage: validate.py message.json [message.json ...] [--actions id1,id2,...]
Exit 0 when every message is valid, 1 otherwise. The host runs the same checks
before it renders anything an agent produced (see GENUI.md, "Host rules").
"""
import json, sys, pathlib
from jsonschema import Draft202012Validator

HERE = pathlib.Path(__file__).parent
SCHEMA = json.loads((HERE / "genui.schema.json").read_text())
ENVELOPE = {
    "type": "object",
    "required": ["v", "op"],
    "properties": {
        "v": {"const": 1},
        "op": {"enum": ["create", "patch", "remove"]},
        "at": {"enum": ["thread", "drawer"]},
        "id": {"type": "string"},
        "block": {"type": "object"},
        "set": {"type": "object"},
    },
    "additionalProperties": False,
}
MAX_DEPTH, MAX_NODES = 6, 200


def validate_node(node, path, errors):
    t = node.get("type") if isinstance(node, dict) else None
    d = SCHEMA["$defs"].get(t)
    if d is None:
        errors.append(f"{path or '/'}: unknown component type {t!r}")
        return
    own = json.loads(json.dumps(d))
    own["properties"]["children"] = {"type": "array", "maxItems": d["properties"]["children"].get("maxItems", 0)}
    for e in Draft202012Validator(own).iter_errors(node):
        where = "/".join(map(str, e.absolute_path))
        errors.append(f"{path}/{where}: {e.message[:140]}")
    for i, ch in enumerate(node.get("children", []) if isinstance(node.get("children"), list) else []):
        validate_node(ch, f"{path}/children/{i}", errors)


def walk(node, depth, parent, out, errors, path):
    if depth > MAX_DEPTH:
        errors.append(f"{path}: deeper than {MAX_DEPTH} levels")
        return
    out.append(node)
    t = node.get("type")
    if t == "Panel" and parent not in ("Shell",):
        errors.append(f"{path}: Panel must be a direct child of Shell")
    if t == "Shell" and parent == "Panel":
        errors.append(f"{path}: Shell inside Panel; use Well or Stack instead")
    for i, ch in enumerate(node.get("children", [])):
        walk(ch, depth + 1, t, out, errors, f"{path}/children/{i}")


def collect_actions(v, acc):
    if isinstance(v, dict):
        for k, x in v.items():
            if k == "action" or k.endswith("Action"):
                acc.append(x)
            else:
                collect_actions(x, acc)
    elif isinstance(v, list):
        for x in v:
            collect_actions(x, acc)


def check(msg, known_actions=None):
    errors = []
    for e in Draft202012Validator(ENVELOPE).iter_errors(msg):
        errors.append("envelope: " + e.message)
    if errors:
        return errors
    if msg["op"] == "create":
        if "block" not in msg or "at" not in msg:
            return ["create needs 'at' and 'block'"]
        block = msg["block"]
        validate_node(block, "", errors)
        if errors:
            return errors[:6]
        nodes = []
        walk(block, 1, None, nodes, errors, "")
        if len(nodes) > MAX_NODES:
            errors.append(f"more than {MAX_NODES} nodes")
        ids = [n["id"] for n in nodes if "id" in n]
        if len(ids) != len(set(ids)):
            errors.append("duplicate node ids")
        primaries = sum(
            1 for n in nodes if n["type"] == "Button" and n.get("props", {}).get("variant") == "primary"
        ) + sum(1 for n in nodes if n["type"] == "ApprovalCard")
        if primaries > 1:
            errors.append("more than one primary action in one block")
        for n in nodes:
            p = n.get("props", {})
            if n["type"] == "ApprovalCard" and p.get("approveAction") == p.get("denyAction"):
                errors.append("ApprovalCard approve and deny share an action id")
        acts = []
        collect_actions([n.get("props", {}) for n in nodes], acts)
        if known_actions is not None:
            for a in acts:
                if a not in known_actions:
                    errors.append(f"unknown action id: {a}")
    if msg["op"] in ("patch", "remove") and "id" not in msg:
        errors.append(f"{msg['op']} needs 'id'")
    if msg["op"] == "patch" and "set" not in msg:
        errors.append("patch needs 'set'")
    return errors


def main(argv):
    known = None
    if "--actions" in argv:
        i = argv.index("--actions")
        known = set(argv[i + 1].split(","))
        argv = argv[:i] + argv[i + 2:]
    bad = 0
    for f in argv:
        errs = check(json.loads(pathlib.Path(f).read_text()), known)
        print(("OK   " if not errs else "FAIL ") + f)
        for e in errs:
            print("     " + e)
        bad += bool(errs)
    return 1 if bad else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
