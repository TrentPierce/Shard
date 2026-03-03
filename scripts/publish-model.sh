#!/usr/bin/env bash
# Usage: ./scripts/publish-model.sh <model-file> <model-id> <version> <s3-bucket>
# Hashes, uploads to S3, and updates manifest.json

set -euo pipefail

MODEL_FILE="$1"
MODEL_ID="$2"
VERSION="$3"
S3_BUCKET="$4"
S3_KEY="models/${MODEL_ID}/${VERSION}/$(basename "$MODEL_FILE")"

echo "Computing SHA256..."
SHA256=$(sha256sum "$MODEL_FILE" | cut -d' ' -f1)
SIZE_BYTES=$(stat -c%s "$MODEL_FILE" 2>/dev/null || stat -f%z "$MODEL_FILE")

echo "Uploading to s3://${S3_BUCKET}/${S3_KEY}..."
aws s3 cp "$MODEL_FILE" "s3://${S3_BUCKET}/${S3_KEY}" --no-progress

DOWNLOAD_URL="https://${S3_BUCKET}.s3.amazonaws.com/${S3_KEY}"

echo "Updating manifest.json..."
python3 - <<PY
import json
from pathlib import Path

manifest_path = Path("deploy/models/manifest.json")
manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
for model in manifest.get("models", []):
    if model.get("id") == "$MODEL_ID":
        model["version"] = "$VERSION"
        model["sha256"] = "$SHA256"
        model["size_bytes"] = $SIZE_BYTES
        model["download_url"] = "$DOWNLOAD_URL"
        break
else:
    manifest.setdefault("models", []).append({
        "id": "$MODEL_ID",
        "display_name": "$MODEL_ID",
        "version": "$VERSION",
        "sha256": "$SHA256",
        "size_bytes": $SIZE_BYTES,
        "download_url": "$DOWNLOAD_URL",
        "min_vram_gb": 4,
        "min_ram_gb": 8,
        "roles": ["shard"],
        "quantization": "1.58bit",
        "architecture": "bitnet",
        "release_notes": "Auto-published"
    })
manifest_path.write_text(json.dumps(manifest, indent=2) + "\n", encoding="utf-8")
print("Manifest updated.")
PY

echo "Done. SHA256: $SHA256"
echo "Download URL: $DOWNLOAD_URL"
