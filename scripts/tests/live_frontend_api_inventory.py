"""Read-only confirmation of missing route inventory against local management.
This is an audit report, not a passing product acceptance test. Never POST/PUT.
"""
import importlib.util
import json
from pathlib import Path
import urllib.error
import urllib.request

ROOT = Path(__file__).resolve().parents[2]


def load(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


local = load("frontend_live_local", ROOT / "scripts/local-service.py")
inventory = load("frontend_live_inventory", ROOT / "scripts/frontend-api-inventory.py")
records = json.loads((local.DATA / "credentials.json").read_text())
token = next(record["token"] for record in records if record["id"] == "admin")
observations = []
for entry in inventory.inventory()["entries"]:
    if entry["status"] != "missing":
        continue
    path = entry["path"].replace("{value}", "inventory-not-an-existing-id")
    request = urllib.request.Request("http://127.0.0.1:9797" + path,
                                     headers={"Authorization": "Bearer " + token})
    try:
        response = urllib.request.urlopen(request, timeout=5)
    except urllib.error.HTTPError as error:
        response = error
    with response:
        value = json.load(response)
        observations.append({"path": entry["path"], "status": response.status,
                             "code": value.get("error", {}).get("code")})
print(json.dumps({"scope": "authenticated_read_only_missing_route_audit",
                  "product_accepted": False,
                  "confirmed_missing": sum(item["code"] == "not_implemented" for item in observations),
                  "observations": observations}, ensure_ascii=False, indent=2))
