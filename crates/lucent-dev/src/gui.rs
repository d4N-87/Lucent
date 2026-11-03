use lucent_core::keyboard::model::{default_model, KeyDef, Model};
use lucent_core::keyboard::{ColorBuffer, GmmkDevice};
use eframe::egui::{self, Align2, Color32, FontId, Pos2, Rect, Sense, Stroke, StrokeKind, Vec2};
use hidapi::HidApi;
use std::collections::BTreeSet;
use std::time::{Duration, Instant};

/// Delay after the last edit before auto-send fires, so dragging the color picker
/// doesn't flood the keyboard with uploads.
const AUTO_SEND_DELAY: Duration = Duration::from_millis(300);

pub fn run() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Lucent - lucent-dev")
            .with_inner_size([1400.0, 560.0])
            .with_min_inner_size([900.0, 420.0]),
        ..Default::default()
    };
    eframe::run_native(
        "Lucent - lucent-dev",
        options,
        Box::new(|_cc| Ok(Box::new(App::new()))),
    )
}

struct App {
    model: Model,
    api: Option<HidApi>,
    device: Option<GmmkDevice>,
    colors: ColorBuffer,
    selected: BTreeSet<usize>,
    brush: Color32,
    brightness: u8,
    auto_send: bool,
    /// Set when `colors` changed and auto-send has not uploaded them yet.
    pending_since: Option<Instant>,
    status: String,
}

impl App {
    fn new() -> Self {
        let mut colors = ColorBuffer::new();
        colors.set_all([255, 255, 255]);
        let mut app = Self {
            model: default_model(),
            api: None,
            device: None,
            colors,
            selected: BTreeSet::new(),
            brush: Color32::from_rgb(255, 0, 0),
            brightness: 4,
            auto_send: true,
            pending_since: None,
            status: String::new(),
        };
        app.connect();
        app
    }

    fn connect(&mut self) {
        self.device = None;
        let api = match self.api.as_mut() {
            Some(api) => {
                let _ = api.refresh_devices();
                api
            }
            None => match HidApi::new() {
                Ok(api) => self.api.insert(api),
                Err(e) => {
                    self.status = format!("Errore inizializzazione HID: {e}");
                    return;
                }
            },
        };
        match GmmkDevice::open(api) {
            Ok(dev) => {
                self.status = "Tastiera GMMK connessa.".to_string();
                self.device = Some(dev);
            }
            Err(e) => self.status = e,
        }
    }

    fn send(&mut self) {
        self.pending_since = None;
        let Some(dev) = &self.device else {
            self.status = "Tastiera non connessa: premi \"Riconnetti\".".to_string();
            return;
        };
        let start = Instant::now();
        match dev.set_custom_colors(&self.colors, self.brightness) {
            Ok(()) => {
                self.status = format!("Colori inviati ({} ms).", start.elapsed().as_millis())
            }
            Err(e) => {
                self.status = format!("Invio fallito: {e}");
                self.device = None;
            }
        }
    }

    /// Records a color change; sends right away or after the auto-send delay.
    fn mark_changed(&mut self) {
        if self.auto_send {
            self.pending_since = Some(Instant::now());
        }
    }

    fn paint_selected(&mut self, color: Color32) {
        for &slot in &self.selected {
            self.colors.set_key(slot, [color.r(), color.g(), color.b()]);
        }
        if !self.selected.is_empty() {
            self.mark_changed();
        }
    }

    fn key_color(&self, key: &KeyDef) -> Color32 {
        let [r, g, b] = self.colors.leds[key.slot];
        Color32::from_rgb(r, g, b)
    }

