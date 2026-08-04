#!/usr/bin/env python3
"""Read-only TR3 tier/descriptor cross-check; never reads tensor payloads."""

import argparse
import hashlib
import json
import os
import re
import struct
import sys


NAME = re.compile(
    r"^model\.layers\.(?P<layer>[0-9]+)\.mlp\.experts\."
    r"(?P<expert>[0-9]+)\.(?P<role>gate_proj|up_proj|down_proj)\."
    r"rank(?P<rank>[0-3])\.trellis$"
)


def parse_args():
    parser = argparse.ArgumentParser()
    parser.add_argument("checkpoint", help="read-only TR3 checkpoint root")
    return parser.parse_args()


def main():
    args = parse_args()
    root = os.path.realpath(args.checkpoint)
    tier_path = os.path.join(root, "tier_bitmap.json")
    with open(tier_path, "rb") as handle:
        tier_bytes = handle.read()
    tier = json.loads(tier_bytes)

    expected_tuples = 0
    observed_tuples = 0
    mismatches = []
    counts = {
        "target_k3": 0,
        "target_k4": 0,
        "draft_k3": 0,
        "draft_k4": 0,
    }

    for layer in range(3, 79):
        shard_path = os.path.join(root, f"model-layer-{layer:03d}.safetensors")
        with open(shard_path, "rb") as shard:
            prefix = shard.read(8)
            if len(prefix) != 8:
                raise RuntimeError(f"short prefix for layer {layer}")
            header_bytes = struct.unpack("<Q", prefix)[0]
            if header_bytes == 0 or header_bytes > 268_435_456:
                raise RuntimeError(f"invalid header length for layer {layer}")
            header_raw = shard.read(header_bytes)
            if len(header_raw) != header_bytes:
                raise RuntimeError(f"short header for layer {layer}")
        header = json.loads(header_raw)

        seen = set()
        if layer < 78:
            widths = tier[str(layer)].get("k")
            if not isinstance(widths, list) or len(widths) != 256:
                raise RuntimeError(f"bad tier k for layer {layer}")
        else:
            if "k" in tier[str(layer)]:
                raise RuntimeError("draft k unexpectedly present")
            widths = [3] * 256

        for tensor_name, descriptor in header.items():
            if tensor_name == "__metadata__":
                continue
            match = NAME.fullmatch(tensor_name)
            if match is None or int(match.group("layer")) != layer:
                continue
            expert = int(match.group("expert"))
            rank = int(match.group("rank"))
            role = match.group("role")
            key = (expert, role, rank)
            if key in seen:
                raise RuntimeError(f"duplicate tuple {layer}:{key}")
            seen.add(key)
            expected_tuples += 1

            shape = descriptor.get("shape")
            dtype = descriptor.get("dtype")
            actual_bits = None
            if (
                dtype == "I16"
                and isinstance(shape, list)
                and len(shape) == 3
                and shape[2] in (48, 64)
            ):
                actual_bits = shape[2] // 16
            expected_bits = widths[expert] if 0 <= expert < 256 else None
            if actual_bits != expected_bits:
                mismatches.append(
                    {
                        "layer": layer,
                        "expert": expert,
                        "role": role,
                        "rank": rank,
                        "expected_bits": expected_bits,
                        "actual_bits": actual_bits,
                    }
                )
            if actual_bits is not None:
                observed_tuples += 1
                domain = "target" if layer < 78 else "draft"
                counts[f"{domain}_k{actual_bits}"] += 1

        required = {
            (expert, role, rank)
            for expert in range(256)
            for role in ("gate_proj", "up_proj", "down_proj")
            for rank in range(4)
        }
        if seen != required:
            missing = sorted(required - seen)
            extra = sorted(seen - required)
            raise RuntimeError(
                f"tuple inventory mismatch layer={layer} "
                f"missing={missing[:4]} extra={extra[:4]}"
            )

    summary = {
        "schema": "glmaxx.tr3-tier-descriptor-crosscheck.diagnostic.v1",
        "tier_sha256": hashlib.sha256(tier_bytes).hexdigest(),
        "layers": 76,
        "expected_tuples": expected_tuples,
        "observed_valid_tuples": observed_tuples,
        "counts": counts,
        "mismatch_count": len(mismatches),
        "first_mismatches": mismatches[:16],
        "claim": (
            "read-only diagnostic over tier bytes plus safetensors "
            "prefixes/headers; no payload or publisher authentication"
        ),
    }
    encoded = json.dumps(summary, sort_keys=True, separators=(",", ":")).encode()
    print(encoded.decode())
    print("summary_sha256=" + hashlib.sha256(encoded).hexdigest(), file=sys.stderr)
    return 2 if mismatches else 0


if __name__ == "__main__":
    raise SystemExit(main())
