/***
 * reboot-to
 * 
 * TUI wrapper around efibootmgr and shutdown. Allows user to chose boot entry to 
 * reboot to in a list.
 * 
 * Licensed under MIT license:
 * 
 * ********************************************************************************
 * Copyright 2024 ciqven
 * 
 * Permission is hereby granted, free of charge, to any person obtaining a copy 
 * of this software and associated documentation files (the “Software”), to deal 
 * in the Software without restriction, including without limitation the rights to 
 * use, copy, modify, merge, publish, distribute, sublicense, and/or sell copies 
 * of the Software, and to permit persons to whom the Software is furnished to do 
 * so, subject to the following conditions:
 * 
 * The above copyright notice and this permission notice shall be included in all 
 * copies or substantial portions of the Software.
 * 
 * THE SOFTWARE IS PROVIDED “AS IS”, WITHOUT WARRANTY OF ANY KIND, EXPRESS OR 
 * IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY, 
 * FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE 
 * AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER 
 * LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING 
 * FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN 
 * THE SOFTWARE.
 * ********************************************************************************
 * 
 ***/
use std::{io::{stdout, Result}, process::{Command, ExitCode, ExitStatus}, str::FromStr};
use regex::Regex;

use clap::Parser;
use ratatui::{
    backend::CrosstermBackend, crossterm::{
        event::{self, KeyCode, KeyEventKind, KeyModifiers},
        terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
        ExecutableCommand,
    }, style::{Color, Modifier, Style, Stylize}, text::Line, widgets::{block::Title, Block, List, ListDirection, ListState}, Terminal
};



#[derive(Debug, Parser)]
#[command(version, about = "Convenience TUI wrapper around efibootmgr", long_about = "reboot-to is a terminal UI (TUI) wrapper around the efibootmgr and shudown commands, intended to provide a simple way to reboot into another UEFI boot entry (typically another operating system).

When executed without any arguments you will be able to select a UEFI boot entry in a TUI.

Some of the options below require specifying a UEFI boot entry using a parameter called <DEST>. Valid values for <DEST> are either a number or a text. Numbers will be matched against the ID of boot entries, this ID can be retrieved by using the --list option, or by running efibootmgr without arguments. Text will be matched against the name of the boot entries, case-sensitive and from the start. For example, a boot entry named \"ubuntu\" will be matched by \"ub\" but not by \"Ub\" nor by \"bun\".

This executable runs the \"shutdown\" and \"efibootmgr\" commands. These should be available in PATH, and the executable should be ran with appropriate permission.
")]
struct Arguments {

    // Lists targets
    #[arg(short, long, action = clap::ArgAction::SetTrue, help = "Output a list of boot entries and their IDs")]
    list: Option<bool>,

    // Set next boot target
    #[arg(short, long, value_name = "DEST", help = "Set the entry specified by <DEST> as the next (one-time) boot target")]
    next: Option<String>,

    // Reboots to provided destination
    #[arg(short, long, value_name = "DEST", help = "Reboot directly to the entry specified by <DEST>")]
    reboot_to: Option<String>,
}

#[derive(Debug)]
struct BootTarget {
    id: u16,
    name: String,
}

#[derive(Debug)]
struct BootTargets {
    targets: Vec<BootTarget>,
    current: Option<u16>,
    next: Option<u16>
}

