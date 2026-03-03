#!/usr/bin/env bash
# Usage: ./scripts/check-bootstrap-health.sh [config-file]
# Reads bootstrap-ring.yaml, curls each node's /health endpoint, reports status.

set -euo pipefail

CONFIG="${1:-deploy/config/bootstrap-ring.yaml}"
HEALTHY=0
TOTAL=0

if [[ ! -f "$CONFIG" ]]; then
  echo "Config not found: $CONFIG" >&2
  exit 1
fi

HEALTH_LINES=$(python3 - <<PY
import re
from pathlib import Path

text = Path("$CONFIG").read_text(encoding="utf-8")
region = None
for line in text.splitlines():
    stripped = line.strip()
    m_region = re.match(r"region:\s*(.+)$", stripped)
    if m_region:
        region = m_region.group(1).strip().strip('"')
    m_health = re.match(r"health_url:\s*(.+)$", stripped)
    if m_health:
        url = m_health.group(1).strip().strip('"')
        print(f"{url} {region or 'unknown'}")
PY
)

while IFS=' ' read -r url region; do
    [[ -z "${url:-}" ]] && continue
    TOTAL=$((TOTAL + 1))
    STATUS=$(curl -s -o /dev/null -w "%{http_code}" --connect-timeout 5 "$url" || echo "000")
    if [ "$STATUS" = "200" ]; then
        echo "[OK] $region ($url) - HEALTHY"
        HEALTHY=$((HEALTHY + 1))
    else
        echo "[FAIL] $region ($url) - UNHEALTHY (HTTP $STATUS)"
    fi
done <<< "$HEALTH_LINES"

echo ""
echo "Healthy: $HEALTHY/$TOTAL"

MIN_CONNECTED=$(python3 - <<PY
import re
from pathlib import Path
text = Path("$CONFIG").read_text(encoding="utf-8")
m = re.search(r"^min_connected_bootstrap:\s*(\d+)", text, re.MULTILINE)
print(m.group(1) if m else "1")
PY
)

if (( HEALTHY < MIN_CONNECTED )); then
  echo "Bootstrap ring below min_connected_bootstrap=$MIN_CONNECTED"
  exit 1
fi

echo "Bootstrap ring meets connectivity threshold"
