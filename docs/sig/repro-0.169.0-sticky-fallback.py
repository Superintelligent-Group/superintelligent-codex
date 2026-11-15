"""Source-bound policy reproduction; does not replace core network integration tests.
Extracts both transport-policy methods verbatim from the immutable target tag, then runs
those methods with inert provider/telemetry/socket stubs. No HTTP or model calls occur.
Usage: python docs/sig/repro-0.169.0-sticky-fallback.py <output-directory>
Requires rustc and the Windows MSVC environment (or an ordinary Unix Rust toolchain).
"""
import hashlib
from pathlib import Path
import subprocess
import sys

BASE = "d32063f96c2dff1dc96af839b3952abb6acfeda4"
source = subprocess.check_output([
    "git", "show", BASE + ":codex-rs/core/src/client.rs"
]).decode("utf-8")

def method(signature):
    start = source.index(signature)
    brace = source.index("{", start)
    depth = 1
    end = brace + 1
    while depth:
        depth += (source[end] == "{") - (source[end] == "}")
        end += 1
    return source[start:end]

methods = "\n".join(method(signature) for signature in [
    "pub(crate) fn force_http_fallback(",
    "pub fn responses_websocket_enabled(",
])
program = r'''
use std::sync::{Arc, atomic::{AtomicBool, Ordering}};
macro_rules! warn { ($($x:tt)*) => {}; }
struct SessionTelemetry;
impl SessionTelemetry { fn counter(&self, _: &str, _: u64, _: &[(&str, &str)]) {} }
struct ModelInfo;
#[derive(Default)] struct WebsocketSession;
struct Provider { supports_websockets: bool }
impl Provider { fn info(&self) -> &Self { self } }
struct State { provider: Provider, disable_websockets: AtomicBool }
#[derive(Clone)] struct ModelClient { state: Arc<State> }
impl ModelClient {
    fn store_cached_websocket_session(&self, _: WebsocketSession) {}
    METHODS
}
#[test]
fn upstream_fallback_survives_the_next_turn() {
    let client = ModelClient { state: Arc::new(State {
        provider: Provider { supports_websockets: true },
        disable_websockets: AtomicBool::new(false),
    }) };
    assert!(client.responses_websocket_enabled());
    assert!(client.force_http_fallback(&SessionTelemetry, &ModelInfo));
    // Actual ModelClient clones share session state across turn-scoped sessions.
    let next_turn_client = client.clone();
    assert!(!next_turn_client.responses_websocket_enabled());
    assert!(!next_turn_client.force_http_fallback(&SessionTelemetry, &ModelInfo));
}
'''.replace("METHODS", methods)
out = Path(sys.argv[1]).resolve()
out.mkdir(parents=True, exist_ok=True)
rust = out / "upstream-sticky-fallback.rs"
binary = out / ("upstream-sticky-fallback.exe" if sys.platform == "win32" else "upstream-sticky-fallback")
rust.write_text(program, encoding="utf-8")
print("base=" + BASE, "methods_sha256=" + hashlib.sha256(methods.encode()).hexdigest(), flush=True)
subprocess.run(["rustc", "--edition=2024", "--test", str(rust), "-o", str(binary)], check=True)
subprocess.run([str(binary), "--nocapture"], check=True)