enum ChosenAction<'a> {
    None,
    RebootTo(&'a BootTarget),
    SetNext(&'a BootTarget),

}

impl BootTargets {
    fn get_names(&self) -> Vec<String> {
        self.targets.iter().map(|target| {
            let mut s = target.name.clone();

            if self.next.is_some_and(|next| next == target.id) {
                s.insert_str(0, "nxt: ");
            } else if self.current.is_some_and(|curr| curr == target.id) {
                s.insert_str(0, "cur: ");
            } else {
                s.insert_str(0, "     ");
            }

            s
        }).collect::<Vec<String>>()
    }

    fn lookup(&self, query: &str) -> Option<&BootTarget> {
        let parsed = query.parse::<u16>();

        if parsed.is_ok() { // Integer provided
            let id = parsed.expect("Query should be a valid number here");
            self.targets.iter().find(|target| target.id == id)
        } else {
            self.targets.iter().find(|target| target.name.starts_with(query))
        }
    }

    fn print_list(&self) {
        for target in self.targets.iter() {
            println!("{} \t {}", target.id, target.name);
        }
    }
}

fn parse_boot_targets(raw: String) -> BootTargets {
    let regex_options = Regex::new(r"(?m)^([a-zA-Z]+):\s+(.*)$")
        .expect("Hardcoded parse_boot_targets regex should compile (1)");
    let regex_targets = Regex::new(r"(?m)^[a-zA-Z]*([0-9]+)\*\s+(.*?)\t.*$")
        .expect("Hardcoded parse_boot_targets regex should compile (2)");

    let mut result = BootTargets {
        targets: vec![],
        current: None,
        next: None
    };

    // Iterate over found options
    for (_, [key, value]) in regex_options.captures_iter(raw.as_str()).map(|res| res.extract()) {
        match key {
            "BootCurrent" => result.current = Some(value.parse::<u16>().unwrap_or(1)),
            "BootNext" => result.next = Some(value.parse::<u16>().unwrap_or(1)),
            _ => (),
        }
    }

    // Iterate over found boot targets
    for (_, [id, name]) in regex_targets.captures_iter(raw.as_str()).map(|res| res.extract()) {
        let parsed_id = id.parse::<u16>();
        
        if parsed_id.is_err() {
            // Skip invalid IDs
            continue;
        }

        result.targets.push(BootTarget {
            id: parsed_id.expect("Parsed id should be valid here"),
            name: String::from_str(name).unwrap_or(String::from_str("Failure parsing name").expect("Hardcoded string should be valid")),
        });
    }

    result
}

fn get_boot_targets() -> BootTargets {
    // Run command
    let result = Command::new("efibootmgr").output().expect("Error running the efibootmgr command");
    let raw = String::from_utf8(result.stdout).expect("Error parsing result of efibootmgr command");

    // Parse results
    parse_boot_targets(raw)
}

fn set_next_boot(target: &BootTarget) -> Result<ExitStatus> {
    Command::new("efibootmgr")
        .arg("--bootnext")
        .arg(format!("{:0>4}", target.id))
        .status()
}

fn reboot_to(target: &BootTarget) {
    
    let mut status = set_next_boot(target);
    if status.is_err() {
        println!("Could not set boot target using efibootmgr, aborting...");
        return;
    } else {
        let s = status.expect("Status should be valid here");
        if !s.success() {
            println!("efibootmgr exited with non-zero status: {}", s.code().unwrap_or(-1));
        }
    }
    

    status = Command::new("shutdown")
        .args(["-r", "now"])
        .status()
    ;
    if status.is_err() || !status.expect("Status should be valid here").success() {
        // TODO: Detail how to clear
        println!("Unable to reboot using shutdown command. Bootnext has been set, either reboot manually or clear");
    }
}

fn set_next_boot_wrapper(target: &BootTarget) {
    let status = set_next_boot(target);
    if status.is_err() {
        println!("Could not set boot target using efibootmgr, aborting...");
        return;
    } else {
        let s = status.expect("Status should be valid here");
        if !s.success() {
            println!("efibootmgr exited with non-zero status: {}", s.code().unwrap_or(-1));
        }
    }
}


fn tui_selection(targets: &BootTargets) -> Result<()>{

    let item_count = targets.targets.len();
    let mut action = ChosenAction::None;

    // Setup clear screen
    stdout().execute(EnterAlternateScreen)?;
    enable_raw_mode()?;
    let mut terminal = Terminal::new(CrosstermBackend::new(stdout()))?;
    terminal.clear()?;

    // List state
    let mut list_state = ListState::default().with_selected(Some(0));

    loop {
        // Draw UI
        let list_items = targets.get_names();
        terminal.draw(|frame| {
            let area = frame.size();

            let block = Block::bordered()
                .gray()
                .title(" List title ".bold().fg(Color::Gray).into_centered_line())
                .title(Title::from(Line::from(vec![
                    " ".into(),
                    "Up/Down".on_gray().black().bold(),
                    " Select ".into(),
                    "Enter".on_gray().black().bold(),
                    " Reboot ".into(),
                    "n".on_gray().black().bold(),
                    " Set next ".into(),
                    "Esc/q".on_gray().black().bold(),
                    " Quit ".into(),

                ]))
                .alignment(ratatui::layout::Alignment::Center)
                .position(ratatui::widgets::block::Position::Bottom)
            );

            let list = List::new(list_items)
            .block(block)
            .style(Style::default().fg(Color::Gray))
            .highlight_style(Style::default().add_modifier(Modifier::REVERSED))
            .repeat_highlight_symbol(true)
            .direction(ListDirection::TopToBottom)
            ;

            frame.render_stateful_widget(
                list,
                area,
                &mut list_state
            );
        })?;


        // Handle events
        if event::poll(std::time::Duration::from_millis(16))? {
            if let event::Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    // Quit loop and UI with q or Escape
                    if key.code == KeyCode::Char('q') || key.code == KeyCode::Esc {
                        break;
                    }

                    // Allow quit with CTRL+C
                    if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
                        break;
                    }

                    // Navigate list with up/down
                    if key.code == KeyCode::Down {
                        if list_state.selected().unwrap_or(0) >= item_count -1 { // Wrap to top
                            list_state.select_first();
                        } else {
                            list_state.select_next();
                        }
                    }
                    if key.code == KeyCode::Up {
                        if list_state.selected().unwrap_or(0) <= 0 { // Wrap to bottom
                            list_state.select_last()
                        } else {
                            list_state.select_previous();
                        }
                    }

                    // Navigate fast with home/end
                    if key.code == KeyCode::Home {
                        list_state.select_first();
                    }
                    if key.code == KeyCode::End {
                        list_state.select_last();
                    }

                    // Reboot to target with Enter
                    if key.code == KeyCode::Enter {
                        let selected =  list_state.selected();
                        if selected.is_some_and(|index| index < item_count) {
                            let index = selected.expect("Selected index is guaranteed to be Some here");
                            let target = targets.targets.get(index);
                            if target.is_some() {
                                action = ChosenAction::RebootTo(target.expect("Target guaranteed to be valid here"));
                            }
                        }
                        break;
                    }

                    // Set target as next with n
                    if key.code == KeyCode::Char('n') {
                        let selected =  list_state.selected();
                        if selected.is_some_and(|index| index < item_count) {
                            let index = selected.expect("Selected index is guaranteed to be Some here");
                            let target = targets.targets.get(index);
                            if target.is_some() {
                                action = ChosenAction::SetNext(target.expect("Target guaranteed to be valid here"));
                            }
                        }
                        break;
                    }


                }
            }
        }
    }

    // Clean up screen
    stdout().execute(LeaveAlternateScreen)?;
    disable_raw_mode()?;

    // Handle action
    match action {
        ChosenAction::None => (),
        ChosenAction::RebootTo(target) => reboot_to(target),
        ChosenAction::SetNext(target) => set_next_boot_wrapper(target),
    }

    Ok(())
}

fn main() -> ExitCode {
    let args = Arguments::parse();

    let targets = get_boot_targets();

    if args.list.unwrap_or(false) {
        targets.print_list();

        return ExitCode::SUCCESS;
    }
    
    if let Some(dest) = args.reboot_to.as_deref() {
        let target = targets.lookup(dest);

        if target.is_some() {
            reboot_to(target.expect("Target checked to be valid"));
        } else {
            eprintln!("Could not find UEFI boot entry from specifier \"{}\"", dest);
            
            return ExitCode::FAILURE;
        }

        return ExitCode::SUCCESS;
    }

    if let Some(dest) = args.next.as_deref() {
        let target = targets.lookup(dest);

        if target.is_some() {
            set_next_boot_wrapper(target.expect("Target checked to be valid"));
        } else {
            eprintln!("Could not find UEFI boot entry from specifier \"{}\"", dest);
            
            return ExitCode::FAILURE;
        }

        return ExitCode::SUCCESS;
    }

    
    tui_selection(&targets).expect("Error in TUI");
    
    ExitCode::SUCCESS
}
