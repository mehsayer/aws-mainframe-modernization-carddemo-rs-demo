#!/usr/bin/env python3
"""Summarise Claude Code session transcripts for this project.

Usage: usage.py [--json] [--window N]

Reports per session: start/end time, wall-clock, active time (gaps > 10 min
excluded), assistant turns, tool calls, token totals (input, cache write,
cache read, output) and the current context-window fill of the latest turn.
"""
import json, sys, glob, os, datetime as dt

PROJ = os.path.expanduser(
    "~/.claude/projects/-home-mehsayer-src-aws-mainframe-modernization-carddemo-rs-demo")
IDLE = 600  # seconds; gaps longer than this are not counted as active time


def parse_ts(s):
    return dt.datetime.fromisoformat(s.replace("Z", "+00:00"))


def session_stats(path):
    st = dict(session=os.path.basename(path)[:-6], turns=0, tools=0,
              input=0, cache_write=0, cache_read=0, output=0, last_ctx=0)
    times, seen = [], set()
    with open(path) as f:
        for line in f:
            try:
                rec = json.loads(line)
            except ValueError:
                continue
            ts = rec.get("timestamp")
            if ts:
                times.append(parse_ts(ts))
            msg = rec.get("message") or {}
            if rec.get("type") != "assistant" or not isinstance(msg, dict):
                continue
            mid = msg.get("id")
            content = msg.get("content") or []
            st["tools"] += sum(1 for c in content
                               if isinstance(c, dict) and c.get("type") == "tool_use")
            if mid in seen:  # one API message may be split into several records
                continue
            seen.add(mid)
            u = msg.get("usage") or {}
            st["turns"] += 1
            st["input"] += u.get("input_tokens", 0)
            st["cache_write"] += u.get("cache_creation_input_tokens", 0)
            st["cache_read"] += u.get("cache_read_input_tokens", 0)
            st["output"] += u.get("output_tokens", 0)
            st["last_ctx"] = (u.get("input_tokens", 0) + u.get("cache_creation_input_tokens", 0)
                              + u.get("cache_read_input_tokens", 0))
    if times:
        times.sort()
        st["start"], st["end"] = times[0].isoformat(), times[-1].isoformat()
        st["wall_s"] = (times[-1] - times[0]).total_seconds()
        st["active_s"] = sum(min((b - a).total_seconds(), IDLE)
                             for a, b in zip(times, times[1:])
                             if (b - a).total_seconds() <= IDLE)
    return st


def main():
    window = 1_000_000
    if "--window" in sys.argv:
        window = int(sys.argv[sys.argv.index("--window") + 1])
    stats = [session_stats(p) for p in sorted(glob.glob(f"{PROJ}/*.jsonl"),
                                              key=os.path.getmtime)]
    if "--json" in sys.argv:
        json.dump(stats, sys.stdout, indent=2)
        return
    tot = {k: sum(s.get(k, 0) for s in stats)
           for k in ("turns", "tools", "input", "cache_write", "cache_read", "output",
                     "wall_s", "active_s")}
    for s in stats:
        print(f"{s['session'][:8]} {s.get('start','?')[:16]}  active {s.get('active_s',0)/3600:5.2f}h "
              f"turns {s['turns']:4d} tools {s['tools']:4d} out {s['output']:>9,} "
              f"in+cache {s['input']+s['cache_write']+s['cache_read']:>12,}")
    print(f"TOTAL active {tot['active_s']/3600:.2f}h wall {tot['wall_s']/3600:.2f}h turns {tot['turns']} "
          f"tools {tot['tools']} output {tot['output']:,} input {tot['input']:,} "
          f"cache_write {tot['cache_write']:,} cache_read {tot['cache_read']:,}")
    cur = stats[-1]
    print(f"CURRENT context fill: {cur['last_ctx']:,} tokens "
          f"({100*cur['last_ctx']/window:.1f}% of {window:,})")


if __name__ == "__main__":
    main()
