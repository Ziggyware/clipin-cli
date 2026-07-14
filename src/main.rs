//! clipin — Rust replica of clipin.ps1 (exact-behavior port)
//! Build: cargo build --release
//! Cargo.toml deps: winapi { features=["winuser","winbase","stringapiset","errhandlingapi"] }, glob, atty

use std::env;
use std::fs;
use std::io::{self, Read};
use std::path::Path;
use std::process::{self, Command};

// ---------------------------------------------------------------------------
mod clipboard {
    use std::ffi::OsStr;
    use std::os::windows::ffi::{OsStrExt, OsStringExt};
    use std::ptr;
    use winapi::shared::windef::HWND;
    use winapi::um::winbase::{GlobalAlloc, GlobalLock, GlobalUnlock, GMEM_MOVEABLE};
    use winapi::um::winuser::{
        CloseClipboard, EmptyClipboard, GetClipboardData, OpenClipboard, SetClipboardData,
    };

    const CF_UNICODETEXT: u32 = 13;
    const CF_HDROP: u32 = 15;

    // DROPFILES header: 20 bytes { pFiles:DWORD=20, pt:{x,y}=0,0, fNC:BOOL=0, fWide:BOOL=1 }
    #[repr(C)]
    struct DropFiles {
        p_files: u32,
        pt_x: i32,
        pt_y: i32,
        f_nc: i32,
        f_wide: i32,
    }

    unsafe fn galloc_copy(bytes: &[u8]) -> Result<*mut winapi::ctypes::c_void, String> {
        let h = GlobalAlloc(GMEM_MOVEABLE, bytes.len());
        if h.is_null() {
            return Err("GlobalAlloc failed".into());
        }
        let p = GlobalLock(h);
        if p.is_null() {
            return Err("GlobalLock failed".into());
        }
        ptr::copy_nonoverlapping(bytes.as_ptr(), p as *mut u8, bytes.len());
        GlobalUnlock(h);
        Ok(h)
    }

    pub fn set_text(text: &str) -> Result<(), String> {
        unsafe {
            if OpenClipboard(ptr::null_mut::<HWND>() as HWND) == 0 {
                return Err("OpenClipboard failed".into());
            }
            EmptyClipboard();
            let wide: Vec<u16> = OsStr::new(text)
                .encode_wide()
                .chain(std::iter::once(0))
                .collect();
            let bytes = std::slice::from_raw_parts(
                wide.as_ptr() as *const u8,
                wide.len() * 2,
            );
            let h = galloc_copy(bytes).map_err(|e| {
                CloseClipboard();
                e
            })?;
            SetClipboardData(CF_UNICODETEXT, h);
            CloseClipboard();
            Ok(())
        }
    }

    pub fn get_text() -> Result<String, String> {
        unsafe {
            if OpenClipboard(ptr::null_mut::<HWND>() as HWND) == 0 {
                return Err("OpenClipboard failed".into());
            }
            let h = GetClipboardData(CF_UNICODETEXT);
            if h.is_null() {
                CloseClipboard();
                return Ok(String::new()); // empty clipboard, not an error (PS parity)
            }
            let p = GlobalLock(h) as *const u16;
            if p.is_null() {
                CloseClipboard();
                return Ok(String::new());
            }
            let mut len = 0usize;
            while *p.add(len) != 0 {
                len += 1;
            }
            let slice = std::slice::from_raw_parts(p, len);
            let s = std::ffi::OsString::from_wide(slice)
                .into_string()
                .unwrap_or_default();
            GlobalUnlock(h);
            CloseClipboard();
            Ok(s)
        }
    }

