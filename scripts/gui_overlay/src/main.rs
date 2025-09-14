use eframe::egui::{self, Color32, Pos2, Vec2};

#[derive(Default)]
struct MyApp {
    attention_x: f32,
    attention_y: f32,
    attention_radius: f32,
}

impl eframe::App for MyApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            // replace with video texture later
            let available_size = ui.available_size();
            let (rect, _response) =
                ui.allocate_exact_size(available_size, egui::Sense::hover());

            let painter = ui.painter();

            // Example data (replace with real values per frame)
            self.attention_x = 200.0;
            self.attention_y = 150.0;
            self.attention_radius = 50.0;

            // Circle center in screen coordinates
            let circle_center = Pos2::new(
                rect.left_top().x + self.attention_x,
                rect.left_top().y + self.attention_y,
            );

            // Draw the circle (saliency region)
            painter.circle_stroke(
                circle_center,
                self.attention_radius,
                (2.0, Color32::RED),
            );

            // Draw the label above the circle
            let label_text = format!(
                "Saliency: r = {:.1}",
                self.attention_radius
            );
            let label_pos = circle_center + Vec2::new(0.0, -self.attention_radius - 20.0);

            painter.text(
                label_pos,
                egui::Align2::CENTER_CENTER,
                label_text,
                egui::FontId::proportional(16.0),
                Color32::WHITE,
            );

            // (Optional) Draw a dynamic bar for value visualization
            let bar_height = self.attention_radius * 2.0;
            let bar_rect = egui::Rect::from_min_size(
                Pos2::new(rect.left() + 20.0, rect.bottom() - bar_height),
                Vec2::new(30.0, bar_height),
            );

            painter.rect_filled(bar_rect, 2.0, Color32::LIGHT_BLUE);
            painter.text(
                bar_rect.center_top() + Vec2::new(0.0, -16.0),
                egui::Align2::CENTER_BOTTOM,
                format!("{:.1}", self.attention_radius),
                egui::FontId::proportional(14.0),
                Color32::WHITE,
            );
        });
    }
}

fn main() -> Result<(), eframe::Error> {
    let options = eframe::NativeOptions::default();
    eframe::run_native(
        "Saliency Overlay Example",
        options,
        Box::new(|_cc| Box::new(MyApp::default())),
    )
}
