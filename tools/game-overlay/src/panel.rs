//! The actual panel UI, shared with headless click/drag regression tests.
use crate::controls::Controls;

#[cfg(test)]
pub(crate) fn show(
    ctx: &egui::Context,
    extent: egui::Vec2,
    controls: &mut Controls,
    error: &str,
) -> egui::InnerResponse<bool> {
    show_tools(ctx, extent, Some(controls), error)
}

pub(crate) fn show_tools(
    ctx: &egui::Context,
    extent: egui::Vec2,
    mako: Option<&mut Controls>,
    error: &str,
) -> egui::InnerResponse<bool> {
    show_with_capture(ctx, extent, mako, error, None)
}

pub(crate) fn show_with_capture(
    ctx: &egui::Context,
    extent: egui::Vec2,
    mut mako: Option<&mut Controls>,
    error: &str,
    capture: Option<&crate::capture::Controller>,
) -> egui::InnerResponse<bool> {
    // The frame margin is outside Resize; reserve it when constraining to
    // the game viewport. Extra tools grow vertically instead of widening it.
    let max_size = (extent - egui::vec2(56.,56.)).max(egui::vec2(120.,120.));
    let min_size = egui::vec2(320.,240.).min(max_size);
    let default_size = egui::vec2((extent.x * 0.22).clamp(380.,460.),extent.y * 0.90)
        .max(min_size).min(max_size);
    let position = egui::pos2((extent.x - default_size.x - 40.).max(8.), 16.);
    egui::Area::new(egui::Id::new("peligames-overlay"))
        .default_pos(position)
        .default_size(default_size+egui::vec2(24.,24.))
        .movable(true)
        .constrain(true)
        .show(ctx, |ui| {
            let mut changed = false;
            egui::Frame::new()
                .fill(egui::Color32::from_rgba_unmultiplied(12, 17, 23, 96))
                .corner_radius(10.)
                .inner_margin(12.)
                .show(ui, |ui| {
                    egui::Resize::default()
                        .id(egui::Id::new("peligames-overlay-size"))
                        .default_size(default_size)
                        .min_size(min_size)
                        .max_size(max_size)
                        .with_stroke(false)
                        .show(ui,|ui| {
                    ui.style_mut().wrap_mode=Some(egui::TextWrapMode::Wrap);
                    ui.horizontal(|ui| {
                        ui.add(
                            egui::Label::new(egui::RichText::new("PeliGames").strong())
                                .selectable(false),
                        )
                        .on_hover_text(
                            "Arraste para mover. Use o canto inferior direito para redimensionar. O mesmo atalho mostra e oculta.",
                        );
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if let Some(controls) = mako.as_mut() {
                                changed |= ui.checkbox(&mut controls.paused, "Pausar").changed();
                            }
                        });
                    });
                    let fps=crate::present_metrics::snapshot();
                    let generator=crate::present_metrics::generator();
                    ui.label(fps_text(fps,generator))
                        .on_hover_text("Contagem de apresentações Vulkan a cada 500 ms. Com AFMF ou MAKO, o total inclui os quadros adicionais; o compositor pode descartar quadros acima da taxa do monitor.");
                    if let Some(generator)=generator.filter(|_|fps.is_some_and(|fps|fps.output.is_none())) {
                        ui.small(format!("{} · aguardando medição do total…",generator.name()));
                    }
                    egui::ScrollArea::vertical()
                        .id_salt("peligames-overlay-body")
                        .auto_shrink([false,false])
                        .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden)
                        .min_scrolled_height(40.)
                        .max_height((ui.available_height()-14.-ui.spacing().item_spacing.y).max(40.))
                        .show(ui, |ui| {
                    if let Some(capture) = capture {capture.draw(ui);ui.separator();}
                    if let Some(controls) = mako.as_mut() {
                        ui.small("MAKO · geração de quadros");
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
                        ui.separator();
                        ui.horizontal(|ui| {
                            ui.label("FPS base");
                            changed |= ui.add(egui::DragValue::new(&mut controls.base_fps_cap).range(0..=1000)).on_hover_text("Limite de quadros reais. 0 desativa.").changed();
                        });
                        ui.horizontal(|ui| {
                            ui.label("Limiar (Hz)");
                            changed |= ui.add(egui::DragValue::new(&mut controls.refresh_threshold).range(0..=1000)).on_hover_text("Pausa a geração se a taxa confirmada pelo Gamescope não superar este valor. 0 desativa.").changed();
                        });
                        if controls.scaling_enabled {
                            ui.separator();
                            ui.small("Escala de imagem");
                            egui::ComboBox::from_id_salt("mako-scaling-method")
                                .selected_text(match controls.scaling_method.as_str() {"mako"=>"MAKO Scaler","ls1"=>"LS1 · qualidade","ls1-performance"=>"LS1 · desempenho",_=>"Nativo"})
                                .show_ui(ui, |ui| {
                                    for (method,label) in [("native","Nativo"),("mako","MAKO Scaler"),("ls1","LS1 · qualidade"),("ls1-performance","LS1 · desempenho")] {
                                        changed |= ui.selectable_value(&mut controls.scaling_method,method.into(),label).changed();
                                    }
                                });
                            ui.horizontal(|ui| {
                                ui.label("Nitidez");
                                changed |= ui.add_enabled(controls.scaling_method!="native",egui::Slider::new(&mut controls.scaling_sharpness,0.0..=1.0)).changed();
                            });
                        }
                        ui.small("GPU, FP16, Ultra e escala inicial: launcher/reinício.");
                        if controls.ultra_performance { ui.small("Ultra Performance ativo; mudar o preset exige reiniciar."); }
                    }
                    ui.separator();
                    if let Some(afmf) = crate::tools::afmf_status() {
                        ui.small(afmf);
                    }
                    ui.small(crate::focus_fps::status());
                    if mako.is_none() && error.is_empty() && crate::tools::afmf_status().is_none() {
                        ui.label("Configure as ferramentas nos detalhes do jogo.");
                    }
                    if !error.is_empty() {
                        ui.colored_label(egui::Color32::from_rgb(255, 180, 120), error);
                    }
                    });
                    // Keep the resize grip clear of buttons at the bottom.
                    ui.add_space(14.);
                    });
                });
            changed
        })
}

