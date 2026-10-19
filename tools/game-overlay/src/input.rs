//! Platform-independent input ownership. Opening the menu must not strand a
//! key/button that the game received before capture began. This is independent
//! of drawing and can be exercised without a compositor or GPU.
use egui::{Event, Modifiers, PointerButton, Pos2, Vec2};
use std::collections::BTreeSet;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Route {
    Game,
    Overlay,
}

pub struct Input {
    pub visible: bool,
    pub focused: bool,
    pub ready: bool,
    pub position: Pos2,
    pub extent: Vec2,
    pub events: Vec<Event>,
    game_keys: BTreeSet<u32>,
    game_buttons: BTreeSet<u32>,
    held_keys: BTreeSet<u32>,
    captured_keys: BTreeSet<u32>,
    captured_buttons: BTreeSet<u32>,
    raw_game_buttons: BTreeSet<u32>,
    raw_captured_buttons: BTreeSet<u32>,
    reset_ui: bool,
    shortcut: u32,
    shortcut_modifiers: u8,
    last_relative: Option<std::time::Instant>,
    last_toggle: Option<std::time::Instant>,
}

impl Default for Input {
    fn default() -> Self {
        Self {
            visible: false,
            focused: false,
            ready: false,
            position: Pos2::new(160., 100.),
            extent: Vec2::new(1280., 720.),
            events: Vec::new(),
            game_keys: BTreeSet::new(),
            game_buttons: BTreeSet::new(),
            held_keys: BTreeSet::new(),
            captured_keys: BTreeSet::new(),
            captured_buttons: BTreeSet::new(),
            raw_game_buttons: BTreeSet::new(),
            raw_captured_buttons: BTreeSet::new(),
            reset_ui: true,
            shortcut: 66,
            shortcut_modifiers: 0,
            last_relative: None,
            last_toggle: None,
        } // evdev KEY_F8 (not an X11 keycode)
    }
}

impl Input {
    pub fn shortcut(&mut self, text: &str) -> Result<(), String> {
        let (key, modifiers) = crate::shortcut::parse(text)?;
        self.shortcut = key;
        self.shortcut_modifiers = modifiers;
        Ok(())
    }

    pub fn seed_keys(&mut self, keys: &[u32]) {
        self.held_keys = keys.iter().copied().collect();
        self.game_keys = self.held_keys.clone();
        self.captured_keys.clear();
    }

    pub fn seed_x11_buttons(&mut self, mask: u32) {
        for (bit, button) in [(8, 272), (9, 274), (10, 273)] {
            if mask & (1 << bit) != 0 && !self.captured_buttons.contains(&button) {
                self.game_buttons.insert(button);
            }
        }
    }

    pub fn filter_x11_buttons(&self, mask: &mut u32) {
        if !self.focused {
            return;
        }
        for (bit, button) in [(8, 272), (9, 274), (10, 273)] {
            if !self.game_buttons.contains(&button)
                && (self.capturing() || self.captured_buttons.contains(&button))
            {
                *mask &= !(1 << bit);
            }
        }
        if self.capturing() {
            *mask &= !((1 << 11) | (1 << 12));
        }
    }

    fn modifiers(&self) -> u8 {
        let has = |a, b| self.held_keys.contains(&a) || self.held_keys.contains(&b);
        u8::from(has(29, 97))
            | u8::from(has(42, 54)) * 2
            | u8::from(has(56, 100)) * 4
            | u8::from(has(125, 126)) * 8
    }

    pub fn toggle(&mut self) -> bool {
        if !self.visible && (!self.ready || !self.focused) {
            return false;
        }
        let now = std::time::Instant::now();
        // Companion event streams may observe the same physical press.
        if self
            .last_toggle
            .is_some_and(|t| now.duration_since(t).as_millis() < 180)
        {
            return true;
        }
        self.last_toggle = Some(now);
        self.visible = !self.visible;
        self.events.clear();
        self.reset_ui = true;
        true
    }

