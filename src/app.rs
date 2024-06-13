use color_eyre::eyre::Result;
use crossterm::event::KeyEvent;
use fantoccini::{Client, ClientBuilder, Locator};
use futures::sink::Send;
use ratatui::{layout::Constraint, prelude::*};
use serde::{Deserialize, Serialize};
use std::process::{Command, Stdio};
use std::{collections::HashMap, env, sync::Arc};
use tokio::sync::{
    mpsc::{self, UnboundedSender},
    Mutex,
};
use tokio::time::{sleep, Duration};

use crate::{
    action::Action,
    components::{login::LoginComponent, Component},
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
    pub web_client: Option<Arc<Mutex<Option<Client>>>>,
}

impl App {
    pub fn new(tick_rate: f64, frame_rate: f64) -> Result<Self> {
        let fivver_username =
            env::var("FIVVER_USERNAME").expect("FIVER_USERNAME environment variable is not set");
        let fivver_password =
            env::var("FIVVER_PASSWORD").expect("FIVVER_PASSWORD environment variable is not set");
        let login = LoginComponent::new();
        let config = Config::new()?;
        let mode = Mode::Home;
        let web_client = None;

        Ok(Self {
            tick_rate,
            frame_rate,
            components: vec![Box::new(login)],
            should_quit: false,
            should_suspend: false,
            config,
            mode,
            last_tick_key_events: Vec::new(),
            fivver_username,
            fivver_password,
            web_client,
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

        self.fetch_data(action_tx.clone()).await?;

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
                    Action::Quit => {
                        self.should_quit = true;
                        self.close_web_client()
                            .await
                            .expect("Failed to close WebDriver client");
                    }
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

    async fn fetch_data(&mut self, tx: UnboundedSender<Action>) -> Result<()> {
        let mut message1 = HashMap::new();
        message1.insert("startup".to_string(), "Starting Geckodriver...".to_string());
        tx.send(Action::Message(message1))?;

        if self.web_client.is_none() {
            self.init_web_client().await?;
        }
        Ok(())
    }

    async fn is_geckodriver_running(&self) -> bool {
        if let Ok(output) = Command::new("pgrep").arg("geckodriver").output() {
            !output.stdout.is_empty()
        } else {
            false
        }
    }

    async fn start_geckodriver(&self) -> Result<()> {
        Command::new("geckodriver")
            .stdout(Stdio::null())
            .stdout(Stdio::null())
            .spawn()
            .expect("Failed to start geckodriver");
        Ok(())
    }

    async fn init_web_client(&mut self) -> Result<()> {
        if !self.is_geckodriver_running().await {
            self.start_geckodriver().await?;
            sleep(Duration::from_secs(2)).await;
        }
        let client = ClientBuilder::native()
            .connect("http://localhost:4444")
            .await
            .expect("failed to connect to WebDriver");
        self.web_client = Some(Arc::new(Mutex::new(Some(client))));
        Ok(())
    }

    async fn close_web_client(&mut self) -> Result<()> {
        if let Some(web_client) = &self.web_client {
            let mut client = web_client.lock().await;
            if let Some(client) = client.take() {
                if let Err(e) = client.close().await {
                    eprintln!("Failed to close WebDriver client: {}", e);
                }
            }
        }
        // kill the gecko driver process
        Command::new("pkill")
            .arg("geckodriver")
            .output()
            .expect("Failed to stop geckodriver");
        Ok(())
    }
}
