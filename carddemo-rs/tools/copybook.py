#!/usr/bin/env python3
"""Minimal COBOL copybook parser for the CardDemo record layouts.

Computes offset/length/type of every elementary item (handles PIC X/9/S9/V,
editing pictures, COMP/COMP-3/BINARY, OCCURS (fixed), REDEFINES, FILLER).

  copybook.py md   <file.cpy>...   -> Markdown data-dictionary tables
  copybook.py json <file.cpy>...   -> JSON layout (consumed by the Rust generator)
"""
import json, re, sys


def statements(path):
    buf = []
    for raw in open(path, errors="replace"):
        line = raw.rstrip("\n")
        if len(line) < 7 or line[6] in "*/":
            continue
        buf.append(line[7:72])
    text = " ".join(buf)
    # split on periods that end a statement (not inside quotes / numeric pics)
    out, cur, q = [], "", None
    for i, ch in enumerate(text):
        if q:
            cur += ch
            if ch == q:
                q = None
            continue
        if ch in "'\"":
            q = ch
            cur += ch
            continue
        if ch == "." and (i + 1 == len(text) or text[i + 1] == " "):
            out.append(cur.strip())
            cur = ""
            continue
        cur += ch
    return [s for s in out if s]


def pic_info(pic):
    """Return (display_length, digits, scale, signed, kind)."""
    p = pic.upper()
    signed = p.startswith("S")
    p = p.lstrip("S")
    exp = re.sub(r"(.)\((\d+)\)", lambda m: m.group(1) * int(m.group(2)), p)
    if set(exp) <= set("X"):
        return len(exp), 0, 0, False, "X"
    if set(exp) <= set("9V"):
        digits = exp.count("9")
        scale = len(exp.split("V")[1]) if "V" in exp else 0
        return digits, digits, scale, signed, "9"
    # numeric-edited
    length = len(exp.replace("V", ""))
    scale = len(re.split(r"[.V]", exp)[1]) if re.search(r"[.V]", exp) else 0
    return length, sum(exp.count(c) for c in "9Z*"), scale, False, "E"


def storage_len(disp, digits, usage):
    if usage in ("COMP-3", "PACKED-DECIMAL"):
        return digits // 2 + 1
    if usage in ("COMP", "BINARY", "COMP-4", "COMP-5"):
        return 2 if digits <= 4 else 4 if digits <= 9 else 8
    return disp


def parse(path):
    items = []
    for st in statements(path):
        m = re.match(r"(\d\d?)\s+([A-Z0-9-]+)\s*(.*)$", st, re.S)
        if not m:
            continue
        lvl, name, rest = int(m.group(1)), m.group(2), m.group(3)
        if lvl == 88:
            continue
        it = dict(level=lvl, name=name, pic=None, usage="DISPLAY", occurs=1,
                  redefines=None, value=None)
        r = re.search(r"REDEFINES\s+([A-Z0-9-]+)", rest)
        if r:
            it["redefines"] = r.group(1)
        r = re.search(r"PIC(?:TURE)?\s+(?:IS\s+)?(\S+)", rest)
        if r:
            it["pic"] = r.group(1)
        r = re.search(r"\b(COMP-3|COMP-4|COMP-5|COMP|BINARY|PACKED-DECIMAL)\b", rest)
        if r:
            it["usage"] = r.group(1)
        r = re.search(r"OCCURS\s+(\d+)", rest)
        if r:
            it["occurs"] = int(r.group(1))
        r = re.search(r"VALUE\s+(.+)$", rest)
        if r:
            it["value"] = r.group(1).strip()
        items.append(it)
    # build tree
    root = dict(level=0, name="<root>", children=[], occurs=1, usage="DISPLAY",
                pic=None, redefines=None)
    stack = [root]
    for it in items:
        it["children"] = []
        while stack[-1]["level"] >= it["level"]:
            stack.pop()
        stack[-1]["children"].append(it)
        stack.append(it)

    def size(node, inherited_usage="DISPLAY"):
        usage = node["usage"] if node["usage"] != "DISPLAY" else inherited_usage
        node["usage"] = usage
        if node["pic"]:
            disp, digits, scale, signed, kind = pic_info(node["pic"])
            node.update(digits=digits, scale=scale, signed=signed, kind=kind,
                        disp_len=disp)
            node["len"] = storage_len(disp, digits, usage) if kind == "9" else disp
        else:
            total, offsets = 0, {}
            for ch in node["children"]:
                size(ch, usage)
                if ch["redefines"]:
                    continue
                total += ch["len"] * ch["occurs"]
            node["len"] = total
        return node["len"]

    size(root)

    def assign(node, off, path):
        node["offset"] = off
        cur = off
        named = {}
        for ch in node["children"]:
            if ch["redefines"]:
                start = named[ch["redefines"]]
            else:
                start = cur
                cur += ch["len"] * ch["occurs"]
            named[ch["name"]] = start
            assign(ch, start, path + [node["name"]])

    assign(root, 0, [])
    return root


def flatten(node, depth=0, out=None):
    out = [] if out is None else out
    for ch in node["children"]:
        out.append((depth, ch))
        flatten(ch, depth + 1, out)
    return out


def describe(n):
    if not n.get("pic"):
        return "group"
    t = n["pic"]
    if n["usage"] != "DISPLAY":
        t += " " + n["usage"]
    return t


def md(paths):
    for p in paths:
        root = parse(p)
        for rec in root["children"]:
            print(f"\n### {rec['name']} — `{p.split('/')[-1]}` ({rec['len']} bytes)\n")
            print("| Offset | Len | Field | Picture | Notes |")
            print("|---:|---:|---|---|---|")
            for depth, n in flatten(rec):
                notes = []
                if n["occurs"] > 1:
                    notes.append(f"OCCURS {n['occurs']}")
                if n["redefines"]:
                    notes.append(f"REDEFINES {n['redefines']}")
                name = "&nbsp;" * (2 * depth) + n["name"]
                print(f"| {n['offset'] - rec['offset'] + 1} | {n['len']} | {name} | {describe(n)} | {', '.join(notes)} |")


def to_json(paths):
    res = {}
    for p in paths:
        root = parse(p)
        for rec in root["children"]:
            fields = []
            for depth, n in flatten(rec):
                fields.append({k: n.get(k) for k in (
                    "name", "level", "offset", "len", "pic", "usage", "occurs",
                    "redefines", "digits", "scale", "signed", "kind", "value")} | {"depth": depth,
                    "group": not n.get("pic")})
            if not fields:  # elementary 01-level item
                fields = [{k: rec.get(k) for k in ("name", "level", "offset", "len", "pic", "usage", "occurs",
                          "redefines", "digits", "scale", "signed", "kind", "value")} | {"depth": 0, "group": False}]
            for f in fields:
                f["offset"] -= rec["offset"]
            res[rec["name"]] = {"file": p, "len": rec["len"], "fields": fields}
    json.dump(res, sys.stdout, indent=1)


if __name__ == "__main__":
    {"md": md, "json": to_json}[sys.argv[1]](sys.argv[2:])
