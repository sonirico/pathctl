use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use std::{
    env,
    error::Error,
    io,
    path::{Path, PathBuf},
};
use tui::{
    backend::{Backend, CrosstermBackend},
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Span, Spans},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
    Frame, Terminal,
};

fn main() -> Result<(), Box<dyn Error>> {
    // Configure terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Run application
    let res = run_app(&mut terminal);

    // Restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    match res {
        Ok(paths) => {
            let command = generate_shell_command(&paths);
            println!("{}", command);
        }
        Err(err) => {
            eprintln!("Error: {:?}", err);
        }
    }

    Ok(())
}

fn run_app<B: Backend>(terminal: &mut Terminal<B>) -> io::Result<Vec<PathBuf>> {
    let mut paths = get_path_entries();
    let mut list_state = ListState::default();
    if !paths.is_empty() {
        list_state.select(Some(0));
    }
    let mut input_mode = InputMode::Normal;
    let mut input = String::new();

    loop {
        terminal.draw(|f| draw(f, &paths, &mut list_state, &input_mode, &input))?;

        if event::poll(std::time::Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                match input_mode {
                    InputMode::Normal => {
                        // Handle quitting the application
                        if key.code == KeyCode::Char('q')
                            || key.code == KeyCode::Esc
                            || (key.code == KeyCode::Char('c')
                                && key.modifiers == KeyModifiers::CONTROL)
                        {
                            return Ok(paths);
                        }

                        match key.code {
                            KeyCode::Char('a') => {
                                // Enter input mode to insert after
                                input_mode = InputMode::InsertAfter;
                                input.clear();
                            }
                            KeyCode::Char('b') => {
                                // Enter input mode to insert before
                                input_mode = InputMode::InsertBefore;
                                input.clear();
                            }
                            KeyCode::Char('d') => {
                                if let Some(selected) = list_state.selected() {
                                    paths.remove(selected);
                                    let new_index = if selected >= paths.len() {
                                        paths.len().saturating_sub(1)
                                    } else {
                                        selected
                                    };
                                    if paths.is_empty() {
                                        list_state.select(None);
                                    } else {
                                        list_state.select(Some(new_index));
                                    }
                                }
                            }
                            KeyCode::Up | KeyCode::Char('k') => {
                                let i = match list_state.selected() {
                                    Some(i) => {
                                        if i > 0 {
                                            Some(i - 1)
                                        } else {
                                            Some(0)
                                        }
                                    }
                                    None => Some(0),
                                };
                                list_state.select(i);
                            }
                            KeyCode::Down | KeyCode::Char('j') => {
                                let i = match list_state.selected() {
                                    Some(i) => {
                                        if i < paths.len() - 1 {
                                            Some(i + 1)
                                        } else {
                                            Some(paths.len() - 1)
                                        }
                                    }
                                    None => Some(0),
                                };
                                list_state.select(i);
                            }
                            _ => {}
                        }
                    }
                    InputMode::InsertAfter => {
                        handle_input_mode(
                            key,
                            &mut input,
                            &mut input_mode,
                            &mut paths,
                            &mut list_state,
                            InsertionPoint::After,
                        );
                    }
                    InputMode::InsertBefore => {
                        handle_input_mode(
                            key,
                            &mut input,
                            &mut input_mode,
                            &mut paths,
                            &mut list_state,
                            InsertionPoint::Before,
                        );
                    }
                }
            }
        }
    }
}

fn handle_input_mode(
    key: event::KeyEvent,
    input: &mut String,
    input_mode: &mut InputMode,
    paths: &mut Vec<PathBuf>,
    list_state: &mut ListState,
    insertion_point: InsertionPoint,
) {
    match key.code {
        KeyCode::Enter => {
            let new_path = PathBuf::from(input.trim());
            if new_path.exists() {
                insert_path_at_selection(paths, list_state, new_path, insertion_point);
            }
            input.clear();
            *input_mode = InputMode::Normal;
        }
        KeyCode::Esc => {
            input.clear();
            *input_mode = InputMode::Normal;
        }
        KeyCode::Char(c) => {
            input.push(c);
        }
        KeyCode::Backspace => {
            input.pop();
        }
        _ => {}
    }
}

#[derive(Clone, Copy)]
enum InputMode {
    Normal,
    InsertAfter,
    InsertBefore,
}

#[derive(Clone, Copy)]
enum InsertionPoint {
    Before,
    After,
}

