"""Render local Mermaid bytes with a bounded, serial WASI client."""

import argparse
import json
from pathlib import Path
import sys

from client import Client, FUEL, GuestError


def write_json(path, value):
    Path(path).write_text(json.dumps(value, indent=2) + "\n", encoding="utf-8")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--module", required=True, type=Path)
    parser.add_argument("--source", required=True, type=Path)
    parser.add_argument("--operation", required=True, choices=("svg", "png"))
    parser.add_argument("--options", type=Path, help="request-options JSON object")
    parser.add_argument("--font", action="append", default=[], type=Path)
    parser.add_argument("--output", required=True, type=Path)
    parser.add_argument("--metadata", type=Path, help="write operation metadata JSON")
    parser.add_argument("--catalog", type=Path, help="write runtime catalog JSON")
    parser.add_argument("--fuel", type=int, default=FUEL, help="positive instruction budget per guest call")
    args = parser.parse_args()
    try:
        options = json.loads(args.options.read_text(encoding="utf-8")) if args.options else None
        with Client(args.module.read_bytes(), fonts=[p.read_bytes() for p in args.font], fuel=args.fuel) as client:
            if args.catalog:
                write_json(args.catalog, client.catalog())
            data, metadata = client.execute(args.operation, args.source.read_bytes(), options)
            args.output.write_bytes(data)
            if args.metadata:
                write_json(args.metadata, metadata)
    except GuestError as error:
        print(json.dumps(error.payload), file=sys.stderr)
        return 1
    except Exception as error:
        print(f"{type(error).__name__}: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
