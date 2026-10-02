"""Linux only: CPU seconds per thread for one workload run.

    python3 bench/threads.py gpui|tauri

Shows where each app spends its CPU (e.g. llvmpipe = software GPU driver).
"""
import os, sys, time, subprocess, collections, re, tempfile, shutil
sys.path.insert(0, "bench")
import bench
app = sys.argv[1]
tmp = __import__("pathlib").Path(tempfile.mkdtemp())
done = tmp / "done"
mock = subprocess.Popen([sys.executable, "bench/mock.py"], env=dict(os.environ, MOCK_PORT=os.environ.get("MOCK_PORT", "8778"), DONE_FILE=str(done)))
time.sleep(0.5)
ws = bench.fresh_ws(tmp, int(os.environ.get("MOCK_PORT", "8778")))
proc, t0, _ = bench.launch(app, ws, int(os.environ.get("MOCK_PORT", "8778")), {"BACKSPACE_GOAL": "Build a todo app with an API and a web UI", "BACKSPACE_LAYOUT": "4"}, tmp)
def threads():
    out = collections.Counter()
    for pid in bench.linux_tree(proc.pid):
        try:
            pcomm = open(f"/proc/{pid}/comm").read().strip()
            tasks = os.listdir(f"/proc/{pid}/task")
        except OSError:
            continue
        for t in tasks:
            try:
                st = open(f"/proc/{pid}/task/{t}/stat").read()
                comm = st[st.index("(")+1:st.rindex(")")]
                f = st[st.rindex(")")+2:].split()
                name = re.sub(r"[0-9]+$", "#", comm)
                out[f"{pcomm}:{name}"] += (int(f[11]) + int(f[12])) / bench.TICK
            except OSError: pass
    return out
a = threads()
while not done.exists(): time.sleep(0.25)
time.sleep(2)
b = threads()
bench.stop(proc); mock.terminate()
d = {k: b[k] - a.get(k, 0) for k in b}
tot = sum(d.values())
print(app, "total", round(tot, 1))
for k, v in sorted(d.items(), key=lambda x: -x[1])[:8]:
    print(f"  {k:40} {v:6.1f}s  {v/tot*100:4.0f}%")
