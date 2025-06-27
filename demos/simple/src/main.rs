use std::sync::{Arc, Mutex};
use egui::{CentralPanel, Style, ViewportBuilder};
use egui::scroll_area::ScrollBarVisibility;
use egui_tool_windows::ToolWindows;
use shared::ExampleWindowState;

fn main() -> eframe::Result<()> {
    let native_options = eframe::NativeOptions {
        viewport: ViewportBuilder::default().with_inner_size([1027.0, 768.0]),
        ..Default::default()
    };
    eframe::run_native(
        "Tool windows",
        native_options,
        Box::new(|_cc| Ok(Box::new(MyApp::default()))),
    )
}

struct MyApp {
    inspection: bool,
    example_state: Arc<Mutex<ExampleWindowState>>,
}

impl Default for MyApp {
    fn default() -> Self {
        Self {
            inspection: false,
            example_state: Arc::new(Mutex::new(ExampleWindowState::default())),
        }
    }
}

impl eframe::App for MyApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label("Tool windows demo");
                ui.checkbox(&mut self.inspection, "🔍 Inspection");
            });
        });

        CentralPanel::default().show(ctx, |ui| {
            ui.vertical_centered(|ui| {
                egui::Frame::group(&Style::default())
                    .outer_margin(40.0)
                    .show(ui, |ui|{
                    egui::ScrollArea::both()
                        .auto_shrink([false, false])
                        .scroll_bar_visibility(ScrollBarVisibility::AlwaysVisible)
                        .show(ui, |ui|{
                            ui.strong("Content inside a frame");
                            
                            ui.weak(shared::LOREM_IPSUM);
                            
                            ToolWindows::new()
                                .windows(ui, |builder|{
                                    builder
                                        .add_window("table_tool_window_1")
                                        //.default_pos([50.0, 50.0])
                                        //.default_size([400.0, 300.0])
                                        .show("Example table 1 (drag or collapse me)".to_string(), |ui| {
                                            shared::draw_table(ui, "table_1");
                                        });

                                    builder
                                        .add_window("table_tool_window_2")
                                        //.default_pos([100.0, 100.0])
                                        //.default_size([400.0, 300.0])
                                        .show("Example table 2 (drag or collapse me) - very very long title".to_string(), {
                                            let example_state_arc = self.example_state.clone();

                                            move |ui| {
                                                let mut example_state = example_state_arc.lock().unwrap();
                                                shared::draw_example_window_contents_1(ui, &mut example_state);
                                            }
                                        });

                                });

                        });
                });
            });
        });
        
        // Inspection window
        egui::Window::new("🔍 Inspection")
            .open(&mut self.inspection)
            .vscroll(true)
            .show(ctx, |ui| {
                ctx.inspection_ui(ui);
            });
    }
}

