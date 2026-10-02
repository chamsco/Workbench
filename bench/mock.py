"""Deterministic OpenAI-compatible model for bench runs.

The main agent plans six tickets in two waves, dispatches them, and ships.
Each worker takes STEPS tool calls (shell output to render, files to write)
with a fixed think time, so both shells receive the identical event stream.
When the main agent submits its final deliverable, DONE_FILE gets a
timestamp (ms) and the run is over.
"""

import json, os, sys, time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

PORT = int(os.environ.get("MOCK_PORT", "8777"))
STEPS = int(os.environ.get("MOCK_STEPS", "12"))
THINK = float(os.environ.get("MOCK_THINK", "0.35"))
DONE = os.environ.get("DONE_FILE", "")
KEYS = ["schema", "auth", "api", "ui", "docs", "tests"]
N = [0]


def call(name, args):
    N[0] += 1
    return {"choices": [{"finish_reason": "tool_calls", "message": {"content": "", "tool_calls": [
        {"id": f"c{N[0]}", "type": "function", "function": {"name": name, "arguments": json.dumps(args)}}]}}],
        "usage": {"prompt_tokens": 4200, "completion_tokens": 380}}


def respond(req):
    m = req["messages"]
    sys_p, first = m[0]["content"], (m[1]["content"] if len(m) > 1 else "")
    tools = [x["content"] for x in m if x["role"] == "tool"]
    last = tools[-1] if tools else None
    if sys_p.startswith("You are the lead agent"):
        if last is None:
            return call("create_tickets", {"tickets": [
                {"key": k, "title": f"Build {k}", "what_to_build": f"The {k} part of the todo app",
                 "acceptance": [f"{k}.txt exists", "tests pass"], "check": f"test -f {k}.txt",
                 "blocked_by": [] if i < 3 else ["schema"]}
                for i, k in enumerate(KEYS)]})
        if "Dispatch" in last:
            return call("work_tickets", {})
        if DONE:
            with open(DONE, "w") as f:
                f.write(str(int(time.time() * 1000)))
        return call("submit_deliverable", {"summary": "Todo app: six tickets merged.", "files": [f"{k}.txt" for k in KEYS]})
    time.sleep(THINK)
    key = next((k for k in KEYS if first.startswith(f"# {k}:")), "x")
    step = len(tools)
    if step < STEPS - 1:
        if step % 3 == 2:
            return call("write", {"path": f"{key}/part{step}.txt", "content": f"{key} {step}\n" * 20})
        return call("bash", {"command": f"seq 1 {40 + step} | sed 's/^/{key} line /'"})
    if step == STEPS - 1:
        return call("write", {"path": f"{key}.txt", "content": f"{key} done\n"})
    return call("submit_deliverable", {"summary": f"{key} built in {STEPS} steps.", "files": [f"{key}.txt"]})


class H(BaseHTTPRequestHandler):
    def do_POST(self):
        body = json.loads(self.rfile.read(int(self.headers["content-length"])))
        out = json.dumps(respond(body)).encode()
        self.send_response(200)
        self.send_header("content-type", "application/json")
        self.send_header("content-length", str(len(out)))
        self.end_headers()
        self.wfile.write(out)

    def log_message(self, *a):
        pass


if __name__ == "__main__":
    ThreadingHTTPServer(("127.0.0.1", PORT), H).serve_forever()
