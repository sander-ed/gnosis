use std::fmt::Write as _;
use std::io::{self, IsTerminal, Write};
use std::ops::ControlFlow;

use anyhow::Result;
use crossterm::cursor::{Hide, MoveToColumn, MoveUp, Show};
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use crossterm::style::{ContentStyle, Stylize};
use crossterm::terminal::{self, Clear, ClearType};
use crossterm::{execute, queue};

use crate::workspace::Available;

pub(super) fn interactive() -> bool {
    io::stdin().is_terminal() && io::stderr().is_terminal()
}

/// Let the user pick packages with one tab per source. Returns indices into
/// `packages`, or `None` when cancelled.
pub(super) fn choose(packages: &[Available]) -> Result<Option<Vec<usize>>> {
    let mut selector = Selector::new(packages);
    let mut out = io::stderr();
    let raw = RawMode::enable()?;
    let mut drawn = 0;
    let outcome = loop {
        let (width, height) = terminal::size()?;
        drawn = draw(
            &mut out,
            &selector.render(width.into(), height.into()),
            drawn,
        )?;
        if let Event::Key(key) = event::read()?
            && key.kind != KeyEventKind::Release
            && let ControlFlow::Break(outcome) = selector.handle(key)
        {
            break outcome;
        }
    };
    let summary = match &outcome {
        Some(chosen) if !chosen.is_empty() => {
            let names: Vec<_> = chosen
                .iter()
                .map(|&index| format!("{}/{}", packages[index].source, packages[index].name))
                .collect();
            vec![format!("Selected {}\r\n", names.join(", "))]
        }
        _ => vec![],
    };
    draw(&mut out, &summary, drawn)?;
    drop(raw);
    Ok(outcome)
}

/// Redraw in place over the `drawn` lines of the previous frame.
fn draw(out: &mut impl Write, lines: &[String], drawn: usize) -> Result<usize> {
    queue!(out, MoveToColumn(0))?;
    if drawn > 1 {
        queue!(out, MoveUp((drawn - 1) as u16))?;
    }
    queue!(out, Clear(ClearType::FromCursorDown))?;
    write!(out, "{}", lines.join("\r\n"))?;
    out.flush()?;
    Ok(lines.len())
}

struct RawMode;

impl RawMode {
    fn enable() -> Result<Self> {
        terminal::enable_raw_mode()?;
        execute!(io::stderr(), Hide)?;
        Ok(Self)
    }
}

impl Drop for RawMode {
    fn drop(&mut self) {
        let _ = execute!(io::stderr(), Show);
        let _ = terminal::disable_raw_mode();
    }
}

struct Selector<'a> {
    packages: &'a [Available],
    /// Sources and the indices of the packages they publish.
    tabs: Vec<(&'a str, Vec<usize>)>,
    tab: usize,
    cursors: Vec<usize>,
    chosen: Vec<bool>,
}

impl<'a> Selector<'a> {
    fn new(packages: &'a [Available]) -> Self {
        let mut tabs: Vec<(&str, Vec<usize>)> = Vec::new();
        for (index, package) in packages.iter().enumerate() {
            match tabs
                .iter_mut()
                .find(|(source, _)| *source == package.source)
            {
                Some((_, members)) => members.push(index),
                None => tabs.push((&package.source, vec![index])),
            }
        }
        tabs.sort_by_key(|(source, _)| *source);
        Self {
            packages,
            cursors: vec![0; tabs.len()],
            tabs,
            tab: 0,
            chosen: vec![false; packages.len()],
        }
    }

