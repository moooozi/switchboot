# Switchboot AI Coding Guidelines

## Architecture Overview

Switchboot is a Tauri + Svelte desktop application for managing EFI boot entries across Windows and Linux platforms.

**Frontend (Svelte/TypeScript):**
- `src/routes/+page.svelte` - Main boot management interface
- `src/lib/components/` - UI components (BootEntriesList, BootEntryItem, ApiService)
- Drag-and-drop boot order management with `svelte-dnd-action`
- Tauri invoke calls to Rust backend via `ApiService.svelte`

**Backend (Rust/Tauri):**
- `src-tauri/src/lib.rs` - Tauri commands and platform-specific logic
- `src-tauri/src/cli/` - CLI daemon for privileged EFI operations
- Platform-specific implementations with `#[cfg(target_os = "...")]` attributes

**Build System:**
- **Icons**: `pnpm icons` runs `tools/prerender-icons.js` to generate platform-specific icons from SVGs
- **Frontend**: `pnpm build` → Vite static build to `build/`
- **Native**: `pnpm tauri build` bundles with Rust backend
- **Full build**: `pnpm tauri build` (runs frontend build automatically)
- **Rust Edition**: 2024

## Key Patterns & Conventions

### Component Communication
```typescript
// ApiService.svelte - Central hub for all Tauri invokes
export async function fetchBootEntries() {
  const entries = (await invoke("get_boot_entries")) as BootEntry[];
  onbootentriesfetched?.(entries);
  return entries;
}
```

### Platform-Specific Code
```rust
#[cfg(target_os = "windows")]
#[tauri::command]
fn get_boot_order() -> Result<Vec<u16>, String> {
    get_cli()?.send_command(&CliCommand::GetBootOrder)
}

#[cfg(not(target_os = "windows"))]
#[tauri::command]
fn get_boot_order() -> Result<Vec<u16>, String> {
    let out = call_cli(&CliCommand::GetBootOrder, false)?;
    serde_json::from_str(&out).map_err(|e| e.to_string())
}
```

### Icon Mapping
```typescript
// src/lib/iconMap.ts - Regex-based OS detection
if (/windows boot manager/i.test(d)) return "windows";
if (/ubuntu|kubuntu|xubuntu|lubuntu|buntu/i.test(d)) return "ubuntu";
```

### Features
- **Custom IPC Implementation**: Uses `pipeguard` crate (https://github.com/moooozi/pipeguard) for secure named pipe IPC
- **EFI Operations**: Uses `firmware_variables` crate (https://github.com/moooozi/firmware_variables) for EFI variable manipulation
- JSON serialization over stdin/stdout between GUI and CLI daemon
- CLI runs with elevated privileges for EFI operations
- Commands defined in `src-tauri/src/types/cli_args.rs`

## Development Workflows

### Local Development
```bash
pnpm install          # Install dependencies
pnpm tauri dev --config src-tauri/tauri.signed.conf.json # Start Tauri dev mode (opens native app)
```

### Building
```bash
pnpm icons           # Generate icons for current platform
pnpm tauri icon ./app-icon.svg  # Generate app icons
pnpm tauri build     # Full native build (takes log time, use equivalently dev command for faster iteration)
```

### Testing EFI Operations
- Use `mockBootEntries.ts` for development without real EFI access
- CLI daemon must run with appropriate privileges (sudo/Admin)

## Cross-Platform Considerations

### Windows
- Portable vs installer versions (affects shortcut creation)
- Uses named pipe IPC for CLI communication
- NSIS installer with custom hooks

### Linux
- DEB/RPM packages with polkit integration
- Symlinks for passwordless CLI execution
- Hicolor icon theme integration

### Icon Generation
- `tools/prerender-icons.js` handles platform differences
- Linux: PNGs in hicolor directories
- Windows: ICO files with multiple sizes
- SVG sources in `src-tauri/icons-raw/`

## Release Process

### Version Management
- Single source of truth: `src-tauri/tauri.conf.json`
- Release branch triggers automated builds
- GitHub Actions generate installers and update repos

### Repository Management
- `scripts/generate_indexes.js` - Creates directory listings
- APT/RPM repos hosted on GitHub Pages
- `repo-config/` contains static repo files

## Common Patterns

### Error Handling
```rust
// CLI responses always include success/error codes
pub struct CommandResponse {
    pub code: i32,       // 0 = success
    pub message: String, // stdout or error
}
```

### State Management
- Reactive Svelte stores for UI state
- Boot order changes tracked with `changed` flag
- Original order preserved for revert operations

### Boot Entry Types
```typescript
type BootEntry = {
  id: number;              // EFI boot entry ID
  description: string;     // Human-readable name
  is_default: boolean | null;  // Default boot entry
  is_bootnext: boolean;    // Next boot selection
  is_current: boolean;     // Currently booted entry
};
```

## Key Files to Reference

- `src/lib/types.ts` - Core TypeScript types
- `src-tauri/src/types/mod.rs` - Rust type definitions
- `src/lib/iconMap.ts` - OS icon mapping logic
- `src-tauri/src/cli/logic.rs` - EFI operation implementations
- `tools/prerender-icons.js` - Icon generation pipeline
- `src-tauri/src/cli/windows/pipe.rs` - Pipeguard IPC implementation
- `.github/workflows/release-pipeline.yml` - Build automation

## Updating this Document
- If you notice inconsistencies or no longer accurate information in this document, please update it to reflect the current codebase and practices.
- Keep the document concise and focused on key architectural and coding guidelines.