fn insert_path_at_selection(
    paths: &mut Vec<PathBuf>,
    list_state: &mut ListState,
    new_path: PathBuf,
    insertion_point: InsertionPoint,
) {
    let selected_index = list_state.selected().unwrap_or(0);
    let insert_index = match insertion_point {
        InsertionPoint::Before => selected_index,
        InsertionPoint::After => selected_index + 1,
    };
    let insert_index = insert_index.min(paths.len()); // Ensure we don't go out of bounds
    paths.insert(insert_index, new_path);
    list_state.select(Some(insert_index));
}

fn draw<B: Backend>(
    f: &mut Frame<B>,
    paths: &[PathBuf],
    list_state: &mut ListState,
    input_mode: &InputMode,
    input: &str,
) {
    let size = f.size();

    // Adjust layout to include commands footer
    let constraints = match input_mode {
        InputMode::Normal => vec![
            Constraint::Min(1),    // List of paths
            Constraint::Length(3), // Commands footer
        ],
        InputMode::InsertAfter | InputMode::InsertBefore => vec![
            Constraint::Min(1),
            Constraint::Length(3), // Input box
            Constraint::Length(3), // Commands footer
        ],
    };

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints(constraints)
        .split(size);

    // Create the list items
    let items: Vec<ListItem> = paths
        .iter()
        .map(|p| ListItem::new(p.display().to_string()))
        .collect();

    // Create the list widget
    let list = List::new(items)
        .block(Block::default().borders(Borders::ALL).title("PATH Entries"))
        .highlight_style(
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol(">> ");

    // Render the list widget with the ListState
    f.render_stateful_widget(list, chunks[0], list_state);

    // Determine which additional widget to render based on input mode
    let mut commands_chunk_index = 1;
    match input_mode {
        InputMode::Normal => {}
        InputMode::InsertAfter => {
            let input_block = Paragraph::new(input)
                .style(Style::default().fg(Color::Cyan))
                .block(Block::default().borders(Borders::ALL).title("Insert After"));
            f.render_widget(input_block, chunks[1]);
            f.set_cursor(chunks[1].x + input.len() as u16 + 1, chunks[1].y + 1);
            commands_chunk_index = 2;
        }
        InputMode::InsertBefore => {
            let input_block = Paragraph::new(input)
                .style(Style::default().fg(Color::Cyan))
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title("Insert Before"),
                );
            f.render_widget(input_block, chunks[1]);
            f.set_cursor(chunks[1].x + input.len() as u16 + 1, chunks[1].y + 1);
            commands_chunk_index = 2;
        }
    }

    // Render the commands footer
    let commands = vec![Spans::from(vec![
        Span::styled("a", Style::default().add_modifier(Modifier::BOLD)),
        Span::raw(": Insert after   "),
        Span::styled("b", Style::default().add_modifier(Modifier::BOLD)),
        Span::raw(": Insert before   "),
        Span::styled("d", Style::default().add_modifier(Modifier::BOLD)),
        Span::raw(": Delete   "),
        Span::styled("↑/k", Style::default().add_modifier(Modifier::BOLD)),
        Span::raw(": Up   "),
        Span::styled("↓/j", Style::default().add_modifier(Modifier::BOLD)),
        Span::raw(": Down   "),
        Span::styled(
            "q/ESC/Ctrl+C",
            Style::default().add_modifier(Modifier::BOLD),
        ),
        Span::raw(": Quit"),
    ])];

    let commands_paragraph = Paragraph::new(commands).block(Block::default().borders(Borders::ALL));

    f.render_widget(commands_paragraph, chunks[commands_chunk_index]);
}

fn get_path_entries() -> Vec<PathBuf> {
    #[cfg(target_os = "windows")]
    {
        get_windows_path_entries()
    }
    #[cfg(not(target_os = "windows"))]
    {
        if let Ok(path_var) = env::var("PATH") {
            env::split_paths(&path_var).collect()
        } else {
            vec![]
        }
    }
}

fn generate_shell_command(paths: &[PathBuf]) -> String {
    let new_path_var = env::join_paths(paths).expect("Failed to join paths");
    let new_path_str = new_path_var.to_string_lossy();

    // Detect the shell to output appropriate commands
    let shell = detect_shell();
    match shell.as_deref() {
        Some("fish") => format!("set -x PATH {}", new_path_str),
        _ => format!("export PATH=\"{}\"", new_path_str),
    }
}

