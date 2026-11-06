#!/usr/bin/env python3
"""Bucket transcript token usage and active time by project phase (phase boundaries =
git commit times on branch rust-port). Prints CSV: phase,end,active_min,turns,output_tokens,input_ctx_tokens."""
import json, glob, os, subprocess, datetime as dt
PROJ = os.path.expanduser("~/.claude/projects/-home-mehsayer-src-aws-mainframe-modernization-carddemo-rs-demo")
commits = subprocess.check_output(["git", "log", "--reverse", "--format=%aI|%s", "rust-port", "^main"], text=True).splitlines()
bounds = []
for c in commits:
    t, s = c.split("|", 1)
    if s.startswith("progress log"):
        continue
    bounds.append((dt.datetime.fromisoformat(t), s.split(":")[0]))
msgs, seen = [], set()
for p in glob.glob(f"{PROJ}/*.jsonl"):
    for line in open(p):
        try:
            r = json.loads(line)
        except ValueError:
            continue
        m = r.get("message") or {}
        if r.get("type") != "assistant" or not isinstance(m, dict) or m.get("id") in seen:
            continue
        seen.add(m.get("id"))
        u = m.get("usage") or {}
        t = dt.datetime.fromisoformat(r["timestamp"].replace("Z", "+00:00"))
        msgs.append((t, u.get("output_tokens", 0), u.get("input_tokens", 0) + u.get("cache_read_input_tokens", 0) + u.get("cache_creation_input_tokens", 0)))
msgs.sort()
print("phase,end,active_min,turns,output_tokens,input_ctx_tokens")
start = msgs[0][0]
for i, (end, name) in enumerate(bounds + [(msgs[-1][0] + dt.timedelta(seconds=1), "Docs & deck")]):
    seg = [m for m in msgs if start <= m[0] < end]
    act = sum(min((b[0] - a[0]).total_seconds(), 600) for a, b in zip(seg, seg[1:]) if (b[0] - a[0]).total_seconds() <= 600)
    print(f"{name},{end.astimezone().strftime('%H:%M')},{act/60:.1f},{len(seg)},{sum(s[1] for s in seg)},{sum(s[2] for s in seg)}")
    start = end
