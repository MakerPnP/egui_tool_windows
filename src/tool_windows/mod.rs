use std::hash::Hash;

use egui::collapsing_header::CollapsingState;
use egui::{
    Align, Color32, Context, CornerRadius, CursorIcon, Frame, Id, Layout, Pos2, Rect, Sense, Stroke, StrokeKind, Ui,
    UiBuilder, Vec2, Vec2b, vec2,
};
use log::trace;

struct ToolWindow {
    id: Id,
    state: ToolWindowState,
}

impl ToolWindow {
    fn show(
        &mut self,
        ui: &mut Ui,
        title: String,
        content_fn: impl FnOnce(&mut Ui) + Sized,
        state: &mut ToolWindowsState,
    ) {
        let ctx = ui.ctx().clone();
        let id = ui.make_persistent_id(
            self.id
                .with("__tool_window_persistent_id"),
        );
        let mut collapsing_state = CollapsingState::load_with_default_open(&ctx, id, true);

        let visuals = ui.visuals().clone();

        let title_bar_height = 24.0;
        let inner_margin = 2;
        let outer_margin = 0;
        let edge_thickness = 4.0;
        let position_margin = 16.0;

        let header_color = visuals.widgets.open.weak_bg_fill;
        let background_color = visuals.window_fill;
        let border_color = visuals.window_stroke.color;
        let resize_corner_size = ui.visuals().resize_corner_size;
        let min_size = vec2(100.0, title_bar_height);

        let ui_clip_rect = ui.clip_rect();
        debug_rect(ui, ui_clip_rect, Color32::BLUE);

        let available = Vec2::new(
            (ui_clip_rect.width() - position_margin).max(position_margin),
            (ui_clip_rect.height() - position_margin).max(position_margin),
        );

        Self::clamp_offset(available, &mut self.state.position);

        let mut resize_delta = vec2(0.0, 0.0);

        let top_left = ui_clip_rect.min + self.state.position.to_vec2();
        let mut actual_size = if self.state.collapsed {
            vec2(self.state.size.x, title_bar_height)
        } else {
            self.state.size
        };

        let border_adjust_splat = (inner_margin + outer_margin) * 2;
        let border_adjust = Vec2::splat(border_adjust_splat as f32);
        actual_size += border_adjust;

        let rect = Rect::from_min_size(top_left, actual_size);
        debug_rect(ui, rect, Color32::BLUE);

        let corner_radius = CornerRadius::same(6);

        let input_response = ui.interact(rect, self.id.with("tool_window_input"), Sense::all());

        if input_response.clicked() {
            trace!(
                "clicked, id: {:?}, rendering_stack: {:?}",
                self.id, state.rendering_stack
            );
            let id = self.id;
            state.bring_to_front(id);
        }

        let edges = [
            (
                "left",
                Rect::from_min_max(rect.left_top(), rect.left_bottom()).expand2(vec2(edge_thickness, 0.0)),
            ),
            ("right", {
                let mut max = rect.right_bottom();
                max.y -= resize_corner_size;
                Rect::from_min_max(rect.right_top(), max).expand2(vec2(edge_thickness, 0.0))
            }),
            (
                "top",
                Rect::from_min_max(rect.left_top(), rect.right_top()).expand2(vec2(0.0, edge_thickness)),
            ),
            ("bottom", {
                let mut max = rect.right_bottom();
                max.x -= resize_corner_size;
                Rect::from_min_max(rect.left_bottom(), max).expand2(vec2(0.0, edge_thickness))
            }),
        ];

        for (edge, edge_rect) in edges {
            debug_rect(ui, edge_rect, Color32::ORANGE);

            let resp = ui.interact(edge_rect, id.with(edge), Sense::drag());

            if resp.hovered() {
                match edge {
                    "left" | "right" => ctx.set_cursor_icon(CursorIcon::ResizeHorizontal),
                    "top" | "bottom" => ctx.set_cursor_icon(CursorIcon::ResizeVertical),
                    _ => {}
                }
            }

            if resp.dragged() {
                match edge {
                    "left" => {
                        resize_delta.x -= resp.drag_delta().x;
                        self.state.position.x += resp.drag_delta().x;
                    }
                    "right" => resize_delta.x += resp.drag_delta().x,
                    "top" => {
                        resize_delta.y -= resp.drag_delta().y;
                        self.state.position.y += resp.drag_delta().y;
                    }
                    "bottom" => resize_delta.y += resp.drag_delta().y,
                    _ => {}
                }
            }
        }

        let corner_id = self
            .state
            .resizable
            .any()
            .then(|| id.with("__resize_corner"));

        let corner_response = if let Some(corner_id) = corner_id {
            let corner_size = Vec2::splat(resize_corner_size);
            let corner_rect = egui::Rect::from_min_size(rect.right_bottom() - corner_size - border_adjust, corner_size);
            debug_rect(ui, corner_rect, Color32::ORANGE);

            Some(ui.interact(corner_rect, corner_id, Sense::drag()))
        } else {
            None
        };

        if let Some(corner_response) = &corner_response {
            if corner_response.hovered() || corner_response.dragged() {
                ui.ctx()
                    .set_cursor_icon(CursorIcon::ResizeNwSe);
            }

            if corner_response.dragged() {
                resize_delta += corner_response.drag_delta();
            }
        }

        if resize_delta != Vec2::ZERO {
            self.state.size += resize_delta;
            self.state.size.x = self.state.size.x.max(min_size.x);
            self.state.size.y = self.state.size.y.max(min_size.y);
        }

        trace!(
            "position: {:?}, size: {:?}, resize_delta: {:?}",
            self.state.position, self.state.size, resize_delta
        );

        //
        // now draw the window
        //

        let painter = ui.painter().clone();
        let border_stroke = Stroke::new(1.0, border_color);
        painter.rect_filled(rect, corner_radius, background_color);
        painter.rect_stroke(rect, corner_radius, border_stroke, StrokeKind::Inside);

        let window_id = id.with("child_id");
        let mut window_ui = Ui::new(
            ctx.clone(),
            window_id,
            UiBuilder::new()
                .max_rect(rect)
                .layout(Layout::top_down(Align::Min)),
        );
        window_ui.set_clip_rect(rect.intersect(ui_clip_rect));
        window_ui.set_min_size(rect.size());

        let window_clip_rect = window_ui.clip_rect();
        debug_rect(ui, window_clip_rect, Color32::YELLOW);

        {
            let ui = &mut window_ui;

            let title_bar_rect = Rect::from_min_size(rect.min, vec2(rect.width(), title_bar_height + border_adjust.y));
            debug_rect(ui, title_bar_rect, Color32::GREEN);

            let title_bar_rect_id = id.with("title_bar_rect_id");
            let mut title_bar_rect_ui = Ui::new(
                ctx.clone(),
                title_bar_rect_id,
                UiBuilder::new()
                    .max_rect(title_bar_rect)
                    .sense(Sense::click_and_drag())
                    .layout(Layout::top_down(Align::Min)),
            );
            title_bar_rect_ui.set_clip_rect(title_bar_rect.intersect(ui_clip_rect));

            let title_bar_response = title_bar_rect_ui.interact(
                title_bar_rect,
                title_bar_rect_id.with("__sense"),
                Sense::click_and_drag(),
            );

            let mut title_bar_rounding = corner_radius;

            if !self.state.collapsed {
                title_bar_rounding.se = 0;
                title_bar_rounding.sw = 0;
            }

            painter.rect_filled(title_bar_rect, title_bar_rounding, header_color);

            Frame::NONE
                .inner_margin(egui::Margin::symmetric(inner_margin, inner_margin))
                .outer_margin(egui::Margin::symmetric(outer_margin, outer_margin))
                .show(ui, |ui| {
                    let style = ui.style_mut();
                    style.wrap_mode = Some(egui::TextWrapMode::Extend);
                    style.interaction.selectable_labels = false;

                    ui.horizontal(|ui| {
                        ui.set_min_height(title_bar_rect.height() - border_adjust.y);
                        collapsing_state.show_toggle_button(ui, egui::collapsing_header::paint_default_icon);
                        self.state.collapsed = !collapsing_state.is_open();

                        ui.label(title);
                    });
                });

            if title_bar_response.drag_started() {
                self.state.drag_state = Some(DragState {
                    drag_pivot: title_bar_response
                        .interact_pointer_pos()
                        .unwrap_or(self.state.position),
                    initial_drag_position: self.state.position,
                })
            } else if title_bar_response.drag_stopped() {
                self.state.drag_state = None;
            }

            if let Some(drag_state) = &self.state.drag_state {
                if let Some(pos) = ctx.input(|i| i.pointer.interact_pos()) {
                    let delta = pos - drag_state.drag_pivot;
                    self.state.position = drag_state.initial_drag_position + delta;
                }
            }

            if !self.state.collapsed {
                content_fn(ui);

                if let Some(corner_response) = corner_response {
                    stolen::paint_resize_corner(ui, &corner_response);
                }
            }
        }

        let min_rect = window_ui.min_rect();
        let min_size = min_rect.size();

        trace!("min_rect: {:?}, min_size: {:?}", min_rect, min_size);

        collapsing_state.store(&ctx);
    }

