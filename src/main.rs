mod app;
use crate::app::{engine, ui};

mod data;
use crate::data::data_types::{AppState, AppConfig, AppElement, AppCommand};

use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use data::data_types::{AppFocus, NodeName};
use std::io;
use tui::{
    backend::{Backend, CrosstermBackend},
    layout::{Constraint, Direction, Layout},
    widgets::{Block, Borders},
    Frame, Terminal, 
};


/*
fn set_up_ui<B: Backend>(f: &mut Frame<B>) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints(
            [
                Constraint::Percentage(20),
                Constraint::Percentage(80),
            ]
            .as_ref(),
        )
        .split(f.size());

    let block = Block::default().title("Block").borders(Borders::ALL);
    f.render_widget(block, chunks[0]);
    let block = Block::default().title("Block 2").borders(Borders::ALL);
    f.render_widget(block, chunks[1]);
}
*/


fn select_prev_element(state: &mut AppState) {
    let len: usize = state.get_elements().len();
    if len != 0 {
        let max: usize = len-1;
        let mut a: usize = state.list_state.selected().unwrap_or(1);
        if a <= 0 {
            a = max
        } else {
            a -= 1
        }
        state.list_state.select(Some(a));
    }
}

fn select_next_element(state: &mut AppState) {
    let len: usize = state.get_elements().len();
    if len != 0 {
        let max: usize = len-1;
        let mut a: usize = state.list_state.selected().unwrap_or(max);
        if a >= max {
            a = 0
        } else {
            a += 1
        }
        state.list_state.select(Some(a));
    }
}

fn select_prev_field(state: &mut AppState) {
    let len = 5;
    let max = len-1;
    let mut a: usize = state.details_state.selected().unwrap_or(1);
    if a <= 0 {
        a = max
    } else {
        a -= 1
    }
    state.details_state.select(Some(a));
}

fn select_next_field(state: &mut AppState) {
    let len = 5;
    let max = len-1;
    let mut a: usize = state.details_state.selected().unwrap_or(max);
    if a >= max {
        a = 0
    } else {
        a += 1
    }
    state.details_state.select(Some(a));
}

fn save_changes(state: &mut AppState) {
    let new_txt: String = state.modify_buffer.clone().unwrap_or("".to_string());
    if let Some(node) = state.get_selected_attribute() {
        let element: &mut AppElement = match state.get_selected_element_mut() {
            Some(element) => element,
            None => engine::create_new(state),
        };
        element.nodes().insert(node.0, new_txt);
        element.modified();
        state.unsynced();
    }
}

