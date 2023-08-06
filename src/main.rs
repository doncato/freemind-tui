mod data;
use crate::data::data_types::{AppState, AppConfig, AppElement, AppCommand, AuthMethod};
use clap::{Arg, Command, ArgMatches, crate_name, crate_authors, crate_description, crate_version, ArgAction};
use std::path::PathBuf;
use std::fs;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use dialoguer::{Input, Confirm, Password, FuzzySelect, Select, theme::ColorfulTheme, console::Term};
use std::{error::Error, io};
use tui::{
    style::Style,
    backend::{Backend, CrosstermBackend},
    layout::{Constraint, Direction, Layout, Alignment},
    widgets::{Block, Borders, Paragraph, Wrap, List, ListItem, Clear, BorderType, Row, Table},
    Frame, Terminal, text::{Spans, Span}, style::{Color, Modifier},
};

/// Read the app configuration
fn obtain_app_config() -> Option<AppConfig> {
    let mut path = dirs::config_dir().unwrap_or(PathBuf::new());
    path.push("freemind/");
    fs::create_dir_all(path.clone()).ok();
    path.push("freemind-cli.config");
    confy::load_path(path).ok()
}

/// Save the app configuration
fn write_app_config(config: &AppConfig) -> Option<()> {
    let mut path = dirs::config_dir().unwrap_or(PathBuf::new());
    path.push("freemind/");
    fs::create_dir_all(path.clone()).ok();
    path.push("freemind-cli.config");
    confy::store_path(path, config).ok();
    Some(())
}

/// Configuration Setup Dialog
fn setup_config(prev_config: &AppConfig) -> Result<AppConfig, std::io::Error> {
    println!("\n   ### Config Setup: ###\n");
    let server_address: String = Input::new()
        .with_prompt("URL of the server to connect to")
        .with_initial_text(&prev_config.server_address)
        .interact_text()?;

    let username: String = Input::new()
        .with_prompt("Your username")
        .with_initial_text(&prev_config.username)
        .interact_text()?;

    let auth_method: AuthMethod = AuthMethod::from(Select::with_theme(&ColorfulTheme::default())
        .with_prompt("How do you want to authenticate?")
        .items(&vec!["API Token", "Password"])
        .default(0)
        .interact_on_opt(&Term::stderr())?.unwrap_or(0));

    let secret: String = match auth_method {
        AuthMethod::Token => Input::new()
            .with_prompt("Your API Token")
            .interact_text()?,
        AuthMethod::Password => Password::new()
            .with_prompt("Your Password")
            .interact()?
    };

    let config: AppConfig = AppConfig::new(
        server_address,
        username,
        secret,
        auth_method,
    );

    println!("\nDone! You entered the following config:\n\n{}\n", config);
    if Confirm::new().with_prompt("Do you want to accept this config?").interact()? {
        return Ok(config);
    } else {
        println!("\n");
        return setup_config(&config);
    }
}

/// Initialize the main config
fn init() -> AppConfig {
    let args: ArgMatches = Command::new("Freemind TUI")
        .author(crate_authors!("\n"))
        .about(crate_description!())
        .version(crate_version!())
        .args_override_self(true)
        .arg(Arg::new("config")
            .short('c')
            .long("config")
            .action(ArgAction::SetTrue)
            .help("Enter the configuration setup")
        )
        .arg(Arg::new("skip-config-load")
            .long("skip-config-load")
            .action(ArgAction::SetTrue)
            .help("Skip loading and saving of the configuration file")
        )
        .get_matches();

    let config_setup: &bool = args.get_one("config").unwrap_or(&false);
    let config_skip: &bool = args.get_one("skip-config-load").unwrap_or(&false);

    let mut config: AppConfig = AppConfig::empty();
    if !config_skip {
        config = obtain_app_config()
            .expect("FATAL! Failed to create or read config! (tried under '~/.config/freemind/freemind-cli.config')\nRun with `--skip-config-load` to avoid this issue, or fix your file permissions!");
    }

    if *config_setup || config.is_default() || config.is_empty() {
        println!("Config could not be read, found or was skipped.\nEntering Configuration Setup:");
        config = setup_config(&config).expect("FATAL! Setup Dailog encountered an error!");
        if write_app_config(&config).is_none() {
            println!("ATTENTION: Config could not be written! Proceeding with supplied config this time...");
        } else {
            println!("Success!\n");
        }
    }

    config
}

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

