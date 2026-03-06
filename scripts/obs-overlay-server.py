#!/usr/bin/env python3
import argparse
import json
import subprocess
import threading
import time
from dataclasses import dataclass
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path


HTML_TEMPLATE = """<!doctype html>
<html>
<head>
  <meta charset=\"utf-8\" />
  <meta http-equiv=\"Cache-Control\" content=\"no-cache, no-store, must-revalidate\" />
  <meta http-equiv=\"Pragma\" content=\"no-cache\" />
  <meta http-equiv=\"Expires\" content=\"0\" />
  <style>
    html, body {
      margin: 0;
      padding: 0;
      background: transparent;
      overflow: hidden;
      font-family: Segoe UI, sans-serif;
      font-size: 48px;
      font-weight: 700;
      color: #ffffff;
      text-shadow: 0 0 8px rgba(0,0,0,0.75);
    }
  </style>
</head>
<body id=\"value\">N/A</body>
<script>
async function tick() {
  try {
    const r = await fetch('/api/soul-memory?t=' + Date.now(), { cache: 'no-store' });
    const d = await r.json();
    const el = document.getElementById('value');
    el.textContent = d.stale ? ('[STALE] ' + d.value) : d.value;
    el.style.color = d.stale ? '#ffd24a' : '#ffffff';
  } catch (_) {
    const el = document.getElementById('value');
    el.textContent = '[STALE] N/A';
    el.style.color = '#ffd24a';
  }
}
setInterval(tick, 500);
tick();
</script>
</html>
"""


@dataclass
class State:
    value: str = "N/A"
    stale: bool = True
    last_error: str = "startup"
    lock: threading.Lock = threading.Lock()


def to_windows_path(path: Path) -> str:
    return subprocess.check_output(["wslpath", "-w", str(path)], text=True).strip()


def run_helper_once(helper_win: str, config_win: str) -> tuple[str, bool, str]:
    cmd = [
        "powershell.exe",
        "-NoProfile",
        "-Command",
        f"& '{helper_win}' --config '{config_win}' --once",
    ]

    try:
        out = subprocess.check_output(cmd, text=True, timeout=10).strip()
    except subprocess.CalledProcessError as exc:
        return ("N/A", True, f"helper exit: {exc.returncode}")
    except Exception as exc:  # noqa: BLE001
        return ("N/A", True, f"helper invoke failed: {exc}")

    if not out:
        return ("N/A", True, "empty helper output")

    try:
        obj = json.loads(out)
    except json.JSONDecodeError:
        return ("N/A", True, f"invalid helper JSON: {out[:120]}")

    if obj.get("status") == "ok" and obj.get("value") is not None:
        return (str(obj["value"]), False, "")

    return ("N/A", True, str(obj.get("error") or "unknown read error"))


def poll_loop(state: State, helper_win: str, config_win: str, interval_ms: int) -> None:
    while True:
        value, stale, err = run_helper_once(helper_win, config_win)
        with state.lock:
            if not stale:
                state.value = value
                state.stale = False
                state.last_error = ""
            else:
                state.stale = True
                if err:
                    state.last_error = err
        time.sleep(max(interval_ms, 200) / 1000.0)


def make_handler(state: State):
    class Handler(BaseHTTPRequestHandler):
        def do_GET(self):  # noqa: N802
            if self.path.startswith("/api/soul-memory"):
                with state.lock:
                    payload = {
                        "value": state.value,
                        "stale": state.stale,
                        "error": state.last_error,
                    }
                body = json.dumps(payload).encode("utf-8")
                self.send_response(200)
                self.send_header("Content-Type", "application/json; charset=utf-8")
                self.send_header("Cache-Control", "no-store")
                self.send_header("Content-Length", str(len(body)))
                self.end_headers()
                self.wfile.write(body)
                return

            if self.path == "/" or self.path.startswith("/soul-memory.html"):
                body = HTML_TEMPLATE.encode("utf-8")
                self.send_response(200)
                self.send_header("Content-Type", "text/html; charset=utf-8")
                self.send_header("Cache-Control", "no-store")
                self.send_header("Content-Length", str(len(body)))
                self.end_headers()
                self.wfile.write(body)
                return

            self.send_response(404)
            self.end_headers()

        def log_message(self, format, *args):  # noqa: A003
            return

    return Handler


def main() -> None:
    root = Path(__file__).resolve().parents[1]
    parser = argparse.ArgumentParser(description="OBS Soul Memory overlay server")
    parser.add_argument("--host", default="127.0.0.1")
    parser.add_argument("--port", type=int, default=8937)
    parser.add_argument("--interval-ms", type=int, default=500)
    parser.add_argument(
        "--helper",
        default=str(root / "target/x86_64-pc-windows-msvc/debug/overlay-helper.exe"),
    )
    parser.add_argument("--config", default=str(root / "config/overlay.toml"))
    args = parser.parse_args()

    helper_win = to_windows_path(Path(args.helper))
    config_win = to_windows_path(Path(args.config))

    state = State()
    poller = threading.Thread(
        target=poll_loop,
        args=(state, helper_win, config_win, args.interval_ms),
        daemon=True,
    )
    poller.start()

    server = ThreadingHTTPServer((args.host, args.port), make_handler(state))
    server.serve_forever()


if __name__ == "__main__":
    main()
