# Investigation: WebGPU browser testing fails in this sandboxed dev environment

## Symptoms

`test_web.py` (and any ad hoc Playwright script) fails every project with:
```
No available adapters.
panicked at bevy_render-0.18.0/src/renderer/mod.rs:281:36: Unable to find a GPU!
```
when run headless in this session's sandboxed Windows environment — reproduced against
`quick_scene` (untouched, known-good) as well as every project touched by the local co-op work,
confirming it's an environment limitation, not an application regression.

## Findings

1. **Headless mode has no GPU adapter at all.** `pw.chromium.launch(headless=True, ...)` — no
   flag combination tried (`--use-vulkan=swiftshader`, `--use-angle=vulkan`, `--headless=new`,
   `--use-angle=vulkan` together) produced an adapter. No software Vulkan implementation
   (e.g. SwiftShader's `vk_swiftshader.dll`) appears to be registered in this environment.

2. **Headed mode (`headless=False`) does find a real adapter**, but device *creation* then fails:
   ```
   AdapterInfo { ... backend: BrowserWebGpu }
   Device failed at creation.
   RequestDeviceError { ... DynamicLib.Open: dxil.dll Windows Error: 126 ... EnsureDXCLibraries
   ...PlatformFunctionsD3D12.cpp:126 }
   ```
   Root cause: Dawn's default D3D12 backend needs `dxil.dll`/`dxcompiler.dll` (the DirectX Shader
   Compiler) for shader compilation. Checked Playwright's managed Chromium install directly
   (`%LOCALAPPDATA%\ms-playwright\chromium-1187\chrome-win\`) — only the older
   `D3DCompiler_47.dll` is present; `dxil.dll`/`dxcompiler.dll` are genuinely absent from this
   specific Chromium binary. Confirmed this is specific to *Playwright's* bundled Chromium, not
   the whole environment: Frank's own separately-installed browser loads every project
   (including `blank_project`) successfully with the exact same project files and server — full
   pipeline warmup, zero errors — because it's a different, complete Chromium/Edge install with
   the proper DirectX shader-compiler DLLs already present.

3. **Workaround found**: launching with `--use-webgpu-adapter=d3d11` forces Dawn to use the older
   D3D11 backend instead of D3D12, which doesn't need `dxil.dll`. With this flag, headed-mode
   Playwright loads every project tried (`quick_scene`, `blank_project`, `entity_logic_demo`) with
   **zero console errors** — only the benign `AudioContext` autoplay warning and the harmless
   `powerPreference ignored on Windows` notice — and reaches `InGame` correctly per the app's own
   `#debug-state` debug element (frame counter advancing, correct scene path reported).

4. **Screenshot capture is still broken, separately from device creation.** With the `d3d11`
   workaround, `page.screenshot()` (Playwright's CDP-based capture) consistently times out
   (30s+) once WebGPU is actively rendering — reproduced across three different projects, so it's
   not terrain-generation-specific (`quick_scene` has terrain; `blank_project`/`entity_logic_demo`
   don't, same hang either way). `canvas.toDataURL()` as an alternative returns a blank white
   image — WebGPU-backed canvases don't reliably snapshot through that legacy 2D-context API.

5. **Visual confirmation is still unresolved.** Frank directly observed the headed Chromium
   window for `blank_project` and saw a black screen (with the *default*-launch, pre-workaround
   console log — i.e. he compared his own browser against a **default** Chromium launch, not one
   using the `d3d11` workaround flag). Whether the `d3d11`-workaround browser actually renders 3D
   content correctly, or renders but the visual output doesn't reach whatever Frank is viewing
   through (a remote-desktop/screen-share layer, if any, could plausibly not relay
   hardware-accelerated overlay content correctly even when local rendering succeeds), was not
   confirmed before this investigation was parked.

## Root cause

Playwright's managed Chromium build (`chromium-1187` as installed in this environment) ships
without `dxil.dll`/`dxcompiler.dll`, so Dawn's default D3D12 WebGPU backend cannot compile
shaders and device creation fails. This is specific to that Chromium binary in this sandboxed
session, not a fundamental "no GPU" limitation of the host — a separately-installed real browser
on the same machine works with zero issues.

## Next steps (not started — parked in favor of local co-op Stage 3 work)

- Relaunch headed Chromium with `--use-webgpu-adapter=d3d11` and have Frank directly confirm
  whether 3D content renders correctly (not just "no console errors") — this is the one
  remaining unconfirmed fact.
- If visual rendering is confirmed working, investigate the `page.screenshot()` timeout
  separately — try `page.screenshot(caret="hide")`, disabling `service_workers`, or capturing via
  `page.video` recording instead of a still screenshot, since CDP's synchronous screenshot path
  may not cooperate with an actively-presenting WebGPU swapchain in this environment even when
  the render itself is fine.
- If this pans out, consider whether `--use-webgpu-adapter=d3d11` (or an equivalent check) is
  worth adding to `test_web.py`'s `CHROMIUM_ARGS_GL`/`CHROMIUM_ARGS_REAL_GPU` for any environment
  hitting the same missing-DLL failure — but only after confirming it doesn't mask real
  rendering differences from the D3D12 path real users' browsers take.
