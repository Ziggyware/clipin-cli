# clipin

A fast, single-binary clipboard-input utility for Windows — copy files onto the clipboard as LLM-ready fenced bundles, plain text, Base64, HTML data URIs, raw images, or Explorer file-drops.

Rust port of the original `clipin.ps1` PowerShell utility, trading ~300–500 ms interpreter startup for a native executable that starts in single-digit milliseconds. The CLI surface and clipboard formats heavily mirror the PowerShell original, optimized for developer workflows.

```text
clipin <path(s)> [flags]
```

## Why

The PowerShell version paid a fixed interpreter tax on every invocation and required STA-thread re-entry for COM clipboard access. This port keeps identical behavior while removing both costs — no runtime, no re-launch, one ~3 MB executable callable from any shell (PowerShell, cmd, Windows Terminal, WSL interop).

## Install

### Build from source

Requires the Rust toolchain ([rustup.rs](https://rustup.rs/)) with the MSVC target (the default on Windows).

```powershell
git clone [https://github.com/ziggyware/clipin-cli.git](https://github.com/ziggyware/clipin-cli.git)
cd clipin-cli
cargo build --release
```

The compiled binary lands at `target\release\clipin.exe`.

### Put it on your PATH

Copy the binary to any directory already on your system or user `PATH`. For example, placing it in a global Windows directory (requires Administrator):

```powershell
Copy-Item .\target\release\clipin.exe C:\Windows\System32\ -Force
```

Alternatively, place it in a user-level binary folder (e.g., `C:\Users\<Name>\bin`) and ensure that folder is in your Environment Variables. Once on your PATH, `clipin` works from any directory in any shell. To confirm which binary resolves:

```powershell
Get-Command clipin -All | Format-Table CommandType, Name, Source -Auto
```

## Usage

### Flags

| Flag | Alias | Effect |
|------|-------|--------|
| `--h` | `--help` | Show help |
| `--t` | `--trace` | Emit debug trace to stderr |
| `--a` | `--append` | Prepend existing clipboard content to the new payload |
| `--i` | `--image` | Force image mode regardless of extension |
| `--b` | `--b64` | Encode image as a Base64 text string |
| `--d` | `--data` | Encode image as an HTML Base64 data URI |
| `--f` | `--file` / `--files` | Copy as an Explorer file-drop (paste into a folder) |
| `--l` | `--llm` / `--tx` | Bundle file(s) with fenced-block markers **(DEFAULT)** |
| `--raw`| | Use legacy raw text mode (disables LLM bundle format) |
| `--r` | `--recursive` | Recurse into directories |
| `-e=..`| `--ext=..` | Filter by multiple extensions (comma-separated, e.g., `-e=rs,toml`) |
| `--fence:<chars>` | | Fence marker sequence (default: three backticks) |
| `--fmt:<ext>` | | Image format hint (`png` \| `jpg` \| `bmp` \| `gif` \| `tif`) |

### Modes

`clipin` selects a mode from the flags and the number of resolved paths. **LLM Bundle** is the default output format unless overridden.

- **LLM bundle** (Default, or `--l`) — concatenates every file into fenced blocks tagged by extension; images are embedded as Base64. Built for pasting a whole set of sources contextually into a chat model.
- **Raw Text** (`--raw`) — writes the `path`, fence, file contents, and closing fence to the clipboard as plain Unicode text without bundle formatting.
- **File-drop** (`--f`) — places the files on the clipboard so they paste into Explorer as if copied directly from a folder.
- **Image Base64** (`--b`) — a single image as a raw Base64 string.
- **Image data URI** (`--d`) — a single image as `data:<mime>;base64,…` for direct HTML embedding.
- **Raw image** (a single image, no encoding flag) — the decoded bitmap itself, pasteable into image editors.

### Examples

```powershell
clipin src\ -r -e=rs,toml      # Recursively bundle all Rust and TOML files for an LLM
clipin notes.md                # Place notes.md on the clipboard as an LLM fenced block
clipin config.json --raw       # Place config.json on the clipboard as raw text
clipin report.pdf spec.docx -f # Two files -> Explorer file-drop
clipin logo.png --b64          # Image as a Base64 string
clipin logo.png --data         # Image as an HTML data URI
clipin diagram.png             # Raw image onto the clipboard
```

### Piped input

Text piped on stdin is placed directly on the clipboard; `--append` prepends whatever was already there. Piped input bypasses file resolution.

```powershell
git log --oneline -20 | clipin
Get-Content changelog.txt | clipin --append
```

## Behavior notes

- **Extension Filtering.** The `-e=` flag supports comma-separated lists and automatically strips leading dots (e.g., `-e=.rs,toml` and `-e=rs,toml` are identical). It applies to both literal files and recursive directory walks.
- **Multi-file auto-promotion.** If you opt *out* of LLM mode (e.g., using `--raw`) but pass multiple file paths without `--append`, `clipin` will automatically promote the operation to an Explorer file-drop to prevent data loss or mangling.
- **Append semantics.** `--append` *prepends* existing clipboard content ahead of the new payload — deliberately preserving the original PowerShell script's ordering logic.
- **Encoding.** Text files are read as UTF-8. Files in other encodings may not round-trip cleanly into the clipboard text buffer.

## Compatibility

Windows only. Clipboard access is implemented directly against the Win32 API (`OpenClipboard`, `SetClipboardData` with `CF_UNICODETEXT` / `CF_HDROP`), so there is no cross-platform build. The raw-image path shells out to PowerShell's `System.Windows.Forms.Clipboard` for device-independent-bitmap synthesis.

## License

MIT