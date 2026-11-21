# Controller Adder

Add native gamepad support to games that lack it, or customize existing controller mappings.

Controller Adder is a desktop application and command-line toolkit that analyzes a game's input system, lets you assign controller buttons and analog sticks to game actions, and generates an engine-specific patch. Instead of emulating keyboard presses, the RPG Maker implementation integrates with the game's input handling through a generated JavaScript plugin.

**Project status:** Early alpha (v0.1.0 development). The first engine implementation is feature-complete, but broader compatibility testing is still needed.

## Supported engines

| Engine | Support |
| --- | --- |
| RPG Maker MZ | Scanning, mapping and patch management |
| RPG Maker MV | Scanning, mapping and patch management; broader gameplay testing needed |
| Other engines | Not currently supported |

The project is designed to support more engines in the future. Compatibility depends on the game, its plugins and its runtime environment.

## Features

- Automatic engine detection.
- Input scanning, plugin registry inspection and JavaScript analysis.
- Effective input reports combining engine defaults and plugin evidence.
- Editable controller mappings for standard gamepad buttons and analog stick directions, with adjustable thresholds.
- Automatic binding suggestions and validation.
- Local profile storage and JSON import/export.
- Automatic plugin generation, installation, status detection, updating and uninstallation.
- Recovery backups and checks for conflicting or unrecognized installations.
- Dark desktop interface with collapsible navigation.
- Standalone Rust CLI for scripting and integration with other applications.

## Getting started

### Requirements

