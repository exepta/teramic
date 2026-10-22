#!/usr/bin/env python3
"""Resolve sample_cpu.c userspace IP samples using their captured process mappings."""
import argparse
import collections
import json
from pathlib import Path
import struct
import subprocess


def elf_loads(path):
    try:
        with open(path, "rb") as source:
            header = source.read(64)
            if header[:6] != b"\x7fELF\x02\x01":
                return []
            phoff = struct.unpack_from("<Q", header, 32)[0]
            phsize, phnum = struct.unpack_from("<HH", header, 54)
            source.seek(phoff)
            entries = source.read(phsize * phnum)
            result = []
            for offset in range(0, len(entries), phsize):
                kind, flags, file_offset, virtual, _, file_size = struct.unpack_from(
                    "<IIQQQQ", entries, offset
                )
                if kind == 1 and flags & 1:
                    result.append((file_offset, file_size, virtual))
            return result
    except (OSError, struct.error):
        return []


def mappings(path):
    result = []
    for line in path.read_text().splitlines():
        fields = line.split(maxsplit=5)
        if len(fields) < 6 or "x" not in fields[1]:
            continue
        low, high = (int(value, 16) for value in fields[0].split("-"))
        binary = fields[5].removesuffix(" (deleted)")
        result.append((low, high, int(fields[2], 16), binary))
    return result


def decode_json_stream(data):
    decoder = json.JSONDecoder()
    result = []
    while data.strip():
        data = data.lstrip()
        value, consumed = decoder.raw_decode(data)
        result.extend(value if isinstance(value, list) else [value])
        data = data[consumed:]
    return result


def symbolize(addresses, maps):
    resolved = {}
    by_binary = collections.defaultdict(list)
    loads = {}
    for address in addresses:
        for low, high, file_offset, binary in maps:
            if not low <= address < high:
                continue
            offset = address - low + file_offset
            if binary not in loads:
                loads[binary] = elf_loads(binary)
            virtual = next(
                (virtual + offset - start for start, size, virtual in loads[binary]
                 if start <= offset < start + size),
                offset,
            )
            resolved[address] = {
                "module": binary,
                "address": hex(address),
                "elf_address": hex(virtual),
                "inline": [],
            }
            if loads[binary]:
                by_binary[binary].append((address, virtual))
            break
        else:
            resolved[address] = {
                "module": "[unmapped]", "address": hex(address), "inline": []
            }
    for binary, entries in by_binary.items():
        for start in range(0, len(entries), 512):
            batch = entries[start:start + 512]
            command = [
                "llvm-symbolizer", "--output-style=JSON", "--inlines", "--demangle",
                f"--obj={binary}", *(hex(virtual) for _, virtual in batch),
            ]
            completed = subprocess.run(command, capture_output=True, text=True, check=True)
            symbols = decode_json_stream(completed.stdout)
            if len(symbols) != len(batch):
                raise ValueError(f"symbolizer returned {len(symbols)} for {len(batch)} addresses")
            for (address, _), symbol in zip(batch, symbols):
                resolved[address]["inline"] = [
                    {"function": frame["FunctionName"], "file": frame["FileName"],
                     "line": frame["Line"]}
                    for frame in symbol.get("Symbol", []) if frame.get("FunctionName")
                ]
    return resolved


def frame_name(symbol, outer=False):
    inline = symbol["inline"]
    if inline:
        return inline[-1 if outer else 0]["function"]
    return f"{Path(symbol['module']).name}+{symbol.get('elf_address', symbol['address'])}"


def table(title, counts, total, top):
    print(f"\n{title}")
    for key, count in counts.most_common(top):
        label = key if len(key) <= 220 else key[:217] + "..."
        print(f"{count:7d} {100 * count / max(total, 1):6.2f}% {label}")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("prefix", type=Path)
    parser.add_argument("--top", type=int, default=20)
    parser.add_argument("--thread", type=int)
    parser.add_argument("--json", type=Path)
    parser.add_argument("--symbols-from", type=Path, help="Reuse symbols from this capture's previous JSON summary")
    args = parser.parse_args()
    prefix = str(args.prefix)
    names = dict(
        line.split("\t", 1)
        for line in Path(prefix + ".threads.tsv").read_text().splitlines()
    )
    samples = []
    addresses = set()
    for line in Path(prefix + ".samples.tsv").read_text().splitlines():
        if not line or line.startswith("#"):
            continue
        fields = line.split("\t")
        tid = int(fields[0])
        if args.thread is not None and tid != args.thread:
            continue
        stack = [int(value, 16) for value in fields[1:] if int(value, 16)]
        if not stack:
            continue
        stack = [stack[0], *(address - 1 for address in stack[1:])]
        samples.append((tid, stack))
        addresses.update(stack)
    resolved = {}
    if args.symbols_from:
        resolved = {
            int(address, 16): symbol
            for address, symbol in json.loads(args.symbols_from.read_text())["symbols"].items()
            if int(address, 16) in addresses
        }
    resolved.update(symbolize(addresses - resolved.keys(), mappings(Path(prefix + ".maps"))))
    modules = collections.Counter()
    threads = collections.Counter()
    thread_groups = collections.Counter()
    leaves = collections.Counter()
    outer = collections.Counter()
    stacks = collections.Counter()
    thread_functions = collections.defaultdict(collections.Counter)
    thread_group_functions = collections.defaultdict(collections.Counter)
    for tid, stack in samples:
        stack = [stack[0], *(address for address in stack[1:]
                            if resolved[address]["module"] != "[unmapped]")]
        leaf = resolved[stack[0]]
        function = frame_name(leaf, outer=True)
        modules[leaf["module"]] += 1
        threads[f"{tid} {names.get(str(tid), 'unknown')}"] += 1
        thread_groups[names.get(str(tid), "unknown")] += 1
        leaves[frame_name(leaf)] += 1
        outer[function] += 1
        thread_functions[str(tid)][function] += 1
        thread_group_functions[names.get(str(tid), "unknown")][function] += 1
        stacks[";".join(frame_name(resolved[address], outer=True) for address in stack)] += 1
    total = len(samples)
    depth = sum(any(resolved[address]["module"] != "[unmapped]"
                    for address in stack[1:]) for _, stack in samples)
    print(f"CPU samples={total} unique_addresses={len(addresses)} samples_with_caller={depth}")
    print("Percentages are sampled userspace CPU time, not frame time. Caller stacks depend on frame pointers.")
    table("Modules", modules, total, args.top)
    table("Thread groups", thread_groups, total, args.top)
    table("Threads", threads, total, args.top)
    table("Enclosing functions", outer, total, args.top)
    table("Innermost inline functions", leaves, total, args.top)
    table("Available caller stacks", stacks, total, min(args.top, 10))
    for group, count in thread_groups.most_common(4):
        table(f"Functions in thread group {group}", thread_group_functions[group], count, min(args.top, 12))
    if args.json:
        args.json.write_text(json.dumps({
            "samples": total, "samples_with_caller": depth,
            "modules": modules.most_common(), "threads": threads.most_common(),
            "thread_groups": thread_groups.most_common(),
            "enclosing_functions": outer.most_common(), "inline_functions": leaves.most_common(),
            "stacks": stacks.most_common(),
            "thread_functions": {tid: counts.most_common() for tid, counts in thread_functions.items()},
            "thread_group_functions": {name: counts.most_common() for name, counts in thread_group_functions.items()},
            "symbols": {hex(address): value for address, value in resolved.items()},
        }, indent=2) + "\n")


if __name__ == "__main__":
    main()