    pub fn set_file_drop(paths: &[String]) -> Result<(), String> {
        unsafe {
            if OpenClipboard(ptr::null_mut::<HWND>() as HWND) == 0 {
                return Err("OpenClipboard failed".into());
            }
            EmptyClipboard();

            let mut names: Vec<u16> = Vec::new();
            for p in paths {
                names.extend(OsStr::new(p).encode_wide());
                names.push(0);
            }
            names.push(0); // double-NUL

            let hdr = DropFiles {
                p_files: std::mem::size_of::<DropFiles>() as u32,
                pt_x: 0,
                pt_y: 0,
                f_nc: 0,
                f_wide: 1,
            };
            let hdr_bytes = std::slice::from_raw_parts(
                (&hdr as *const DropFiles) as *const u8,
                std::mem::size_of::<DropFiles>(),
            );
            let name_bytes = std::slice::from_raw_parts(
                names.as_ptr() as *const u8,
                names.len() * 2,
            );
            let mut buf = Vec::with_capacity(hdr_bytes.len() + name_bytes.len());
            buf.extend_from_slice(hdr_bytes);
            buf.extend_from_slice(name_bytes);

            let h = galloc_copy(&buf).map_err(|e| {
                CloseClipboard();
                e
            })?;
            SetClipboardData(CF_HDROP, h);
            CloseClipboard();
            Ok(())
        }
    }

    pub fn clear() {
        unsafe {
            if OpenClipboard(ptr::null_mut::<HWND>() as HWND) != 0 {
                EmptyClipboard();
                CloseClipboard();
            }
        }
    }
}

// ---------------------------------------------------------------------------
mod imgutil {
    use std::path::Path;

    pub fn is_image(path: &str) -> bool {
        matches!(
            Path::new(path)
                .extension()
                .and_then(|e| e.to_str())
                .map(|s| s.to_lowercase())
                .as_deref(),
            Some("png") | Some("jpg") | Some("jpeg") | Some("bmp") | Some("gif")
                | Some("tif") | Some("tiff")
        )
    }

    pub fn mime(path: &str) -> &'static str {
        match Path::new(path)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase()
            .as_str()
        {
            "jpg" | "jpeg" => "image/jpeg",
            "bmp" => "image/bmp",
            "gif" => "image/gif",
            "tif" | "tiff" => "image/tiff",
            _ => "image/png",
        }
    }

    pub fn to_base64(data: &[u8]) -> String {
        const C: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
        let mut out = String::with_capacity((data.len() + 2) / 3 * 4);
        for ch in data.chunks(3) {
            let b0 = ch[0] as u32;
            let b1 = if ch.len() > 1 { ch[1] as u32 } else { 0 };
            let b2 = if ch.len() > 2 { ch[2] as u32 } else { 0 };
            let n = (b0 << 16) | (b1 << 8) | b2;
            out.push(C[((n >> 18) & 63) as usize] as char);
            out.push(C[((n >> 12) & 63) as usize] as char);
            out.push(if ch.len() > 1 { C[((n >> 6) & 63) as usize] as char } else { '=' });
            out.push(if ch.len() > 2 { C[(n & 63) as usize] as char } else { '=' });
        }
        out
    }
}

// ---------------------------------------------------------------------------
mod pathutil {
    use glob::glob;
    use std::path::Path;

    pub fn expand(patterns: &[String], recursive: bool, exts: &[String]) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        
        let matches_ext = |p: &Path| -> bool {
            if exts.is_empty() { return true; }
            let ext = p.extension().and_then(|s| s.to_str()).unwrap_or("").to_lowercase();
            exts.iter().any(|x| x.to_lowercase() == ext)
        };

        for pat in patterns {
            let p = Path::new(pat);
            if p.is_dir() {
                let g = if recursive {
                    format!("{}/**/*", pat)
                } else {
                    format!("{}/*", pat)
                };
                if let Ok(it) = glob(&g) {
                    for e in it.flatten() {
                        if e.is_file() && matches_ext(&e) {
                            out.push(abs(&e));
                        }
                    }
                }
            } else if let Ok(it) = glob(pat) {
                let mut matched = false;
                for e in it.flatten() {
                    if e.is_file() && matches_ext(&e) {
                        out.push(abs(&e));
                        matched = true;
                    }
                }
                if !matched && p.is_file() && matches_ext(p) {
                    out.push(abs(p)); // literal fallback
                }
            } else if p.is_file() && matches_ext(p) {
                out.push(abs(p));
            } else {
                eprintln!("No files matched: {}", pat);
            }
        }
        out.sort();
        out.dedup();
        out
    }

    fn abs(p: &Path) -> String {
        std::fs::canonicalize(p)
            .map(|c| {
                let s = c.to_string_lossy().to_string();
                s.strip_prefix(r"\\?\").map(|x| x.to_string()).unwrap_or(s)
            })
            .unwrap_or_else(|_| p.to_string_lossy().to_string())
    }
}

