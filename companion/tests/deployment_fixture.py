"""Public native-package → Board-staging interoperability fixture. Temporary files only."""
import base64
import hashlib
import json
from pathlib import Path
import shutil
import sys
from tinkercrew_companion.deployment import RuntimeDeployment

root = Path(sys.argv[1]) / "app"
source = Path(__file__).resolve().parents[1]
shutil.copytree(source / "python", root / "python", ignore=shutil.ignore_patterns("__pycache__"))
shutil.copytree(source / "sketch", root / "sketch")
shutil.copy(source / "app.yaml", root / "app.yaml")
with (root / "python/tinkercrew_companion/core.py").open("a") as handle:
    handle.write("\n# older baseline fixture\n")
(root / "data").mkdir(mode=0o700)
(root / "data/identity-sentinel").write_text("untouched")
raw = sys.stdin.buffer.read(2097153)
bundle = json.loads(raw)
store = RuntimeDeployment(root)
job = store.dispatch("begin", {"release": bundle["release"], "contract": bundle["contract"], "sha256": hashlib.sha256(raw).hexdigest(), "size": len(raw)})
for offset in range(0, len(raw), 8192):
    store.dispatch("chunk", {"token": job["token"], "offset": offset, "data": base64.b64encode(raw[offset:offset + 8192]).decode()})
status = store.dispatch("stage", {"token": job["token"]})
print(json.dumps({"phase": status["phase"], "release": status["release"], "credentials": (root / "data/identity-sentinel").read_text()}))
