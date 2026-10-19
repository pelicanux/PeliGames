"""PeliGames' per-invocation Epic adapter. Never log game arguments or environment values."""
import json
import hashlib
import os
import sys
import time
from pathlib import Path

MARKER = b"# PeliGames Epic runtime adapter v1"
root = Path(__file__).resolve().parent
original, *arguments = sys.argv[1:]


def read_routes():
    fd = os.open(root / "routes.json", os.O_RDONLY | os.O_NOFOLLOW)
    with os.fdopen(fd) as stream:
        if os.fstat(stream.fileno()).st_size > 4 * 1024 * 1024:
            raise ValueError("routes too large")
        return json.load(stream)


def record(game, runner, status):
    # Bounded diagnostic output: no launch arguments, tokens or variable values.
    try:
        path = root / "launch.jsonl"
        if path.exists() and path.stat().st_size > 1024 * 1024:
            os.replace(path, root / "launch.previous.jsonl")
        fd = os.open(path, os.O_WRONLY | os.O_APPEND | os.O_CREAT | os.O_NOFOLLOW, 0o600)
        with os.fdopen(fd, "a") as stream:
            stream.write(json.dumps({"time": time.time(), "pid": os.getpid(), "game": game,
                                     "runner": runner, "status": status}) + "\n")
    except OSError:
        pass


def canonical(value):
    return os.path.realpath(value) if value else ""


def original_runner(path):
    # A selected Epic runtime may itself have our shim: use its original script.
    candidate = Path(path)
    with candidate.open("rb") as stream:
        header = stream.read(1024)
    if MARKER in header:
        manifest = json.loads(candidate.with_name(".peligames-runtime.json").read_text())
        backup = manifest["backup"]
        if Path(backup).name != backup or not backup.startswith("proton.peligames-original-"):
            raise ValueError("invalid original runtime")
        candidate = candidate.parent / backup
        if candidate.is_symlink() or candidate.stat().st_size > 4 * 1024 * 1024:
            raise ValueError("invalid original runtime")
        if hashlib.sha256(candidate.read_bytes()).hexdigest() != manifest["original_sha256"]:
            raise ValueError("original runtime integrity mismatch")
    if not candidate.is_file() or not os.access(candidate, os.X_OK):
        raise OSError("runner missing")
    return str(candidate)


try:
    routes = read_routes()
    data = canonical(os.environ.get("STEAM_COMPAT_DATA_PATH"))
    prefix = canonical(os.environ.get("WINEPREFIX"))
    executable = canonical(arguments[1]) if len(arguments) > 1 else ""
    matches = [route for route in routes.values()
               if ((data and data == route["compat"]) or (prefix and prefix == route["prefix"]))
               and (not route.get("executable") or executable == route["executable"])]
    route = matches[0] if len(matches) == 1 else None
except (OSError, ValueError, TypeError, KeyError, AttributeError):
    route = None

if route is None:
    os.execv(original, [original, *arguments])

runner = original
status = "epic-default"
if route.get("proton"):
    try:
        runner = original_runner(route["proton"])
        status = "selected"
    except (OSError, ValueError, KeyError):
        status = "selected-unavailable-using-epic"

environment = os.environ.copy()
for key, value in route["environment"].items():
    if key in ("WINEPREFIX", "STEAM_COMPAT_DATA_PATH"):
        continue
    if value is None:
        environment.pop(key, None)
    else:
        environment[key] = value
# Drop only the obsolete adapter produced by older PeliGames versions.
cleaned = []
i = 0
while i < len(arguments):
    if arguments[i] == route.get("legacy_adapter"):
        i += 1
        if i < len(arguments) and arguments[i] == "%command%":
            i += 1
        continue
    cleaned.append(arguments[i])
    i += 1
command = [runner if part == "__PELIGAMES_EPIC_PROTON__" else part for part in route["command"]]
command.extend(cleaned)
command.extend(route["arguments"])
record(route["id"], runner, status)
os.execvpe(command[0], command, environment)
