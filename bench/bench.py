#!/usr/bin/env python3
"""GPUI vs Tauri: the same harness, the same UI, the same scripted run.

    cargo build --release -p backspace -p backspace-tauri
    python3 bench/bench.py [--runs 3] [--out bench/results.json]

Per app and run, two launches against a fresh workspace and the mock model
(bench/mock.py, auto-approve on):
  idle      launch → first frame with data (startup), then 10 s at rest:
            CPU and memory of the whole process tree.
  workload  BACKSPACE_GOAL set, 2x2 terminals: six tickets in two waves run
            to the final deliverable. CPU seconds the UI process tree burns
            (the harness is in-process in both apps, so it is the same
            constant in both totals), peak memory, wall time.

Memory is RSS summed over the tree, and on Linux also PSS (shared pages
split fairly, so WebKit's helper processes are not double counted). On
macOS the WebKit helpers are XPC services, not children; they are matched
by name and start time.
"""

import argparse, json, os, platform, shutil, signal, statistics, subprocess, sys, tempfile, time
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
APPS = {
    "gpui": ROOT / "target/release/backspace",
    "tauri": ROOT / "target/release/backspace-tauri",
}
LINUX = platform.system() == "Linux"
TICK = os.sysconf("SC_CLK_TCK") if LINUX else 100

CONFIG = """
[router]
backend = "heuristic"
jev_endpoint = ""
jev_model = ""
jev_api_key_env = "NONE"
jev_usd_per_mtok = 0.0
main_min_effort = "high"

[orchestrator]
max_parallel_calls = 4
max_subagents = 8
auto_approve = true
bash_timeout_secs = 10

[providers.mock]
kind = "openai"
base_url = "http://127.0.0.1:{port}/v1"
api_key_env = "NONE"

[[models]]
id = "mock-cheap"
provider = "mock"
tier = 1
description = "cheap"
input_usd_per_mtok = 1.0
output_usd_per_mtok = 2.0
max_output_tokens = 1000

[[models]]
id = "mock-strong"
provider = "mock"
tier = 4
description = "strong"
input_usd_per_mtok = 10.0
output_usd_per_mtok = 20.0
max_output_tokens = 1000
efforts = ["low", "medium", "high", "xhigh", "max"]
"""


# ---------------------------------------------------------------- process tree

def linux_tree(pid):
    kids = {}
    for d in os.listdir("/proc"):
        if d.isdigit():
            try:
                stat = open(f"/proc/{d}/stat").read()
                ppid = int(stat[stat.rindex(")") + 2:].split()[1])
                kids.setdefault(ppid, []).append(int(d))
            except (OSError, ValueError):
                pass
    out, todo = [], [pid]
    while todo:
        p = todo.pop()
        out.append(p)
        todo += kids.get(p, [])
    return out


def linux_sample(pids):
    cpu = rss = pss = 0
    for p in pids:
        try:
            f = open(f"/proc/{p}/stat").read()
            fields = f[f.rindex(")") + 2:].split()
            cpu += (int(fields[11]) + int(fields[12])) / TICK
            for line in open(f"/proc/{p}/smaps_rollup"):
                if line.startswith("Rss:"):
                    rss += int(line.split()[1])
                elif line.startswith("Pss:"):
                    pss += int(line.split()[1])
        except OSError:
            pass
    return cpu, rss / 1024, pss / 1024


def mac_sample(pid, since):
    """CPU seconds and RSS of pid, its children, and WebKit XPC helpers born after `since`."""
    out = subprocess.run(["ps", "-A", "-o", "pid=,ppid=,rss=,time=,lstart=,comm="],
                         capture_output=True, text=True).stdout
    rows = []
    for line in out.splitlines():
        parts = line.split(None, 9)
        if len(parts) < 10:
            continue
        p, pp, rss, t = int(parts[0]), int(parts[1]), int(parts[2]), parts[3]
        started = time.mktime(time.strptime(" ".join(parts[4:9]), "%a %b %d %H:%M:%S %Y"))
        rows.append((p, pp, rss, t, started, parts[9]))
    tree = {pid}
    changed = True
    while changed:
        changed = False
        for p, pp, *_ in rows:
            if pp in tree and p not in tree:
                tree.add(p)
                changed = True
    cpu = rss = 0
    for p, pp, r, t, started, comm in rows:
        if p in tree or ("com.apple.WebKit" in comm and started >= since - 1):
            m, s = t.split(":")
            cpu += int(m) * 60 + float(s)
            rss += r
    return cpu, rss / 1024, None


def sample(pid, since):
    return linux_sample(linux_tree(pid)) if LINUX else mac_sample(pid, since)


# ---------------------------------------------------------------- one launch

