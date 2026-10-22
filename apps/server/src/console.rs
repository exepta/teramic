//! Portable console input without blocking the ECS or requiring a GUI dependency.
use bevy::prelude::*;
#[cfg(unix)]
use std::io::Read;
use std::{
    io::{self, BufRead, IsTerminal, Write},
    process::{Child, Command, Stdio},
    sync::{Arc, Mutex, mpsc},
};
use terra_network::{
    protocol::commands::console_suggestions,
    server::{ConsoleOutput, ServerConsole},
};

pub(crate) struct ConsoleGuard {
    #[cfg(unix)]
    saved_mode: Option<String>,
    key_process: Option<Arc<Mutex<Child>>>,
}

impl Drop for ConsoleGuard {
    fn drop(&mut self) {
        if let Some(process) = &self.key_process
            && let Ok(mut child) = process.lock()
        {
            let _ = child.kill();
            let _ = child.wait();
        }
        #[cfg(unix)]
        if let Some(mode) = &self.saved_mode {
            let _ = Command::new("stty")
                .arg(mode)
                .stdin(Stdio::inherit())
                .status();
        }
    }
}

#[derive(Default)]
struct Editor {
    draft: String,
    history: Vec<String>,
    index: Option<usize>,
    history_draft: String,
    candidates: Vec<String>,
    completion_prefix: Option<String>,
    completion_index: usize,
    rendered_width: usize,
}

#[derive(Debug, PartialEq)]
enum Key {
    Text(char),
    Enter,
    Backspace,
    Tab,
    Up,
    Down,
    Escape,
}

impl Editor {
    fn key(&mut self, key: Key, players: &[String]) -> Option<String> {
        match key {
            Key::Enter => {
                let value = self.draft.trim().to_owned();
                if value.is_empty() {
                    return None;
                }
                if self.history.last() != Some(&value) {
                    self.history.push(value.clone());
                }
                if self.history.len() > 64 {
                    self.history.remove(0);
                }
                self.draft.clear();
                self.index = None;
                self.completion_prefix = None;
                self.candidates.clear();
                return Some(value);
            }
            Key::Tab => {
                let prefix = self
                    .completion_prefix
                    .get_or_insert_with(|| self.draft.clone());
                self.candidates = console_suggestions(prefix, players);
                if !self.candidates.is_empty() {
                    self.draft.clone_from(
                        &self.candidates[self.completion_index % self.candidates.len()],
                    );
                    self.completion_index += 1;
                }
                return None;
            }
            Key::Up | Key::Down => {
                let up = matches!(key, Key::Up);
                if self.history.is_empty() || (!up && self.index.is_none()) {
                    return None;
                }
                if self.index.is_none() {
                    self.history_draft.clone_from(&self.draft);
                }
                let index = self.index.unwrap_or(self.history.len());
                let next = if up {
                    index.saturating_sub(1)
                } else {
                    (index + 1).min(self.history.len())
                };
                self.draft = self
                    .history
                    .get(next)
                    .cloned()
                    .unwrap_or_else(|| self.history_draft.clone());
                self.index = (next < self.history.len()).then_some(next);
            }
            Key::Backspace => {
                self.draft.pop();
            }
            Key::Escape => {
                self.draft.clear();
                self.index = None;
            }
            Key::Text(c) => {
                if !c.is_control() && self.draft.chars().count() < 256 {
                    self.draft.push(c);
                }
            }
        }
        self.completion_prefix = None;
        self.completion_index = 0;
        self.candidates.clear();
        None
    }
}

#[derive(Resource)]
struct ConsoleInput {
    receiver: Mutex<mpsc::Receiver<String>>,
    players: Arc<Mutex<Vec<String>>>,
    editor: Arc<Mutex<Editor>>,
    interactive: bool,
    ansi: bool,
}