    fn keyboard_view(&mut self, ui: &mut egui::Ui) {
        let avail = ui.available_size();
        let scale = (avail.x / self.model.width).min(avail.y / self.model.height).max(20.0);
        let size = Vec2::new(self.model.width, self.model.height) * scale;
        let (response, painter) = ui.allocate_painter(avail, Sense::click_and_drag());
        let origin = response.rect.center() - size / 2.0;

        let key_rect = |key: &KeyDef| {
            Rect::from_min_size(
                origin + Vec2::new(key.x, key.y) * scale,
                Vec2::new(key.w, key.h) * scale,
            )
            .shrink(scale * 0.05)
        };
        let key_at = |pos: Pos2| self.model.keys.iter().find(|k| key_rect(k).contains(pos));

        // Click selects one key (Ctrl/Shift toggles); dragging adds every key it passes over.
        let modifiers = ui.input(|i| i.modifiers);
        let additive = modifiers.ctrl || modifiers.shift;
        if response.drag_started() && !additive {
            self.selected.clear();
        }
        if let Some(pos) = response.interact_pointer_pos() {
            if let Some(key) = key_at(pos) {
                if response.clicked() {
                    if additive {
                        if !self.selected.remove(&key.slot) {
                            self.selected.insert(key.slot);
                        }
                    } else {
                        self.selected.clear();
                        self.selected.insert(key.slot);
                    }
                } else if response.dragged() {
                    self.selected.insert(key.slot);
                }
            } else if response.clicked() && !additive {
                self.selected.clear();
            }
        }

        let accent = ui.visuals().selection.stroke.color;
        for key in &self.model.keys {
            let rect = key_rect(key);
            let fill = self.key_color(key);
            let selected = self.selected.contains(&key.slot);
            painter.rect_filled(rect, scale * 0.08, fill);
            let stroke = if selected {
                Stroke::new(scale * 0.07, accent)
            } else {
                Stroke::new(1.0_f32, Color32::from_gray(90))
            };
            painter.rect_stroke(rect, scale * 0.08, stroke, StrokeKind::Inside);

            let luminance = 0.299 * fill.r() as f32 + 0.587 * fill.g() as f32 + 0.114 * fill.b() as f32;
            let text_color = if luminance > 140.0 { Color32::BLACK } else { Color32::WHITE };
            // The default egui font has no arrow glyphs, so arrow keys are drawn as shapes.
            let arrow = match key.label.as_str() {
                "â†" => Some(Vec2::new(-1.0, 0.0)),
                "â†’" => Some(Vec2::new(1.0, 0.0)),
                "â†‘" => Some(Vec2::new(0.0, -1.0)),
                "â†“" => Some(Vec2::new(0.0, 1.0)),
                _ => None,
            };
            if let Some(dir) = arrow {
                let len = scale * 0.4;
                painter.arrow(
                    rect.center() - dir * len / 2.0,
                    dir * len,
                    Stroke::new(scale * 0.05, text_color),
                );
                continue;
            }

            let font_size = if key.label.chars().count() > 5 { scale * 0.18 } else { scale * 0.26 };
            painter.text(
                rect.center(),
                Align2::CENTER_CENTER,
                &key.label,
                FontId::proportional(font_size),
                text_color,
            );
        }
    }

    fn side_panel(&mut self, ui: &mut egui::Ui) {
        ui.heading("Colore");
        let before = self.brush;
        egui::color_picker::color_picker_color32(ui, &mut self.brush, egui::color_picker::Alpha::Opaque);
        if self.brush != before {
            self.paint_selected(self.brush);
        }

        ui.add_space(8.0);
        ui.label(format!("Tasti selezionati: {}", self.selected.len()));
        ui.horizontal(|ui| {
            if ui.button("Seleziona tutto").clicked() {
                self.selected = self.model.slots().into_iter().collect();
            }
            if ui.button("Deseleziona").clicked() {
                self.selected.clear();
            }
        });
        ui.horizontal(|ui| {
            if ui.button("Colora selezionati").clicked() {
                self.paint_selected(self.brush);
            }
            if ui.button("Spegni selezionati").clicked() {
                self.paint_selected(Color32::BLACK);
            }
        });
        if ui.button("Colora tutta la tastiera").clicked() {
            self.colors.set_all([self.brush.r(), self.brush.g(), self.brush.b()]);
            self.mark_changed();
        }

        ui.separator();
        let mut brightness = self.brightness;
        ui.add(egui::Slider::new(&mut brightness, 0..=4).text("Luminosità"));
        if brightness != self.brightness {
            self.brightness = brightness;
            self.mark_changed();
        }
        ui.checkbox(&mut self.auto_send, "Invio automatico");
        if ui
            .add_enabled(self.device.is_some(), egui::Button::new("Invia alla tastiera"))
            .clicked()
        {
            self.send();
        }

        ui.separator();
        ui.small("Clic: seleziona un tasto\nCtrl/Shift + clic: aggiungi o togli\nTrascina: seleziona più tasti");
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if let Some(since) = self.pending_since {
            let elapsed = since.elapsed();
            if elapsed >= AUTO_SEND_DELAY {
                self.send();
            } else {
                ctx.request_repaint_after(AUTO_SEND_DELAY - elapsed);
            }
        }

        egui::TopBottomPanel::top("top").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.heading("Lucent - lucent-dev");
                ui.separator();
                match &self.device {
                    Some(_) => ui.colored_label(Color32::from_rgb(80, 200, 120), "â— Connessa"),
                    None => ui.colored_label(Color32::from_rgb(220, 80, 80), "â— Non connessa"),
                };
                if ui.button("Riconnetti").clicked() {
                    self.connect();
                }
            });
        });
        egui::TopBottomPanel::bottom("status").show(ctx, |ui| {
            ui.label(&self.status);
        });
        egui::SidePanel::right("tools")
            .resizable(false)
            .exact_width(260.0)
            .show(ctx, |ui| self.side_panel(ui));
        egui::CentralPanel::default().show(ctx, |ui| self.keyboard_view(ui));
    }
}