fn ui<B: Backend>(f: &mut Frame<B>, state: &mut AppState) {
    let standard_block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Rgb(0x05, 0xD9, 0xE8)))
        .border_type(BorderType::Rounded);
    let standard_style = Style::default()
        .fg(Color::Rgb(0xD1, 0xF7, 0xFF))
        .bg(Color::Rgb(0x01, 0x01, 0x2B));
    let alt_block = Block::default()
        .borders(Borders::NONE);
    let alt_style = Style::default()
        .fg(Color::Rgb(0xFF, 0x2A, 0x6D))
        .bg(Color::Rgb(0x01, 0x01, 0x2B));

    let chunks = Layout::default()
        .margin(0)
        .direction(Direction::Vertical)
        .constraints(
            [
                Constraint::Length(1),
                Constraint::Min(1),
                Constraint::Length(1),
            ]
            .as_ref(),
        )
        .split(f.size());

    let top_bar = Layout::default()
        .direction(Direction::Horizontal)
        .constraints(
            [
                Constraint::Percentage(50),
                Constraint::Percentage(50),
            ]
            .as_ref(),
        )
        .split(chunks[0]);

    // Top Bar
    let top_left_text = format!(
        ":// {} {} - {}",
        crate_name!(),
        crate_version!(),
        state.modified_string()
    );
    let top_left = Paragraph::new(top_left_text)
        .block(alt_block.clone())
        .style(alt_style)
        .alignment(Alignment::Left)
        .wrap(Wrap { trim: true });
    f.render_widget(top_left, top_bar[0]);

    let top_right_text = state.message.clone().unwrap_or("".to_string());
    let top_right = Paragraph::new(top_right_text)
        .block(alt_block.clone())
        .style(alt_style)
        .alignment(Alignment::Right)
        .wrap(Wrap { trim: true });
    f.render_widget(top_right, top_bar[1]);

    // Main View
    let main_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .margin(0)
        .constraints(
            [
                Constraint::Percentage(15),
                Constraint::Percentage(55),
                Constraint::Percentage(30)
            ]
            .as_ref(),
        )
        .split(chunks[1]);

    let elements: Vec<ListItem<'_>> = state
        .get_elements()
        .iter()
        .map(|e| e.to_list_item())
        .collect();
    let elements_list = List::new(elements)
        .block(standard_block.clone())
        .style(standard_style)
        .highlight_style(Style::default().add_modifier(Modifier::REVERSED));
    f.render_stateful_widget(elements_list, main_chunks[0], &mut state.list_state);

    /*
    let element_string = if let Some(selected_element) = state.get_selected_element() {
        selected_element.to_string()
    } else {
        "".to_string()
    };
    let details = Paragraph::new(element_string)
        .block(standard_block.clone())
        .style(standard_style)
        .wrap(Wrap { trim: false });
    f.render_widget(details, main_chunks[1]);
    */
    /*
    if let Some(selected_element) = state.get_selected_element() {
        state.prompt = Some(selected_element.get_vecs()[0].len().to_string());
    }
    */

    let vec_details: Vec<Row> = if let Some(selected_element) = state.get_selected_element() {
        selected_element.get_vecs().into_iter().map(|e| Row::new(e).height(2)).collect()
    } else {
        Vec::new()
    };
    let details_table = Table::new(vec_details)
        .block(standard_block.clone())
        .widths(&[Constraint::Percentage(30), Constraint::Percentage(70)])
        .column_spacing(1)
        .style(standard_style)
        .highlight_style(Style::default().add_modifier(Modifier::REVERSED));
    
    f.render_stateful_widget(details_table, main_chunks[1], &mut state.details_state);

    let right_pane = Paragraph::new("")
        .block(standard_block.clone())
        .style(standard_style)
        .wrap(Wrap { trim: false });
    f.render_widget(right_pane, main_chunks[2]);

    // Footer
    let actions_text = AppCommand::get_command_list_string().join(" ");
    let actions = Paragraph::new(actions_text)
        .block(alt_block)
        .style(alt_style)
        .alignment(Alignment::Center)
        .wrap(Wrap { trim: true });
    f.render_widget(actions, chunks[2]);
}

