use std::sync::{Arc, Mutex};
use eframe::emath::{Rect, Vec2};
use egui::{CentralPanel, Context, Id, ViewportBuilder, Window};
use egui::scroll_area::ScrollBarVisibility;
use egui_tool_windows::ToolWindows;
use shared::ExampleWindowState;

fn main() -> eframe::Result<()> {
    // run with `RUST_LOG=egui_tool_windows=trace` to see trace logs
    env_logger::init();
    
    let native_options = eframe::NativeOptions {
        viewport: ViewportBuilder::default().with_inner_size([1027.0, 768.0]),
        ..Default::default()
    };
    eframe::run_native(
        "Document System with Contained Tool Windows",
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

enum ExampleWindowKind {
    Table1,
    Example1,
    ToolWindows1,
}

impl MyApp {
    fn create_document_window(&mut self, ctx: &Context, constrain_rect: Rect, title: &str, kind: ExampleWindowKind, default_pos: [f32; 2], default_size: [f32; 2]) {
        let min_size = Vec2::from([200.0, 100.0]);
        
        // Create the document window
        Window::new(title)
            .id(Id::new(title))
            .default_pos(default_pos)
            .default_size(default_size)
            .min_size(min_size)
            .constrain_to(constrain_rect)
            .resizable(true)
            .show(ctx, |ui| {
                // if this isn't done, the window will be too small
                ui.set_min_size(min_size);
          
                match kind {
                    ExampleWindowKind::Table1 => {
                        shared::draw_table(ui, "table_1");
                    }
                    ExampleWindowKind::Example1 => {
                        let mut example_state = self.example_state.lock().unwrap();
                        shared::draw_example_window_contents_1(ui, &mut example_state);
                    }
                    ExampleWindowKind::ToolWindows1 => {
                        
                        egui::ScrollArea::both()
                            .auto_shrink([false, false])
                            .scroll_bar_visibility(ScrollBarVisibility::AlwaysVisible)
                            .show(ui, |ui|{
                            ToolWindows::new()
                                .windows(ui, |builder|{
                                    builder
                                        .add_window("table_tool_window_1")
                                        .default_pos([50.0, 50.0])
                                        .default_size([400.0, 300.0])
                                        .show("Example table (drag or collapse me)".to_string(), |ui| {
                                            shared::draw_table(ui, "table_1");
                                        });
    
                                    builder
                                        .add_window("control_tool_window_1")
                                        .default_pos([100.0, 100.0])
                                        .default_size([400.0, 300.0])
                                        .show("Example controls (drag or collapse me) - very very long title".to_string(), {
                                            let example_state_arc = self.example_state.clone();
    
                                            move |ui| {
                                                let mut example_state = example_state_arc.lock().unwrap();
                                                shared::draw_example_window_contents_1(ui, &mut example_state);
                                            }
                                        });
    
                                });
                            
                        });
                    }
                }
                
            });
    }
}

impl eframe::App for MyApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label("Document System");
                ui.checkbox(&mut self.inspection, "🔍 Inspection");
            });
        });

        let response = CentralPanel::default().show(ctx, |ui| {
            ui.label("Example document system!");
        });
        
        let central_panel_rect = response.response.rect;

        // Create document windows with proper z-ordering
        self.create_document_window(ctx, central_panel_rect, "Document 1", ExampleWindowKind::Example1, [250.0, 100.0], [300.0, 200.0]);
        self.create_document_window(ctx, central_panel_rect, "Document 2", ExampleWindowKind::Table1, [350.0, 150.0], [300.0, 200.0]);
        self.create_document_window(ctx, central_panel_rect, "Document 3", ExampleWindowKind::ToolWindows1, [150.0, 200.0], [800.0, 400.0]);

        // Inspection window
        egui::Window::new("🔍 Inspection")
            .open(&mut self.inspection)
            .vscroll(true)
            .show(ctx, |ui| {
                ctx.inspection_ui(ui);
            });
    }
}

