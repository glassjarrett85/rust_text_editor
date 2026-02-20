#![allow(unused_imports)]

// Simplify: Create a terminal.
use std::{io, thread, time::Duration, panic};
use ratatui::{
    backend::{Backend, CrosstermBackend},
    widgets::{ListState, Wrap, Paragraph, List, ListItem, Block, Borders},
    layout::{Layout, Constraint, Direction},
    style::{Style, Color, Modifier},
    Frame,
    Terminal
};
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use textwrap::{
    wrap,
    Options
};

struct App {
    menu_items: Vec<String>,
    menu_state: ListState,
    current: CurrentScreen, // The brain. Quitting? Popup? Active?
    input: String, // The text typed into editor box
}
impl App {
    fn new() -> App {
        let mut state = ListState::default();
        state.select(Some(0)); // Highlights the first item by default
        App {
            menu_items: vec![
                "New File".into(),
                "Open".into(),
                "Save".into(),
                "Quit".into(),
            ],
            menu_state: state,
            current: CurrentScreen::Main,
            input: String::new(), // Contents of the edit box
        }
    }

    // Move highlight down
    fn next(&mut self) {
        let i = match self.menu_state.selected() {
            Some(i) => if i >= self.menu_items.len() - 1 {0} else {i+1},
            None => 0,
        };
        self.menu_state.select(Some(i));
    }
    // Move highlight up
    fn previous(&mut self) {
        let i = match self.menu_state.selected() {
            Some(i) => if i == 0 { self.menu_items.len() - 1 } else { i - 1 },
            None => 0,
        };
        self.menu_state.select(Some(i));
    }
}

// An Enum to establish the state of the app. What's showing on the screen?
enum CurrentScreen {
    Main,
    Editing,
    Popup,
    Exiting,
}

// A helper function for our popup
fn centered_rect(percent_x: u16,
                 percent_y: u16, 
                 r: ratatui::layout::Rect) -> ratatui::layout::Rect {
    // Have to build this in parts. First build the vertical 'grid' for this to go
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100-percent_y)/2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100-percent_y)/2),
        ])
        .split(r);
    // Then, bop the window into the middle 'vertical grid', and segmnet based on horizontal lines.
    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100-percent_x)/2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100-percent_x)/2),
        ])
        .split(popup_layout[1])[1] // Pops it into the spot signifified within `percent_y`
}

// For customizing the terminal layout
fn ui(f: &mut Frame, app: &mut App) {
    // Create vertical chunks
    let main_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Header is 3 rows high. An absolute size.
            Constraint::Min(0),    // Main body takes up the rest
            Constraint::Length(1)  // The footer is just 1 row high
        ])
        .split(f.size());

    // Create the horizontal chunks now within the main section
    let middle_layout = Layout::default()
        .direction(Direction::Horizontal) // Side-by-side split
        .constraints([
            Constraint::Percentage(30), // Menu
            Constraint::Percentage(70)  // Editor
        ])
        .split(main_layout[1]); // Because I'm no longer splitting to the full area, just to this one chunk!
    
    // Render now unto the header area specifically
    let header = Block::default()
        .title(" My Rust Notepad ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));
    f.render_widget(header, main_layout[0]); 
    // I followed this so far. Build the header using Block::default(),
    // with this title and border style,
    // and render it to the first 'chunk' in main_layout.

    // Next: Render the Menu
    let items: Vec<ListItem> = app.menu_items
        .iter()
        .map(|i| ListItem::new(i.as_str()))
        .collect();

    // If the Popup is active, it will dim the background borders.
    let menu_color = if let CurrentScreen::Main = app.current { Color::White } else { Color::DarkGray };
    let list = List::new(items)
        .block(Block::default()
            .title("Menu")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(menu_color)))
        .highlight_style(Style::default().bg(Color::Blue).add_modifier(Modifier::BOLD))
        .highlight_symbol(">> ");
    f.render_stateful_widget(list, middle_layout[0], &mut app.menu_state);
    // Makes sense. I made the middle chunk to "middle_layout", and menu is on the left.

    // Next, render the editor. The right-part of middle_layout chunk.
    let editor_color = match app.current {
        CurrentScreen::Popup => Color::DarkGray,
        CurrentScreen::Editing => Color::Yellow,
        _ => Color::White
    };
    let editor = Paragraph::new(app.input.as_str())
        .block(Block::default()
            .title(" Editor (Press 'e' to Edit, 'Esc' to Stop) ")
            .borders(Borders::ALL)
            // Dim borders if app.show_popup is true
            .border_style(Style::default().fg(editor_color))
        ).wrap(Wrap { trim:true }); // Wrap text within the widget block
    f.render_widget(editor, middle_layout[1]);

    // Finally, render the footer.
    // Show the selected item name in the status bar
    let cur_sel = app.menu_items[
        app.menu_state.selected().unwrap_or(0)
    ].as_str();
    let footer_text = format!(" Current Selection: {} | Press 'q' to Quit",cur_sel);

    let footer = Block::default()
        .title(footer_text)
        .title_alignment(ratatui::layout::Alignment::Center);
    f.render_widget(footer, main_layout[2]);

    // Draw the popup widget
    if let CurrentScreen::Popup = app.current {
        let area = centered_rect(60, 20, f.size()); // 60% wide, 20% tall
        f.render_widget(ratatui::widgets::Clear, area);
        let popup_block = Block::default()
            .title(" Open File ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Yellow));
        let popup_text = ratatui::widgets::Paragraph::new("Enter file path: [Coming soon]")
            .block(popup_block)
            .alignment(ratatui::layout::Alignment::Center);
        f.render_widget(popup_text, area);
    }

    // Need to set up to allow correct wrapping, for Cursor placement.
    let editor_area = middle_layout[1];
    let max_width = (editor_area.width.saturating_sub(2)) as usize;

    // Show the cursor only if we are in Editing mode
    if let CurrentScreen::Editing = app.current {
        // Wrap the text using the same width as the UI block
        let options = Options::new(max_width);
        let wrapped_lines = wrap(&app.input, options);

        // What is the number of wrapped lines?
        let mut y_offset = wrapped_lines.len().saturating_sub(1) as u16;
        // Add 1 if `input` ends with a new line
        if app.input.ends_with('\n') { y_offset += 1; }

        // What is the length of the last line?
        let x_offset = wrapped_lines.last().map(|l| l.len()).unwrap_or(0) as u16;

        // And finally, set the cursor
        f.set_cursor(
            editor_area.x + 1 + x_offset,
            editor_area.y + 1 + y_offset,
        );
    }
}