    fn handle(&mut self, key: KeyEvent) -> ControlFlow<Option<Vec<usize>>> {
        let tabs = self.tabs.len();
        let members = &self.tabs[self.tab].1;
        let cursor = &mut self.cursors[self.tab];
        match key.code {
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                return ControlFlow::Break(None);
            }
            KeyCode::Esc | KeyCode::Char('q') => return ControlFlow::Break(None),
            KeyCode::Enter => {
                let chosen = (0..self.chosen.len()).filter(|&i| self.chosen[i]);
                return ControlFlow::Break(Some(chosen.collect()));
            }
            KeyCode::Up | KeyCode::Char('k') => {
                *cursor = (*cursor + members.len() - 1) % members.len()
            }
            KeyCode::Down | KeyCode::Char('j') => *cursor = (*cursor + 1) % members.len(),
            KeyCode::Left | KeyCode::Char('h') | KeyCode::BackTab => {
                self.tab = (self.tab + tabs - 1) % tabs;
            }
            KeyCode::Right | KeyCode::Char('l') | KeyCode::Tab => self.tab = (self.tab + 1) % tabs,
            KeyCode::Char(' ') => {
                let index = members[*cursor];
                self.chosen[index] = !self.chosen[index];
            }
            KeyCode::Char('a') => {
                let all = members.iter().all(|&index| self.chosen[index]);
                for &index in members {
                    self.chosen[index] = !all;
                }
            }
            _ => {}
        }
        ControlFlow::Continue(())
    }

    fn render(&self, width: usize, height: usize) -> Vec<String> {
        // Stay clear of the last column and row so the terminal never wraps or scrolls.
        let width = width.saturating_sub(1);
        let mut lines = Vec::new();
        let mut line = Line::new(width);
        line.push("? ", ContentStyle::new().cyan());
        line.push(
            "Select packages to install or update",
            ContentStyle::new().bold(),
        );
        lines.push(line.text);

        let mut line = Line::new(width);
        for (index, (source, members)) in self.tabs.iter().enumerate() {
            let count = members.iter().filter(|&&i| self.chosen[i]).count();
            let label = format!(" {source} ({count}/{}) ", members.len());
            let style = if index == self.tab {
                ContentStyle::new().reverse()
            } else {
                ContentStyle::new().dim()
            };
            line.push(&label, style);
            line.push(" ", ContentStyle::new());
        }
        lines.push(line.text);

        let members = &self.tabs[self.tab].1;
        let cursor = self.cursors[self.tab];
        let rows = height.saturating_sub(5).max(1);
        let first = cursor.saturating_sub(rows - 1);
        let names = members
            .iter()
            .map(|&i| self.packages[i].name.chars().count())
            .max()
            .unwrap_or(0);
        for (row, &index) in members.iter().enumerate().skip(first).take(rows) {
            let package = &self.packages[index];
            let mut line = Line::new(width);
            let current = row == cursor;
            line.push(
                if current { "❯ " } else { "  " },
                ContentStyle::new().cyan(),
            );
            let (mark, style) = if self.chosen[index] {
                ("◉ ", ContentStyle::new().green())
            } else {
                ("◯ ", ContentStyle::new())
            };
            line.push(mark, style);
            let style = if current {
                ContentStyle::new().bold()
            } else {
                ContentStyle::new()
            };
            line.push(&format!("{:names$}", package.name), style);
            if package.installed {
                line.push("  installed", ContentStyle::new().green());
            }
            let description: String = package
                .description
                .chars()
                .map(|c| if c.is_control() { ' ' } else { c })
                .collect();
            line.push(&format!("  {description}"), ContentStyle::new().dim());
            lines.push(line.text);
        }

        let mut line = Line::new(width);
        let selected = self.chosen.iter().filter(|&&chosen| chosen).count();
        let position = if members.len() > rows {
            format!(" · {}/{}", cursor + 1, members.len())
        } else {
            String::new()
        };
        line.push(
            &format!("{selected} selected{position}"),
            ContentStyle::new(),
        );
        lines.push(line.text);
        let mut line = Line::new(width);
        let switch = if self.tabs.len() > 1 {
            " · ←/→ source"
        } else {
            ""
        };
        line.push(
            &format!("↑/↓ move · space toggle · a all{switch} · enter confirm · esc cancel"),
            ContentStyle::new().dim(),
        );
        lines.push(line.text);
        lines
    }
}

/// A styled line truncated to the terminal width.
struct Line {
    text: String,
    remaining: usize,
}

impl Line {
    fn new(width: usize) -> Self {
        Self {
            text: String::new(),
            remaining: width,
        }
    }

    fn push(&mut self, text: &str, style: ContentStyle) {
        let fitted: String = text.chars().take(self.remaining).collect();
        self.remaining -= fitted.chars().count();
        if !fitted.is_empty() {
            let _ = write!(self.text, "{}", style.apply(fitted));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn package(source: &str, name: &str) -> Available {
        Available {
            source: source.into(),
            name: name.into(),
            description: String::new(),
            installed: false,
        }
    }

    fn press(selector: &mut Selector, codes: &[KeyCode]) -> ControlFlow<Option<Vec<usize>>> {
        let mut flow = ControlFlow::Continue(());
        for &code in codes {
            flow = selector.handle(KeyEvent::new(code, KeyModifiers::NONE));
        }
        flow
    }

    #[test]
    fn selections_persist_across_source_tabs() {
        let packages = [
            package("team", "core"),
            package("other", "core"),
            package("team", "platform"),
            package("other", "docs"),
        ];
        let mut selector = Selector::new(&packages);
        use KeyCode::*;
        // Tabs are sorted by source: other (core, docs), then team (core, platform).
        let flow = press(
            &mut selector,
            &[
                Down,
                Char(' '),
                Right,
                Char('a'),
                Char(' '),
                Left,
                Up,
                Enter,
            ],
        );
        assert_eq!(flow, ControlFlow::Break(Some(vec![2, 3])));
    }

    #[test]
    fn select_all_toggles_the_current_tab() {
        let packages = [package("team", "core"), package("team", "platform")];
        let mut selector = Selector::new(&packages);
        use KeyCode::*;
        let _ = press(&mut selector, &[Char(' '), Char('a')]);
        assert_eq!(selector.chosen, [true, true]);
        let _ = press(&mut selector, &[Char('a')]);
        assert_eq!(selector.chosen, [false, false]);
        assert_eq!(press(&mut selector, &[Esc]), ControlFlow::Break(None));
    }

    #[test]
    fn rendering_keeps_the_cursor_visible_within_the_terminal() {
        let packages: Vec<_> = (0..20).map(|i| package("team", &format!("p{i}"))).collect();
        let mut selector = Selector::new(&packages);
        let _ = press(&mut selector, &[KeyCode::Up]);
        let lines = selector.render(40, 10);
        assert!(lines.len() < 10);
        assert!(lines.iter().any(|line| line.contains("p19")));
        assert!(lines.iter().any(|line| line.contains("20/20")));
    }
}