// ---------------------------------------------------------------------------
mod bundle {
    pub struct Rec {
        pub name: String,
        pub typ: String,
        pub content: String,
    }

    pub fn to_llm(files: &[Rec], fence: &str) -> String {
        let mut s = String::new();
        for f in files {
            s.push_str(&f.name);
            s.push('\n');
            s.push_str(fence);
            s.push_str(&f.typ);
            s.push('\n');
            s.push_str(&f.content);
            if !f.content.is_empty() && !f.content.ends_with('\n') {
                s.push('\n');
            }
            s.push_str(fence);
            s.push('\n');
            s.push('\n');
        }
        s.trim_end_matches(['\r', '\n']).to_string()
    }
}

// ---------------------------------------------------------------------------
#[derive(Default)]
struct Cfg {
    append: bool,
    force_image: bool,
    as_b64: bool,
    as_data: bool,
    as_filedrop: bool,
    as_llm: bool,
    recursive: bool,
    trace: bool,
    help: bool,
    fence: String,
    extensions: Vec<String>,
    paths: Vec<String>,
}

fn parse() -> Cfg {
    let mut c = Cfg {
        as_llm: true, // Default mode is LLM now
        fence: "```".into(),
        ..Default::default()
    };
    for a in env::args().skip(1) {
        match a.as_str() {
            "--a" | "--append" => c.append = true,
            "--i" | "--image" => c.force_image = true,
            "--b" | "--b64" => c.as_b64 = true,
            "--d" | "--data" => c.as_data = true,
            "--f" | "--file" | "--files" => { c.as_filedrop = true; c.as_llm = false; },
            "--raw" => c.as_llm = false, // explicit opt-out to fallback TEXT
            "--l" | "--llm" | "--tx" | "--text" => c.as_llm = true,
            "--r" | "--recursive" => c.recursive = true,
            "--t" | "--trace" => c.trace = true,
            "--h" | "--help" => c.help = true,
            s if s.starts_with("--fence:") => c.fence = s[8..].to_string(),
            s if s.starts_with("--ext=") => {
                c.extensions.extend(s[6..].split(',').map(|x| x.trim().trim_start_matches('.').to_string()));
            },
            s if s.starts_with("-e=") => {
                c.extensions.extend(s[3..].split(',').map(|x| x.trim().trim_start_matches('.').to_string()));
            },
            s if s.starts_with("--fmt:") => {} 
            s if s.starts_with("--") => {}
            s => c.paths.push(s.to_string()),
        }
    }
    c
}

fn tr(on: bool, tag: &str, msg: &str) {
    if on {
        eprintln!("  [{}] {}", tag, msg);
    }
}

