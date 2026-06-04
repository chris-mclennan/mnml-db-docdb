//! App state — same pattern as the SQL siblings but with a Mongo
//! client + the user's chosen default DB.

use crate::config::{Config, Connection};
use crate::docdb::{QueryResult, connect, run_query};
use anyhow::Result;
use mongodb::Client;

pub struct App {
    #[allow(dead_code)]
    pub cfg: Config,
    pub connections: Vec<ConnState>,
    pub active: usize,
    pub query: String,
    pub cursor: usize,
    pub last_result: Option<QueryResult>,
    pub result_row: usize,
    pub status: String,
    pub row_limit: u32,
}

pub struct ConnState {
    pub cfg: Connection,
    pub client: Option<Client>,
    pub last_error: Option<String>,
}

impl App {
    pub async fn new(cfg: Config) -> Result<Self> {
        let connections: Vec<ConnState> = cfg
            .connections
            .iter()
            .map(|c| ConnState {
                cfg: c.clone(),
                client: None,
                last_error: None,
            })
            .collect();
        let row_limit = cfg.row_limit;
        Ok(App {
            cfg,
            connections,
            active: 0,
            query: String::new(),
            cursor: 0,
            last_result: None,
            result_row: 0,
            status: "press Ctrl+Enter to run · Alt+1-9 switch connection · q quit".to_string(),
            row_limit,
        })
    }

    pub fn active_name(&self) -> &str {
        &self.connections[self.active].cfg.name
    }

    pub fn switch_connection(&mut self, idx: usize) {
        if idx < self.connections.len() {
            self.active = idx;
            self.status = format!("connection: {}", self.connections[idx].cfg.name);
        }
    }

    async fn ensure_connected(&mut self) -> Result<()> {
        let idx = self.active;
        if self.connections[idx].client.is_some() {
            return Ok(());
        }
        let uri = self.connections[idx].cfg.uri.clone();
        match connect(&uri).await {
            Ok(c) => {
                self.connections[idx].client = Some(c);
                self.connections[idx].last_error = None;
                Ok(())
            }
            Err(e) => {
                self.connections[idx].last_error = Some(e.to_string());
                Err(e)
            }
        }
    }

    pub async fn run_query(&mut self) {
        if self.query.trim().is_empty() {
            self.status = "query is empty".to_string();
            return;
        }
        self.status = format!("running on {}…", self.active_name());
        if let Err(e) = self.ensure_connected().await {
            self.status = format!("connect failed: {e}");
            return;
        }
        let idx = self.active;
        let q = self.query.clone();
        let limit = self.row_limit;
        let default_db = self.connections[idx].cfg.default_db.clone();
        let client = self.connections[idx].client.as_ref().unwrap();
        match run_query(client, &default_db, &q, limit).await {
            Ok(result) => {
                let n = result.rows.len();
                let total = result.server_row_count;
                let ms = result.elapsed.as_millis();
                self.status = if result.truncated {
                    format!("{n} of {total} docs · {ms}ms · truncated (press R to double limit)")
                } else {
                    format!("{n} docs · {ms}ms")
                };
                self.result_row = 0;
                self.last_result = Some(result);
            }
            Err(e) => {
                self.last_result = None;
                self.status = format!("error: {e}");
                self.connections[idx].last_error = Some(e.to_string());
            }
        }
    }

    pub fn move_result_row(&mut self, delta: isize) {
        let Some(result) = self.last_result.as_ref() else {
            return;
        };
        if result.rows.is_empty() {
            return;
        }
        let s = self.result_row as isize + delta;
        let new = s.clamp(0, result.rows.len() as isize - 1) as usize;
        self.result_row = new;
    }

    pub fn query_insert(&mut self, c: char) {
        let byte = self
            .query
            .char_indices()
            .nth(self.cursor)
            .map(|(b, _)| b)
            .unwrap_or_else(|| self.query.len());
        self.query.insert(byte, c);
        self.cursor += 1;
    }

    pub fn query_backspace(&mut self) {
        if self.cursor == 0 {
            return;
        }
        let start = self
            .query
            .char_indices()
            .nth(self.cursor - 1)
            .map(|(b, _)| b)
            .unwrap_or(0);
        let end = self
            .query
            .char_indices()
            .nth(self.cursor)
            .map(|(b, _)| b)
            .unwrap_or_else(|| self.query.len());
        self.query.replace_range(start..end, "");
        self.cursor -= 1;
    }

    pub fn query_clear(&mut self) {
        self.query.clear();
        self.cursor = 0;
    }

    pub fn double_row_limit(&mut self) {
        self.row_limit = self.row_limit.saturating_mul(2);
        self.status = format!("row_limit = {} — re-run with Ctrl+Enter", self.row_limit);
    }
}