    fn clamp_offset(available: Vec2, offset: &mut Pos2) {
        offset.x = offset.x.clamp(0.0, available.x);
        offset.y = offset.y.clamp(0.0, available.y);
    }
}

#[derive(Clone)]
struct DragState {
    drag_pivot: Pos2,
    initial_drag_position: Pos2,
}

#[derive(Clone)]
struct ToolWindowState {
    collapsed: bool,
    position: Pos2,
    size: Vec2,

    drag_state: Option<DragState>,

    /// If false, we are no enabled
    resizable: Vec2b,
}

impl Default for ToolWindowState {
    fn default() -> Self {
        Self {
            resizable: Vec2b::TRUE,
            collapsed: false,
            position: Pos2::ZERO,
            size: vec2(300.0, 200.0),
            drag_state: None,
        }
    }
}

impl ToolWindow {
    pub fn load_or_create_from_params(ctx: &Context, id: Id, builder: &ToolWindowParameters) -> Self {
        Self::load(ctx, id).unwrap_or({
            Self {
                id,
                state: ToolWindowState {
                    position: builder.default_pos,
                    size: builder.default_size,
                    ..Default::default()
                },
            }
        })
    }

    pub fn load(ctx: &Context, id: Id) -> Option<Self> {
        ctx.data_mut(|d| {
            d.get_persisted::<ToolWindowState>(id)
                .map(|state| Self {
                    id,
                    state,
                })
        })
    }

