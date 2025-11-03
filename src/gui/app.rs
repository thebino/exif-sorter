use eframe::egui;

#[derive(Default)]
pub(crate) struct MyApp {
    counter: i32,
}

impl eframe::App for MyApp {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            egui::MenuBar::new().ui(ui, |ui| {
                ui.menu_button("File", |ui| {
                    if ui.button("Quit").clicked() {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                });
                ui.add_space(16.0);

                // egui::widgets::global_theme_preference_buttons(ui);
            });
        });
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("exif-sorter");
            if ui.button("Click me").clicked() {
                self.counter += 1;
            }
            ui.label(format!("Clicked {} times", self.counter));
        });
    }

    fn save(&mut self, _storage: &mut dyn eframe::Storage) {}
}
