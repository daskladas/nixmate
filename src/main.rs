//! nixmate - NixOS Multi-Tool
//!
//! A unified TUI bringing together all your NixOS utilities:
//! - Generation management
//! - Error translation
//! - Services & Ports dashboard
//! - Storage analysis & cleanup
//! - And more to come
//!
//! Usage: nixmate [--help] [--version]
//! Pipe:  nixos-rebuild switch 2>&1 | nixmate

mod app;
mod config;
mod i18n;
mod modules;
mod nix;
mod types;
mod ui;

use anyhow::{Context, Result};
use app::App;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::prelude::*;
use std::io::{self, stdout, IsTerminal, Read, Write};
use std::time::Duration;

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();

    if args.iter().any(|a| a == "--help" || a == "-h") {
        print_help();
        return Ok(());
    }

    if args.iter().any(|a| a == "--version" || a == "-v") {
        println!("nixmate {}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }

    // Check for piped input BEFORE starting TUI
    let piped_input = read_piped_input();

    // If stdin was a pipe, reattach to /dev/tty so crossterm can read key events
    if piped_input.is_some() {
        reattach_stdin_to_tty()
            .context("Failed to reattach stdin to terminal. Are you running in a TTY?")?;
    }

    let result = run_app(piped_input);

    if let Err(e) = result {
        eprintln!("Error: {:#}", e);
        std::process::exit(1);
    }

    Ok(())
}

/// Read all of stdin if it's a pipe (not a terminal).
/// Returns None if stdin is a terminal (normal interactive mode).
/// Limits input to 1 MB to prevent excessive memory usage.
fn read_piped_input() -> Option<String> {
    if io::stdin().is_terminal() {
        return None;
    }

    const MAX_PIPE_SIZE: usize = 1024 * 1024; // 1 MB — more than enough for any build log

    let mut input = String::new();
    match io::stdin().take(MAX_PIPE_SIZE as u64).read_to_string(&mut input) {
        Ok(_) => {}
        Err(_) => return None, // Non-UTF8 or read error
    }

    let trimmed = input.trim();
    if trimmed.is_empty() {
        return None;
    }

    Some(trimmed.to_string())
}

/// After reading piped stdin, reopen /dev/tty as fd 0 so crossterm
/// can read keyboard events. This is the standard Unix approach used
/// by tools like fzf, bat, less, etc.
#[cfg(unix)]
fn reattach_stdin_to_tty() -> Result<()> {
    use std::os::unix::io::AsRawFd;

    let tty = std::fs::File::open("/dev/tty")
        .context("Cannot open /dev/tty — pipe mode requires a controlling terminal")?;

    let tty_fd = tty.as_raw_fd();
    let result = unsafe { libc::dup2(tty_fd, libc::STDIN_FILENO) };
    if result == -1 {
        anyhow::bail!("dup2 failed: could not reattach stdin to /dev/tty");
    }

    // Let `tty` drop naturally — it closes the original fd, but fd 0 now
    // independently points to /dev/tty via the dup2 copy.
    drop(tty);

    Ok(())
}

#[cfg(not(unix))]
fn reattach_stdin_to_tty() -> Result<()> {
    anyhow::bail!("Pipe mode is only supported on Unix systems (Linux, macOS)")
}

fn print_help() {
    println!(
        r#"nixmate - NixOS Multi-Tool

       _                      _
      (_)                    | |
 _ __  ___  ___ __ ___   __ _| |_ ___
| '_ \| \ \/ / '_ ` _ \ / _` | __/ _ \
| | | | |>  <| | | | | | (_| | ||  __/
|_| |_|_/_/\_\_| |_| |_|\__,_|\__\___|

Made with ♥ by daskladas

USAGE:
    nixmate [OPTIONS]
    nixos-rebuild switch 2>&1 | nixmate     # pipe errors directly

OPTIONS:
    -h, --help       Print help information
    -v, --version    Print version information

KEYBINDINGS:
    1-9,0            Switch modules
    j/k              Navigate up/down
    Enter            Select/confirm
    F1-F4            Switch sub-tabs
    q                Quit

MODULES:
    [1] Generations       View & manage NixOS generations
    [2] Error Translator  Translate Nix error messages
    [3] Services & Ports  Server dashboard
    [4] Storage           Analyze & clean the Nix store
    [5] Config Showcase   System poster & config diagram
    [6] Options Explorer  Browse 20,000+ NixOS options
    [7] Rebuild           Live nixos-rebuild dashboard
    [8] Flake Inputs      Selective flake input updates
    [9] Package Search    Search & browse nixpkgs
    [0] Nix Doctor        System health checks
    [,] Settings          Theme, language, layout
    [?] Help / About      What nixmate does

PIPE MODE:
    Pipe build output into nixmate to auto-analyze errors:
      nixos-rebuild switch 2>&1 | nixmate
      nix build .#foo 2>&1 | nixmate

CONFIG:
    ~/.config/nixmate/config.toml
"#
    );
}

fn run_app(piped_input: Option<String>) -> Result<()> {
    // Load configuration
    let config = config::Config::load()
        .context("Failed to load configuration")?;

    // Create application state (with optional piped input)
    let mut app = App::new(config, piped_input)
        .context("Failed to initialize application")?;

    // Setup terminal
    enable_raw_mode().context("Failed to enable raw mode")?;
    let mut stdout = stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)
        .context("Failed to setup terminal")?;

    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)
        .context("Failed to create terminal")?;

    // Install panic handler so terminal is restored on panic
    // (without this, a panic leaves the terminal in raw mode + alternate screen)
    let is_kitty = app.image_protocol == modules::splash::ImageProtocol::Kitty;
    let original_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        // Best-effort terminal cleanup on panic
        // Send Kitty image delete command directly
        if is_kitty {
            let _ = write!(std::io::stdout(), "\x1b_Ga=d,d=A,q=2;\x1b\\");
            let _ = std::io::stdout().flush();
        }
        let _ = disable_raw_mode();
        let _ = execute!(std::io::stdout(), LeaveAlternateScreen, DisableMouseCapture);
        let _ = execute!(std::io::stdout(), crossterm::cursor::Show);
        original_hook(info);
    }));

    // Run main loop
    let result = main_loop(&mut terminal, &mut app);

    // IMPORTANT: Clean up terminal images BEFORE leaving the alternate screen.
    // The Kitty Graphics Protocol stores images in the terminal's GPU memory.
    // We must send the delete command while still in the alternate screen,
    // then flush + brief delay to ensure the terminal processes it.
    app.cleanup_images();
    // Flush stdout to ensure the delete escape sequences reach the terminal
    let _ = std::io::Write::flush(terminal.backend_mut());
    // Brief delay to let the terminal process the image deletion
    std::thread::sleep(Duration::from_millis(50));

    // Drop large data structures before terminal restore
    // (the image cache holds a base64-encoded PNG in RAM)
    app.image_cache = None;

    // Restore terminal
    disable_raw_mode().context("Failed to disable raw mode")?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )
    .context("Failed to restore terminal")?;
    terminal.show_cursor().context("Failed to show cursor")?;

    result
}

fn main_loop<B: Backend>(terminal: &mut Terminal<B>, app: &mut App) -> Result<()> {
    loop {
        terminal.draw(|frame| {
            ui::render(frame, app);
        })?;

        // Display terminal images AFTER ratatui has flushed its frame buffer.
        // This uses native protocols (Kitty/iTerm2) to overlay the real PNG
        // on top of the blank area reserved by the render functions.
        app.handle_image()?;

        // Update module timers (undo countdown etc.)
        app.update_timers()?;

        // Poll for events with timeout (for flash message expiry etc.)
        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    app.handle_key(key)?;
                }
            }
        }

        if app.should_quit {
            break;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_help_does_not_panic() {
        print_help();
    }
}
