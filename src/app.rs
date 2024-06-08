use color_eyre::eyre::Result;
use crossterm::event::KeyEvent;
use futures::sink::Send;
use ratatui::{layout::Constraint, prelude::*};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, env};
use tokio::sync::mpsc::{self, UnboundedSender};

use crate::{
    action::Action,
    components::{
        fps::FpsCounter, home::Home, login_gauge::LoginGauge, login_splash::LoginSplash, Component,
    },
    config::Config,
    mode::Mode,
    tui,
};

pub struct App {
    pub config: Config,
    pub tick_rate: f64,
    pub frame_rate: f64,
    pub components: Vec<Box<dyn Component>>,
    pub should_quit: bool,
    pub should_suspend: bool,
    pub mode: Mode,
    pub last_tick_key_events: Vec<KeyEvent>,
    pub fivver_username: String,
    pub fivver_password: String,
}

impl App {
    pub fn new(tick_rate: f64, frame_rate: f64) -> Result<Self> {
        let fivver_username =
            env::var("FIVVER_USERNAME").expect("FIVER_USERNAME environment variable is not set");
        let fivver_password =
            env::var("FIVVER_PASSWORD").expect("FIVVER_PASSWORD environment variable is not set");
        let home = Home::new();
        let fps = FpsCounter::default();
        let login_splash = LoginSplash::new();
        let login_gauge = LoginGauge::new();
        let config = Config::new()?;
        let mode = Mode::Home;
        Ok(Self {
            tick_rate,
            frame_rate,
            components: vec![Box::new(login_gauge), Box::new(login_splash), Box::new(fps)],
            should_quit: false,
            should_suspend: false,
            config,
            mode,
            last_tick_key_events: Vec::new(),
            fivver_username,
            fivver_password,
        })
    }

    pub async fn run(&mut self) -> Result<()> {
        let (action_tx, mut action_rx) = mpsc::unbounded_channel();

        let mut tui = tui::Tui::new()?
            .tick_rate(self.tick_rate)
            .frame_rate(self.frame_rate);
        // tui.mouse(true);
        tui.enter()?;

        for component in self.components.iter_mut() {
            component.register_action_handler(action_tx.clone())?;
        }

        for component in self.components.iter_mut() {
            component.register_config_handler(self.config.clone())?;
        }

        for component in self.components.iter_mut() {
            component.init(tui.size()?)?;
        }

        loop {
            if let Some(e) = tui.next().await {
                match e {
                    tui::Event::Quit => action_tx.send(Action::Quit)?,
                    tui::Event::Tick => action_tx.send(Action::Tick)?,
                    tui::Event::Render => action_tx.send(Action::Render)?,
                    tui::Event::Resize(x, y) => action_tx.send(Action::Resize(x, y))?,
                    tui::Event::Key(key) => {
                        if let Some(keymap) = self.config.keybindings.get(&self.mode) {
                            if let Some(action) = keymap.get(&vec![key]) {
                                log::info!("Got action: {action:?}");
                                action_tx.send(action.clone())?;
                            } else {
                                // If the key was not handled as a single key action,
                                // then consider it for multi-key combinations.
                                self.last_tick_key_events.push(key);

                                // Check for multi-key combinations
                                if let Some(action) = keymap.get(&self.last_tick_key_events) {
                                    log::info!("Got action: {action:?}");
                                    action_tx.send(action.clone())?;
                                }
                            }
                        };
                    }
                    _ => {}
                }
                for component in self.components.iter_mut() {
                    if let Some(action) = component.handle_events(Some(e.clone()))? {
                        action_tx.send(action)?;
                    }
                }
            }

            while let Ok(action) = action_rx.try_recv() {
                if action != Action::Tick && action != Action::Render {
                    log::debug!("{action:?}");
                }
                match action {
                    Action::Tick => {
                        self.last_tick_key_events.drain(..);
                    }
                    Action::Quit => self.should_quit = true,
                    Action::Suspend => self.should_suspend = true,
                    Action::Resume => self.should_suspend = false,
                    Action::Resize(w, h) => {
                        tui.resize(Rect::new(0, 0, w, h))?;
                        tui.draw(|f| {
                            for component in self.components.iter_mut() {
                                let r = component.draw(f, f.size());
                                if let Err(e) = r {
                                    action_tx
                                        .send(Action::Error(format!("Failed to draw: {:?}", e)))
                                        .unwrap();
                                }
                            }
                        })?;
                    }
                    Action::Render => {
                        tui.draw(|f| {
                            for component in self.components.iter_mut() {
                                let r = component.draw(f, f.size());
                                if let Err(e) = r {
                                    action_tx
                                        .send(Action::Error(format!("Failed to draw: {:?}", e)))
                                        .unwrap();
                                }
                            }
                        })?;
                    }
                    _ => {}
                }
                for component in self.components.iter_mut() {
                    if let Some(action) = component.update(action.clone())? {
                        action_tx.send(action)?
                    };
                }
            }
            if self.should_suspend {
                tui.suspend()?;
                action_tx.send(Action::Resume)?;
                tui = tui::Tui::new()?
                    .tick_rate(self.tick_rate)
                    .frame_rate(self.frame_rate);
                // tui.mouse(true);
                tui.enter()?;
            } else if self.should_quit {
                tui.stop()?;
                break;
            }
        }
        tui.exit()?;
        Ok(())
    }

    pub async fn send_startup_message(tx: &UnboundedSender<Action>, message: &str) -> Result<()> {
        let mut map = HashMap::new();
        map.insert("startup".to_string(), message.to_string());
        tx.send(Action::Message(map))?;
        Ok(())
    }
}
