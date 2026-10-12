//! Deployment progress for the interface (`nexus-deploy-progress`): extracting
//! mods into the profile and placing files in the game. Throttled; never fails
//! the operation it reports.
use serde::Serialize;
use std::{sync::OnceLock, time::{Duration, Instant}};
use tauri::Emitter;

static APP: OnceLock<tauri::AppHandle> = OnceLock::new();

pub fn init(app: &tauri::AppHandle) { let _ = APP.set(app.clone()); }

#[derive(Clone, Serialize)]
struct Progress<'a> { game_id: &'a str, phase: &'a str, detail: &'a str, done: usize, total: usize }

fn emit(game_id: &str, phase: &str, detail: &str, done: usize, total: usize) {
    if let Some(app) = APP.get() { let _ = app.emit("nexus-deploy-progress", Progress { game_id, phase, detail, done, total }); }
}

pub(super) struct Reporter { game_id: String, phase: &'static str, detail: String, total: usize, done: usize, last: Option<Instant> }
impl Reporter {
    pub(super) fn new(game_id: &str, phase: &'static str, detail: &str, total: usize) -> Self {
        let mut reporter = Self { game_id: game_id.into(), phase, detail: detail.into(), total, done: 0, last: None };
        reporter.send(true);
        reporter
    }
    pub(super) fn step(&mut self) { self.done += 1; self.send(self.done >= self.total); }
    fn send(&mut self, force: bool) {
        if !force && self.last.is_some_and(|time| time.elapsed() < Duration::from_millis(100)) { return; }
        self.last = Some(Instant::now());
        emit(&self.game_id, self.phase, &self.detail, self.done.min(self.total), self.total);
    }
}
/// The operation ended (successfully or not): the interface hides the counter.
pub(super) fn clear(game_id: &str) { emit(game_id, "", "", 0, 0); }
