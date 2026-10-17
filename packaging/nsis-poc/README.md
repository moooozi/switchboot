# NSIS custom UI proof of concept

A separate, **safe visual prototype**, not a replacement for production packaging.
It uses **stock NSIS 3** and a small Rust/Win32 Unicode plug-in. NSIS itself is not
modified or recompiled. No Qt, webview, Tauri runtime, or third-party UI plug-in.

The compact light/dark layout follows the existing Inno installer: Segoe UI heading,
application icon, editable rounded destination field, Browse button, and rounded
cyan Install / neutral Cancel actions. Rounded geometry is 4× supersampled, then
composed with native-resolution text in an off-screen buffer. Hover only invalidates
on transitions; captured press/drag behavior remains native. Native edit/button controls retain normal
text editing, tab navigation, mnemonics and focus behavior. High-contrast mode
uses system colors and native button rendering; title-bar customization falls
back to Windows defaults when unavailable.

## Build

Requirements: Windows 10/11, stock NSIS 3 Unicode (`makensis` on PATH or installed
under `Program Files (x86)/NSIS`), Rust with MSVC build tools and the x86 target,
Python (the commands below use `uv`), and the repository's Node/pnpm icon tooling
(`pnpm install --frozen-lockfile`). NSIS's stock installer runtime is x86,
even when an eventual application payload is x64; the plug-in must match it.

From the repository root:

```sh
rustup target add i686-pc-windows-msvc

uv run --no-project python packaging/nsis-poc/build.py --renderer svg
uv run --no-project python packaging/nsis-poc/build.py --renderer raster
```

Outputs: `artifacts/Switchboot_NSIS-preview-svg.exe` and
`artifacts/Switchboot_NSIS-preview-raster.exe`. The default is `--renderer svg`.
The old `Switchboot_NSIS-preview.exe` is not overwritten by these A/B builds.
Native SVG requires Windows 10 1703+; unsupported/failed native rendering falls
back to the embedded ICO without blocking the preview.

Light/dark colors follow Windows' app-theme preference at window creation.
`SWITCHBOOT_NSIS_THEME=light` or `dark` overrides it for one process without
changing system settings. The native SVG/ICO and DPI behavior are shared by both.

A harmless already-installed screen is available separately:

```sh
uv run --no-project python packaging/nsis-poc/build.py --maintenance
uv run --no-project python packaging/nsis-poc/test_maintenance.py
```

`artifacts/Switchboot_NSIS-maintenance-preview.exe` only returns action exit codes;
its Uninstall / Reinstall / Launch buttons never change or launch an installation.
Removal/reinstall has a safe-default confirmation. Escape cancels, not reinstalls.

The plug-in has its own manifest and lockfile. It does not change the application
workspace, versions, release workflows, installed executable, or Inno scripts.
The raster build generates DPI-specific icon frames; the SVG build uses the
existing application ICO for EXE/title-bar icons and embeds the original SVG in
the UI DLL. Both compile, check DLL loading/exports in an x86 process, and package;
they never start Setup. Both load icon group 103 from the running NSIS executable:
there is no duplicate ICO payload or extracted icon file. Close a running
preview before rebuilding its EXE. The MSVC toolchain may require Visual Studio
Build Tools with the C++ workload; adding the Rust target alone is insufficient.

## Preview behavior and safety

Run the EXE normally. **No UAC prompt is requested.**

- The displayed `Program Files\Switchboot` path is **preview-only**. Editing or
  browsing to a directory does not create it or write anything into it.
- Browse uses a native Windows folder picker. Picking a parent appends
  `Switchboot`; cancel preserves the exact typed destination.
- Install checks that the preview path is absolute, then returns control to the
  **NSIS engine**, which extracts only `fixture.txt` into `$PLUGINSDIR`.
- A matching completion window explains the result. Closing Setup lets NSIS
  clean up its temporary plug-in and fixture files.
- Cancel/window close before Install returns exit code 1; successful fixture
  extraction returns 0. Extraction failure returns 2.
- There are **no** firmware calls, application shutdown events, service commands,
  shortcuts, registry writes, uninstallers, network requests or restarts.