fn main() -> Result<(), io::Error> {
    // A panic hook in case of crash
    let original_hook = panic::take_hook();
    panic::set_hook(Box::new(move |panic_info| {
        // Run this only if the app crashes
        let _ = disable_raw_mode();
        let mut stdout = io::stdout();
        execute!(stdout, LeaveAlternateScreen, DisableMouseCapture);
        original_hook(panic_info);
    }));

    // Set up the terminal
    let _ = enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new();

    loop {
        // Draw the terminal, using the size of the terminal screen to do this.
        terminal.draw(|f| ui(f, &mut app))?;
        // Check if a key was pressed - wait up to 50 ms for an event.
        // Limits how frequently the loop runs, helps with processing power
        if event::poll(Duration::from_millis(50))? {
            if let Event::Key(key) = event::read()? {
                match app.current {
                    // If "Exiting", disregard.
                    CurrentScreen::Exiting => {},
                    
                    // Keybindings for pop-up activated
                    CurrentScreen::Popup => {
                        match key.code {
                            KeyCode::Esc | KeyCode::Enter => { app.current = CurrentScreen::Main; },
                            _ => {}
                        }
                    },

                    // Keystrokes for when the Main screen is showing.
                    CurrentScreen::Main => {    
                        match key.code {
                            // Menu movement
                            KeyCode::Down => app.next(),
                            KeyCode::Up => app.previous(),

                            // Perform some action
                            KeyCode::Enter => {
                                if let Some(index) = app.menu_state.selected() {
                                    match app.menu_items[index].as_str() {
                                        "Quit" => app.current = CurrentScreen::Exiting,
                                        "Open" => app.current = CurrentScreen::Popup,
                                        _ => {}
                                    }
                                }
                            },

                            // And same emergency exit
                            KeyCode::Char('q') => break,
                            KeyCode::Char('e') => app.current = CurrentScreen::Editing,

                            // Anything else, do nothing.
                            _ => {}
                        }
                    },

                    // When 'e' is pressed to go edit
                    CurrentScreen::Editing => match key.code {
                        KeyCode::Esc => { app.current = CurrentScreen::Main; } , // Press Esc to get out of edit screen
                        KeyCode::Char(c) => { app.input.push(c); },
                        KeyCode::Backspace => { app.input.pop(); },
                        KeyCode::Enter => { app.input.push('\n'); },
                        _ => {}
                    }
                }
            }
        }
        // If the app.should_quit flag is now 'true'...
        if let CurrentScreen::Exiting = app.current { break; }
    }

    // Restore the terminal
    let _ = disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen, DisableMouseCapture)?;
    terminal.show_cursor()?;

    Ok(())
}