def launch(app, ws, port, env_extra, tmp):
    ready = tmp / "ready"
    ready.unlink(missing_ok=True)
    env = dict(os.environ, BACKSPACE_READY_FILE=str(ready), **env_extra)
    t0 = time.time()
    proc = subprocess.Popen([str(APPS[app]), str(ws)], env=env,
                            stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
                            start_new_session=True)
    while not ready.exists():
        if proc.poll() is not None:
            raise RuntimeError(f"{app} exited with {proc.returncode}")
        if time.time() - t0 > 60:
            raise RuntimeError(f"{app}: no first frame after 60 s")
        time.sleep(0.005)
    time.sleep(0.05)
    startup_ms = int(ready.read_text()) - int(t0 * 1000)
    return proc, t0, startup_ms


def stop(proc):
    try:
        os.killpg(proc.pid, signal.SIGTERM)
        proc.wait(5)
    except Exception:
        try:
            os.killpg(proc.pid, signal.SIGKILL)
        except Exception:
            pass


def fresh_ws(tmp, port):
    ws = tmp / "ws"
    shutil.rmtree(ws, ignore_errors=True)
    ws.mkdir()
    (ws / "backspace.toml").write_text(CONFIG.format(port=port))
    return ws


def idle_run(app, tmp, port):
    ws = fresh_ws(tmp, port)
    proc, t0, startup = launch(app, ws, port, {}, tmp)
    try:
        time.sleep(5)
        c0, _, _ = sample(proc.pid, t0)
        time.sleep(10)
        c1, rss, pss = sample(proc.pid, t0)
        return {"startup_ms": startup, "idle_cpu_pct": round((c1 - c0) / 10 * 100, 2),
                "idle_rss_mb": round(rss, 1), "idle_pss_mb": pss and round(pss, 1)}
    finally:
        stop(proc)


def work_run(app, tmp, port, done):
    ws = fresh_ws(tmp, port)
    done.unlink(missing_ok=True)
    env = {"BACKSPACE_GOAL": "Build a todo app with an API and a web UI", "BACKSPACE_LAYOUT": "4"}
    proc, t0, _ = launch(app, ws, port, env, tmp)
    try:
        c0, _, _ = sample(proc.pid, t0)
        peak_rss = peak_pss = 0
        start = time.time()
        while not done.exists():
            if time.time() - start > 300:
                raise RuntimeError(f"{app}: workload did not finish in 300 s")
            _, rss, pss = sample(proc.pid, t0)
            peak_rss, peak_pss = max(peak_rss, rss), max(peak_pss, pss or 0)
            time.sleep(0.25)
        wall = int(done.read_text()) / 1000 - start
        time.sleep(2)  # final deliverable lands and renders
        c1, rss, pss = sample(proc.pid, t0)
        return {"work_cpu_s": round(c1 - c0, 2), "work_wall_s": round(wall, 1),
                "work_peak_rss_mb": round(max(peak_rss, rss), 1),
                "work_peak_pss_mb": round(max(peak_pss, pss or 0), 1) if LINUX else None}
    finally:
        stop(proc)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--runs", type=int, default=3)
    ap.add_argument("--port", type=int, default=8777)
    ap.add_argument("--out", default=str(ROOT / "bench/results.json"))
    ap.add_argument("--apps", default="gpui,tauri")
    a = ap.parse_args()
    apps = a.apps.split(",")
    for app in apps:
        if not APPS[app].exists():
            sys.exit(f"missing {APPS[app]}: cargo build --release -p backspace -p backspace-tauri")

    tmp = Path(tempfile.mkdtemp(prefix="bs-bench-"))
    done = tmp / "done"
    mock = subprocess.Popen([sys.executable, str(ROOT / "bench/mock.py")],
                            env=dict(os.environ, MOCK_PORT=str(a.port), DONE_FILE=str(done)))
    time.sleep(0.5)
    results = {"host": {"os": platform.platform(), "machine": platform.machine(),
                        "python": platform.python_version(), "display": os.environ.get("DISPLAY")},
               "binary_mb": {app: round(APPS[app].stat().st_size / 2**20, 1) for app in apps},
               "runs": {app: [] for app in apps}}
    try:
        for i in range(a.runs):
            for app in (apps if i % 2 == 0 else apps[::-1]):  # alternate order
                r = idle_run(app, tmp, a.port)
                r.update(work_run(app, tmp, a.port, done))
                results["runs"][app].append(r)
                print(app, i + 1, r, flush=True)
    finally:
        mock.terminate()
        shutil.rmtree(tmp, ignore_errors=True)

    med = {}
    for app, rs in results["runs"].items():
        med[app] = {k: statistics.median([r[k] for r in rs]) if rs[0][k] is not None else None for k in rs[0]}
    results["median"] = med
    Path(a.out).write_text(json.dumps(results, indent=2))
    keys = list(next(iter(med.values())).keys())
    print(f"\n{'metric':<20}" + "".join(f"{app:>12}" for app in apps))
    for k in ["binary_mb"] + keys:
        vals = [results["binary_mb"][app] if k == "binary_mb" else med[app][k] for app in apps]
        print(f"{k:<20}" + "".join(f"{v if v is not None else '-':>12}" for v in vals))


if __name__ == "__main__":
    main()