#[cfg(target_os = "windows")]
fn get_windows_path_entries() -> Vec<PathBuf> {
    use winreg::enums::*;
    use winreg::RegKey;

    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let env = hkcu.open_subkey("Environment").unwrap();
    let path_var: String = env.get_value("Path").unwrap_or_default();
    env::split_paths(&path_var).collect()
}

#[cfg(not(target_os = "windows"))]
fn detect_shell() -> Option<String> {
    env::var("SHELL").ok().and_then(|path| {
        Path::new(&path)
            .file_name()
            .and_then(|os_str| os_str.to_str())
            .map(|s| s.to_string())
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;
    use std::path::PathBuf;

    #[test]
    fn test_get_path_entries() {
        // Backup the original PATH
        let original_path = env::var("PATH").unwrap_or_default();

        // Set a known PATH value
        let test_paths = ["/usr/bin", "/bin", "/usr/local/bin"];
        let test_path_var = env::join_paths(test_paths.iter().map(PathBuf::from)).unwrap();
        env::set_var("PATH", &test_path_var);

        // Call the function
        let paths = get_path_entries();

        // Verify the result
        let expected_paths: Vec<PathBuf> = test_paths.iter().map(PathBuf::from).collect();
        assert_eq!(paths, expected_paths);

        // Restore the original PATH
        env::set_var("PATH", &original_path);
    }

    #[test]
    fn test_generate_shell_command() {
        // Prepare new paths
        let new_paths = vec![PathBuf::from("/custom/bin"), PathBuf::from("/another/bin")];

        // Generate the shell command
        let command = generate_shell_command(&new_paths);

        // Detect shell
        let shell = detect_shell();
        let expected_command = {
            let new_path_var = env::join_paths(&new_paths).expect("Failed to join paths");
            let new_path_str = new_path_var.to_string_lossy();

            match shell.as_deref() {
                Some("fish") => format!("set -x PATH {}", new_path_str),
                _ => format!("export PATH=\"{}\"", new_path_str),
            }
        };

        // Verify the command
        assert_eq!(command, expected_command);
    }

    #[test]
    fn test_insert_path_at_selection() {
        let mut paths = vec![
            PathBuf::from("/usr/bin"),
            PathBuf::from("/bin"),
            PathBuf::from("/usr/local/bin"),
        ];
        let mut list_state = ListState::default();
        list_state.select(Some(1)); // Select the second item

        let new_path = PathBuf::from("/custom/bin");
        insert_path_at_selection(
            &mut paths,
            &mut list_state,
            new_path.clone(),
            InsertionPoint::Before,
        );

        let expected_paths = vec![
            PathBuf::from("/usr/bin"),
            PathBuf::from("/custom/bin"), // New path inserted here
            PathBuf::from("/bin"),
            PathBuf::from("/usr/local/bin"),
        ];
        assert_eq!(paths, expected_paths);
        assert_eq!(list_state.selected(), Some(1));

        // Test inserting after
        let new_path2 = PathBuf::from("/another/bin");
        insert_path_at_selection(
            &mut paths,
            &mut list_state,
            new_path2.clone(),
            InsertionPoint::After,
        );

        let expected_paths_after = vec![
            PathBuf::from("/usr/bin"),
            PathBuf::from("/custom/bin"),
            PathBuf::from("/another/bin"), // New path inserted here
            PathBuf::from("/bin"),
            PathBuf::from("/usr/local/bin"),
        ];
        assert_eq!(paths, expected_paths_after);
        assert_eq!(list_state.selected(), Some(2));
    }

    #[cfg(not(target_os = "windows"))]
    #[test]
    fn test_detect_shell() {
        // Backup the original SHELL
        let original_shell = env::var("SHELL").ok();

        // Set a test SHELL value
        env::set_var("SHELL", "/bin/bash");

        let shell_name = detect_shell().expect("Shell should be detected");
        assert_eq!(shell_name, "bash");

        // Test with another shell
        env::set_var("SHELL", "/usr/bin/zsh");
        let shell_name = detect_shell().expect("Shell should be detected");
        assert_eq!(shell_name, "zsh");

        // Restore the original SHELL
        if let Some(shell) = original_shell {
            env::set_var("SHELL", shell);
        } else {
            env::remove_var("SHELL");
        }
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn test_get_windows_path_entries() {
        // For Windows, testing registry interactions requires caution.
        // Ensure tests do not modify the actual registry or use mocking techniques.
        let paths = get_windows_path_entries();
        // Verify that paths are returned
        assert!(!paths.is_empty());
    }
}