    pub fn store(&self, ctx: &Context) {
        ctx.data_mut(|d| d.insert_persisted(self.id, self.state.clone()));
    }
}

#[cfg(feature = "layout_debugging")]
fn debug_rect(ui: &mut Ui, rect: Rect, debug_color: Color32) {
    let debug_stroke = egui::Stroke::new(1.0, debug_color);
    ui.painter().rect(
        rect,
        CornerRadius::ZERO,
        Color32::TRANSPARENT,
        debug_stroke,
        egui::StrokeKind::Outside,
    );
}

#[cfg(not(feature = "layout_debugging"))]
fn debug_rect(_ui: &mut Ui, _rect: Rect, _debug_color: Color32) {}

/// private methods copied/pasted from the egui's source for UI consistency.
mod stolen {
    use egui::emath::GuiRounding;
    use egui::emath::{Align2, Rect, pos2};
    use egui::epaint::{Color32, Stroke};
    use egui::{Response, Ui};

    /// source: [`egui::containers::resize::paint_resize_corner`]
    pub fn paint_resize_corner(ui: &Ui, response: &Response) {
        let stroke = ui.style().interact(response).fg_stroke;
        paint_resize_corner_with_style(ui, &response.rect, stroke.color, Align2::RIGHT_BOTTOM);
    }

    /// source: [`egui::containers::resize::paint_resize_corner_with_style`]
    pub fn paint_resize_corner_with_style(ui: &Ui, rect: &Rect, color: impl Into<Color32>, corner: Align2) {
        let painter = ui.painter();
        let cp = corner
            .pos_in_rect(rect)
            .round_to_pixels(ui.pixels_per_point());
        let mut w = 2.0;
        let stroke = Stroke {
            width: 1.0, // Set width to 1.0 to prevent overlapping
            color: color.into(),
        };

        while w <= rect.width() && w <= rect.height() {
            painter.line_segment(
                [
                    pos2(cp.x - w * corner.x().to_sign(), cp.y),
                    pos2(cp.x, cp.y - w * corner.y().to_sign()),
                ],
                stroke,
            );
            w += 4.0;
        }
    }
}

pub struct ToolWindows {}

pub struct ToolWindowsStatePersistence {
    id: Id,
    state: ToolWindowsState,
}

#[derive(Default, Clone)]
pub struct ToolWindowsState {
    /// The order in which windows are rendered, the LAST one appears on TOP, the FIRST one on BOTTOM.
    rendering_stack: Vec<Id>,
}

impl ToolWindowsState {
    pub fn bring_to_front(&mut self, id: Id) {
        self.rendering_stack
            .retain(|&stack_id| stack_id != id);
        self.rendering_stack.push(id);
        trace!("new rendering_stack: {:?}", self.rendering_stack);
    }
}

