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
    if let Some(indx) = state.details_state.selected() {
        let new_txt: String = state.modify_buffer.clone().unwrap_or("".to_string());
        let element: &mut AppElement = match state.get_selected_element_mut() {
            Some(element) => element,
            None => engine::create_new(state),
        };
        match indx {
            0 => {
                state.message = Some("ID may not be edited manually".to_string());
            },
            _ => {},
        }
        state.unsynced();
    }
}

async fn run_app<'t, B: Backend>(terminal: &'t mut Terminal<B>, cfg: AppConfig) -> io::Result<()> {
    let mut state: AppState = AppState::new(cfg);
    //let mut last_result: Option<reqwest::Error> = None;


    let view = ui::ui;

    loop {
        terminal.draw(|f| view(f, &mut state))?;
        if state.prompt.is_some() {
            terminal.draw(|f| ui::prompt_ui(f, &mut state))?;
        }

        // Match Keyboard Events
        if let Event::Key(key) = event::read()? {
            // Match CTRL+C
            if key.code == KeyCode::Char('c') && key.modifiers == KeyModifiers::CONTROL {
                return Ok(());
            }
            // Match whether any key was pressed
            if key.code != KeyCode::Null {
                // Clear Message
                state.message = None;
            } else {
                // We can also savely skip further evaluation when no key was
                // pressed
                continue;
            }
            // If we currently edit something we need to pass the chars:
            if state.buffer_modification() {
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
                                if key.modifiers == KeyModifiers::SHIFT {
                                    c.to_ascii_uppercase()
                                } else {
                                    c
                                }
                            );
                        }
                        continue;
                    },
                    _ => {
                        continue;
                    },
                }
            }
            // Match all keys controlling Commands functionality
            let command: AppCommand = AppCommand::from_key(key.code);
            match command {
                AppCommand::Sync => {
                    engine::sync(&mut state).await;
                }
                AppCommand::List => {
                    engine::disable_editing(&mut state);
                }
                AppCommand::Add => {
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
                AppCommand::Remove => {
                    match state.focused_on {
                        AppFocus::Elements => {
                            if state.remove_selected() {
                                state.unsynced();
                            };
                        },
                        AppFocus::Attributes => {

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
            // Match other keys for selection
            if key.modifiers == KeyModifiers::SHIFT {
                match key.code {
                    KeyCode::Up => {
                        engine::switch_up(&mut state);
                    },
                    KeyCode::Down => {
                        engine::switch_down(&mut state);
                    },
                    _ => (),
                }
            }
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
                    } else if state.list_state.selected().is_some() && state.details_state.selected().is_none() {
                        engine::enable_editing(&mut state);
                    } else if state.details_state.selected().is_some() {
                        engine::edit_selected(&mut state);
                    }
                }
                KeyCode::Up => {
                    if state.details_state.selected().is_none() {
                        select_prev_element(&mut state);
                    } else {
                        select_prev_field(&mut state);
                    }
                },
                KeyCode::Down => {
                    if state.details_state.selected().is_none() {
                        select_next_element(&mut state);
                    } else {
                        select_next_field(&mut state);
                    }
                },
                KeyCode::Left => {
                    engine::disable_editing(&mut state);
                }
                KeyCode::Right => {
                    if state.list_state.selected().is_some() && state.details_state.selected().is_none() {
                        engine::enable_editing(&mut state);
                    } else if state.details_state.selected().is_some() {
                        engine::edit_selected(&mut state);
                    }
                }
                _ => {}
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
