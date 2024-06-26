use std::{collections::HashMap, time::Duration};

use canvas::Line;
use color_eyre::eyre::Result;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{prelude::*, widgets::*};
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc::UnboundedSender;

use super::{Component, Frame};
use crate::{action::Action, config::Config};

#[derive(Default)]
pub struct Home {
    progress: f64,
}

impl Home {
    pub fn new() -> Self {
        Self { progress: 0.0 }
    }

    fn set_progress(&mut self, progress: f64) {
        self.progress = progress;
    }
}

impl Component for Home {
    fn register_action_handler(&mut self, tx: UnboundedSender<Action>) -> Result<()> {
        Ok(())
    }

    fn register_config_handler(&mut self, config: Config) -> Result<()> {
        Ok(())
    }

    fn update(&mut self, action: Action) -> Result<Option<Action>> {
        if let Action::Message(map) = action {
            if map.get("startup").is_some() {
                if self.progress < 1.0 {
                    self.progress += 0.1;
                }
            }
        }
        Ok(None)
    }

    fn draw(&mut self, f: &mut Frame<'_>, area: Rect) -> Result<()> {
        let gauge = LineGauge::default().ratio(self.progress);
        f.render_widget(gauge, area);
        Ok(())
    }
}