pub(crate) fn install(app: &mut App) -> io::Result<ConsoleGuard> {
    let tty = io::stdin().is_terminal() && io::stdout().is_terminal();
    let ansi = tty
        && (std::env::var("TERM").is_ok_and(|term| term != "dumb")
            || std::env::var_os("WT_SESSION").is_some()
            || std::env::var_os("ANSICON").is_some());
    let (tx, rx) = mpsc::sync_channel(64);
    let players = Arc::new(Mutex::new(Vec::new()));
    let editor = Arc::new(Mutex::new(Editor::default()));
    let mut guard = ConsoleGuard {
        #[cfg(unix)]
        saved_mode: None,
        key_process: None,
    };
    let mut interactive = false;
    #[cfg(unix)]
    if ansi
        && let Ok(saved) = Command::new("stty")
            .arg("-g")
            .stdin(Stdio::inherit())
            .output()
        && saved.status.success()
    {
        let mode = String::from_utf8_lossy(&saved.stdout).trim().to_owned();
        if !mode.is_empty() {
            // Save first: even a failing stty may already have changed one flag.
            guard.saved_mode = Some(mode.clone());
            interactive = Command::new("stty")
                .args(["-icanon", "-echo", "min", "1", "time", "0"])
                .stdin(Stdio::inherit())
                .status()
                .is_ok_and(|s| s.success());
            if !interactive {
                let _ = Command::new("stty")
                    .arg(&mode)
                    .stdin(Stdio::inherit())
                    .status();
            }
        }
    }
    #[cfg(windows)]
    let windows_keys = if tty {
        // ReadKey uses Windows console APIs (including Windows Terminal/ConPTY), not Unix escapes.
        let script = "$ErrorActionPreference='Stop'; while ($true) { $k=[Console]::ReadKey($true); if ([int]$k.KeyChar -ge 32) { [Console]::Out.WriteLine('T'+[int]$k.KeyChar) } else { [Console]::Out.WriteLine('K'+$k.Key.ToString()) }; [Console]::Out.Flush() }";
        match Command::new("powershell.exe")
            .args([
                "-NoLogo",
                "-NoProfile",
                "-NonInteractive",
                "-Command",
                script,
            ])
            .stdin(Stdio::inherit())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
        {
            Ok(mut child) => {
                let stdout = child.stdout.take();
                guard.key_process = Some(Arc::new(Mutex::new(child)));
                interactive = stdout.is_some();
                stdout
            }
            Err(_) => None,
        }
    } else {
        None
    };
    let thread_players = Arc::clone(&players);
    let thread_editor = Arc::clone(&editor);
    std::thread::Builder::new()
        .name("terra-console".into())
        .spawn(move || {
            if interactive {
                #[cfg(unix)]
                read_terminal(tx, thread_players, thread_editor);
                #[cfg(windows)]
                if let Some(stdout) = windows_keys {
                    read_windows(stdout, tx, thread_players, thread_editor, ansi);
                }
            } else {
                for line in io::stdin().lock().lines() {
                    let Ok(line) = line else {
                        break;
                    };
                    if let Some((prefix, _)) = line.split_once('\t') {
                        let names = thread_players.lock().map(|p| p.clone()).unwrap_or_default();
                        println!("{}", console_suggestions(prefix, &names).join("\n"));
                    } else if tx.send(line).is_err() {
                        break;
                    }
                }
            }
        })?;
    app.insert_resource(ConsoleInput {
        receiver: Mutex::new(rx),
        players,
        editor,
        interactive,
        ansi,
    })
    .add_systems(Update, pump);
    println!(
        "Console ready. Use help; Tab completes commands and players, Up/Down browse history. Redirected input accepts one command per line."
    );
    Ok(guard)
}

fn input_key(
    key: Key,
    tx: &mpsc::SyncSender<String>,
    players: &Mutex<Vec<String>>,
    editor: &Mutex<Editor>,
    ansi: bool,
) -> bool {
    let names = players.lock().map(|p| p.clone()).unwrap_or_default();
    let Ok(mut editor) = editor.lock() else {
        return false;
    };
    let tab = matches!(key, Key::Tab);
    let command = editor.key(key, &names);
    if let Some(command) = command.as_ref() {
        clear_line(ansi, editor.rendered_width);
        println!("> {command}");
    }
    if tab && !editor.candidates.is_empty() {
        clear_line(ansi, editor.rendered_width);
        println!(
            "{}",
            editor
                .candidates
                .iter()
                .take(8)
                .cloned()
                .collect::<Vec<_>>()
                .join("  ")
        );
    }
    redraw(&mut editor, ansi);
    // Backpressure must not retain the mutex the ECS needs to render replies.
    drop(editor);
    command.is_none_or(|command| tx.send(command).is_ok())
}

fn clear_line(ansi: bool, width: usize) {
    if ansi {
        print!("\r\x1b[2K");
    } else {
        print!("\r{}\r", " ".repeat(width));
    }
}

fn redraw(editor: &mut Editor, ansi: bool) {
    clear_line(ansi, editor.rendered_width);
    // Keep the editing prompt on one line even in narrow legacy consoles.
    let tail: String = editor
        .draft
        .chars()
        .rev()
        .take(58)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();
    print!("> {tail}");
    editor.rendered_width = tail.chars().count() + 2;
    let _ = io::stdout().flush();
}

