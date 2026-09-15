---
name: asset-fetch-failure-handling
description: What happens when a WASM asset fetch fails (no retry anywhere; failed .scene.ron = permanent LoadingScene hang), the exact Bevy 0.18 primitives that make a fix cheap, and why serve.py dev-server failures do NOT generalize to GitHub Pages
metadata:
  type: project
---

Verified 2026-09-15 while triaging a burst of `ERR_CONNECTION_REFUSED` + `Failed to fetch path`
errors Frank saw during a local playtest.

## The engine has zero retry, and one failure class is a hard hang

- Nothing in `ironhold_core` reads `UntypedAssetLoadFailedEvent`. `AssetPlugin` is configured with
  only `file_path` + `meta_check` (`lib.rs`).
- Only three `LoadState::Failed` read sites exist: `project_loader.rs` (8 arms, catalogs/rules/FSM),
  `entity_spawner.rs` (pending behavior), `dialogue.rs`. All log-and-degrade; **none retries**.
  Ordinary scene assets (GLB, textures, audio, preloaded `.scene.ron`) have no Failed handling at all.
- **Severity is not uniform.** A failed texture/GLB = missing visual for the session (degraded).
  A failed `.scene.ron` = *unrecoverable hang*: `scene_loader.rs::spawn_scene_v2` gates on
  `AssetEvent::is_loaded_with_dependencies` with a per-frame fallback of
  `params.scenes.get(handle).is_some()`; if the RON itself failed, neither ever becomes true, so the
  system returns early forever and `AppState` sits in `LoadingScene`/`LoadingProject`. No timeout
  exists. With the loading-screen overlay still unbuilt (backlog "Beta 0.7a"), the user sees a black
  screen and a console error.
- **A failed `Action::PreloadScene` self-heals**, though — see the retry primitive below: the later
  real `LoadScene` re-kicks the load.

## Bevy 0.18 primitives that make a retry cheap (all verified in the vendored crate)

- `AssetInfos::get_or_create_path_handle_internal` (`bevy_asset/src/server/info.rs` ~235-243):
  under `HandleLoadingMode::Request`, `should_load = true` when state is `NotLoaded | Failed(_)`.
  **So a plain repeat `asset_server.load(path)` already retries a failed asset**, same `AssetId`, so
  every existing handle holder picks it up. `AssetServer::reload(path)` (`server/mod.rs` ~879) is the
  untyped path-based forcing variant.
- `UntypedAssetLoadFailedEvent` is registered globally (`bevy_asset/src/lib.rs` ~421) and carries
  `id`/`path`/`error` — **one generic system, no per-asset-type registration needed**.
- Failure classes are cleanly distinguishable, which is what makes a *safe* policy possible
  (`bevy_asset/src/io/wasm.rs` ~78-90): 403/404 → `AssetReaderError::NotFound` (designer typo —
  never retry; `ironhold validate` / `asset_checker.py` own that); other status → `HttpError(status)`
  (retry 429/5xx only); network-level fetch rejection → `AssetReaderError::Io` (retry — this is the
  transient class). Loader/RON-parse errors are a separate variant — never retry.
- Bevy's own log line buries the path (`Failed to fetch path: {js_str}`); our own retry system would
  incidentally fix that by logging `UntypedAssetLoadFailedEvent.path`.

## serve.py failures do not generalize to production — don't let them drive engine changes

`serve.py` is `socketserver.TCPServer` (single-threaded, `request_queue_size = 5`) wrapping
`SimpleHTTPRequestHandler`, whose `protocol_version` defaults to **HTTP/1.0** — i.e. no keep-alive,
one fresh TCP connection per asset. Chrome's 6-concurrent-per-origin HTTP/1.x cap is *exactly* the
server's capacity (1 accepted + 5 queued): zero margin. Windows RSTs a SYN when the accept queue is
full (Linux silently drops and the client retransmits), so this surfaces as `ERR_CONNECTION_REFUSED`
and is a Windows-dev-only symptom. `test_web.py` launches `serve.py` as a subprocess on the *same*
port 8000, so a killed/competing test server is the other candidate cause of a refusal burst.

Also: `serve.py`'s `httpd.allow_reuse_address = True` runs *after* `TCPServer.__init__` already called
`server_bind()` — a **no-op**. `socketserver.TCPServer.allow_reuse_address` defaults to `False`
(unlike `http.server.HTTPServer`, which sets `1`), so SO_REUSEADDR is never set.

**GitHub Pages serves the same static files over HTTP/2 via a CDN** — all asset fetches multiplex
over one TCP connection, so the entire connection-count/accept-queue failure class is structurally
absent there. When triaging a browser-only asset error, first establish whether the mechanism is
TCP-layer (dev-server-only) or HTTP/network-layer (reproducible in production).

Related: [[wasm-pitfalls]], [[fragile-modules]].
