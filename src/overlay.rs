use eframe::egui;
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, Ordering};

pub struct OverlayState {
    pub spectrum: Arc<Mutex<Vec<f32>>>,
    pub visible: Arc<AtomicBool>,
    pub loading: Arc<AtomicBool>,
    pub cursor_x: Arc<Mutex<f32>>,
    pub cursor_y: Arc<Mutex<f32>>,
}

impl OverlayState {
    pub fn new() -> Self {
        Self {
            spectrum: Arc::new(Mutex::new(vec![0.0; 20])),
            visible: Arc::new(AtomicBool::new(false)),
            loading: Arc::new(AtomicBool::new(false)),
            cursor_x: Arc::new(Mutex::new(0.0)),
            cursor_y: Arc::new(Mutex::new(0.0)),
        }
    }
}

struct OverlayApp {
    state: OverlayState,
}

impl OverlayApp {
    fn new(state: OverlayState) -> Self {
        Self { state }
    }
}

impl eframe::App for OverlayApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let visible = self.state.visible.load(Ordering::SeqCst);
        let loading = self.state.loading.load(Ordering::SeqCst);

        if !visible && !loading {
            ctx.request_repaint_after(std::time::Duration::from_millis(50));
            return;
        }

        // Get cursor position
        let cursor_x = *self.state.cursor_x.lock().unwrap();
        let cursor_y = *self.state.cursor_y.lock().unwrap();

        // Position window near cursor
        let window_width = 200.0;
        let window_height = 60.0;
        let offset_x = 20.0;
        let offset_y = 20.0;

        // Set window position
        ctx.send_viewport_cmd(egui::ViewportCommand::OuterPosition(
            egui::pos2(cursor_x + offset_x, cursor_y + offset_y)
        ));

        egui::CentralPanel::default()
            .frame(egui::Frame::NONE.fill(egui::Color32::TRANSPARENT))
            .show(ctx, |ui| {
                let rect = ui.available_rect_before_wrap();

                // Background with rounded corners
                let bg_rect = egui::Rect::from_min_size(
                    rect.min,
                    egui::vec2(window_width, window_height),
                );
                ui.painter().rect_filled(
                    bg_rect,
                    8.0,
                    egui::Color32::from_rgba_unmultiplied(30, 30, 30, 220),
                );

                if loading {
                    // Show loading spinner
                    let center = bg_rect.center();
                    let time = ctx.input(|i| i.time);
                    let angle = time * 3.0;
                    let radius = 15.0;

                    for i in 0..8 {
                        let a = angle + (i as f64 * std::f64::consts::PI / 4.0);
                        let alpha = ((i as f32 + 1.0) / 8.0 * 255.0) as u8;
                        let pos = egui::pos2(
                            center.x + (a.cos() as f32) * radius,
                            center.y + (a.sin() as f32) * radius,
                        );
                        ui.painter().circle_filled(
                            pos,
                            3.0,
                            egui::Color32::from_rgba_unmultiplied(100, 200, 255, alpha),
                        );
                    }
                } else {
                    // Draw spectrum bars
                    let spectrum = self.state.spectrum.lock().unwrap().clone();
                    let bar_count = spectrum.len();
                    let bar_width = (window_width - 20.0) / bar_count as f32;
                    let max_height = window_height - 20.0;

                    for (i, &value) in spectrum.iter().enumerate() {
                        let bar_height = value * max_height;
                        let x = rect.min.x + 10.0 + i as f32 * bar_width;
                        let y = rect.min.y + window_height - 10.0 - bar_height;

                        let bar_rect = egui::Rect::from_min_size(
                            egui::pos2(x, y),
                            egui::vec2(bar_width - 2.0, bar_height),
                        );

                        // Color gradient from blue to cyan
                        let hue = 0.5 + (i as f32 / bar_count as f32) * 0.15;
                        let color = egui::Color32::from_rgb(
                            (100.0 + hue * 50.0) as u8,
                            (180.0 + value * 75.0) as u8,
                            255,
                        );

                        ui.painter().rect_filled(bar_rect, 2.0, color);
                    }
                }
            });

        // Request continuous repaint for animation
        ctx.request_repaint_after(std::time::Duration::from_millis(30));
    }

    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        [0.0, 0.0, 0.0, 0.0] // Fully transparent
    }
}

pub fn run_overlay(state: OverlayState) -> Result<(), eframe::Error> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([220.0, 80.0])
            .with_decorations(false)
            .with_transparent(true)
            .with_always_on_top()
            .with_mouse_passthrough(true)
            .with_taskbar(false),
        ..Default::default()
    };

    eframe::run_native(
        "Flov Overlay",
        options,
        Box::new(|_cc| Ok(Box::new(OverlayApp::new(state)))),
    )
}
