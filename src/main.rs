use anyhow::Result;
use crossterm::{
    event::{
        self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind,
        KeyModifiers, MouseButton, MouseEventKind,
    },
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::io;

use nvis::app::App;
use nvis::ui;
use std::env;

fn main() -> Result<()> {
    enable_raw_mode()?;

    // Set up panic hook to restore terminal before printing backtrace
    let panic_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = disable_raw_mode();
        let _ = execute!(
            std::io::stdout(),
            LeaveAlternateScreen,
            DisableMouseCapture
        );
        panic_hook(info);
    }));

    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut registry = nvis::core::registry::Registry::new();
    nvis::backends::register_all(&mut registry);
    nvis::viz::register_all(&mut registry);

    let mut app = App::new(registry);

    if let Some(path) = env::args().nth(1) {
        let p = std::path::Path::new(&path);
        let _ = app.load_database(p);
    }

    let res = run_app(&mut terminal, &mut app);

    restore_terminal(&mut terminal)?;

    if let Err(err) = res {
        println!("Error: {:?}", err);
    }

    Ok(())
}

fn restore_terminal<B: ratatui::backend::Backend + std::io::Write>(
    terminal: &mut Terminal<B>,
) -> Result<()> {
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;
    Ok(())
}

fn run_app<B: ratatui::backend::Backend + std::io::Write>(
    terminal: &mut Terminal<B>,
    app: &mut App,
) -> Result<()> {
    let mut mouse_captured = true;

    loop {
        terminal.draw(|f| {
            ui::draw(f, app);
        })?;
        let size = terminal.size()?;
        let full_area = ratatui::layout::Rect::new(0, 0, size.width, size.height);

        let event = event::read()?;
        match event {
            Event::Key(key) if key.kind == KeyEventKind::Press => {
                let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
                let alt = key.modifiers.contains(KeyModifiers::ALT);
                let shift = key.modifiers.contains(KeyModifiers::SHIFT);

                match (key.code, ctrl, alt) {
                    // Ctrl+C always exits
                    (KeyCode::Char('c'), true, _) => {
                        return Ok(());
                    }
                    // Ctrl+M: toggle mouse capture for terminal text selection/copy
                    (KeyCode::Char('m'), true, false) => {
                        mouse_captured = !mouse_captured;
                        if mouse_captured {
                            execute!(terminal.backend_mut(), EnableMouseCapture)?;
                        } else {
                            execute!(terminal.backend_mut(), DisableMouseCapture)?;
                        }
                    }
                    // Ctrl+A: go to start of input (shell convention)
                    (KeyCode::Char('a'), true, false) => {
                        app.on_input_ctrl_a();
                    }
                    // Ctrl+E: go to end of input (shell convention)
                    (KeyCode::Char('e'), true, false) => {
                        app.on_input_ctrl_e();
                    }
                    // Ctrl+U: delete from cursor to start of line
                    (KeyCode::Char('u'), true, false) => {
                        app.on_input_ctrl_u();
                    }
                    // Ctrl+K: delete from cursor to end of line
                    (KeyCode::Char('k'), true, false) => {
                        app.on_input_ctrl_k();
                    }
                    // Ctrl+W: delete previous word
                    (KeyCode::Char('w'), true, false) => {
                        app.on_input_ctrl_w();
                    }
                    // Alt+B: go to previous word
                    (KeyCode::Char('b'), false, true) => {
                        app.on_input_alt_b();
                    }
                    // Alt+F: go to next word
                    (KeyCode::Char('f'), false, true) => {
                        app.on_input_alt_f();
                    }
                    // q: quit when not inputting, otherwise insert
                    (KeyCode::Char('q'), false, false) => {
                        if !app.is_inputting() && !app.is_sql_inputting() {
                            return Ok(());
                        } else {
                            app.on_char('q');
                        }
                    }
                    // s/b: only act when not inputting
                    (KeyCode::Char('s'), false, false) if !app.is_inputting() && !app.is_sql_inputting() => {
                        let _ = app.enter_stats_view();
                    }
                    (KeyCode::Char('b'), false, false) if !app.is_inputting() && !app.is_sql_inputting() => {
                        app.leave_stats_view();
                    }
                    // Timeline zoom/pan when chart is focused and not inputting
                    (KeyCode::Char('+'), false, false) | (KeyCode::Char('='), false, false)
                        if !app.is_inputting() && !app.is_sql_inputting() =>
                    {
                        app.on_timeline_zoom_in();
                    }
                    (KeyCode::Char('-'), false, false) | (KeyCode::Char('_'), false, false)
                        if !app.is_inputting() && !app.is_sql_inputting() =>
                    {
                        app.on_timeline_zoom_out();
                    }
                    (KeyCode::Char('h'), false, false) if !app.is_inputting() && !app.is_sql_inputting() => {
                        app.on_timeline_pan_left();
                    }
                    (KeyCode::Char('l'), false, false) if !app.is_inputting() && !app.is_sql_inputting() => {
                        app.on_timeline_pan_right();
                    }
                    (KeyCode::Char('r'), false, false) if !app.is_inputting() && !app.is_sql_inputting() => {
                        app.on_timeline_reset_view();
                    }
                    // Esc always exits
                    (KeyCode::Esc, _, _) => {
                        return Ok(());
                    }
                    // Navigation keys
                    (KeyCode::Up, _, _) => app.on_up(),
                    (KeyCode::Down, _, _) => app.on_down(),
                    (KeyCode::Left, false, false) => {
                        if app.is_inputting() || app.is_sql_inputting() {
                            app.on_input_left();
                        } else if !shift {
                            app.on_left();
                        }
                    }
                    (KeyCode::Right, false, false) => {
                        if app.is_inputting() || app.is_sql_inputting() {
                            app.on_input_right();
                        } else if !shift {
                            app.on_right();
                        }
                    }
                    (KeyCode::Home, _, _) => app.on_home(),
                    (KeyCode::End, _, _) => app.on_end(),
                    (KeyCode::Enter, _, _) => app.on_enter()?,
                    (KeyCode::Tab, _, _) => app.on_tab(),
                    // Regular char input (no modifiers)
                    (KeyCode::Char(c), false, false) => app.on_char(c),
                    (KeyCode::Backspace, _, _) => app.on_backspace(),
                    (KeyCode::Delete, _, _) => app.on_delete(),
                    _ => {}
                }
            }
            Event::Mouse(mouse) if mouse_captured => match mouse.kind {
                MouseEventKind::Down(MouseButton::Left) => {
                    app.on_mouse_down(MouseButton::Left, mouse.column, mouse.row, full_area);
                }
                MouseEventKind::Down(MouseButton::Right) => {
                    app.on_mouse_down(MouseButton::Right, mouse.column, mouse.row, full_area);
                }
                MouseEventKind::Down(MouseButton::Middle) => {
                    app.on_mouse_down(MouseButton::Middle, mouse.column, mouse.row, full_area);
                }
                MouseEventKind::Up(MouseButton::Left) => {
                    app.on_mouse_up(MouseButton::Left, mouse.column, mouse.row, full_area);
                }
                MouseEventKind::Up(MouseButton::Right) => {
                    app.on_mouse_up(MouseButton::Right, mouse.column, mouse.row, full_area);
                }
                MouseEventKind::Up(MouseButton::Middle) => {
                    app.on_mouse_up(MouseButton::Middle, mouse.column, mouse.row, full_area);
                }
                MouseEventKind::Drag(MouseButton::Left) => {
                    app.on_mouse_drag(MouseButton::Left, mouse.column, mouse.row, full_area);
                }
                MouseEventKind::Drag(MouseButton::Right) => {
                    app.on_mouse_drag(MouseButton::Right, mouse.column, mouse.row, full_area);
                }
                MouseEventKind::Drag(MouseButton::Middle) => {
                    app.on_mouse_drag(MouseButton::Middle, mouse.column, mouse.row, full_area);
                }
                MouseEventKind::Moved => {
                    app.on_mouse_move(mouse.column, mouse.row, full_area);
                }
                MouseEventKind::ScrollDown => {
                    if mouse.modifiers.contains(KeyModifiers::SHIFT) {
                        app.on_timeline_pan_right();
                    } else {
                        app.on_scroll_down();
                    }
                }
                MouseEventKind::ScrollUp => {
                    if mouse.modifiers.contains(KeyModifiers::SHIFT) {
                        app.on_timeline_pan_left();
                    } else {
                        app.on_scroll_up();
                    }
                }
                MouseEventKind::ScrollLeft => app.on_timeline_pan_left(),
                MouseEventKind::ScrollRight => app.on_timeline_pan_right(),
            },
            _ => {}
        }
    }
}
