#!/usr/bin/env python3
"""crap_calc.py — 受限近似 CRAP（bug27 轮既定口径）

输入: llvm-cov --lcov 报告（bin ezr 套件口径）
口径:
- comp = 语法决策点计数 + 1（if/loop/while/for/match/&&/||/?）
- cov = 函数行覆盖（DA 行口径；if-let 收尾 `}` 幻影行按 bug27 判据不计缺口）
- CRAP = comp²(1-cov)³ + comp
- 闭包与外层共享行区间: 同组取极值（comp 取 max，行集取并）
- 函数行区间归属: FN 起始行排序, 后一 FN 起始行 −1 为前 FN 终行
"""

import re
import sys
from collections import defaultdict

LCOVS = sys.argv[1:-1]
SRC_ROOT = sys.argv[-1] if len(sys.argv) > 2 else "/home/z/swarm-ezr2/project/ezr"

# 多份 lcov 合并: 同 SF 的 DA 行取 max hits（覆盖并集），FN/FNDA 去重取极值
raw = {}
for LCOV in LCOVS:
    cur_sf = None
    fns, das = [], []

    def flush():
        global cur_sf, fns, das
        if cur_sf is not None:
            entry = raw.setdefault(cur_sf, {"fn": {}, "da": {}, "fnda": {}})
            for start, name in fns:
                key = (start, name)
                entry["fn"].setdefault(key, start)
            for tag, a, b in das:
                if tag == "DA":
                    entry["da"][a] = max(entry["da"].get(a, 0), b)
                else:
                    entry["fnda"][b] = max(entry["fnda"].get(b, 0), a)
        fns, das = [], []

    for line in open(LCOV, encoding="utf-8", errors="replace"):
        line = line.rstrip("\n")
        if line.startswith("SF:"):
            cur_sf = line[3:]
        elif line.startswith("FN:"):
            ln, name = line[3:].split(",", 1)
            fns.append((int(ln), name))
        elif line.startswith("FNDA:"):
            hits, name = line[5:].split(",", 1)
            das.append(("FNDA", int(hits), name))
        elif line.startswith("DA:"):
            ln, hits = line[3:].split(",")
            das.append(("DA", int(ln), int(hits)))
        elif line.startswith("end_of_record"):
            flush()
    flush()

files = {}
for sf, entry in raw.items():
    fns = sorted((k[0], k[1]) for k in entry["fn"].keys())
    das = [("DA", ln, h) for ln, h in sorted(entry["da"].items())]
    das += [("FNDA", h, name) for name, h in entry.get("fnda", {}).items()]
    files[sf] = (fns, das)

def strip_noise(text: str) -> str:
    text = re.sub(r'"(?:[^"\\]|\\.)*"', '""', text)
    text = re.sub(r"'(?:[^'\\]|\\.)'", "''", text)
    text = re.sub(r"r#*\"(?:[^\"\\]|\\.)*\"#*", '""', text)
    text = re.sub(r"//[^\n]*", "", text)
    return text

DEC = re.compile(r"\b(if|while|for|match|loop)\b")

def comp_of(lines) -> int:
    n = 0
    for t in lines:
        s = strip_noise(t)
        n += len(DEC.findall(s))
        n += s.count("&&") + s.count("||")
        n += s.count("?")
    return n + 1

total_da = 0
total_cov = 0
report = []
for sf, (fns, das) in files.items():
    if "/src/" not in sf:
        continue
    fn_line = {name: start for start, name in fns}
    hits = {name: h for _, h, name in [d for d in das if d[0] == "FNDA"]}
    da = [(ln, h) for tag, ln, h in das if tag == "DA"]
    if not fns or not da:
        continue
    da.sort()
    max_line = da[-1][0]
    da_map = dict(da)
    sorted_fns = sorted(fns)
    ranges = []
    for i, (start, name) in enumerate(sorted_fns):
        end = (sorted_fns[i + 1][0] - 1) if i + 1 < len(sorted_fns) else max_line
        ranges.append([start, end, name])
    merged = []
    for r in ranges:
        if merged and r[0] <= merged[-1][1]:
            merged[-1][1] = max(merged[-1][1], r[1])
            merged[-1].append(r[2])
        else:
            merged.append([r[0], r[1], r[2]])
    src_lines = {}
    try:
        with open(sf, encoding="utf-8", errors="replace") as fh:
            src_lines = {i + 1: l for i, l in enumerate(fh.readlines())}
    except OSError:
        continue
    for grp in merged:
        start, end, names = grp[0], grp[1], grp[2]
        rng_da = [(ln, h) for ln, h in da if start <= ln <= end]
        if not rng_da:
            continue
        covered = sum(1 for _, h in rng_da if h > 0)
        uncov = [(ln, h) for ln, h in rng_da if h == 0]
        phantom = 0
        for ln, _ in uncov:
            txt = src_lines.get(ln, "").strip()
            if txt == "}":
                phantom += 1
                covered += 1
        n_da = len(rng_da)
        cov = covered / n_da if n_da else 1.0
        total_da += n_da
        total_cov += covered
        comp = comp_of([src_lines.get(l, "") for l in range(start, end + 1)])
        crap = comp * comp * (1 - cov) ** 3 + comp
        report.append((crap, comp, cov, n_da, phantom, sf, names[0], start))

report.sort(reverse=True)
print(f"== 行覆盖: {total_cov}/{total_da} = {total_cov / total_da * 100:.1f}%")
print("== CRAP > 6 的逻辑函数（comp, cov, 行数, 幻影, 文件:行, 名）==")
for crap, comp, cov, n_da, ph, sf, name, start in report:
    if crap > 6:
        rel = sf.replace("/home/z/swarm-ezr2/project/ezr/", "")
        flag = "REG(cov=1)" if cov >= 1.0 else "ACTIONABLE"
        print(f"CRAP={crap:8.2f} comp={comp:3d} cov={cov:.3f} lines={n_da:4d} ph={ph} "
              f"[{flag}] {rel}:{start} {name}")
print("== comp>=7 且 cov<1.0 的 CRAP 可修点 ==")
act = [(crap, comp, cov, sf, name, start) for crap, comp, cov, n_da, ph, sf, name, start in report
       if comp >= 7 and cov < 1.0]
for crap, comp, cov, sf, name, start in act:
    rel = sf.replace("/home/z/swarm-ezr2/project/ezr/", "")
    print(f"CRAP={crap:8.2f} comp={comp:3d} cov={cov:.3f} {rel}:{start} {name}")
print(f"ACTIONABLE_COUNT={len(act)}")
