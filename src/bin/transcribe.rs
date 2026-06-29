use anyhow::Result;
use clap::Parser;
use std::path::PathBuf;
use transcribe::runner::{self, Config, Event};

#[derive(Parser, Debug)]
#[command(name = "transcribe", version, about = "Transcribe MP3/MP4 files to .txt")]
struct Cli {
    /// File or directory to process (recursive). Defaults to the current directory,
    /// or to the executable's own directory when double-clicked on Windows.
    path: Option<PathBuf>,

    /// Language code (e.g. "nl", "en") or "auto" for auto-detect
    #[arg(long, default_value = "auto")]
    language: String,

    /// Whisper model: tiny, base, small, medium, large-v3
    #[arg(long, default_value = "large-v3")]
    model: String,

    /// Re-transcribe even if a .txt file already exists
    #[arg(long)]
    force: bool,

    /// Wait for Enter before exiting. Automatic when double-clicked on Windows.
    #[arg(long)]
    pause: bool,
}

fn main() {
    let orphan = is_orphan_console();
    let result = cli_run(orphan);
    if let Err(ref e) = result {
        eprintln!("\nError: {e:#}");
    }
    let want_pause = orphan || std::env::args().any(|a| a == "--pause");
    if want_pause {
        pause_for_user();
    }
    if result.is_err() {
        std::process::exit(1);
    }
}

fn cli_run(orphan: bool) -> Result<()> {
    let cli = Cli::parse();
    let path = cli.path.unwrap_or_else(|| default_path(orphan));
    let config = Config {
        path,
        language: cli.language,
        model: cli.model,
        force: cli.force,
    };
    runner::run(config, |event| match event {
        Event::NoFilesFound { path } => {
            println!("No .mp3 or .mp4 files found in {}", path.display());
        }
        Event::FilesFound { count } => println!("Found {count} media file(s)"),
        Event::ModelLoading { path } => println!("Loading model: {}", path.display()),
        Event::FileStart { index, total, path } => {
            println!("[{}/{}] {}", index + 1, total, path.display());
        }
        Event::FileSkipped { index, total, path } => {
            println!("[{}/{}] skip (txt exists): {}", index + 1, total, path.display());
        }
        Event::FileDecoded {
            samples,
            duration_s,
            elapsed_s,
        } => println!(
            "  decoded {} samples ({:.1}s audio, decode {:.1}s)",
            samples, duration_s, elapsed_s
        ),
        Event::FileTranscribed {
            txt_path,
            chars,
            elapsed_s,
        } => println!(
            "  -> {} ({} chars, {:.1}s)",
            txt_path.display(),
            chars,
            elapsed_s
        ),
        Event::FileError { path, error } => {
            eprintln!("  ! {}: {}", path.display(), error);
        }
        Event::Done => {}
    })
}

fn default_path(orphan: bool) -> PathBuf {
    if orphan {
        if let Ok(exe) = std::env::current_exe() {
            if let Some(parent) = exe.parent() {
                return parent.to_path_buf();
            }
        }
    }
    PathBuf::from(".")
}

fn pause_for_user() {
    use std::io::{stdin, stdout, Write};
    let _ = writeln!(stdout(), "\nPress Enter to close...");
    let _ = stdout().flush();
    let mut buf = String::new();
    let _ = stdin().read_line(&mut buf);
}

#[cfg(windows)]
fn is_orphan_console() -> bool {
    use windows_sys::Win32::System::Console::GetConsoleProcessList;
    let mut list = [0u32; 4];
    let count = unsafe { GetConsoleProcessList(list.as_mut_ptr(), list.len() as u32) };
    count == 1
}

#[cfg(not(windows))]
fn is_orphan_console() -> bool {
    false
}