    pub fn release(&mut self) {
        self.visible = false;
        self.events.clear();
        self.reset_ui = true;
        // Keep game-down sets until physical releases are forwarded. Focus loss
        // is a separate boundary: Wayland's leave resets the application's state.
    }

    pub fn focus(&mut self, focused: bool) {
        if self.focused != focused {
            self.reset_ui = true;
        }
        self.focused = focused;
        if !focused {
            // Alt+Tab releases input ownership, not the user's requested menu.
            // Returning to the game restores the same panel; Esc never closes.
            self.events.clear();
            self.game_keys.clear();
            self.game_buttons.clear();
            self.held_keys.clear();
            self.captured_keys.clear();
            self.captured_buttons.clear();
            self.raw_game_buttons.clear();
            self.raw_captured_buttons.clear();
        }
    }

    /// egui's raw `focused` flag does not itself cancel pointer-down/drag
    /// state. Reset that state before the next UI pass, retaining panel memory
    /// and position. Synthetic releases must not click an old hovered control.
    pub fn frame(&mut self, context: &egui::Context, time: f64) -> egui::RawInput {
        if self.reset_ui {
            context.input_mut(|input| input.pointer = Default::default());
            context.stop_dragging();
            self.reset_ui = false;
            if self.focused {
                self.events.insert(0, Event::PointerMoved(self.position));
            }
        }
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(Pos2::ZERO, self.extent)),
            events: std::mem::take(&mut self.events),
            focused: self.focused,
            time: Some(time),
            ..Default::default()
        }
    }

    pub(crate) fn capturing(&self) -> bool {
        self.visible && self.focused && self.ready
    }
    pub(crate) fn visibility_age(&self) -> std::time::Duration {
        self.last_toggle.map(|t| t.elapsed()).unwrap_or_default()
    }
    pub(crate) fn x11_repeat(&mut self, key: u32, route: Route) -> Route {
        self.held_keys.insert(key);
        match route {
            Route::Game => {
                self.game_keys.insert(key);
            }
            Route::Overlay => {
                self.captured_keys.insert(key);
            }
        }
        route
    }
    pub(crate) fn filter_x11_keymap(&self, keys: &mut [u8; 32]) {
        if !self.focused {
            return;
        }
        for xcode in 8..256usize {
            let key = (xcode - 8) as u32;
            if !self.game_keys.contains(&key)
                && (self.capturing() || self.captured_keys.contains(&key))
            {
                keys[xcode / 8] &= !(1 << (xcode % 8));
            }
        }
    }

    fn queue(&mut self, event: Event) {
        // Never let an input flood grow memory without bound in a game process.
        if self.events.len() < 512 {
            self.events.push(event);
        } else {
            self.release();
        }
    }

    pub fn key(&mut self, key: u32, down: bool) -> Route {
        let first = if down {
            self.held_keys.insert(key)
        } else {
            self.held_keys.remove(&key);
            false
        };
        if !down && self.game_keys.remove(&key) {
            return Route::Game;
        }
        if !down && self.captured_keys.remove(&key) {
            return Route::Overlay;
        }
        if key == self.shortcut
            && down
            && self.ready
            && self.focused
            && self.modifiers() == self.shortcut_modifiers
        {
            if self.game_keys.contains(&key) {
                return Route::Game;
            }
            if first {
                self.toggle();
            }
            self.captured_keys.insert(key);
            return Route::Overlay;
        }
        // Escape deliberately has no dismissal action.
        if key == 1 && down {
            self.game_keys.insert(key);
            return Route::Game;
        }
        if self.capturing() {
            if down {
                if !self.game_keys.contains(&key) {
                    self.captured_keys.insert(key);
                }
                return Route::Overlay;
            }
            // A release can belong to a press before interception/focus began.
            // Suppress only a release paired with a known captured press.
            return Route::Game;
        }
        // A key pressed while capturing must not become a game press merely
        // because a repeated event arrives after capture is closed.
        if down && self.captured_keys.contains(&key) {
            return Route::Overlay;
        }
        if down {
            self.game_keys.insert(key);
        }
        Route::Game
    }

    pub fn button(&mut self, button: u32, down: bool) -> Route {
        if !down && self.game_buttons.remove(&button) {
            return Route::Game;
        }
        let captured_release = !down && self.captured_buttons.remove(&button);
        if self.capturing() {
            let mapped = match button {
                272 => Some(PointerButton::Primary),
                273 => Some(PointerButton::Secondary),
                274 => Some(PointerButton::Middle),
                _ => None,
            };
            if let Some(button) = mapped
                .filter(|_| (down && !self.game_buttons.contains(&button)) || captured_release)
            {
                self.queue(Event::PointerButton {
                    pos: self.position,
                    button,
                    pressed: down,
                    modifiers: Modifiers::default(),
                });
            }
        }
        if captured_release {
            return Route::Overlay;
        }
        if self.capturing() {
            if down {
                if !self.game_buttons.contains(&button) {
                    self.captured_buttons.insert(button);
                }
                return Route::Overlay;
            }
            return Route::Game;
        }
        // Only pair a release with a press actually forwarded to the game.
        if down && self.captured_buttons.contains(&button) {
            Route::Overlay
        } else if down {
            self.game_buttons.insert(button);
            Route::Game
        } else {
            Route::Game
        }
    }

    /// XI2 raw buttons belong to the game's raw-input stream. Native pointer
    /// events are the only click source for egui (also respects left-handed
    /// mapping); raw companions must never generate another UI click/release.
    pub fn raw_button(&mut self, button: u32, down: bool) -> Route {
        if !down && self.raw_game_buttons.remove(&button) {
            return Route::Game;
        }
        if !down && self.raw_captured_buttons.remove(&button) {
            return Route::Overlay;
        }
        if self.capturing() {
            if down && !self.raw_game_buttons.contains(&button) {
                self.raw_captured_buttons.insert(button);
            }
            return if down { Route::Overlay } else { Route::Game };
        }
        if down && self.raw_captured_buttons.contains(&button) {
            return Route::Overlay;
        }
        if down {
            self.raw_game_buttons.insert(button);
        }
        Route::Game
    }

    pub fn relative(&mut self, dx: f32, dy: f32) -> Route {
        self.last_relative = Some(std::time::Instant::now());
        if self.capturing() {
            self.position = Pos2::new(
                (self.position.x + dx).clamp(0., self.extent.x),
                (self.position.y + dy).clamp(0., self.extent.y),
            );
            self.queue(Event::PointerMoved(self.position));
            Route::Overlay
        } else {
            Route::Game
        }
    }

    pub fn absolute(&mut self, x: f32, y: f32) -> Route {
        if self.capturing() {
            // A seat can emit both absolute and relative events for one motion.
            // Prefer its relative stream while it is active; don't move twice.
            if self
                .last_relative
                .is_some_and(|t| t.elapsed() < std::time::Duration::from_millis(50))
            {
                return Route::Overlay;
            }
            self.position = Pos2::new(x.clamp(0., self.extent.x), y.clamp(0., self.extent.y));
            self.queue(Event::PointerMoved(self.position));
            Route::Overlay
        } else {
            Route::Game
        }
    }

    /// The native cursor is authoritative once its game grab is suspended.
    /// Raw relative companion events must not move this position a second time.
    pub fn native_absolute(&mut self, x: f32, y: f32) -> Route {
        if self.capturing() {
            self.last_relative = None;
            self.position = Pos2::new(x.clamp(0., self.extent.x), y.clamp(0., self.extent.y));
            self.queue(Event::PointerMoved(self.position));
            Route::Overlay
        } else {
            Route::Game
        }
    }

    pub fn scroll(&mut self, dx: f32, dy: f32) -> Route {
        if self.capturing() {
            self.queue(Event::MouseWheel {
                unit: egui::MouseWheelUnit::Point,
                delta: Vec2::new(-dx, -dy),
                modifiers: Modifiers::default(),
            });
            Route::Overlay
        } else {
            Route::Game
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn ready() -> Input {
        Input {
            ready: true,
            focused: true,
            ..Input::default()
        }
    }
    #[test]
    fn cannot_capture_without_renderer_and_focus() {
        let mut s = Input::default();
        assert!(!s.toggle());
        s.ready = true;
        assert!(!s.toggle());
        s.focused = true;
        assert!(s.toggle());
    }
    #[test]
    fn release_of_game_press_is_forwarded_while_menu_open() {
        let mut s = ready();
        assert_eq!(s.key(17, true), Route::Game);
        assert!(s.toggle());
        assert_eq!(s.key(17, false), Route::Game);
        assert_eq!(s.key(18, true), Route::Overlay);
        s.release();
        assert_eq!(s.key(18, false), Route::Overlay);
    }
    #[test]
    fn buttons_remain_balanced_across_capture() {
        let mut s = ready();
        assert_eq!(s.button(272, true), Route::Game);
        s.toggle();
        assert_eq!(s.button(272, false), Route::Game);
        assert_eq!(s.button(273, true), Route::Overlay);
        s.release();
        assert_eq!(s.button(273, false), Route::Overlay);
    }
    #[test]
    fn escape_does_not_dismiss_and_repeat_does_not_toggle() {
        let mut s = ready();
        s.key(66, true);
        assert!(s.visible);
        s.key(66, true);
        assert_eq!(s.key(1, true), Route::Game);
        assert_eq!(s.key(1, false), Route::Game);
        assert!(s.visible);
    }
    #[test]
    fn focus_loss_releases_capture() {
        let mut s = ready();
        s.toggle();
        s.focus(false);
        assert!(s.visible);
        assert_eq!(s.relative(4., 3.), Route::Game);
        s.focus(true);
        assert_eq!(s.relative(4., 3.), Route::Overlay);
    }
    #[test]
    fn preserves_a_press_received_before_renderer_became_ready() {
        let mut s = Input::default();
        s.focused = true;
        assert_eq!(s.key(66, true), Route::Game);
        s.ready = true;
        assert_eq!(s.key(66, false), Route::Game);
    }
    #[test]
    fn modifiers_and_recorded_key_change_are_respected() {
        let mut s = ready();
        s.shortcut("Control_L+F12").unwrap();
        assert_eq!(s.key(88, true), Route::Game);
        s.key(88, false);
        assert!(!s.visible);
        s.key(29, true);
        assert_eq!(s.key(88, true), Route::Overlay);
        assert!(s.visible);
        assert_eq!(s.key(29, false), Route::Game);
        assert_eq!(s.key(88, false), Route::Overlay);
    }
    #[test]
    fn closing_does_not_leak_repeat_of_a_captured_key() {
        let mut s = ready();
        s.toggle();
        s.key(18, true);
        s.release();
        assert_eq!(s.key(18, true), Route::Overlay);
        assert_eq!(s.key(18, false), Route::Overlay);
    }
    #[test]
    fn focus_snapshot_preserves_releases_of_already_held_keys() {
        let mut s = ready();
        s.seed_keys(&[29, 17]);
        s.toggle();
        assert_eq!(s.key(29, false), Route::Game);
        assert_eq!(s.key(17, false), Route::Game);
    }
    #[test]
    fn unknown_releases_are_not_swallowed_at_startup_or_after_focus_loss() {
        let mut s = ready();
        s.toggle();
        assert_eq!(s.key(17, false), Route::Game);
        assert_eq!(s.button(272, false), Route::Game);
        s.focus(false);
        s.focus(true);
        assert_eq!(s.key(17, false), Route::Game);
    }
    #[test]
    fn x11_polling_does_not_resurrect_captured_keys_after_closing() {
        let mut s = ready();
        s.key(17, true);
        s.toggle();
        s.key(18, true);
        let mut keys = [255u8; 32];
        s.filter_x11_keymap(&mut keys);
        assert_ne!(keys[25 / 8] & (1 << (25 % 8)), 0); // game-held W
        assert_eq!(keys[26 / 8] & (1 << (26 % 8)), 0); // menu-held E
        s.release();
        let mut keys = [255u8; 32];
        s.filter_x11_keymap(&mut keys);
        assert_eq!(keys[26 / 8] & (1 << (26 % 8)), 0);
    }
    #[test]
    fn core_x11_repeat_pairs_do_not_toggle_the_menu_again() {
        let mut s = ready();
        s.key(66, true);
        assert!(s.visible);
        s.key(66, false);
        s.last_toggle = None; // A repeat must stay harmless even after debounce.
        s.x11_repeat(66, Route::Overlay);
        assert!(s.visible);
        assert_eq!(s.key(66, false), Route::Overlay);
        assert_eq!(s.key(66, true), Route::Overlay);
        assert!(!s.visible); // A distinct physical press still closes normally.
    }
    #[test]
    fn physical_cursor_is_the_click_position_and_polling_cannot_fire_in_game() {
        let mut s = ready();
        s.toggle();
        s.events.clear();
        s.relative(90., 45.);
        s.native_absolute(800., 300.);
        assert_eq!(s.button(272, true), Route::Overlay);
        assert!(
            matches!(s.events.last(),Some(Event::PointerButton {pos,pressed:true,..}) if *pos==Pos2::new(800.,300.))
        );
        let mut mask = 1 << 8;
        s.filter_x11_buttons(&mut mask);
        assert_eq!(mask, 0);
        s.release();
        let mut mask = 1 << 8;
        s.filter_x11_buttons(&mut mask);
        assert_eq!(mask, 0);
        assert_eq!(s.button(272, false), Route::Overlay);
        assert_eq!(s.button(272, true), Route::Game);
    }
    #[test]
    fn button_held_before_opening_releases_only_to_the_game() {
        let mut s = ready();
        s.toggle();
        s.seed_x11_buttons(1 << 8);
        s.events.clear();
        let mut mask = 1 << 8;
        s.filter_x11_buttons(&mut mask);
        assert_eq!(mask, 1 << 8);
        assert_eq!(s.button(272, false), Route::Game);
        assert!(s.events.is_empty());
    }
    #[test]
    fn raw_buttons_do_not_duplicate_native_clicks_or_leak_releases_after_closing() {
        for raw_first in [true, false] {
            let mut s = ready();
            s.toggle();
            s.events.clear();
            // A left-handed mapping may turn raw button 1 into native secondary.
            assert_eq!(s.raw_button(1, true), Route::Overlay);
            assert!(s.events.is_empty());
            assert_eq!(s.button(273, true), Route::Overlay);
            assert_eq!(
                s.events
                    .iter()
                    .filter(|e| matches!(e, Event::PointerButton { pressed: true, .. }))
                    .count(),
                1
            );
            s.release();
            if raw_first {
                assert_eq!(s.raw_button(1, false), Route::Overlay);
                assert_eq!(s.button(273, false), Route::Overlay);
            } else {
                assert_eq!(s.button(273, false), Route::Overlay);
                assert_eq!(s.raw_button(1, false), Route::Overlay);
            }
            assert_eq!(s.raw_button(1, true), Route::Game);
            assert_eq!(s.button(273, true), Route::Game);
        }
    }
    #[test]
    fn reopening_resets_a_drag_released_while_the_renderer_was_hidden() {
        let mut s = ready();
        s.toggle();
        s.events.clear();
        let context = egui::Context::default();
        s.button(272, true);
        let raw = s.frame(&context, 0.);
        let _ = context.run(raw, |_| {});
        assert!(context.input(|i| i.pointer.any_down()));
        s.release();
        assert_eq!(s.button(272, false), Route::Overlay);
        s.last_toggle = None;
        assert!(s.toggle());
        let raw = s.frame(&context, 1.);
        assert!(!raw
            .events
            .iter()
            .any(|e| matches!(e, Event::PointerButton { .. })));
        assert!(!context.input(|i| i.pointer.any_down()));
    }
}
