"""Read declarations as data; do not import Skill code or install anything."""
import importlib.metadata
import json
import re
import shutil
import subprocess
import sys
import tomllib
from pathlib import Path

data = json.loads(Path("/work/request.json").read_text())
missing, versions = [], {"python": sys.version.split()[0]}
for runtime in data["runtimes"]:
    command = {"python": "python", "shell": "bash", "javascript": "node", "typescript": "node"}[runtime]
    if not shutil.which(command):
        missing.append("interpreter:" + command)
if "typescript" in data["runtimes"] and shutil.which("node"):
    result = subprocess.run(["node", "--experimental-transform-types", "--eval", ""], capture_output=True, timeout=5)
    if result.returncode:
        missing.append("typescript:native-transform-types")

requirements = []
if data["requirements"]:
    for line in data["requirements"].splitlines():
        line = re.sub(r"\s+#.*$", "", line).strip()
        if line and not line.startswith("#"):
            requirements.append(line)
if data["pyproject"]:
    project = tomllib.loads(data["pyproject"]).get("project", {})
    if project.get("dynamic"):
        missing.append("pyproject:dynamic-dependencies-not-verified")
    requirements.extend(project.get("dependencies", []))
    constraint = project.get("requires-python")
    if constraint:
        from packaging.specifiers import SpecifierSet
        if not SpecifierSet(constraint).contains(versions["python"]):
            missing.append("python-version:" + constraint)
if len(requirements) > 128:
    raise ValueError("too many dependencies")
for text in requirements:
    if not isinstance(text, str) or len(text) > 256 or text.startswith("-"):
        raise ValueError("unsupported requirement")
    from packaging.requirements import Requirement
    requirement = Requirement(text)
    if requirement.url:
        missing.append("python:direct-url-not-verified:" + requirement.name)
        continue
    if requirement.marker and not requirement.marker.evaluate():
        continue
    if requirement.extras:
        missing.append("python:extras-not-verified:" + requirement.name)
        continue
    try:
        version = importlib.metadata.version(requirement.name)
        versions["python:" + requirement.name] = version
        if not requirement.specifier.contains(version):
            missing.append("python-version:" + text)
    except importlib.metadata.PackageNotFoundError:
        missing.append("python:" + requirement.name)

if data["package"]:
    package = json.loads(data["package"])
    dependencies = {**package.get("dependencies", {}), **package.get("devDependencies", {})}
    if not isinstance(dependencies, dict) or len(dependencies) > 128:
        raise ValueError("unsupported node dependencies")
    if dependencies and not shutil.which("node"):
        missing.append("node:dependencies-not-verified")
    elif dependencies:
        # Names/constraints are argv data. This trusted source never evaluates
        # dependency strings or calls npm/package lifecycle hooks.
        source = """
const deps = JSON.parse(process.argv[1]);
const missing=[], versions={};
for (const [name, wanted] of Object.entries(deps)) {
  if (!/^(?:@[a-z0-9_.-]+\\/)?[a-z0-9_.-]+$/i.test(name) || typeof wanted !== 'string' || wanted.length>256) throw Error('declaration');
  let value;
  try { value=require(name+'/package.json').version; } catch (_) { missing.push('node:'+name); continue; }
  versions['node:'+name]=value;
  if (wanted !== '*' && wanted !== value) {
    // Conservative until a declared semver verifier is available. Never mark
    // a mismatching/unchecked range ready based only on module presence.
    missing.push('node-version-range-not-verified:'+name);
  }
}
console.log(JSON.stringify({missing,versions}));
"""
        result = subprocess.run(["node", "--eval", source, json.dumps(dependencies)], capture_output=True, text=True, timeout=10)
        if result.returncode:
            raise ValueError("node dependency check failed")
        node = json.loads(result.stdout)
        missing.extend(node["missing"])
        versions.update(node["versions"])

print(json.dumps({"ready": not missing, "missing": missing, "versions": versions}, ensure_ascii=False))
