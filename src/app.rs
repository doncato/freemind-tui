pub(crate) mod engine {
    use crate::ui;
    use crate::data::data_types::{AppState, AppConfig, AppElement};
    use clap::{Arg, Command, ArgMatches, crate_authors, crate_description, crate_version, ArgAction};
    use std::collections::HashMap;
    use std::{fs, path::PathBuf};


    /// This Enum represents all possible outcomes from checks
    /*
    enum InitializerStatus {
        Passed,
        Failed,
        Skipped,
    }

    impl fmt::Display for InitializerStatus {
        fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
            match self {
                Passed => write!(fmt, "PASSED"),
                Failed => write!(fmt, "FAILED"),
                Skipped => write!(fmt, "SKIPPED"),

            }
        }
    }
    */

    /// Syncs the current app state with the configured remote
    /// returns true on success, false on failure
    pub async fn sync<'t>(state: &'t mut AppState<'t>) -> bool {
        state.sync().await.is_err()
    }

    pub fn enable_editing(state: &mut AppState) {
        if state.details_state.selected().is_none() {
            state.details_state.select(Some(1));
        }
    }
    
    pub fn disable_editing(state: &mut AppState) {
        if state.details_state.selected().is_some() {
            state.details_state.select(None);
        }
    }
    
    pub fn create_new<'t>(state: &mut AppState) -> &'t mut AppElement<'t> {
        let new_element: AppElement = AppElement::new(
            None,
            HashMap::new()
        );
        state.push(Some(new_element));
        let indx = state.get_elements().len()-1;
        state.list_state.select(Some(indx));
        return state.get_selected_element_mut().expect("FATAL New element not found");
    }

    pub fn create_attribute(state: &mut AppState) -> () {
        let element: &AppElement = match state.get_selected_element() {
            Some(element) => element,
            None => return (),
        };
        //element.attributes.push(String::new(), String::new());

    }
    
    
    pub fn edit_selected(state: &mut AppState) {
        if let Some(indx) = state.details_state.selected() {
            match indx {
                0 => {
                    state.message = Some("ID may not be edited manually".to_string());
                },
                1 | 2 | 3 | 4 => {
                    state.modify_buffer = ui::get_selected_details(state);
                },
                _ => {},
            }
        }
    }

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

    /// Obtains the runtime params
    fn get_app_config() -> AppConfig {
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
            println!("Config could not be read, found or was skipped.\nMake sure to enter your configuration!");
        }

        config
    }

    /// Initialize the app
    pub fn init() -> AppConfig {
        println!("Initializing...");
        let config = get_app_config();

        config
    }
}
pub(crate) mod ui {
    use crate::{AppState, AppCommand, AppElement, data::data_types::AppFocus};
    use clap::{crate_name, crate_version};
    use chrono::{Utc, LocalResult, TimeZone};
    use tui::{
        style::Style,
        backend::{Backend, CrosstermBackend},
        layout::{Constraint, Direction, Layout, Alignment},
        widgets::{Block, Borders, Paragraph, Wrap, List, ListItem, BorderType, Row, Table},
        Frame, Terminal, text::{Spans, Span}, style::{Color, Modifier}, 
    };

    pub fn get_selected_details(state: &AppState) -> Option<String> {
        if let Some(indx) = state.details_state.selected() {
            let element: &AppElement = match state.get_selected_element() {
                Some(element) => element,
                None => return None,
            };
        return match indx {
            1 => {
                Some(element.title().unwrap_or(&"".to_string()).to_string())
            },
            2 => {
                Some(element.description().unwrap_or(&"".to_string()).to_string())
            },
            3 => {
                Some({
                    if let Some(due) = element.due() {
                        let due_timestamp: i64 = due.into();
                        match Utc.timestamp_opt(due_timestamp, 0) {
                            LocalResult::None => "None".to_string(),
                            LocalResult::Single(val) | LocalResult::Ambiguous(val, _) => {
                                val.with_timezone(&chrono::Local).format("%d.%m.%y %H:%M").to_string()
                            }
                        }
                    } else {
                        "None".to_string()
                    }
                })
            },
            _ => None
        };
        }
        else {None}
    }

    pub fn prompt_ui<B: Backend>(f: &mut Frame<B>, state: &mut AppState) {
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

    pub fn ui<B: Backend>(f: &mut Frame<B>, state: &mut AppState) {
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
    
        let main_layout = Layout::default()
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
    
        // Top Bar
        {
            let top_bar = Layout::default()
                .direction(Direction::Horizontal)
                .constraints(
                    [
                        Constraint::Percentage(50),
                        Constraint::Percentage(50),
                    ]
                    .as_ref(),
                )
                .split(main_layout[0]);
        
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
        
            let top_right_text = state
                .message
                .clone()
                .unwrap_or(""
                .to_string());

            let top_right = Paragraph::new(top_right_text)
                .block(alt_block.clone())
                .style(alt_style)
                .alignment(Alignment::Right)
                .wrap(Wrap { trim: true });
            f.render_widget(top_right, top_bar[1]);
        }
    
        // Main View
        let main_view = Layout::default()
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
            .split(main_layout[1]);
    
        let elements: Vec<ListItem<'_>> = state
            .get_elements()
            .iter()
            .map(|e| e.to_list_item())
            .collect();
        let elements_list = List::new(elements)
            .block(standard_block.clone().title("Events"))
            .style(standard_style)
            .highlight_style(
                Style::default()
                .add_modifier(Modifier::REVERSED)
            );
        f.render_stateful_widget(
            elements_list,
            main_view[0],
            &mut state.list_state
        );
        
        let vec_details: Vec<Row> = if let Some(selected_element) = state.get_selected_element() {
            selected_element
                .get_vecs()
                .into_iter()
                .map(|e| Row::new(e).bottom_margin(1))
                .collect::<Vec<Row>>()
        } else {
            Vec::new()
        };
    
        let attributes_table = Table::new(vec_details)
            .block(standard_block.clone().title("Attributes"))
            .widths(&[Constraint::Percentage(30), Constraint::Percentage(70)])
            .column_spacing(1)
            .style(standard_style)
            .highlight_style(Style::default().add_modifier(Modifier::REVERSED));
        
        f.render_stateful_widget(
            attributes_table,
            main_view[1],
            &mut state.details_state
        );
    
        let editing_content = if state.buffer_modification() {
            Spans::from(vec![
                Span::styled(
                    match state.focused_on {
                        AppFocus::Attributes => {"New Attribute: "},
                        _ => { "New Value: " }
                    },
                    Style::default().add_modifier(Modifier::BOLD)
                ),
                Span::raw(state.modify_buffer.clone().unwrap_or("".to_string())),
                Span::styled(
                    "_",
                    Style::default().add_modifier(Modifier::SLOW_BLINK)
                )
            ])    
        } else {
            let txt = get_selected_details(state).unwrap_or("".to_string());
            Spans::from(vec![Span::raw(txt)])
        };
    
        let editing_view = Paragraph::new(editing_content)
            .block(standard_block.clone().title("Details"))
            .style(standard_style)
            .wrap(Wrap { trim: false });
    
        f.render_widget(editing_view, main_view[2]);
    
        // Footer
        let actions_text = AppCommand::get_command_list_string().join(" ");
        let actions = Paragraph::new(actions_text)
            .block(alt_block)
            .style(alt_style)
            .alignment(Alignment::Center)
            .wrap(Wrap { trim: true });
        f.render_widget(actions, main_layout[2]);
    }
}