use std::{collections::HashMap, time::Duration};

use color_eyre::eyre::Result;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{prelude::*, widgets::*};
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc::UnboundedSender;

use super::{Component, Frame};
use crate::{
    action::Action,
    config::{Config, KeyBindings},
};

#[derive(Default)]
pub struct LoginComponent {
    // Splash screen related fields
    command_tx: Option<UnboundedSender<Action>>,
    config: Config,
    counter: usize,
    logo_frames: Vec<String>,
    is_animated: bool,
    loading_messages: Vec<String>,

    // Gauge related fields
    progress: f64,
    total_loading_messages: usize,
}

impl LoginComponent {
    pub fn new() -> Self {
        let logo_frames = vec![
            r"     __
    / /
   / / 
  / /  
 / /   
/_/    "
                .to_string(),
            r"     __ __
   _/ // /
  / __/ / 
 (_  ) /  
/  _/ /   
/_//_/    "
                .to_string(),
            r"     __ __  __
   _/ // /_/ /
  / __/ __/ / 
 (_  ) /_/ /  
/  _/\__/ /   
/_/    /_/    "
                .to_string(),
            r"     __ __        __
   _/ // /_____ _/ /
  / __/ __/ __ `/ / 
 (_  ) /_/ /_/ / /  
/  _/\__/\__,_/ /   
/_/          /_/    "
                .to_string(),
            r"     __ __             __
   _/ // /_____ ______/ /
  / __/ __/ __ `/ ___/ / 
 (_  ) /_/ /_/ (__  ) /  
/  _/\__/\__,_/____/ /   
/_/               /_/    "
                .to_string(),
            r"     __ __             __    __
   _/ // /_____ ______/ /_  / /
  / __/ __/ __ `/ ___/ __ \/ / 
 (_  ) /_/ /_/ (__  ) / / / /  
/  _/\__/\__,_/____/_/ /_/ /   
/_/                     /_/    "
                .to_string(),
            r"     __ __             __      
   _/ // /_____ ______/ /_     
  / __/ __/ __ `/ ___/ __ \    
 (_  ) /_/ /_/ (__  ) / / /    
/  _/\__/\__,_/____/_/ /_/     
/_/                            "
                .to_string(),
        ];

        Self {
            counter: 0,
            logo_frames,
            progress: 0.0,
            total_loading_messages: 3,
            is_animated: true,
            ..Self::default()
        }
    }

    fn set_progress(&mut self, progress: f64) {
        self.progress = progress;
    }

    fn update_progress(&mut self) {
        let message_count = self.loading_messages.len();
        if message_count >= self.total_loading_messages {
            self.progress = 1.0;
        } else {
            self.progress = message_count as f64 / self.total_loading_messages as f64
        }
    }
}

impl Component for LoginComponent {
    fn register_action_handler(&mut self, tx: UnboundedSender<Action>) -> Result<()> {
        self.command_tx = Some(tx);
        Ok(())
    }

    fn register_config_handler(&mut self, config: Config) -> Result<()> {
        self.config = config;
        Ok(())
    }

    fn update(&mut self, action: Action) -> Result<Option<Action>> {
        match action {
            Action::Tick => {
                if self.is_animated {
                    self.counter += 1;
                    if self.counter >= self.logo_frames.len() {
                        self.counter = self.logo_frames.len() - 1;
                        self.is_animated = false;
                    }
                }
            }
            Action::Message(map) => {
                if let Some(startup_message) = map.get("startup") {
                    self.loading_messages.push(startup_message.clone());
                    self.update_progress();
                }
            }
            _ => {}
        }
        Ok(None)
    }

    fn draw(&mut self, f: &mut Frame<'_>, area: Rect) -> Result<()> {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Percentage(99), Constraint::Percentage(1)].as_ref())
            .split(area);

        // Draw the splash screen in the upper part
        let frame = &self.logo_frames[self.counter];
        let frame_lines: Vec<&str> = frame.lines().collect();
        let total_lines = frame_lines.len() + 1;
        let lines_above = (chunks[0].height as usize - total_lines) / 2;
        let lines_below = chunks[0].height as usize - lines_above - total_lines;

        let mut text = Text::default();
        for _ in 0..lines_above {
            text.lines.push(Line::from(""));
        }
        for line in frame_lines {
            text.lines.push(Line::from(line));
        }

        // Add a blank line between the logo and loading message
        text.lines.push(Line::from(""));

        let loading_message = if !self.loading_messages.is_empty() {
            &self.loading_messages[self.counter % self.loading_messages.len()]
        } else {
            "Loading..."
        };
        text.lines.push(Line::from(loading_message));

        for _ in 0..lines_below {
            text.lines.push(Line::from(""));
        }

        let p = Paragraph::new(text).alignment(Alignment::Center);
        f.render_widget(p, chunks[0]);

        // Draw the progress gauge in the bottom part
        let gauge = LineGauge::default().ratio(self.progress);
        f.render_widget(gauge, chunks[1]);

        Ok(())
    }
}
