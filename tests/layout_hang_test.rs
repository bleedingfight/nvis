use ratatui::layout::{Constraint, Flex, Layout, Rect};

#[test]
fn test_spacebetween_min_no_hang() {
    // Simulate a case where Min constraints + SpaceBetween
    let constraints = vec![
        Constraint::Min(6),
        Constraint::Min(30),
        Constraint::Min(6),
        Constraint::Min(6),
        Constraint::Min(6),
    ];
    let area = Rect::new(0, 0, 78, 1);
    let _rects = Layout::horizontal(constraints)
        .flex(Flex::SpaceBetween)
        .spacing(1)
        .split(area);
}

#[test]
fn test_spacebetween_min_overflow() {
    // Total Min exceeds available space
    let constraints = vec![
        Constraint::Min(30),
        Constraint::Min(30),
        Constraint::Min(30),
    ];
    let area = Rect::new(0, 0, 50, 1);
    let _rects = Layout::horizontal(constraints)
        .flex(Flex::SpaceBetween)
        .spacing(1)
        .split(area);
}
