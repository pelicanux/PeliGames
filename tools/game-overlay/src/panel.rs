//! The actual panel UI, shared with headless click/drag regression tests.
use crate::controls::Controls;

pub(crate) fn show(
    ctx: &egui::Context,
    extent: egui::Vec2,
    controls: &mut Controls,
    error: &str,
) -> egui::InnerResponse<bool> {
    let position = egui::pos2((extent.x - 310.).max(8.), 16.);
    egui::Area::new(egui::Id::new("peligames-mako"))
        .default_pos(position)
        .movable(true)
        .constrain(true)
        .show(ctx, |ui| {
            let mut changed = false;
            egui::Frame::new()
                .fill(egui::Color32::from_rgba_unmultiplied(12, 17, 23, 96))
                .corner_radius(10.)
                .inner_margin(12.)
                .show(ui, |ui| {
                    ui.set_width(270.);
                    ui.horizontal(|ui| {
                        ui.add(
                            egui::Label::new(egui::RichText::new("PeliGames · MAKO").strong())
                                .selectable(false),
                        )
                        .on_hover_text(
                            "Arraste para mover o painel. O mesmo atalho mostra e oculta.",
                        );
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            changed |= ui.checkbox(&mut controls.paused, "Pausar").changed();
                        });
                    });
                    ui.horizontal(|ui| {
                        for n in 2..=5 {
                            changed |= ui
                                .selectable_value(&mut controls.multiplier, n, format!("{n}×"))
                                .changed();
                        }
                    });
                    changed |= ui.checkbox(&mut controls.adaptive, "Adaptativo").changed();
                    if controls.adaptive {
                        ui.horizontal(|ui| {
                            ui.label(format!("Alvo: {} FPS", controls.target_fps));
                            changed |= ui
                                .add(
                                    egui::Slider::new(&mut controls.target_fps, 10..=1000)
                                        .show_value(false),
                                )
                                .changed();
                        });
                    }
                    ui.label(format!("Movimento: {:.0}%", controls.flow_scale * 100.));
                    changed |= ui
                        .add(
                            egui::Slider::new(&mut controls.flow_scale, 0.25..=1.)
                                .show_value(false),
                        )
                        .changed();
                    changed |= ui
                        .checkbox(&mut controls.performance, "Modelo leve")
                        .changed();
                    if !error.is_empty() {
                        ui.colored_label(egui::Color32::from_rgb(255, 180, 120), error);
                    }
                });
            changed
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::{Input, Route};
    use egui::{
        epaint::{ClippedShape, Shape},
        Pos2, Rect, Vec2,
    };

    struct Harness {
        ctx: egui::Context,
        controls: Controls,
        input: Input,
        time: f64,
        bounds: Rect,
        changed: bool,
    }
    impl Harness {
        fn new() -> Self {
            let mut input = Input::default();
            input.ready = true;
            input.focused = true;
            input.extent = egui::vec2(1280., 720.);
            input.toggle();
            let mut result = Self {
                ctx: egui::Context::default(),
                controls: Controls {
                    multiplier: 2,
                    flow_scale: 0.5,
                    performance: false,
                    adaptive: false,
                    target_fps: 180,
                    paused: false,
                },
                input,
                time: 0.,
                bounds: Rect::NOTHING,
                changed: false,
            };
            for _ in 0..3 {
                result.frame();
            }
            result
        }
        fn frame(&mut self) -> egui::FullOutput {
            self.time += 1. / 60.;
            self.ctx.run(self.input.frame(&self.ctx, self.time), |ctx| {
                let response = show(ctx, self.input.extent, &mut self.controls, "");
                self.bounds = response.response.rect;
                self.changed = response.inner;
            })
        }
        fn move_to(&mut self, pos: Pos2) {
            assert_eq!(self.input.native_absolute(pos.x, pos.y), Route::Overlay);
            self.frame();
        }
        fn click(&mut self, pos: Pos2) -> bool {
            self.move_to(pos);
            assert_eq!(self.input.button(272, true), Route::Overlay);
            self.frame();
            assert_eq!(self.input.button(272, false), Route::Overlay);
            self.frame();
            self.changed
        }
    }
    fn text_position(shapes: &[ClippedShape], text: &str) -> Pos2 {
        fn find(shape: &Shape, text: &str) -> Option<Pos2> {
            match shape {
                Shape::Text(s) if s.galley.text() == text => Some(s.pos + s.galley.size() * 0.5),
                Shape::Vec(shapes) => shapes.iter().find_map(|s| find(s, text)),
                _ => None,
            }
        }
        shapes
            .iter()
            .find_map(|s| find(&s.shape, text))
            .expect("text in the actual panel")
    }
    #[test]
    fn native_cursor_click_changes_real_panel_controls_without_game_clicks() {
        let mut h = Harness::new();
        let output = h.frame();
        let multiplier = text_position(&output.shapes, "3×");
        assert!(h.click(multiplier));
        assert_eq!(h.controls.multiplier, 3);
        let output = h.frame();
        let performance = text_position(&output.shapes, "Modelo leve");
        assert!(h.click(performance));
        assert!(h.controls.performance);
    }
    #[test]
    fn real_panel_can_be_dragged_and_cannot_leave_the_game_image() {
        let mut h = Harness::new();
        let before = h.bounds;
        let header = Pos2::new(before.left() + 36., before.top() + 18.);
        h.move_to(header);
        assert_eq!(h.input.button(272, true), Route::Overlay);
        h.frame();
        h.move_to(header + Vec2::new(-180., 140.));
        h.frame();
        assert_eq!(h.input.button(272, false), Route::Overlay);
        h.frame();
        assert!(
            (h.bounds.left() - (before.left() - 180.)).abs() < 2.,
            "before={before:?}, after={:?}",
            h.bounds
        );
        assert!(
            (h.bounds.top() - (before.top() + 140.)).abs() < 2.,
            "before={before:?}, after={:?}",
            h.bounds
        );
        let output = h.frame();
        let header = text_position(&output.shapes, "PeliGames · MAKO");
        h.move_to(header);
        h.input.button(272, true);
        h.frame();
        h.move_to(Pos2::new(0., 0.));
        h.frame();
        h.input.button(272, false);
        h.frame();
        assert!(h.bounds.left() >= 0. && h.bounds.top() >= 0.);
        assert!(h.bounds.right() <= 1280. && h.bounds.bottom() <= 720.);
    }
    #[test]
    fn focus_loss_cancels_drag_and_controls_work_when_returning() {
        let mut h = Harness::new();
        let header = Pos2::new(h.bounds.left() + 36., h.bounds.top() + 18.);
        h.move_to(header);
        h.input.button(272, true);
        h.frame();
        h.input.focus(false);
        h.frame();
        h.input.focus(true);
        h.frame();
        let before = h.bounds;
        h.move_to(header + Vec2::new(-80., 100.));
        h.frame();
        assert_eq!(before, h.bounds);
        let output = h.frame();
        let multiplier = text_position(&output.shapes, "4×");
        assert!(h.click(multiplier));
        assert_eq!(h.controls.multiplier, 4);
    }
}
