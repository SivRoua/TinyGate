"""Package standalone node sources; do not add them to TinyGate's Git repository."""
import argparse
import hashlib
import json
import zipfile
from pathlib import Path


def main():
    root = Path(__file__).resolve().parents[1]
    p = argparse.ArgumentParser()
    p.add_argument("--source", type=Path, default=root.parent / "TinyGate-ZigbeeNode")
    p.add_argument("--output", type=Path, default=root / "test-results/TinyGate-ZigbeeNode.zip")
    args = p.parse_args()
    source = args.source.resolve()
    if not (source / "main/zigbee_node.c").is_file():
        p.error("Node source not found")
    root_files = {"CMakeLists.txt", "README.md", "sdkconfig.defaults", "partitions.csv",
                  "dependencies.lock", "build.ps1"}
    files = []
    for f in source.rglob("*"):
        if not f.is_file():
            continue
        relative = f.relative_to(source)
        if any(part.startswith(".") or part in {"build", "managed_components", "__pycache__"}
               for part in relative.parts):
            continue
        is_source = relative.parts[0] in {"main", "tests"} and (
            f.suffix in {".c", ".h", ".py", ".yml", ".txt"} or f.name == "Kconfig.projbuild")
        if relative.as_posix() in root_files or is_source:
            files.append(f)
    args.output.parent.mkdir(parents=True, exist_ok=True)
    hashes = {f.relative_to(source).as_posix(): hashlib.sha256(f.read_bytes()).hexdigest() for f in files}
    with zipfile.ZipFile(args.output, "w", zipfile.ZIP_DEFLATED) as archive:
        for f in files:
            archive.write(f, "TinyGate-ZigbeeNode/" + f.relative_to(source).as_posix())
        archive.writestr("TinyGate-ZigbeeNode/SHA256.json", json.dumps(hashes, indent=2))
    print(args.output)


if __name__ == "__main__":
    main()