async fn run_app<'t, B: Backend>(terminal: &'t mut Terminal<B>, cfg: AppConfig) -> io::Result<()> {
    let mut state: AppState = AppState::new(cfg);
    //let mut last_result: Option<reqwest::Error> = None;


    let view = ui::ui;

    'main: loop {
        terminal.draw(|f| view(f, &mut state))?;
        if state.prompt.is_some() {
            terminal.draw(|f| ui::prompt_ui(f, &mut state))?;
        }

        // Match Keyboard Events
        if let Event::Key(key) = event::read()? {
            // Ignore if no key was pressed
            if key.code == KeyCode::Null {
                continue;
            }

            // Clear Message
            state.message = None;

            // Match Events with Control
            if key.modifiers == KeyModifiers::CONTROL {
                match key.code {
                    KeyCode::Char('c') | KeyCode::Char('q') => {
                        break 'main Ok(());
                    }
                    _ => (),
                }
            } else if state.buffer_modification() { // If we currently edit something we need to pass the chars:
                match key.code {
                    KeyCode::Esc | KeyCode::Left => {
                        state.modify_buffer = None;
                        continue;
                    },
                    KeyCode::Enter | KeyCode::Up | KeyCode::Down => {
                        save_changes(&mut state);
                        state.modify_buffer = None;
                        if key.code == KeyCode::Enter {
                            continue;
                        }
                    },
                    KeyCode::Backspace => {
                        if let Some(buf) = state.modify_buffer.as_mut() {
                            buf.pop();
                        }
                        continue;
                    }
                    KeyCode::Char(c) => {
                        if let Some(buf) = state.modify_buffer.as_mut() {
                            buf.push(
                                c
                            );
                        }
                        continue;
                    },
                    _ => (),
                }
            } else if key.modifiers == KeyModifiers::SHIFT {
                match state.focused_on {
                    AppFocus::Elements => {
                        match key.code {
                            KeyCode::Up | KeyCode::Char('W') => {
                                engine::switch_up(&mut state);
                                select_prev_element(&mut state);
                            },
                            KeyCode::Down | KeyCode::Char('S') => {
                                engine::switch_down(&mut state);
                                select_next_element(&mut state);
                            },
                            _ => (),
                        }
                    },
                    _ => ()
                }
            } else {
                let command: AppCommand = AppCommand::from_key(key.code);

                // Match all keys controlling Commands functionality
                match command {
                    AppCommand::Refresh => {
                        engine::sync(&mut state).await;
                    }
                    AppCommand::Fill => {
                        match state.focused_on {
                            AppFocus::Elements => {
                                engine::create_new(&mut state);
                                engine::enable_editing(&mut state);
                                engine::edit_selected(&mut state);
                            },
                            AppFocus::Attributes => {
                                engine::create_attribute(&mut state);
                            },
                            _ => ()
                        }
                    }
                    AppCommand::Clear => {
                        match state.focused_on {
                            AppFocus::Elements => {
                                if state.remove_element() {
                                    state.unsynced();
                                };
                            },
                            AppFocus::Attributes => {
                                if state.remove_attribute() {
                                    state.unsynced();
                                };
                            },
                            _ => ()
                        }
                    }
                    AppCommand::Edit => {
                        engine::enable_editing(&mut state);
                        engine::edit_selected(&mut state);
                    }
                    AppCommand::Quit => {
                        if state.is_synced() || state.prompt.is_some() {
                            return Ok(())
                        } else {
                            state.prompt = Some("You have unsynced changes!\nDo you really want to exit?".to_string());
                        }
                    },
                    AppCommand::None => {},
                    _ => {}
                };
                match key.code {
                    KeyCode::Esc => {
                        if state.prompt.is_some() {
                            state.prompt = None;
                        }
                        else if state.details_state.selected().is_some() {
                            state.details_state.select(None);
                        } else {
                            state.list_state.select(None)
                        }
                    },
                    KeyCode::Enter => {
                        if state.prompt.is_some() {
                            state.prompt = None;
                        } else if state.focused_on.elements() {
                            engine::enable_editing(&mut state);
                        } else if state.focused_on.attributes() {
                            engine::edit_selected(&mut state);
                        }
                    }
                    KeyCode::Up | KeyCode::Char('w') => {
                        if state.focused_on.elements() {
                            select_prev_element(&mut state);
                        } else {
                            select_prev_field(&mut state);
                        }
                    },
                    KeyCode::Down | KeyCode::Char('s') => {
                        if state.focused_on.elements() {
                            select_next_element(&mut state);
                        } else {
                            select_next_field(&mut state);
                        }
                    },
                    KeyCode::Left | KeyCode::Char('a') => {
                        engine::disable_editing(&mut state);
                    }
                    KeyCode::Right | KeyCode::Char('d') => {
                        if state.focused_on.elements() {
                            engine::enable_editing(&mut state);
                        } else if state.focused_on.attributes() {
                            engine::edit_selected(&mut state);
                        }
                    }
                    _ => {}
                }
            }
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), io::Error> {
    // Obtain Config
    let config: AppConfig = engine::init();

    // Set up Terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // create app and run it
    let res: Result<(), io::Error> = run_app(&mut terminal, config).await;

    // restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    if let Err(err) = res {
        println!("{:?}", err)
    }

    Ok(())
}