- Git
- A current stable Rust toolchain (Rust 2024 edition support)
- A Node.js version compatible with the project's Vite 8 toolchain
- pnpm
- Platform-specific [Tauri v2 prerequisites](https://v2.tauri.app/start/prerequisites/) for the graphical application

The application has been developed and tested on Linux. Windows and handheld compatibility require further verification. The CLI does not require the graphical application's webview dependencies.

### Build and run from source

```bash
git clone https://github.com/Duniek9/controller-adder.git
cd controller-adder
pnpm install
pnpm tauri dev
```

On Linux systems experiencing NVIDIA/WebKit rendering issues, the following development workaround may help:

```bash
WEBKIT_DISABLE_DMABUF_RENDERER=1 __NV_DISABLE_EXPLICIT_SYNC=1 pnpm tauri dev
```

To build desktop packages for your current platform:

```bash
pnpm tauri build
```

Prebuilt release binaries are not documented as available yet.

## Using the graphical application

1. Open **Home** and select the game's root directory.
2. Confirm that the game engine is supported.
3. Use **Effective Inputs** to inspect the detected input actions and possible plugin conflicts.
4. Open **Controller Mapping** and scan the game's actions.
5. Fill standard defaults, auto-assign plugin actions or configure bindings manually.
6. Review validation warnings and save or export your profile if desired.
7. Close the game and its editor, then choose **Install patch** in the patch management panel.
8. Launch the game and test the controller.

To revise your mappings, edit or import a profile and choose **Update patch**. To restore the original input handling, close the game and choose **Uninstall patch**.

Saving or exporting a profile does not modify the game. Installation and updating use the bindings currently displayed in the editor; a JSON export is optional.

## Command-line interface

Build the standalone CLI:

```bash
cargo build -p controller-cli --release
```

The executable is located at `target/release/controller-cli` (with an `.exe` suffix on Windows). The following examples use Cargo so they work without adding the executable to PATH.

### Inspect a game

```bash
cargo run -p controller-cli -- detect "/path/to/game"
cargo run -p controller-cli -- inputs "/path/to/game" --json
cargo run -p controller-cli -- plugins "/path/to/game" --json
cargo run -p controller-cli -- javascript "/path/to/game" --json
cargo run -p controller-cli -- effective "/path/to/game" --save
cargo run -p controller-cli -- resolve "/path/to/game" --save
```

### Create and edit a profile

```bash
cargo run -p controller-cli -- mapping scan "/path/to/game"
cargo run -p controller-cli -- mapping suggest "/path/to/game" --output profile.json
cargo run -p controller-cli -- mapping set profile.json --action ok --button 0
cargo run -p controller-cli -- mapping validate profile.json
```

Use `mapping suggest --help` and `mapping set --help` to explore suggestion modes, analog axes, thresholds and multiple inputs per action. Button indices follow the standard browser Gamepad API layout.

### Manage a patch

Close the game and its editor before making changes.

```bash
# Generate JavaScript without modifying the game
cargo run -p controller-cli -- patch generate profile.json --save

# Install or update using the game path stored in the profile
cargo run -p controller-cli -- patch apply profile.json
cargo run -p controller-cli -- patch update profile.json

# Inspect or remove using the game directory
cargo run -p controller-cli -- patch status "/path/to/game"
cargo run -p controller-cli -- patch uninstall "/path/to/game"
```

**Important:** Apply, update and generate accept a JSON profile file. Status and uninstall accept the game directory.

Use `--json` with patch apply, update, status and uninstall for machine-readable reports. Run any command with `--help` for available options.

### Generated output

When using the default export location from the project directory:

| Directory | Contents |
| --- | --- |
| `output/scans/` | Engine and input reports |
| `output/profiles/` | Controller mapping profiles |
| `output/patches/` | Generated JavaScript plugins |

Exports refuse to overwrite existing files. Use a new filename for a revised export. CLI mapping edit commands intentionally update the specified profile.

## How RPG Maker patching works

Controller Adder supports a standard RPG Maker game root, including the common MV `www/` layout. It generates `js/plugins/ControllerAdder.js` and registers it in `js/plugins.js`.

The generated plugin uses the browser Gamepad API and replaces RPG Maker's stock gamepad polling implementation while retaining engine keyboard handling and action processing. It does not synthesize keyboard events.

- Installation creates a backup of the original plugin registry.
- Updating replaces the recognized generated plugin and backs up its previous version without rewriting the registry or enabling a disabled plugin.
- Uninstallation removes only the Controller Adder entry and plugin, preserving unrelated registry entries and retaining recovery backups.

The patch manager reports Installed, Not Installed, Disabled, Incomplete or Conflict. It refuses to update or uninstall installations it cannot safely recognize.

## Limitations and safety

- Back up important game files before patching. Close the game and its editor during all patch operations.
- Static analysis cannot guarantee that every detected action is usable at runtime or discover every dynamically generated action.
- Custom input plugins, nonstandard registries, modified engine runtimes or unsupported controllers may cause incompatibilities.
- The runtime expects gamepads exposed through the standard Gamepad API mapping. It is not a universal operating-system input remapper.
- Existing controller mappings may be replaced by the generated plugin. Check for conflicts with other input layers such as Steam Input when troubleshooting.
- Game updates may overwrite patched files. Recheck patch status after updating a game.
- JSON profiles contain a game directory path; review that path before sharing a profile. GUI importing currently requires the same selected game and engine.
- Keep recovery backups until you have verified that the game works correctly.

## Architecture

| Location | Purpose |
| --- | --- |
| `crates/controller-core/` | Shared engine detection, scanning, mapping, patch generation and installation logic |
| `crates/controller-cli/` | Standalone command-line application |
| `src-tauri/` | Tauri v2 desktop backend |
| `src/` | SvelteKit, Svelte 5 and TypeScript interface |

Both the CLI and graphical application use the same Rust core, allowing new engine implementations to be shared between them.

## Development

```bash
cargo test --workspace
pnpm check
pnpm tauri dev
```

Report reproducible problems through [GitHub Issues](https://github.com/Duniek9/controller-adder/issues). Include the engine, operating system, controller, expected behavior and any relevant errors. Do not upload copyrighted game files or profiles containing private paths without reviewing them.

## Roadmap

- Broader RPG Maker MV/MZ compatibility testing and first packaged alpha release.
- Usability improvements for desktop and handheld devices.
- Additional game engines using the shared scanning, mapping and patch architecture.

Controller Adder is an independent project and is not affiliated with RPG Maker or its publishers.