fn fps_text(snapshot:Option<crate::present_metrics::Snapshot>, generator:Option<crate::present_routing::Generator>)->String {
    let Some(fps)=snapshot else {return "FPS · aguardando medição…".into();};
    if let (Some(output),Some(generator))=(fps.output,generator) {
        format!("FPS · Jogo {:.0} · {} +{:.0} · Total {:.0}",fps.game,generator.name(),output.generated,output.total)
    } else {
        format!("FPS · {:.0}",fps.game)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::{Input, Route};
    use egui::{
        epaint::{ClippedShape, Shape},
        Pos2, Rect, Vec2,
    };

    #[test]
    fn one_panel_composes_optional_mako_with_monitoring_status() {
        fn texts(shape: &Shape, output: &mut Vec<String>) {
            match shape {
                Shape::Text(text) => output.push(text.galley.text().into()),
                Shape::Vec(shapes) => shapes.iter().for_each(|s| texts(s, output)),
                _ => (),
            }
        }
        for mako in [false, true] {
            let context = egui::Context::default();
            let mut controls = Controls {
                multiplier: 2,
                flow_scale: 1.,
                performance: false,
                adaptive: false,
                target_fps: 60,
                paused: false,
                ..Controls::default()
            };
            let render = || egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1280., 720.))),
                ..Default::default()
            };
            // egui measures a new area before its first visible frame.
            let _ = context.run(render(), |ctx| {
                show_tools(
                    ctx,
                    Vec2::new(1280., 720.),
                    mako.then_some(&mut controls),
                    "",
                );
            });
            let frame = context.run(render(), |ctx| {
                show_tools(
                    ctx,
                    Vec2::new(1280., 720.),
                    mako.then_some(&mut controls),
                    "",
                );
            });
            let mut labels = Vec::new();
            for shape in &frame.shapes {
                texts(&shape.shape, &mut labels);
            }
            assert_eq!(
                labels.iter().filter(|s| s.as_str() == "PeliGames").count(),
                1
            );
            assert_eq!(
                labels.iter().any(|s| s == "MAKO · geração de quadros"),
                mako
            );
            assert!(labels.contains(&crate::focus_fps::status()));
            assert!(labels.iter().any(|text|text.starts_with("FPS ·")), "FPS remains visible without MAKO");
        }
    }

    #[test]
    fn fps_label_supports_both_generators_without_inventing_missing_counts() {
        use crate::present_metrics::{Snapshot,Output};
        use crate::present_routing::Generator;
        let base=Snapshot{game:60.,output:None};
        assert_eq!(fps_text(Some(base),None),"FPS · 60");
        for generator in [Generator::Afmf, Generator::Mako] {
            assert_eq!(fps_text(Some(base),Some(generator)),"FPS · 60");
            assert_eq!(fps_text(None,Some(generator)),"FPS · aguardando medição…");
            let generated=Snapshot{game:60.,output:Some(Output{generated:40.,total:100.})};
            assert_eq!(fps_text(Some(generated),Some(generator)),
                format!("FPS · Jogo 60 · {} +40 · Total 100",generator.name()));
            let paused=Snapshot{game:60.,output:Some(Output{generated:0.,total:60.})};
            assert_eq!(fps_text(Some(paused),Some(generator)),
                format!("FPS · Jogo 60 · {} +0 · Total 60",generator.name()));
            assert_eq!(fps_text(Some(generated),None),"FPS · 60");
        }
    }

    struct Harness {
        ctx: egui::Context,
        controls: Controls,
        input: Input,
        time: f64,
        bounds: Rect,
        changed: bool,
        capture: Option<crate::capture::Controller>,
        error: String,
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
                ..Controls::default()
                },
                input,
                time: 0.,
                bounds: Rect::NOTHING,
                changed: false,
                capture: None,
                error: String::new(),
            };
            for _ in 0..3 {
                result.frame();
            }
            result
        }
        fn frame(&mut self) -> egui::FullOutput {
            self.time += 1. / 60.;
            self.ctx.run(self.input.frame(&self.ctx, self.time), |ctx| {
                let response = show_with_capture(ctx, self.input.extent, Some(&mut self.controls), &self.error, self.capture.as_ref());
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
    fn actual_panel_is_taller_and_narrower_and_stays_inside_the_game() {
        fn render(extent:Vec2, expanded:bool)->Rect {
            let ctx=egui::Context::default();
            let mut controls=Controls{adaptive:expanded,scaling_enabled:expanded,..Default::default()};
            let mut bounds=Rect::NOTHING;
            for _ in 0..4 {
                let _=ctx.run(egui::RawInput{screen_rect:Some(Rect::from_min_size(Pos2::ZERO,extent)),..Default::default()},|ctx|{
                    bounds=show(ctx,extent,&mut controls,"").response.rect;
                });
            }
            assert!(bounds.left()>=0. && bounds.top()>=0. && bounds.right()<=extent.x && bounds.bottom()<=extent.y);
            bounds
        }
        let small=render(Vec2::new(960.,720.),false);
        let desktop=render(Vec2::new(1920.,1080.),false);
        let expanded=render(Vec2::new(1920.,1080.),true);
        assert!(small.width()>=340. && desktop.width()<500.);
        assert!(desktop.height()>desktop.width()*1.5);
        assert!(small.height()>600.);
        assert!((expanded.width()-desktop.width()).abs()<2.);
        assert!((expanded.height()-desktop.height()).abs()<2.);
    }
    #[test]
    fn dragging_the_corner_resizes_and_retains_size_when_reopened() {
        let mut h=Harness::new();let before=h.bounds;
        let grip=before.max-Vec2::splat(18.);
        h.move_to(grip);h.input.button(272,true);h.frame();
        h.move_to(grip+Vec2::new(-56.,-180.));h.frame();
        h.input.button(272,false);h.frame();h.frame();
        let after=h.bounds;
        assert!(after.width()<before.width()-30.,"before={before:?}, after={after:?}");
        assert!(after.height()<before.height()-130.,"before={before:?}, after={after:?}");
        assert!(after.width()>=344. && after.height()>=264.);
        h.input.release();h.frame();h.input.visible=true;h.frame();h.frame();
        assert!((h.bounds.width()-after.width()).abs()<2.);
        assert!((h.bounds.height()-after.height()).abs()<2.,"after={after:?}, reopened={:?}",h.bounds);
        let output=h.frame();let multiplier=text_position(&output.shapes,"3×");
        assert!(h.click(multiplier));assert_eq!(h.controls.multiplier,3);
    }
    #[test]
    fn resize_is_clamped_after_the_game_resolution_changes() {
        let mut h=Harness::new();h.input.extent=Vec2::new(800.,480.);
        for _ in 0..5 {h.frame();}
        assert!(h.bounds.left()>=0. && h.bounds.top()>=0.);
        assert!(h.bounds.right()<=800. && h.bounds.bottom()<=480.,"{:?}",h.bounds);
    }
    #[test]
    fn capture_and_mako_fit_the_narrow_panel_and_wheel_reaches_all_controls() {
        let mut h=Harness::new();h.input.extent=Vec2::new(1920.,1080.);
        h.ctx.style_mut(|style|for font in style.text_styles.values_mut(){font.size*=1.2;});
        h.controls.adaptive=true;h.controls.scaling_enabled=true;
        h.capture=Some(crate::capture::Controller::fixture(serde_json::from_value(serde_json::json!({
            "prepared":"Jogo de teste com um nome bem comprido para conferir a quebra de linha",
            "selection":{"microphone":true,"input_source":"Microfone USB com um nome longo","output_source":"game:auto"},
            "encoder":{"vendor":"AMD","encoder":"VAAPI","gpu":"GPU de teste com um nome longo para verificar a largura","card_path":"/fixture/card0","pci_address":"0000:01:00.0","codecs":["h264","hevc","av1"],"containers":["mp4","mkv","webm"]}
        })).unwrap()));
        h.error="Fim do painel".into();
        for _ in 0..6 {h.frame();}
        assert!(h.bounds.width()<500. && h.bounds.bottom()<=1080.,"{:?}",h.bounds);
        h.move_to(h.bounds.center());
        let mut reached=false;
        for _ in 0..12 {
            assert_eq!(h.input.scroll(0.,180.),Route::Overlay);
            let output=h.frame();
            reached|=output.shapes.iter().any(|s|match &s.shape {
                Shape::Text(t)=>t.galley.text()=="Fim do painel" && s.clip_rect.intersects(Rect::from_min_size(t.pos,t.galley.size())),_=>false
            });
        }
        assert!(reached,"A roda deve permitir alcançar o fim sem uma barra visível.");
        assert!(h.bounds.width()<500. && h.bounds.bottom()<=1080.);
    }
    #[test]
    fn losing_focus_during_resize_does_not_leave_the_grip_captured() {
        let mut h=Harness::new();let grip=h.bounds.max-Vec2::splat(18.);
        h.move_to(grip);h.input.button(272,true);h.frame();
        h.input.focus(false);h.frame();h.input.focus(true);h.frame();
        let before=h.bounds;h.move_to(grip-Vec2::new(60.,80.));h.frame();
        assert_eq!(h.bounds,before);
    }
    #[test]
    fn live_scaling_method_is_editable_in_the_actual_panel() {
        let mut h=Harness::new();
        h.controls.scaling_enabled=true;
        h.frame();let output=h.frame();
        let native=text_position(&output.shapes,"Nativo");
        h.click(native);
        let output=h.frame();
        let method=text_position(&output.shapes,"MAKO Scaler");
        assert!(h.click(method));
        assert_eq!(h.controls.scaling_method,"mako");
        let output=h.frame();
        text_position(&output.shapes,"FPS base");
        text_position(&output.shapes,"Limiar (Hz)");
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
        h.move_to(header + Vec2::new(-180., 24.));
        h.frame();
        assert_eq!(h.input.button(272, false), Route::Overlay);
        h.frame();
        assert!(
            (h.bounds.left() - (before.left() - 180.)).abs() < 2.,
            "before={before:?}, after={:?}",
            h.bounds
        );
        assert!(
            (h.bounds.top() - (before.top() + 24.)).abs() < 2.,
            "before={before:?}, after={:?}",
            h.bounds
        );
        let output = h.frame();
        let header = text_position(&output.shapes, "PeliGames");
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
