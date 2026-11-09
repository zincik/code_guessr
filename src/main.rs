use std::path::PathBuf;
use std::io::{stdout, Write};
use crossterm::cursor;
use rand::prelude::IndexedRandom;
use std::env;
use walkdir::WalkDir;
use crossterm::{
    cursor::{Hide, MoveTo, Show},
    event::{self, Event, KeyCode, KeyEventKind},
    execute, queue,
    terminal::{
        self, disable_raw_mode, enable_raw_mode, Clear, ClearType,
        EnterAlternateScreen, LeaveAlternateScreen,
    },
    style::{Print, Stylize}
};

struct TerminalGuard;

impl TerminalGuard {
    fn new() -> std::io::Result<Self> {
        enable_raw_mode()?;
        execute!(stdout(), EnterAlternateScreen, Hide)?;
        Ok(TerminalGuard)
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = execute!(stdout(), Show, LeaveAlternateScreen);
        let _ = disable_raw_mode();
    }
}


#[derive(Clone, Debug)]
struct SourceLine {
    source: PathBuf,
    line_number: usize,
    line: String,
}

fn pick_line(dir: &PathBuf, path_extension: &str, min_chars: usize) -> Option<SourceLine> {
    let files = WalkDir::new(dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .filter(|e| e.path().extension().is_some() && e.path().extension().unwrap() == path_extension)
        .collect::<Vec<_>>();
    let mut lines: Vec<SourceLine> = Vec::new();

    for file in files {
        if let Ok(content) = std::fs::read_to_string(file.path()) {
            for (i, line) in content.lines().enumerate() {
                if line.chars().filter(|c| !c.is_whitespace()).count() >= min_chars {
                    lines.push(SourceLine {
                        source: file.path().to_path_buf(),
                        line_number: i + 1,
                        line: line.to_string(),
                    });
                }
            }
        }
    }

    let picked_line = lines.choose(&mut rand::rng());
    if let Some(line) = picked_line {
        return Some(line.clone());
    }
    None
}

enum InspectorReturn {
    Submit,
    WrongSubmit,
    Quit,
}

// less-style viewer; runs inside the caller's TerminalGuard, so it creates no guard of its own
fn file_inspector(path: &PathBuf, source_line: &SourceLine) -> std::io::Result<InspectorReturn> {
    let content = String::from_utf8_lossy(&std::fs::read(path)?).into_owned();
    let lines: Vec<String> = content.lines().map(|l| l.replace('\t', "    ")).collect();
    let mut out = stdout();
    let mut top = 0usize;
    let mut cursor = 0usize;

    loop {
        let (w, h) = terminal::size()?;
        let page = (h as usize).saturating_sub(2).max(1); // last two rows: target line and status bar
        let max_top = lines.len().saturating_sub(page);
        cursor = cursor.min(lines.len().saturating_sub(1));
        // scroll the view just enough to keep the cursor visible
        if cursor < top {
            top = cursor;
        } else if cursor >= top + page {
            top = cursor + 1 - page;
        }
        top = top.min(max_top);

        queue!(out, Clear(ClearType::All))?;
        for (row, line) in lines.iter().skip(top).take(page).enumerate() {
            let shown: String = line.chars().take(w as usize).collect();
            if row + top == cursor {
                queue!(out, MoveTo(0, row as u16), Print(shown.reverse()))?;
            } else {
                queue!(out, MoveTo(0, row as u16), Print(shown))?;
            }
        }
        let pct = if max_top == 0 { 100 } else { top * 100 / max_top };
        let target: String = source_line.line.chars().take(w as usize).collect();
        let status = format!(
            "{}  line {}/{}  {}%  (q to go back) (Enter to submit)",
            path.display(),
            cursor + 1,
            lines.len(),
            pct
        );
        let status: String = status.chars().take(w as usize).collect();
        queue!(
            out,
            MoveTo(0, h.saturating_sub(2)), Print(target),
            MoveTo(0, h.saturating_sub(1)), Print(status.reverse())
        )?;
        out.flush()?;

        match event::read()? {
            Event::Key(k) if k.kind == KeyEventKind::Press => match k.code {
                KeyCode::Char('q') | KeyCode::Esc => break,
                KeyCode::Char('j') | KeyCode::Down => cursor += 1,
                KeyCode::Char('k') | KeyCode::Up => cursor = cursor.saturating_sub(1),
                KeyCode::Char(' ') | KeyCode::PageDown => cursor += page,
                KeyCode::Char('b') | KeyCode::PageUp => cursor = cursor.saturating_sub(page),
                KeyCode::Char('g') | KeyCode::Home => cursor = 0,
                KeyCode::Char('G') | KeyCode::End => cursor = lines.len().saturating_sub(1),
                KeyCode::Enter => {
                    if source_line.line.replace('\t', "    ") == lines[cursor] {
                        return Ok(InspectorReturn::Submit);
                    } else {
                        return Ok(InspectorReturn::WrongSubmit);
                    }
                },
                _ => {}
            },
            _ => {} // resize and other events just trigger a redraw
        }
    }
    Ok(InspectorReturn::Quit)
}

fn file_browser(dir_path: &PathBuf, ext: &str, source_line: &SourceLine) -> std::io::Result<InspectorReturn> {
    let _guard = TerminalGuard::new()?;
    let mut out = stdout();
    let mut current_dir = dir_path.clone();
    let mut current_selection_idx: usize = 0;

    loop {
        execute!(out, Clear(ClearType::All), MoveTo(0, 0))?;
        // Raw mode needs explicit carriage returns
        write!(out, "----- Looking for line ----\r\n")?;
        write!(out, "{}\r\n", source_line.line)?;
        write!(out, "---------------------------\r\n")?;
        write!(out, "{}\r\n", current_dir.display())?;
        let mut idx = 0;
        let entries = WalkDir::new(&current_dir).min_depth(1).max_depth(1).into_iter().filter_map(|e| e.ok()); 
        let mut selected_entry: Option<walkdir::DirEntry> = None;
        for entry in entries {
            if (entry.file_type().is_file() && entry.path().extension().is_some() && entry.path().extension().unwrap() == ext) || entry.file_type().is_dir() {
                if idx == current_selection_idx {
                    write!(out, "> {}\r\n", entry.path().display().to_string().reverse())?;
                    selected_entry = Some(entry);
                } else {
                    write!(out, "  {}\r\n", entry.path().display())?;
                }
                idx += 1;
            }
        }
        out.flush()?;

        match event::read()? {
            Event::Key(k) if k.kind == KeyEventKind::Press => match k.code {
                KeyCode::Char('q') | KeyCode::Esc => return Ok(InspectorReturn::Quit),
                KeyCode::Char('j') | KeyCode::Down => {
                    if current_selection_idx + 1 < idx {
                        current_selection_idx += 1;
                    }
                }
                KeyCode::Char('k') | KeyCode::Up => {
                    current_selection_idx = current_selection_idx.saturating_sub(1)
                }
                KeyCode::Char('o') | KeyCode::Right | KeyCode::Enter => {
                    if let Some(entry) = selected_entry {
                        if entry.file_type().is_dir() {
                            current_dir = entry.path().to_path_buf();
                            current_selection_idx = 0;
                        } else {
                            match file_inspector(&entry.path().to_path_buf(), source_line)? {
                                InspectorReturn::Quit => {continue},
                                InspectorReturn::Submit => return Ok(InspectorReturn::Submit),
                                InspectorReturn::WrongSubmit => return Ok(InspectorReturn::WrongSubmit),
                            }
                        }
                    }
                }
                KeyCode::Char('i') | KeyCode::Left | KeyCode::Backspace => {
                    if current_dir == *dir_path {
                        continue;
                    }
                    current_dir = current_dir.parent().unwrap_or(&current_dir).to_path_buf();
                    current_selection_idx = 0;
                }
                _ => {}
            },
            _ => {} // resize and other events just trigger a redraw
        }
    }
}

fn main() {
    let args: Vec<String> = env::args().collect();

    let ext = args.get(1)
        .map(|s| s.as_str())
        .unwrap_or_else(|| {
            eprintln!("File extension is required");
            std::process::exit(1);
        });

    let dir: PathBuf = env::args_os()
        .nth(2)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));

    if let Some(dir_path) = PathBuf::from(dir).canonicalize().ok() {
        if !dir_path.is_dir() {
            eprintln!("Directory path is not valid");
            return;
        }

        if let Some(line) = pick_line(&dir_path, ext, 40) {
            let time = std::time::Instant::now();
            match file_browser(&dir_path, &ext, &line) {
                Ok(InspectorReturn::Quit) => {
                    println!("Exited Code Guessr");
                },
                Ok(InspectorReturn::Submit) => {
                    println!("{}","Line found!".green());
                    println!("Source line:");
                    println!("{}:{} {}", line.source.display(), line.line_number.to_string().yellow(), line.line);
                    println!("Time taken: {:?}", time.elapsed());
                },
                Ok(InspectorReturn::WrongSubmit) => {
                    println!("{}","WRONG LINE BRO!".red());
                    println!("Source line:");
                    println!("{}:{} {}", line.source.display(), line.line_number.to_string().yellow(), line.line);
                    println!("Time taken: {:?}", time.elapsed());
                },
                Err(e) => eprintln!("{e}"),
            }
        } else {
            println!("No suitable line found in the directory");
        }
    }
}
