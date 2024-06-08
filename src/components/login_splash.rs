use std::time::Instant;
use std::{collections::HashMap, time::Duration};

use color_eyre::eyre::Result;
use crossterm::event::{KeyCode, KeyEvent};
use futures::future::Inspect;
use libc::posix_spawn_file_actions_addtcsetpgrp_np;
use ratatui::{prelude::*, widgets::*};
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc::UnboundedSender;

use super::{Component, Frame};
use crate::{
    action::Action,
    config::{Config, KeyBindings},
};

#[derive(Default)]
pub struct LoginSplash {
    command_tx: Option<UnboundedSender<Action>>,
    config: Config,
    counter: usize,
    logo_frames: Vec<String>,
    is_animated: bool,
    loading_messages: Vec<String>,
}

impl LoginSplash {
    pub fn new() -> Self {
        let counter = 0;
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
        let is_animated = true;

        Self {
            counter,
            logo_frames,
            is_animated,
            ..Self::default()
        }
    }
}

impl Component for LoginSplash {
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
                    self.loading_messages.push(startup_message.clone())
                }
            }

            _ => {}
        }
        Ok(None)
    }

    fn draw(&mut self, f: &mut Frame<'_>, area: Rect) -> Result<()> {
        let frame = &self.logo_frames[self.counter];
        let frame_lines: Vec<&str> = frame.lines().collect();
        let total_lines = frame_lines.len() + 1;
        let lines_above = (area.height as usize - total_lines) / 2;
        let lines_below = area.height as usize - lines_above - total_lines;

        let mut text = Text::default();
        for _ in 0..lines_above {
            text.lines.push(Line::from(""));
        }
        for line in frame_lines {
            text.lines.push(Line::from(line));
        }

        // add a blank line between the logo and loading message
        text.lines.push(Line::from(""));

        let loading_message = if !self.loading_messages.is_empty() {
            &self.loading_messages[self.counter % self.loading_messages.len()]
        } else {
            // Add better error messages
            "Loading..."
        };
        text.lines.push(Line::from(loading_message));

        for _ in 0..lines_below {
            text.lines.push(Line::from(""));
        }

        let p = Paragraph::new(text).alignment(Alignment::Center);
        f.render_widget(p, area);
        Ok(())
    }
}
