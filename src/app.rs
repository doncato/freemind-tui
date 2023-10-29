pub(crate) mod engine {
    use crate::data::data_types::{AppState, AppConfig};
    use clap::{Arg, Command, ArgMatches, crate_authors, crate_description, crate_version, ArgAction};
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


    pub fn switch_up(state: &mut AppState) {
        if let Some(i) = state.list_state.selected() {
            if i > 0 {
                state.get_elements_mut().swap(i, i-1);
            }

        };
    }

    pub fn switch_down(state: &mut AppState) {
        if let Some(i) = state.list_state.selected() {
            if i < state.get_elements().len()-1 {
                state.get_elements_mut().swap(i, i+1);
            }

        };
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
    use std::str::FromStr;

    use crate::{AppState, AppCommand, data::data_types::AppFocus};
    use cron::Schedule;
    use chrono::{TimeZone, Utc, LocalResult, Local};
    use clap::{crate_name, crate_version};
    use tui::{
        style::Style,
        backend::{Backend},
        layout::{Constraint, Direction, Layout, Alignment, Rect},
        widgets::{Block, Borders, Paragraph, Wrap, List, ListItem, BorderType, Row, Table},
        Frame, text::{Spans, Span}, style::{Color, Modifier}, 
    };

    /// Takes a timestamp and converts it to a Human Readable string in the current
    /// timezone
    fn display_timestamp(timestamp: i64) -> String {
        let due: String = match Utc.timestamp_opt(timestamp, 0) {
            LocalResult::None => "None".to_string(),
            LocalResult::Single(val) => val.with_timezone(&chrono::Local).to_rfc2822(),
            LocalResult::Ambiguous(val, _) => val.with_timezone(&chrono::Local).to_rfc2822(),
        };
        due
    }

    /// Returns the value of the currently selected attribute of node
    pub fn get_selected_value(state: &AppState) -> Option<String> {
        if let Some(node) = state.get_selected_attribute() {
            Some(node.1)
        } else {
            None
        }
    }

    /// Returns a Hashmap that maps different representations of the value
    /// of the currently selected attribute
    pub fn get_selected_details(state: &AppState) -> Vec<(String, String)> {
        let mut cnt: Vec<(String, String)> = Vec::new();
        if let Some(node) = state.get_selected_attribute() {
            cnt.push(("Raw".to_string(), node.1.clone()));

            let value: String = node.1;

            if let Some(num_val) = value.parse::<i64>().ok() {
                let due = display_timestamp(num_val);
                cnt.push(("As UNIX Timestamp".to_string(), due));
            }

            if let Ok(schedule) = Schedule::from_str(&value) {
                let next_events: Vec<String> = schedule
                    .upcoming(Local)
                    .take(5)
                    .map(|e| {
                        e.to_rfc2822()
                    })
                    .collect();
                cnt.push(("As cron expression".to_string(), next_events.join("\n")));
            }
        }
        cnt
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

    /// Builds the top bar of the layout
    fn build_top_bar<'a, B: tui::backend::Backend>(f: &mut Frame<B>, layout: Rect, state: &mut AppState, block: Block<'a>, style: &Style) {
        let top_bar: Vec<Rect> = Layout::default()
                .direction(Direction::Horizontal)
                .constraints(
                    [
                        Constraint::Percentage(50),
                        Constraint::Percentage(50),
                    ]
                    .as_ref(),
                )
                .split(layout);
        
            let top_left_text: String = format!(
                ":// {} {} - {}",
                crate_name!(),
                crate_version!(),
                state.modified_string()
            );
            let top_left: Paragraph<'_> = Paragraph::new(top_left_text)
                .block(block.clone())
                .style(*style)
                .alignment(Alignment::Left)
                .wrap(Wrap { trim: true });
            
            let top_right_text: &str = state
            .message
                .unwrap_or("");
            
            let top_right = Paragraph::new(top_right_text)
            .block(block)
            .style(*style)
            .alignment(Alignment::Right)
            .wrap(Wrap { trim: true });
        
        
            f.render_widget(top_left, top_bar[0]);
            f.render_widget(top_right, top_bar[1]);
    }

    /// Build main view of the layout
    fn build_main_view<'a, B: tui::backend::Backend>(f: &mut Frame<B>, layout: Rect, state: &mut AppState, block: Block<'a>, style: &Style) {
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
            .split(layout);
    
        let elements: Vec<ListItem<'_>> = state
            .get_elements()
            .iter()
            .map(|e| e.to_list_item())
            .collect();

        let elements_list = List::new(elements)
            .block(block.clone().title("Events"))
            .style(*style)
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
                .iter()
                .map(|(k, v)| {
                    Row::new(
                        vec![k.to_string(), v.to_string()]
                    )
                    .bottom_margin(1)
                })
                .collect::<Vec<Row>>()
        } else {
            Vec::new()
        };
    
        let attributes_table = Table::new(vec_details)
            .block(block.clone().title("Attributes"))
            .widths(&[Constraint::Percentage(30), Constraint::Percentage(70)])
            .column_spacing(1)
            .style(*style)
            .highlight_style(Style::default().add_modifier(Modifier::REVERSED));
        
        f.render_stateful_widget(
            attributes_table,
            main_view[1],
            &mut state.details_state
        );
    
        let details_content = {
            let style = Style::default()
                .add_modifier(Modifier::ITALIC);

            let content: Vec<(String, String)> = get_selected_details(state);

            let content_spans: Vec<Spans<'_>> = content
                .into_iter()
                .map(|e| {
                    let mut key: String = e.0;
                    let value: String = e.1;

                    key.push_str(":");

                    let mut r: Vec<Spans<'_>> = Vec::new();
                    r.push(Spans::from(Span::styled(key,style)));
                    value
                        .split("\n")
                        .for_each(|e| {
                            r.push(Spans::from(Span::raw(e.to_string())));
                        });
                    r.push(Spans::from(Span::raw("")));

                    r
                })
                .flatten()
                .collect();

            content_spans
        };
    
        let details_view = Paragraph::new(details_content)
            .block(block.clone().title("Details"))
            .style(*style)
            .wrap(Wrap { trim: false });
    
        f.render_widget(details_view, main_view[2]);
    }

    /// Builds the bottom bar of the layout
    fn build_bottom_bar<'a, B: tui::backend::Backend>(f: &mut Frame<B>, layout: Rect, state: &mut AppState, block: Block<'a>, style: &Style) {
        let bottom_content: Spans<'_> = {
            Spans::from(vec![
                Span::styled(
                    match state.focused_on {
                        AppFocus::Elements => {
                            "LST "
                        },
                        AppFocus::Attributes => {
                            "ATR "
                        },
                        AppFocus::Edit => {
                            "INS: "
                        },
                    },
                    Style::default().add_modifier(Modifier::BOLD)
                ),
                Span::raw(state.get_edit().unwrap_or("".to_string())),
                if state.is_editing() {
                    Span::styled(
                        "_",
                        Style::default().add_modifier(Modifier::SLOW_BLINK)
                    )
                } else {
                    Span::raw("")
                }
            ])    
        };

        let bottom_editor: Paragraph<'_> = Paragraph::new(bottom_content)
            .block(block)
            .style(*style)
            .wrap(Wrap { trim: true });

        f.render_widget(bottom_editor, layout);
    }

    /// Builds the footer of the layout
    fn build_footer<'a, B: tui::backend::Backend>(f: &mut Frame<B>, layout: Rect, state: &mut AppState, block: Block<'a>, style: &Style) {
        let actions_text: String = AppCommand::get_command_list_string().join(" | ");
        let actions: Paragraph<'_> = Paragraph::new(actions_text)
            .block(block)
            .style(*style)
            .alignment(Alignment::Center)
            .wrap(Wrap { trim: true });
        f.render_widget(actions, layout);
    }

    /// Builds the main UI
    pub fn ui<B: Backend>(f: &mut Frame<B>, state: &mut AppState) {
        // Define standard block and style properties
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
                    Constraint::Length(1),
                ]
                .as_ref(),
            )
            .split(f.size());
    
        // Top Bar
        build_top_bar(f, main_layout[0], state, alt_block.clone(), &alt_style);
    
        // Main View
        build_main_view(f, main_layout[1], state, standard_block, &standard_style);
    
        // Bottom Text Lane
        build_bottom_bar(f, main_layout[2], state, alt_block.clone(), &standard_style);

        // Footer
        build_footer(f, main_layout[3], state, alt_block, &alt_style);
    }
}