#[cfg(unix)]
#[derive(Default)]
struct TerminalDecoder {
    escape: u8,
    utf8: Vec<u8>,
}

#[cfg(unix)]
impl TerminalDecoder {
    fn key(&mut self, value: u8) -> Option<Key> {
        if self.escape == 1 {
            self.escape = 0;
            // CSI and SS3 are both used by POSIX/application-cursor terminals.
            if matches!(value, b'[' | b'O') {
                self.escape = 2;
                return None;
            }
            // A standalone Escape must not eat the next ordinary character.
        } else if self.escape == 2 {
            if value.is_ascii_digit() || matches!(value, b';' | b'?') {
                return None;
            }
            self.escape = 0;
            return match value {
                b'A' => Some(Key::Up),
                b'B' => Some(Key::Down),
                _ => None,
            };
        }
        if value.is_ascii_control() || value == 127 {
            self.utf8.clear();
        }
        match value {
            27 => {
                self.escape = 1;
                None
            }
            10 | 13 => Some(Key::Enter),
            127 | 8 => Some(Key::Backspace),
            9 => Some(Key::Tab),
            21 => Some(Key::Escape),
            value => {
                self.utf8.push(value);
                match std::str::from_utf8(&self.utf8) {
                    Ok(text) => {
                        let c = text.chars().next();
                        self.utf8.clear();
                        c.map(Key::Text)
                    }
                    Err(error) if error.error_len().is_none() && self.utf8.len() < 4 => None,
                    Err(_) => {
                        self.utf8.clear();
                        None
                    }
                }
            }
        }
    }
}

#[cfg(unix)]
fn read_terminal(
    tx: mpsc::SyncSender<String>,
    players: Arc<Mutex<Vec<String>>>,
    editor: Arc<Mutex<Editor>>,
) {
    let mut stdin = io::stdin().lock();
    let mut byte = [0];
    let mut decoder = TerminalDecoder::default();
    while stdin.read_exact(&mut byte).is_ok() {
        if byte[0] == 4 {
            break;
        }
        if let Some(key) = decoder.key(byte[0])
            && !input_key(key, &tx, &players, &editor, true)
        {
            break;
        }
    }
}

#[cfg(windows)]
fn read_windows(
    stdout: std::process::ChildStdout,
    tx: mpsc::SyncSender<String>,
    players: Arc<Mutex<Vec<String>>>,
    editor: Arc<Mutex<Editor>>,
    ansi: bool,
) {
    for line in io::BufReader::new(stdout).lines() {
        let Ok(line) = line else {
            break;
        };
        let key = match line.as_str() {
            "KEnter" => Some(Key::Enter),
            "KTab" => Some(Key::Tab),
            "KBackspace" => Some(Key::Backspace),
            "KUpArrow" => Some(Key::Up),
            "KDownArrow" => Some(Key::Down),
            "KEscape" => Some(Key::Escape),
            text => text
                .strip_prefix('T')
                .and_then(|s| s.parse().ok())
                .and_then(char::from_u32)
                .map(Key::Text),
        };
        if let Some(key) = key
            && !input_key(key, &tx, &players, &editor, ansi)
        {
            return;
        }
    }
    eprintln!("Interactive console input unavailable; falling back to one command per line.");
    for line in io::stdin().lock().lines() {
        let Ok(line) = line else {
            break;
        };
        if tx.send(line).is_err() {
            break;
        }
    }
}