- This is an interactive visual fixture: `/S` still shows its custom UI and `/D`
  never changes the extraction destination. It is not updater-compatible yet.

Do not distribute this as a real Switchboot installer.

## Architecture

`setup.nsi` embeds the generated plug-in and invokes `Choose` from `.onInit`.
The plug-in owns a top-level native window and returns an action and Unicode
preview destination through the standard NSIS stack ABI. All windows, subclasses
and GDI resources are released before returning. The hidden/silent NSIS engine
then executes the harmless extraction section and invokes `Complete`.

This is deliberately a two-phase UI feasibility test, **not** a finished progress
or cancellation protocol: the selection window closes before the engine runs,
and the completion window opens afterwards. A real installer needs a designed
progress/error/cancellation bridge rather than simulated progress.

`ui/src/lib.rs` contains presentation and the NSIS ABI adapter. Preview builds
omit the `installer` feature and its OS lifecycle exports. The real installer
uses the separate feature-gated `ui/src/installer.rs` adapter for shutdown and
unelevated launch; service/payload work remains in NSIS, and neither build calls
firmware. The DLL is compiled for the
stock `x86-unicode` runtime and embedded automatically using `!addplugindir`.

## DPI and image strategy

The EXE declares **Per-Monitor V2**, with a Per-Monitor fallback, and the UI thread
also requests PMv2. Initial DPI comes from the actual window, not the system DPI.
`WM_GETDPISCALEDSIZE` preserves the 796×394-DIP client area. `WM_DPICHANGED` follows
Windows' suggested rectangle, recreates fonts and icon handles, and lays out every
control from original DIP coordinates. Repeated monitor changes do not compound
rounding errors or recreate the edit control, so typed paths and selections survive.
There is no hard-coded 225% DPI cap.

### Native SVG variant

The optional `native-svg` Cargo feature adds Windows API bindings, not a bundled
SVG parser/rasterizer. `ui/src/svg.rs` creates a Direct2D `ID2D1DeviceContext5`
using **D3D11 WARP software rendering**, avoiding hardware/driver requirements for
one small logo. It parses the embedded `app-icon.svg` once per window and renders
a premultiplied BGRA bitmap at the current physical pixel size. GDI AlphaBlend
composites the cached bitmap; ordinary paints do not parse or rasterize SVG.

The source's fixed 300px root width/height must be set to 100% on the native SVG
DOM, retaining the original viewBox; setting the document viewport alone crops
rather than scales this source. Its clip paths, transforms and dashed strokes
were compared with Sharp's build-time rasterization, not merely checked for
successful API calls.

DPI changes rebuild the bitmap from vector data. A render failure recreates native
resources once, then falls back to the existing ICO. Device-context creation or
SVG parsing failure also falls back. COM resources are released before the UI's
COM apartment is uninitialized. `SWITCHBOOT_NSIS_FORCE_ICO=1` exercises fallback.
The window property `Switchboot.NsisLogoRenderer` reports 1=SVG, 2=raster build,
3=SVG-build ICO fallback, so tests cannot silently pass on fallback.

### Raster comparison variant

`generate-icon.mjs` rasterizes the SVG **at build time**, packing exact 60-DIP logo
frames at 60/75/90/105/120/135 pixels (100/125/150/175/200/225%) into one multi-image
ICO, plus 16/32-DIP window-icon frames and a 256px fallback. It reloads an original
frame on each DPI change. This remains available for direct A/B comparison.

## Validation

```sh
cargo fmt --manifest-path packaging/nsis-poc/ui/Cargo.toml -- --check
cargo clippy --locked --manifest-path packaging/nsis-poc/ui/Cargo.toml --target i686-pc-windows-msvc --all-targets --features native-svg -- -D warnings
cargo test --locked --manifest-path packaging/nsis-poc/ui/Cargo.toml --target i686-pc-windows-msvc --features native-svg
node --test packaging/nsis-poc/generate-icon.test.mjs
uv run --no-project python packaging/nsis-poc/test_preview.py --renderer raster
uv run --no-project python packaging/nsis-poc/test_preview.py --renderer svg --theme dark
uv run --no-project python packaging/nsis-poc/test_preview.py --renderer svg --theme light
```

