use std::sync::{Arc, Mutex};
use egui::{CentralPanel, Id, ViewportBuilder};
use egui::scroll_area::ScrollBarVisibility;
use egui_dock::{DockArea, DockState, NodeIndex, Style};
use egui_dock::egui::{Ui, WidgetText};
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
        "Document System using egui_dock with egui_tool_windows",
        native_options,
        Box::new(|_cc| Ok(Box::new(MyApp::default()))),
    )
}

struct Tab {
    name: &'static str,
    kind: TabKind,
}

enum TabKind {
    Table1,
    Example1 { state: Arc<Mutex<ExampleWindowState>> },
    ToolWindows { state: Arc<Mutex<ExampleWindowState>>, salt: &'static str },
}

struct MyApp {
    inspection: bool,
    tree: DockState<Tab>,
}

impl Default for MyApp {
    fn default() -> Self {
        let mut tree = DockState::new(vec![
            Tab { name: "Tool windows in a tab", kind: TabKind::ToolWindows { state: Arc::new(Mutex::new(ExampleWindowState::default())), salt: "tab 1"} },
        ]);

        // You can modify the tree before constructing the dock
        let [a, _b] =
            tree.main_surface_mut()
                .split_left(NodeIndex::root(), 0.3, vec![
                    Tab { name: "Example Table", kind: TabKind::Table1 },
                ]);
        let [_, _] = tree
            .main_surface_mut()
            .split_below(a, 0.7, vec![
                Tab { name: "Example Controls", kind: TabKind::Example1 { state: Arc::new(Mutex::new(ExampleWindowState::default()))} },
            ]);
        let _ = tree
            .add_window( vec![
                Tab { name: "Tool windows in initially floating dock window", kind: TabKind::ToolWindows { state: Arc::new(Mutex::new(ExampleWindowState::default())), salt: "tab 4" } },
                Tab { name: "Example Table", kind: TabKind::Table1 },
            ]);

        Self {
            inspection: false,
            tree
        }
    }
}

impl TabKind {
    pub fn ui(&mut self, ui: &mut egui::Ui) {
        match self {
            TabKind::Table1 => {
                shared::draw_table(ui, "table_1");
            }
            TabKind::Example1 { state } => {
                let mut example_state = state.lock().unwrap();
                shared::draw_example_window_contents_1(ui, &mut example_state);
            }
            TabKind::ToolWindows { state , salt} => {
                egui::ScrollArea::both()
                    .auto_shrink([false, false])
                    .scroll_bar_visibility(ScrollBarVisibility::AlwaysVisible)
                    .show(ui, |ui|{
                        
                        // using a block to make sure the state guard is dropped
                        {
                            let mut example_state = state.lock().unwrap();
                            shared::draw_example_window_contents_1(ui, &mut example_state);
                        }
                        
                        shared::draw_table(ui, "table_2");
                        ToolWindows::new()
                            .windows(ui, |builder|{
                                builder
                                    .add_window(Id::new("table_tool_window_1").with(*salt))
                                    .default_pos([50.0, 50.0])
                                    .default_size([400.0, 300.0])
                                    .show("Example table 1 (drag or collapse me)".to_string(), |ui| {
                                        shared::draw_table(ui, "table_3");
                                    });

                                builder
                                    .add_window(Id::new("controls_tool_window_1").with(*salt))
                                    .default_pos([100.0, 100.0])
                                    .default_size([400.0, 300.0])
                                    .show("Example table 2 (drag or collapse me) - very very long title".to_string(), {
                                        let example_state_arc = state.clone();

                                        move |ui| {
                                            let mut example_state = example_state_arc.lock().unwrap();
                                            shared::draw_example_window_contents_1(ui, &mut example_state);
                                        }
                                    });

                            });
                    });

            }
        }
    }
}

struct TabViewer {}

impl egui_dock::TabViewer for TabViewer {
    type Tab = Tab;

    fn title(&mut self, tab: &mut Self::Tab) -> WidgetText {
        (&*tab.name).into()
    }

    fn ui(&mut self, ui: &mut Ui, tab: &mut Self::Tab) {
        ui.push_id(ui.id().with(tab.name), |ui|{
            tab.kind.ui(ui);
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

            DockArea::new(&mut self.tree)
                .style(Style::from_egui(ctx.style().as_ref()))
                .show_inside(ui, &mut TabViewer {});

        });
        
        let central_panel_rect = response.response.rect;

        // Inspection window
        egui::Window::new("🔍 Inspection")
            .open(&mut self.inspection)
            .constrain_to(central_panel_rect)
            .vscroll(true)
            .show(ctx, |ui| {
                ctx.inspection_ui(ui);
            });
    }
}

