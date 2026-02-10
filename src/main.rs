mod app;
mod db;
mod ui;
mod visualization;

use anyhow::Result;
use crossterm::{
    event::{
        self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind, MouseEventKind,
    },
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::io;

use app::App;

fn main() -> Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new();
    let res = run_app(&mut terminal, &mut app);

    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    if let Err(err) = res {
        println!("Error: {:?}", err);
    }

    Ok(())
}

fn run_app<B: ratatui::backend::Backend>(terminal: &mut Terminal<B>, app: &mut App) -> Result<()> {
    loop {
        terminal.draw(|f| ui::draw(f, app))?;

        if let Event::Key(key) = event::read()? {
            if key.kind == KeyEventKind::Press {
                match key.code {
                    KeyCode::Char('q') => {
                        if !app.is_inputting() {
                            return Ok(());
                        } else {
                            app.on_char('q');
                        }
                    }
                    KeyCode::Esc => {
                        if !app.is_inputting() {
                            return Ok(());
                        }
                    }
                    KeyCode::Up => app.on_up(),
                    KeyCode::Down => app.on_down(),
                    KeyCode::Left => {
                        if app.is_inputting() {
                            app.on_input_left();
                        } else {
                            app.on_left();
                        }
                    }
                    KeyCode::Right => {
                        if app.is_inputting() {
                            app.on_input_right();
                        } else {
                            app.on_right();
                        }
                    }
                    KeyCode::Home => app.on_home(),
                    KeyCode::End => app.on_end(),
                    KeyCode::Enter => app.on_enter()?,
                    KeyCode::Tab => app.on_tab(),
                    KeyCode::Char(c) => app.on_char(c),
                    KeyCode::Backspace => app.on_backspace(),
                    KeyCode::Delete => app.on_delete(),
                    _ => {}
                }
            }
        } else if let Event::Mouse(mouse) = event::read()? {
            match mouse.kind {
                MouseEventKind::Down(_button) => {
                    app.on_mouse_click(mouse.column, mouse.row)?;
                }
                MouseEventKind::ScrollDown => app.on_scroll_down(),
                MouseEventKind::ScrollUp => app.on_scroll_up(),
                _ => {}
            }
        }
    }
}