const HELP: &str = r#"@clipin — Clipboard Input Utility

  USAGE
    clipin <path(s)> [flags]

  FLAGS
    --h --help            Show this message
    --t --trace           Debug trace output
    --a --append          Append to clipboard
    --i --image           Force image mode (ignore extension)
    --b --b64             Encode image as Base64 text
    --d --data            Encode image as HTML base64 data URI
    --f --file / --files  Copy as Explorer file-drop
    --l --llm             Bundle file(s) with fenced-block markers (DEFAULT)
    --raw                 Use raw text mode (disable LLM bundle format)
    --r --recursive       Recursive directory expansion
    -e=.., --ext=..       Filter by multiple extensions (comma-separated, e.g., -e=rs,toml)
    --tx --text           Alias for --llm
    --fmt:<ext>           Override output image format (png | jpg | bmp | gif | tif)
    --fence:<chars>       Fence marker series (default: ```)
"#;

fn set_text_or_die(s: &str) {
    if let Err(e) = clipboard::set_text(s) {
        eprintln!("Error: {}", e);
        process::exit(1);
    }
}

fn main() {
    let cfg = parse();

    if cfg.help {
        print!("{}", HELP);
        process::exit(0);
    }
    if cfg.paths.is_empty() {
        print!("{}", HELP);
        process::exit(0);
    }
    tr(cfg.trace, "PARSE", &format!("paths: {:?}", cfg.paths));

    // -----------------------------------------------------------------------
    // PIPED INPUT
    // -----------------------------------------------------------------------
    let mut piped = String::new();
    let stdin_is_tty = atty::is(atty::Stream::Stdin);
    if !stdin_is_tty {
        let _ = io::stdin().read_to_string(&mut piped);
    }
    let piped_lines: Vec<&str> = if piped.is_empty() {
        Vec::new()
    } else {
        piped.split('\n').collect()
    };

    if !piped_lines.is_empty() {
        tr(cfg.trace, "PIPE", &format!("{} line(s)", piped_lines.len()));
        let mut parts = String::new();
        let mut lines = 0usize;
        if cfg.append {
            let cur = clipboard::get_text().unwrap_or_default();
            if !cur.is_empty() {
                lines += cur.split('\n').count();
                parts.push_str(&cur);
                parts.push_str("\n\n");
            }
            clipboard::clear();
        }
        parts.push_str(&piped_lines.join("\n"));
        set_text_or_die(&parts);
        lines += piped_lines.len();
        println!("({} line(s)) placed on clipboard.", lines);
        process::exit(0);
    }

    // -----------------------------------------------------------------------
    // RESOLVE PATHS
    // -----------------------------------------------------------------------
    let files = pathutil::expand(&cfg.paths, cfg.recursive, &cfg.extensions);
    let count = files.len();
    tr(cfg.trace, "RESOLVE", &format!("{} file(s)", count));

    // -----------------------------------------------------------------------
    // FILE-DROP
    // -----------------------------------------------------------------------
    if cfg.as_filedrop {
        let mut list: Vec<String> = files.clone();
        if cfg.append {
            clipboard::clear();
        }
        if let Err(e) = clipboard::set_file_drop(&list) {
            eprintln!("Error: {}", e);
            process::exit(1);
        }
        println!(
            "{} file(s) placed on clipboard as Explorer file-drop.",
            list.len()
        );
        let _ = &mut list;
        process::exit(0);
    }

    // Capture existing clipboard text for append
    let mut clip_seed = String::new();
    if cfg.append {
        clip_seed = clipboard::get_text().unwrap_or_default();
        clipboard::clear();
    }

    // -----------------------------------------------------------------------
    // AUTO-PROMOTE multi-file → file-drop
    // (Bypassed if as_llm is true, which is now the default)
    // -----------------------------------------------------------------------
    if count > 1 && !cfg.as_llm && !cfg.append {
        tr(cfg.trace, "AUTO", &format!("multi-file {} → file-drop", count));
        if let Err(e) = clipboard::set_file_drop(&files) {
            eprintln!("Error: {}", e);
            process::exit(1);
        }
        println!("{} file(s) placed on clipboard as Explorer file-drop.", count);
        process::exit(0);
    }

    // -----------------------------------------------------------------------
    // LLM BUNDLE (Now Default)
    // -----------------------------------------------------------------------
    if cfg.as_llm {
        let mut recs = Vec::new();
        for f in &files {
            let name = Path::new(f)
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default();
            if imgutil::is_image(f) {
                match fs::read(f) {
                    Ok(d) => recs.push(bundle::Rec {
                        name,
                        typ: "base64".into(),
                        content: imgutil::to_base64(&d),
                    }),
                    Err(e) => eprintln!("Skip {}: {}", f, e),
                }
            } else {
                let ext = Path::new(f)
                    .extension()
                    .map(|e| e.to_string_lossy().to_lowercase())
                    .filter(|s| !s.is_empty())
                    .unwrap_or_else(|| "txt".into());
                match fs::read_to_string(f) {
                    Ok(content) => recs.push(bundle::Rec { name, typ: ext, content }),
                    Err(e) => eprintln!("Skip {}: {}", f, e),
                }
            }
        }
        let mut out = String::new();
        if cfg.append && !clip_seed.is_empty() {
            out.push_str(&clip_seed);
            out.push('\n');
        }
        out.push_str(&bundle::to_llm(&recs, &cfg.fence));
        set_text_or_die(&out);
        println!("LLM bundle ({} file(s)) placed on clipboard.", count);
        process::exit(0);
    }

    // -----------------------------------------------------------------------
    // TEXT (raw mode, opted into via --raw)
    // -----------------------------------------------------------------------
    if !cfg.as_b64 && !cfg.as_data && !cfg.force_image {
        let mut buf: Vec<String> = Vec::new();
        if cfg.append && !clip_seed.is_empty() {
            buf.push(clip_seed.clone());
        }
        for f in &files {
            match fs::read_to_string(f) {
                Ok(content) => {
                    tr(cfg.trace, "TEXT", &format!("{} chars from {}", content.len(), f));
                    buf.push(f.clone());
                    buf.push(cfg.fence.clone());
                    if !content.trim().is_empty() {
                        buf.push(content);
                    }
                    buf.push(cfg.fence.clone());
                    buf.push(String::new());
                    buf.push(String::new());
                }
                Err(e) => tr(cfg.trace, "TEXT", &format!("read failed {}: {}", f, e)),
            }
        }
        let joined = buf.join("\n");
        if !joined.is_empty() {
            clipboard::clear();
            set_text_or_die(&joined);
            println!("{} text file(s) placed on clipboard.", count);
        } else {
            eprintln!("Clipboard is empty.");
        }
        process::exit(0);
    }

    // -----------------------------------------------------------------------
    // SINGLE IMAGE
    // -----------------------------------------------------------------------
    let file = match files.first() {
        Some(f) => f.clone(),
        None => {
            eprintln!("No file for image mode.");
            process::exit(1);
        }
    };
    let data = match fs::read(&file) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("Error loading image: {}", e);
            process::exit(1);
        }
    };

    if cfg.as_b64 {
        let mut out = String::new();
        if cfg.append && !clip_seed.is_empty() {
            out.push_str(&clip_seed);
            out.push('\n');
        }
        out.push_str(&imgutil::to_base64(&data));
        clipboard::clear();
        set_text_or_die(&out);
        println!("Image placed on clipboard as Base64.");
        process::exit(0);
    }

    if cfg.as_data {
        let uri = format!("data:{};base64,{}", imgutil::mime(&file), imgutil::to_base64(&data));
        let mut out = String::new();
        if cfg.append && !clip_seed.is_empty() {
            out.push_str(&clip_seed);
            out.push('\n');
        }
        out.push_str(&uri);
        set_text_or_die(&out);
        println!("Image placed on clipboard as HTML Base64 data URI.");
        process::exit(0);
    }

    let ps = format!(
        "Add-Type -AssemblyName System.Drawing,System.Windows.Forms; \
         $b=[System.Drawing.Image]::FromFile('{}'); \
         [System.Windows.Forms.Clipboard]::SetImage($b)",
        file.replace('\'', "''")
    );
    match Command::new("powershell")
        .args(["-STA", "-NoProfile", "-NonInteractive", "-Command", &ps])
        .status()
    {
        Ok(s) if s.success() => println!("Image placed on clipboard."),
        _ => {
            eprintln!("Error: raw-image clipboard requires PowerShell fallback (unavailable).");
            process::exit(1);
        }
    }
}