fn pump(
    input: Res<ConsoleInput>,
    mut console: ResMut<ServerConsole>,
    mut exit: MessageWriter<AppExit>,
) {
    if let Ok(mut players) = input.players.lock() {
        players.clone_from(&console.players);
    }
    if let Ok(receiver) = input.receiver.lock() {
        for command in receiver.try_iter().take(8) {
            if !console.submit(command) {
                eprintln!("Command rejected: invalid input or command queue full.");
            }
        }
    }
    let had_output = !console.output.is_empty();
    for output in console.output.drain(..) {
        match output {
            ConsoleOutput::Clear if input.ansi => print!("\x1b[2J\x1b[H"),
            ConsoleOutput::Clear => println!("--- console cleared ---"),
            ConsoleOutput::Line(line) => {
                if input.interactive {
                    let width = input.editor.lock().map(|e| e.rendered_width).unwrap_or(60);
                    clear_line(input.ansi, width);
                }
                if input.ansi {
                    let color = if line.starts_with("TERAMIC") {
                        "33"
                    } else if line.contains(" - ") {
                        "32"
                    } else {
                        "37"
                    };
                    println!("\x1b[{color}m{line}\x1b[0m");
                } else {
                    println!("{line}");
                }
            }
        }
    }
    if console.stop_requested {
        exit.write(AppExit::Success);
    }
    if had_output
        && input.interactive
        && let Ok(mut editor) = input.editor.lock()
    {
        redraw(&mut editor, input.ansi);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn a_saturated_queue_releases_the_editor_and_keeps_every_command() {
        let (tx, rx) = mpsc::sync_channel(1);
        tx.send("first".into()).unwrap();
        let editor = Arc::new(Mutex::new(Editor {
            draft: "time set 17:45".into(),
            ..Default::default()
        }));
        let worker_editor = Arc::clone(&editor);
        let (done, completion) = mpsc::channel();
        let worker = std::thread::spawn(move || {
            done.send(input_key(
                Key::Enter,
                &tx,
                &Mutex::default(),
                &worker_editor,
                false,
            ))
            .unwrap();
        });
        let started = std::time::Instant::now();
        loop {
            if editor
                .try_lock()
                .is_ok_and(|editor| editor.draft.is_empty())
            {
                break;
            }
            assert!(
                started.elapsed() < std::time::Duration::from_secs(2),
                "full queue retained the editor mutex"
            );
            std::thread::yield_now();
        }
        assert!(
            completion.try_recv().is_err(),
            "the producer must wait rather than drop its command"
        );
        assert_eq!(rx.recv().unwrap(), "first");
        assert_eq!(
            rx.recv_timeout(std::time::Duration::from_secs(2)).unwrap(),
            "time set 17:45"
        );
        assert!(
            completion
                .recv_timeout(std::time::Duration::from_secs(2))
                .unwrap()
        );
        worker.join().unwrap();
    }
    #[cfg(unix)]
    #[test]
    fn terminal_decoder_handles_csi_ss3_modifiers_and_text_after_escape() {
        let mut decoder = TerminalDecoder::default();
        let keys: Vec<_> = b"\x1b[A\x1b[B\x1bOA\x1bOB\x1b[1;5A\x1bx\xc3\xb6\x7f"
            .iter()
            .filter_map(|byte| decoder.key(*byte))
            .collect();
        assert_eq!(
            keys,
            vec![
                Key::Up,
                Key::Down,
                Key::Up,
                Key::Down,
                Key::Up,
                Key::Text('x'),
                Key::Text('ö'),
                Key::Backspace
            ]
        );
    }
    #[test]
    fn editor_completes_shared_commands_players_and_restores_history_drafts() {
        let mut editor = Editor {
            draft: "weather set s".into(),
            ..Default::default()
        };
        editor.key(Key::Tab, &[]);
        assert_eq!(editor.draft, "weather set snow");
        editor.key(Key::Tab, &[]);
        assert_eq!(editor.draft, "weather set storm");
        assert_eq!(
            editor.key(Key::Enter, &[]),
            Some("weather set storm".into())
        );
        editor.key(Key::Text('x'), &[]);
        editor.key(Key::Up, &[]);
        assert_eq!(editor.draft, "weather set storm");
        editor.key(Key::Down, &[]);
        assert_eq!(editor.draft, "x");
        editor.draft = "as Al".into();
        editor.key(Key::Tab, &["Alice".into()]);
        assert_eq!(editor.draft, "as Alice ");
        editor.key(Key::Escape, &[]);
        assert!(editor.draft.is_empty());
    }

    #[test]
    fn editor_backspace_preserves_unicode_and_caps_input_history() {
        let mut editor = Editor::default();
        for c in "as Jörg".chars() {
            editor.key(Key::Text(c), &[]);
        }
        editor.key(Key::Backspace, &[]);
        assert_eq!(editor.draft, "as Jör");
        editor.key(Key::Backspace, &[]);
        editor.key(Key::Backspace, &[]);
        assert_eq!(editor.draft, "as J");
        for _ in 0..300 {
            editor.key(Key::Text('ä'), &[]);
        }
        assert_eq!(editor.draft.chars().count(), 256);
        for i in 0..100 {
            editor.draft = format!("help {}", i % 2 + 1);
            editor.key(Key::Enter, &[]);
        }
        assert_eq!(editor.history.len(), 64);
    }
}