The Python test commands are **opt-in interactive desktop tests**. SVG tests also
assert the native renderer is active through monitor moves and verify forced ICO
fallback. The raster variant skips the SVG-only fallback case. It launches only this
preview, tests Cancel, fixture extraction/completion, Unicode destination isolation,
Browse cancellation, relative-path rejection, PMv2 context, client/control/font
sizes across the available physical monitors (selection and completion), lack of
continuous hover invalidation, and anti-aliased edge pixels. The hover test briefly
moves the pointer over its button and restores it; monitor tests move only the
preview. Mixed-DPI coverage is explicitly skipped if no different-DPI monitors are
available. It captures only its own window
into `artifacts/nsis-preview.png` and `artifacts/nsis-preview-complete.png` using
standard-library Python and Win32. It is intentionally outside production test
discovery and must not be pointed at another installer.

Per-monitor captures are also saved as `artifacts/nsis-preview-dpi-<DPI>.png`.
Unit checks cover layout geometry through 400% and exact image frames through 225%;
this is not a claim of physical-display validation at every scale. Manual acceptance
remains necessary for high contrast, screen readers, keyboard-only use, press/drag
behavior and visual motion/flicker. The folder picker is Windows-native, not a
recreation of issconfig's custom picker.

## Reproducing the SVG evaluation

Build both variants first, then run from the repository root:

```sh
cargo run --locked --release --manifest-path packaging/nsis-poc/ui/Cargo.toml --target i686-pc-windows-msvc --features native-svg --example svg_probe -- artifacts/nsis-svg-evaluation
node packaging/nsis-poc/evaluate-svg.mjs
uv run --no-project python packaging/nsis-poc/benchmark_preview.py
```

The probe renders 60/75/90/105/120/135px plus 63/150/180/240px, validates alpha and
cache reuse, and exercises fresh native-resource recreation. Actual device-loss
injection is not covered. The Node comparison checks composited pixel differences
against build-time SVG rendering and creates `comparison.png` and `comparison.json`.
It allows different antialiasing but rejects large differences such as root clipping.
The benchmark opens only the two harmless previews: one warm-up and seven alternating
measured launches each, using 2ms polling. It measures spawn-to-visible startup,
not cold boot or time-to-first-input, and saves `startup-comparison.json` plus a
full-window capture of each variant. Physical DPI checks were run at 100/125%;
off-screen pixel renders through 400% are not physical-display acceptance.

### Local measurements (2026-10-17)

| Build | Installer bytes | UI DLL bytes | Median warm startup |
| --- | ---: | ---: | ---: |
| Previous preview, duplicate ICO payload | 238,059 | 119,808 | Not measured |
| Raster, icon resource reused | 182,771 | 120,320 | 76.9 ms |
| Direct2D SVG, standard icon resource reused | 136,807 | 132,096 | 94.9 ms |

The native code adds 11.5 KiB to the DLL, but avoiding extra raster frames makes
its packaged EXE about 25% smaller than the comparable raster build. Removing the
duplicate ICO benefits **both** new variants and must not be credited to SVG alone.
A separate native render probe measured 13.5ms initialization, 2.0ms first 60px
render, and roughly 0.5–0.9ms for subsequent sizes. These are local observations,
not performance guarantees. Common-scale mean RGB differences versus the Sharp
reference were about 1.2–2.2 on a 0–255 scale, concentrated at antialiased edges.

## Before production adoption

Tauri's NSIS template is a useful reference, but sharing the same installer engine
is not automatic compatibility. Preserve the existing native and legacy updater
arguments, signature/feed compatibility, original-user relaunch, exact-process
shutdown and executable-release checks, installed-directory policy, and service
failure behavior. Existing Inno registrations/uninstallers still require an explicit
migration path; copying an NSIS template does not retire their ownership records.

The previews intentionally implement none of those privileged lifecycle changes.
The first real installer now lives in [`../nsis`](../nsis/README.md), sharing the
UI but adding explicit ownership/migration, checked service hooks, native shutdown
and unelevated launch. Build it with `uv run --no-project python packaging/nsis/build.py`.
It is a separate unsigned candidate, not an automatic production replacement.
Its mock tests do not establish real service/UAC or signed updater acceptance.
