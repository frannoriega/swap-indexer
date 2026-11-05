#!/usr/bin/env python3
"""Find module-level dependency cycles in a crate, from `cargo modules` DOT output.

Usage:
    scripts/mod_cycles.sh <package>        # wrapper, see below
    cargo modules dependencies -p <pkg> --lib --no-externs --no-sysroot --no-owns \
        | python3 scripts/mod_cycles.py

Every item (struct, fn, impl method, ...) is collapsed into the module that
contains it, so `Trade::entry -> Trade` becomes `crate -> crate` and is dropped
as a self-edge. What remains are genuine module -> module `uses` edges.

Exit status: 1 if any cycle exists, 0 otherwise (usable in CI).
"""
import re
import sys
from collections import defaultdict

NODE = re.compile(r'^\s*"([^"]+)"\s*\[label="([^|"]+)\|')
EDGE = re.compile(r'^\s*"([^"]+)"\s*->\s*"([^"]+)"')


def parse(src):
    modules, edges = set(), []
    for line in src:
        if m := NODE.match(line):
            path, kind = m.groups()
            # kind looks like "crate", "pub mod", "pub(self) mod", "pub struct", ...
            if kind == "crate" or kind.split()[-1] == "mod":
                modules.add(path)
        elif m := EDGE.match(line):
            edges.append(m.groups())
    return modules, edges


def owner(path, modules):
    """Nearest enclosing module: walk up `a::b::C::method` until a module matches."""
    parts = path.split("::")
    while parts:
        cand = "::".join(parts)
        if cand in modules:
            return cand
        parts.pop()
    return None


def sccs(graph):
    """Tarjan's algorithm, iterative so deep graphs can't blow the stack."""
    index, low, on, stack, out, counter = {}, {}, set(), [], [], [0]
    for root in list(graph):
        if root in index:
            continue
        work = [(root, iter(graph[root]))]
        index[root] = low[root] = counter[0]; counter[0] += 1
        stack.append(root); on.add(root)
        while work:
            v, it = work[-1]
            for w in it:
                if w not in index:
                    index[w] = low[w] = counter[0]; counter[0] += 1
                    stack.append(w); on.add(w)
                    work.append((w, iter(graph[w])))
                    break
                elif w in on:
                    low[v] = min(low[v], index[w])
            else:
                work.pop()
                if work:
                    low[work[-1][0]] = min(low[work[-1][0]], low[v])
                if low[v] == index[v]:
                    comp = []
                    while True:
                        w = stack.pop(); on.discard(w); comp.append(w)
                        if w == v:
                            break
                    out.append(comp)
    return out


def example_cycle(graph, comp):
    """Shortest cycle through the first node of the component (BFS)."""
    members, start = set(comp), sorted(comp)[0]
    prev, queue = {}, [start]
    for v in queue:
        for w in sorted(graph[v]):
            if w not in members:
                continue
            if w == start:
                path = [v]
                while path[-1] != start:
                    path.append(prev[path[-1]])
                return [start] + path[::-1][1:] + [start] if v != start else [start, start]
            if w not in prev:
                prev[w] = v
                queue.append(w)
    return comp


def main():
    modules, raw = parse(sys.stdin)
    graph = defaultdict(set)
    why = defaultdict(set)  # (src_mod, dst_mod) -> {"src_item -> dst_item"}
    for m in modules:
        graph[m]
    for src, dst in raw:
        a, b = owner(src, modules), owner(dst, modules)
        if a and b and a != b:
            graph[a].add(b)
            why[(a, b)].add(f"{src} -> {dst}")

    cycles = [c for c in sccs(graph) if len(c) > 1]
    if not cycles:
        print(f"OK: {len(modules)} modules, no cycles")
        return 0

    for comp in cycles:
        cyc = example_cycle(graph, comp)
        print(f"CYCLE among {len(comp)} modules: {' -> '.join(cyc)}")
        for a, b in zip(cyc, cyc[1:]):
            for item_edge in sorted(why[(a, b)]):
                print(f"    {a} -> {b}   via {item_edge}")
    return 1


if __name__ == "__main__":
    sys.exit(main())
