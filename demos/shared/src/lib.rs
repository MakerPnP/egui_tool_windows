use std::hash::Hash;
use egui::{RichText, Ui};
use egui_extras::{Column, TableBuilder};

#[derive(Default)]
pub struct ExampleWindowState {
    number: f32,
    toggle: bool,
}

#[allow(dead_code)]
pub fn draw_example_window_contents_1(ui: &mut Ui, state: &mut ExampleWindowState) {
    {
        let style = ui.style_mut();
        style.wrap_mode = Some(egui::TextWrapMode::Extend);
        style.interaction.selectable_labels = false;

        ui.heading("A very very very long title");
    }

    {
        let style = ui.style_mut();
        style.wrap_mode = Some(egui::TextWrapMode::Wrap);

        ui.label("This content is wrapped and clipped.");
    }
    
    if ui.toggle_value(&mut state.toggle, "Toggle me").changed() {
        println!("Toggled!");   
    };
    
    if ui.button("Clickable button").clicked() {
        // Do something
        println!("Clicked!");
    }
    
    ui.horizontal(|ui| {
        ui.label("drag me");
        if ui.add(egui::DragValue::new(&mut state.number)).changed() {
            println!("changed: {}", state.number);
        };
    });
}


/// Fictional row data
struct TableRow {
    index: usize,
    name: &'static str,
    path: &'static str,
}

/// Static rows for this example
fn fake_data() -> Vec<TableRow> {
    vec![
        TableRow {
            index: 1,
            name: "Alpha",
            path: "/alpha/file.txt",
        },
        TableRow {
            index: 2,
            name: "Beta",
            path: "/beta/image.png",
        },
        TableRow {
            index: 3,
            name: "Gamma",
            path: "/gamma/document.docx",
        },
        TableRow {
            index: 4,
            name: "Delta",
            path: "/delta/music.mp3",
        },
        TableRow {
            index: 5,
            name: "Epsilon",
            path: "/epsilon/video.mp4",
        },
        TableRow {
            index: 6,
            name: "Zeta",
            path: "/zeta/presentation.pptx",
        },
    ]
}

pub fn draw_table(ui: &mut egui::Ui, id_salt: impl Hash) {
    let rows = fake_data();

    let row_height = 24.0;

    let id = ui.id().with(id_salt);
    TableBuilder::new(ui)
        .id_salt(id)
        .striped(true)
        .resizable(true)
        .auto_shrink(false)
        .min_scrolled_height(50.0)
        //.cell_layout(egui::Layout::left_to_right(egui::Align::Center))
        .column(Column::auto()) // #
        .column(Column::auto()) // Name
        .column(Column::remainder()) // Path (fills remaining)
        //.min_scrolled_height(200.0) // minimum to create blank space if taller
        .header(row_height, |mut header| {
            header.col(|ui| {
                ui.label(RichText::new("#").strong());
            });
            header.col(|ui| {
                ui.label(RichText::new("Name").strong());
            });
            header.col(|ui| {
                ui.label(RichText::new("Path").strong());
            });
        })
        .body(|mut body| {
            for row in &rows {
                body.row(row_height, |mut row_ui| {
                    row_ui.col(|ui| {
                        ui.label(row.index.to_string());
                    });
                    row_ui.col(|ui| {
                        ui.label(row.name);
                    });
                    row_ui.col(|ui| {
                        ui.label(row.path);
                    });
                });
            }
        });
}

pub const LOREM_IPSUM: &str = r#"Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor incididunt ut \
labore et dolore magna aliqua. Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris nisi ut aliquip ex ea \
commodo consequat. Duis aute irure dolor in reprehenderit in voluptate velit esse cillum dolore eu fugiat nulla \
pariatur. Excepteur sint occaecat cupidatat non proident, sunt in culpa qui officia deserunt mollit anim id est \
laborum."#;