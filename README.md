# clipin

A fast, single-binary clipboard-input utility for Windows — copy files onto the clipboard as text, LLM-ready fenced bundles, Base64, HTML data URIs, raw images, or Explorer file-drops.

Rust port of the original `clipin.ps1` PowerShell utility, trading ~300–500 ms interpreter startup for a native executable that starts in single-digit milliseconds. The CLI surface, flags, and clipboard formats are a deliberate 1:1 reproduction of the PowerShell original.

```
clipin <path(s)> [flags]
```

## Why

The PowerShell version paid a fixed interpreter tax on every invocation and required STA-thread re-entry for COM clipboard access. This port keeps identical behavior while removing both costs — no runtime, no re-launch, one ~3 MB executable callable from any shell (PowerShell, cmd, Windows Terminal, WSL interop).

## Install

### Build from source

Requires the Rust toolchain ([rustup.rs](https://rustup.rs/)) with the MSVC target (the default on Windows).

```powershell
git clone https://github.com/ziggyware/clipin-cli.git
cd clipin-cli
cargo build --release
```

The binary lands at `target\release\clipin.exe`.

### Put it on your PATH

Copy the binary anywhere already on `PATH`. Simplest:

```powershell
Copy-Item .\target\release\clipin.exe C:\Windows\System32\ -Force
```

Then `clipin` works from any directory in any shell. To confirm which binary resolves:

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
| `--l` | `--llm` / `--tx` / `--text` | Bundle file(s) with fenced-block markers |
| `--r` | `--recursive` | Recurse into directories |
| `--fence:<chars>` | | Fence marker sequence (default: three backticks) |
| `--fmt:<ext>` | | Image format hint (`png` \| `jpg` \| `bmp` \| `gif` \| `tif`) |

### Modes

`clipin` selects a mode from the flags and the number of resolved paths:

- **Text** (default, single or with `--append`) — writes `path`, fence, file contents, fence to the clipboard as Unicode text.
- **File-drop** (`--f`, or multiple paths with no `--llm`/`--append`) — places the files on the clipboard so they paste into Explorer as if copied from a folder.
- **LLM bundle** (`--l` / `--text`) — concatenates every file into fenced blocks tagged by extension; images are embedded as Base64. Built for pasting a whole set of sources into a chat model.
- **Image Base64** (`--b`) — a single image as a raw Base64 string.
- **Image data URI** (`--d`) — a single image as `data:<mime>;base64,…` for direct HTML embedding.
- **Raw image** (a single image, no encoding flag) — the decoded bitmap itself, pasteable into image editors.

### Examples

```powershell
clipin notes.md                    # File path + fenced contents onto the clipboard
clipin *.rs --llm                  # Bundle every Rust file as fenced blocks for an LLM
clipin *.md --text                 # Same bundle; --text is an alias for --llm
clipin src\ --llm --recursive      # Recurse a directory into one bundle
clipin report.pdf spec.docx        # Two files -> Explorer file-drop
clipin logo.png --b64              # Image as a Base64 string
clipin logo.png --data             # Image as an HTML data URI
clipin diagram.png                 # Raw image onto the clipboard
```

### Piped input

Text piped on stdin is placed directly on the clipboard; `--append` prepends whatever was already there.

```powershell
git log --oneline -20 | clipin
Get-Content changelog.txt | clipin --append
```

## Behavior notes

- **Multi-file auto-promotion.** Passing several paths with no `--llm` and no `--append` promotes automatically to a file-drop, matching the original.
- **Append semantics.** `--append` *prepends* existing clipboard content ahead of the new payload — deliberately preserving the PowerShell original's ordering.
- **Encoding.** Text files are read as UTF-8. Files in other encodings may not round-trip cleanly.

## Compatibility

Windows only. Clipboard access is implemented directly against the Win32 API (`OpenClipboard`, `SetClipboardData` with `CF_UNICODETEXT` / `CF_HDROP`), so there is no cross-platform build. The raw-image path shells out to PowerShell's `System.Windows.Forms.Clipboard` for device-independent-bitmap synthesis.

## License

MIT — see [LICENSE](LICENSE).