fn prompt_ui<B: Backend>(f: &mut Frame<B>, state: &mut AppState) {
    let vert_chunks = Layout::default()
        .margin(1)
        .direction(Direction::Vertical)
        .constraints(
            [
                Constraint::Percentage(30),
                Constraint::Percentage(30),
                Constraint::Percentage(30),
            ]
            .as_ref(),
        )
        .split(f.size());

    let horz_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .margin(1)
        .constraints(
            [
                Constraint::Percentage(30),
                Constraint::Percentage(30),
                Constraint::Percentage(30),
            ]
            .as_ref(),
        )
        .split(vert_chunks[1]);

    let the_chunk = horz_chunks[1];

    //f.render_widget(Clear, the_chunk);

    let prompt_text = format!(
        "{}\n\nPress [Q] to quit\nPress [Enter] key to close",
        state.prompt.clone().unwrap_or("".to_string()),
    );
    let prompt = Paragraph::new(prompt_text)
        .block(Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Rgb(0xFF, 0x2A, 0x6D)))
            .border_type(BorderType::Rounded)
        )
        .style(Style::default().fg(Color::Rgb(0xD1, 0xF7, 0xFF)).bg(Color::Rgb(0x00, 0x56, 0x78)))
        .alignment(Alignment::Center)
        .wrap(Wrap { trim: false });

    f.render_widget(prompt, the_chunk);

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

fn enable_editing(state: &mut AppState) {
    if state.details_state.selected().is_none() {
        state.details_state.select(Some(0));
    }
}

fn disable_editing(state: &mut AppState) {
    if state.details_state.selected().is_some() {
        state.details_state.select(None);
    }
}

async fn run_app<B: Backend>(terminal: &mut Terminal<B>) -> io::Result<()> {
    let cfg: AppConfig = init();
    let mut state: AppState = AppState::new(cfg);
    let mut last_result: Option<reqwest::Error> = None;

    let mut view = ui;
    loop {
        terminal.draw(|f| view(f, &mut state))?;
        if state.prompt.is_some() {
            terminal.draw(|f| prompt_ui(f, &mut state))?;
        }

        
        if let Event::Key(key) = event::read()? {
            // Match CTRL+C
            if key.code == KeyCode::Char('c') && key.modifiers == KeyModifiers::CONTROL {
                return Ok(());
            }
            // Match whether any key was pressed
            if key.code != KeyCode::Null {
                // Clear Message
                state.message = None;
            }
            // Match all keys controlling Commands functionality
            let command: AppCommand = AppCommand::from_key(key.code);
            match command {
                AppCommand::Sync => {
                    last_result = state.sync().await.err()
                }
                AppCommand::List => {
                    disable_editing(&mut state);
                }
                AppCommand::Remove => {
                    if state.remove_selected() {
                        state.unsynced();
                    };
                }
                AppCommand::Edit => {
                    enable_editing(&mut state);
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
                        enable_editing(&mut state);
                    } else if state.details_state.selected().is_some() {
                        //edit_selected(&mut state);
                        state.message = Some("Not yet implemented!".to_string());
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
                    disable_editing(&mut state);
                }
                KeyCode::Right => {
                    enable_editing(&mut state);
                }
                _ => {}
            }
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), io::Error> {
    let mut config: AppConfig = init();

    // setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // create app and run it
    let res = run_app(&mut terminal).await;

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