impl ToolWindowsStatePersistence {
    pub fn load_or_default(ctx: &Context, id: Id) -> Self {
        Self::load(ctx, id).unwrap_or({
            Self {
                id,
                state: ToolWindowsState::default(),
            }
        })
    }

    pub fn load(ctx: &Context, id: Id) -> Option<Self> {
        ctx.data_mut(|d| {
            d.get_persisted::<ToolWindowsState>(id)
                .map(|state| Self {
                    id,
                    state,
                })
        })
    }

    pub fn store(&self, ctx: &Context) {
        ctx.data_mut(|d| d.insert_persisted(self.id, self.state.clone()));
    }
}

impl ToolWindows {
    pub fn new() -> Self {
        Self {}
    }

    pub fn windows<F>(self, ui: &mut Ui, mut collect_windows: F)
    where
        F: FnMut(&mut ToolWindowsBuilder),
    {
        let mut builder = ToolWindowsBuilder::default();

        // Collect panel functions
        collect_windows(&mut builder);

        let ctx = ui.ctx().clone();
        let state_id = ui.id().with("__tool_windows_state");
        let mut state_persistence = ToolWindowsStatePersistence::load_or_default(&ctx, state_id);

        //
        // sync layer ordering with the id's collected
        //
        {
            // remove now-unknown ids
            state_persistence
                .state
                .rendering_stack
                .retain(|seen_id| {
                    let retain = builder
                        .windows
                        .iter()
                        .find(|(id, ..)| id == seen_id)
                        .is_some();
                    if retain {
                        trace!("Retained window. id: {:?}", seen_id);
                    } else {
                        trace!("Removing window. id: {:?}", seen_id);
                    }
                    retain
                });

            // add new ids
            for (id, _, _) in builder.windows.iter() {
                if !state_persistence
                    .state
                    .rendering_stack
                    .contains(&id)
                {
                    trace!("adding new window. id: {:?}", id);
                    state_persistence
                        .state
                        .rendering_stack
                        .push(*id);
                }
            }
        }

        // Create a map of windows by ID for faster lookup
        let mut windows_map: std::collections::HashMap<Id, (ToolWindowParameters, Box<dyn FnOnce(&mut Ui)>)> = builder
            .windows
            .drain(..)
            .map(|(id, params, content_fn)| (id, (params, content_fn)))
            .collect();

        // Render windows in the stored order
        let rendering_order = state_persistence
            .state
            .rendering_stack
            .clone();
        for id in rendering_order {
            if let Some((params, content_fn)) = windows_map.remove(&id) {
                trace!("rendering window: {:?}", id);

                let ctx = ui.ctx().clone();
                let mut tool_window = ToolWindow::load_or_create_from_params(&ctx, id, &params);
                ui.push_id(id.with("__tool_window"), |ui| {
                    tool_window.show(ui, params.title, content_fn, &mut state_persistence.state);
                });
                tool_window.store(&ctx);
            }
        }

        state_persistence.store(&ctx);
    }
}

#[derive(Default)]
pub struct ToolWindowsBuilder {
    windows: Vec<(Id, ToolWindowParameters, Box<dyn FnOnce(&mut Ui)>)>,
}

impl ToolWindowsBuilder {
    pub fn add_window(&mut self, id_salt: impl Hash) -> ToolWindowInstanceBuilder {
        let id = Id::new(id_salt);
        ToolWindowInstanceBuilder {
            id,
            builder: self,
            params: ToolWindowParameters::default(),
        }
    }
}

pub struct ToolWindowInstanceBuilder<'a> {
    id: Id,
    builder: &'a mut ToolWindowsBuilder,
    params: ToolWindowParameters,
}

#[derive(Default, Debug)]
pub struct ToolWindowParameters {
    title: String,
    default_pos: Pos2,
    default_size: Vec2,
}

impl<'a> ToolWindowInstanceBuilder<'a> {
    #[inline]
    pub fn default_pos(mut self, pos: impl Into<Pos2>) -> Self {
        self.params.default_pos = pos.into();
        self
    }

    #[inline]
    pub fn default_size(mut self, default_size: impl Into<Vec2>) -> Self {
        self.params.default_size = default_size.into();
        self
    }

    pub fn show<F>(mut self, title: String, content_fn: F)
    where
        F: FnOnce(&mut Ui) + 'static,
    {
        self.params.title = title;
        self.builder
            .windows
            .push((self.id, self.params, Box::new(content_fn)));
    }
}
