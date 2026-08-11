use std::collections::HashMap;
use std::fmt::Debug;
use std::hash::Hash;

use egui::collapsing_header::CollapsingState;
use egui::emath::easing;
use egui::{
    Align, Color32, Context, CornerRadius, CursorIcon, Frame, Id, Layout, Pos2, Rect, Sense, Style, Ui, UiBuilder,
    Vec2, Vec2b, vec2,
};
use log::trace;

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum ToolWindowAction {
    CloseRequested,
}

/// What a single window contributed this frame, for `ToolWindows::windows` to aggregate across
/// all windows once every window has been processed (see `ToolWindowsState::sticky_content_extent`
/// for why this can't just be reported per-window as it's produced).
struct ToolWindowFrameResult {
    actions: Vec<ToolWindowAction>,
    /// This window's extent for the current frame, in content space (i.e. relative to
    /// `content_origin`, not translated into absolute/screen coordinates) so it stays valid even
    /// if `content_origin` itself moves (e.g. due to scrolling) before it's used.
    content_space_rect: Rect,
    /// Whether this window is currently being dragged or resized.
    dragging: bool,
}

struct ToolWindow {
    id: Id,
    state: ToolWindowState,
}

impl ToolWindow {
    fn show(
        &mut self,
        ui: &mut Ui,
        params: ToolWindowParameters,
        state: &mut ToolWindowsState,
        scrollable: bool,
        content_origin: Pos2,
    ) -> ToolWindowFrameResult {
        let mut actions = vec![];

        let is_topmost = state.is_topmost(self.id);

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

        let resize_corner_size = ui.visuals().resize_corner_size;
        let baseline_min_size = vec2(100.0, title_bar_height);

        // The content is only rendered (and therefore only measurable) when expanded and when a
        // content closure was actually supplied.
        let can_measure_content = !self.state.collapsed && params.content_fn.is_some();

        let ui_clip_rect = ui.clip_rect();
        debug_rect(ui, ui_clip_rect, Color32::BLUE);

        // In `scrollable` mode the window lives in the container's scrollable content space (so
        // it can be scrolled into view when clipped, see below) and is anchored to
        // `content_origin`, which moves with the content as the container is scrolled. Otherwise
        // it's anchored to (and clamped within) the container's currently visible viewport, so it
        // stays fully reachable even though it can never be scrolled to.
        let top_left = if scrollable {
            self.state.position.x = self.state.position.x.max(0.0);
            self.state.position.y = self.state.position.y.max(0.0);
            content_origin + self.state.position.to_vec2()
        } else {
            let available = Vec2::new(
                (ui_clip_rect.width() - position_margin).max(position_margin),
                (ui_clip_rect.height() - position_margin).max(position_margin),
            );

            Self::clamp_offset(available, &mut self.state.position);

            ui_clip_rect.min + self.state.position.to_vec2()
        };

        let border_adjust_splat = (inner_margin + outer_margin) * 2;
        let border_adjust = Vec2::splat(border_adjust_splat as f32);

        // Builds the outer window rect for a given (uncollapsed) content size.
        let rect_for_size = |size: Vec2| {
            let actual_size = if self.state.collapsed {
                vec2(size.x, title_bar_height)
            } else {
                size
            } + border_adjust;
            Rect::from_min_size(top_left, actual_size)
        };

        let rect = rect_for_size(self.state.size);
        debug_rect(ui, rect, Color32::BLUE);

        // This window's full extent - including any part currently clipped by the container's
        // viewport - expressed in content space. The caller unions this across all windows and
        // registers the result with `ui`, so it contributes to the bounding box an enclosing
        // `ScrollArea` uses to size its content and scrollbars. Without this, a window that
        // doesn't fully fit is silently clipped with no way to scroll to the rest of it.
        let content_space_rect = rect.translate(-content_origin.to_vec2());

        let corner_radius = CornerRadius::same(6);

        //
        // input shield
        //
        // Register a widget that covers the whole window and senses both clicks and drags.
        //
        // This is the FIRST interactive widget registered for this window, so every widget we
        // register afterwards (resize handles, title bar, content) is registered *later* and
        // therefore sits on top of the shield in egui's hit-test ordering - those widgets keep
        // working normally.
        //
        // Relative to windows *below* this one in the rendering stack (and to any widgets in the
        // container behind the windows), the shield is on top, so it wins the hit-test for click,
        // drag and hover. Because egui only marks the single top-most sensing widget as
        // `hovered`/`clicked`/`dragged` at a given position, this is what stops pointer events -
        // including the cursor-icon changes driven by `hovered()` - from reaching an obscured tool
        // window. Without a *drag*-sensing shield, drag-only widgets underneath (table column
        // dividers, drag-values, ...) would still be picked as the drag hit and respond through the
        // window on top of them.
        let shield_response = ui.interact(rect, self.id.with("__tool_window_shield"), Sense::click_and_drag());
        if shield_response.clicked() || shield_response.drag_started() {
            trace!(
                "shield interaction, bringing to front. id: {:?}, rendering_stack: {:?}",
                self.id, state.rendering_stack
            );
            state.bring_to_front(self.id);
        }

        let mut left_dragging = false;
        let mut right_dragging = false;
        let mut top_dragging = false;
        let mut bottom_dragging = false;

        // Whichever edge/corner just started being dragged this frame, captured so we can seed
        // `resize_drag_state` below with a pivot taken at the moment the drag began.
        let mut drag_started: Option<(bool, bool, bool, bool, Pos2)> = None;

        let corner_response = {
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
                        "left" => left_dragging = true,
                        "right" => right_dragging = true,
                        "top" => top_dragging = true,
                        "bottom" => bottom_dragging = true,
                        _ => {}
                    }
                }

                if resp.drag_started() {
                    if let Some(pointer) = resp.interact_pointer_pos() {
                        let (left, right, top, bottom) = match edge {
                            "left" => (true, false, false, false),
                            "right" => (false, true, false, false),
                            "top" => (false, false, true, false),
                            "bottom" => (false, false, false, true),
                            _ => (false, false, false, false),
                        };
                        drag_started = Some((left, right, top, bottom, pointer));
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
                let corner_rect =
                    egui::Rect::from_min_size(rect.right_bottom() - corner_size - border_adjust, corner_size);
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
                    right_dragging = true;
                    bottom_dragging = true;
                }

                if corner_response.drag_started() {
                    if let Some(pointer) = corner_response.interact_pointer_pos() {
                        drag_started = Some((false, true, false, true, pointer));
                    }
                }
            }

            corner_response
        };

        let dragging_x = left_dragging || right_dragging;
        let dragging_y = top_dragging || bottom_dragging;
        let is_actively_resizing = dragging_x || dragging_y;

        if let Some((left, right, top, bottom, drag_pivot)) = drag_started {
            // While collapsed there's no content on screen to resize, so height must not change
            // - mask out `top`/`bottom` regardless of which handle was actually grabbed, which
            // leaves `size.y`/`position.y` untouched below for the rest of this drag.
            let (top, bottom) = if self.state.collapsed {
                (false, false)
            } else {
                (top, bottom)
            };
            self.state.resize_drag_state = Some(ResizeDragState {
                left,
                right,
                top,
                bottom,
                drag_pivot,
                initial_size: self.state.size,
                initial_position: self.state.position,
            });
        }

        // `content_min_size` is deliberately left alone here: it's the best-known minimum and
        // stays valid (if possibly stale) across drags, which matters while collapsed (see
        // `can_measure_content` above). Only the per-drag anchor and measurement flag reset,
        // so the *next* drag starts from a fresh pivot and re-measures once.
        if !is_actively_resizing {
            self.state.measured_for_current_drag = false;
            self.state.resize_drag_state = None;
        }

        // The first frame of a drag doesn't yet know the content's true minimum size, so it runs
        // a "sizing pass": render the content into a squished probe rect (offering only the
        // dragged axes at `baseline_min_size`) to discover what it actually needs, discard that
        // invisible probe frame, and let egui immediately redo the frame - by which point
        // `content_min_size` is up to date and the real drag can be clamped against it.
        //
        // While collapsed the content isn't rendered, so it can't be measured this way;
        // `min_size` then falls back to whatever `content_min_size` was last measured as (or
        // just the baseline, if it's never been measured).
        let needs_sizing_pass = can_measure_content && is_actively_resizing && !self.state.measured_for_current_drag;

        let min_size = baseline_min_size.max(self.state.content_min_size);

        // Resize by re-deriving the desired size/position each frame from the pointer's total
        // displacement since the drag started (`pointer - drag_pivot`) relative to
        // `initial_size`/`initial_position`, rather than from a per-frame delta. Because the
        // computation always starts fresh from the fixed pivot, once a min-size clamp holds the
        // size steady, the window resumes growing exactly when the pointer's displacement from
        // the pivot crosses back past the point where the clamp took effect.
        if !needs_sizing_pass {
            if let Some(drag) = self.state.resize_drag_state {
                if let Some(pointer) = ctx.input(|i| i.pointer.interact_pos()) {
                    let delta = pointer - drag.drag_pivot;

                    let mut size = drag.initial_size;
                    let mut position = drag.initial_position;

                    if drag.right {
                        size.x = (drag.initial_size.x + delta.x).max(min_size.x);
                    } else if drag.left {
                        size.x = (drag.initial_size.x - delta.x).max(min_size.x);
                        position.x = drag.initial_position.x + drag.initial_size.x - size.x;
                    }

                    if drag.bottom {
                        size.y = (drag.initial_size.y + delta.y).max(min_size.y);
                    } else if drag.top {
                        size.y = (drag.initial_size.y - delta.y).max(min_size.y);
                        position.y = drag.initial_position.y + drag.initial_size.y - size.y;
                    }

                    self.state.size = size;
                    self.state.position = position;
                }
            }
        }

        trace!(
            "position: {:?}, size: {:?}, needs_sizing_pass: {:?}",
            self.state.position, self.state.size, needs_sizing_pass
        );

        // What the content is actually offered this frame: the real rect, unless we're running
        // the throwaway sizing pass above, in which case the dragged axes are squished down to
        // `baseline_min_size` to force content to reveal its true minimum instead of padding out
        // to fill whatever's currently available.
        let content_rect = if needs_sizing_pass {
            let probe_size = vec2(
                if dragging_x {
                    baseline_min_size.x
                } else {
                    self.state.size.x
                },
                if dragging_y {
                    baseline_min_size.y
                } else {
                    self.state.size.y
                },
            );
            rect_for_size(probe_size)
        } else {
            rect
        };

        //
        // draw the window frame
        //

        let layer_id = ui.layer_id();
        let mut painter = ctx.layer_painter(layer_id);
        painter.set_clip_rect(ui.clip_rect());

        let frame = Frame::window(&Style::default())
            .inner_margin(egui::Margin::symmetric(inner_margin, inner_margin))
            .outer_margin(egui::Margin::symmetric(outer_margin, outer_margin));
        let shape = frame.paint(rect);
        painter.add(shape);

        //
        // draw the window content
        //

        let window_id = id.with("child_id");
        let mut window_ui = Ui::new(
            ctx.clone(),
            window_id,
            UiBuilder::new()
                .layer_id(layer_id)
                .max_rect(content_rect)
                .layout(Layout::top_down(Align::Min)),
        );
        window_ui.set_clip_rect(content_rect.intersect(ui_clip_rect));

        let window_clip_rect = window_ui.clip_rect();
        debug_rect(ui, window_clip_rect, Color32::YELLOW);

        {
            let ui = &mut window_ui;

            //
            // draw the title bar
            //
            let title_bar_rect = Rect::from_min_size(rect.min, vec2(rect.width(), title_bar_height + border_adjust.y));
            debug_rect(ui, title_bar_rect, Color32::GREEN);

            let title_bar_rect_id = id.with("title_bar_rect_id");
            let mut title_bar_rect_ui = Ui::new(
                ctx.clone(),
                title_bar_rect_id,
                UiBuilder::new()
                    .layer_id(layer_id)
                    .max_rect(title_bar_rect)
                    .sense(Sense::click_and_drag())
                    .layout(Layout::top_down(Align::Min)),
            );

            let title_bar_ui_rect = title_bar_rect.intersect(ui_clip_rect);
            debug_rect(ui, title_bar_ui_rect, Color32::MAGENTA);
            title_bar_rect_ui.set_clip_rect(title_bar_ui_rect);

            let title_bar_response = title_bar_rect_ui.interact(
                title_bar_rect,
                title_bar_rect_id.with("__sense"),
                Sense::click_and_drag(),
            );

            // Bring the window to the front when its title bar is clicked or a drag on it starts.
            // The title bar is registered after the input shield, so it (and the window content)
            // correctly receives interactions instead of the shield.
            if title_bar_response.clicked() || title_bar_response.drag_started() {
                state.bring_to_front(self.id);
            }

            let mut title_bar_rounding = corner_radius;

            if !self.state.collapsed {
                title_bar_rounding.se = 0;
                title_bar_rounding.sw = 0;
            }

            let title_bar_color = if is_topmost {
                visuals.widgets.active.bg_fill
            } else {
                visuals.widgets.open.bg_fill
            };

            painter.rect_filled(title_bar_rect, title_bar_rounding, title_bar_color);

            Frame::NONE
                .inner_margin(egui::Margin::symmetric(inner_margin, inner_margin))
                .outer_margin(egui::Margin::symmetric(outer_margin, outer_margin))
                .show(ui, |ui| {
                    let style = ui.style_mut();
                    style.wrap_mode = Some(egui::TextWrapMode::Extend);
                    style.interaction.selectable_labels = false;

                    ui.set_clip_rect(title_bar_rect.intersect(ui_clip_rect));

                    egui::Sides::new()
                        .spacing(10.0)
                        // lay out the right side (close button + custom content) first, then constrain
                        // the left side (title) to the remaining space so a long title truncates with an
                        // ellipsis instead of extending underneath the right-side widgets.
                        .shrink_left()
                        .truncate()
                        .show(
                            ui,
                            |ui| {
                                ui.set_min_height(title_bar_rect.height() - border_adjust.y);
                                collapsing_state.show_toggle_button(ui, egui::collapsing_header::paint_default_icon);
                                self.state.collapsed = !collapsing_state.is_open();
                                ui.label(params.title);
                            },
                            |ui| {
                                ui.set_min_height(title_bar_rect.height() - border_adjust.y);
                                if params.closable {
                                    let button = egui::Button::new("X")
                                        .min_size(vec2(20.0, title_bar_height))
                                        .frame(false);

                                    if ui.add(button).clicked() {
                                        trace!("closing window: {:?}", self.id);
                                        actions.push(ToolWindowAction::CloseRequested);
                                    }
                                }
                                if let Some(title_fn) = params.titlebar_content_fn {
                                    title_fn(ui);
                                }
                            },
                        );
                });

            // Dragging the title bar moves the window.  The input shield ensures an obscured
            // title bar can't receive a drag, so only the title bar that is actually visible at the pointer will start a move.
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

            //
            // draw the content and resize corner
            //

            if !self.state.collapsed {
                if let Some(content_fn) = params.content_fn {
                    content_fn(ui);
                }

                if needs_sizing_pass {
                    // Discover the content's true minimum size (including any
                    // `set_min_height`/`set_min_width` reservations it made) from the squished
                    // probe render above, so the rest of this drag (and any future collapsed
                    // drag, until the next measurement) can be clamped against it.
                    let measured = (ui.min_rect().size() - border_adjust).max(Vec2::ZERO);
                    self.state.content_min_size = measured.max(baseline_min_size);
                    self.state.measured_for_current_drag = true;
                    ctx.request_discard("egui_tool_windows: measuring content min size for resize clamp");
                }

                if let Some(corner_response) = corner_response {
                    stolen::paint_resize_corner(ui, &corner_response);
                }
            }
        }
        collapsing_state.store(&ctx);

        ToolWindowFrameResult {
            actions,
            content_space_rect,
            dragging: self.state.drag_state.is_some() || self.state.resize_drag_state.is_some(),
        }
    }

    fn clamp_offset(available: Vec2, offset: &mut Pos2) {
        offset.x = offset.x.clamp(0.0, available.x);
        offset.y = offset.y.clamp(0.0, available.y);
    }
}

#[derive(Clone)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
struct DragState {
    drag_pivot: Pos2,
    initial_drag_position: Pos2,
}

/// Captured once when a resize drag starts (on whichever edge/corner the pointer grabbed) and
/// kept for the duration of that drag. `left`/`right`/`top`/`bottom` mark which edge(s) of
/// `initial_size`/`initial_position` the current drag moves; `size`/`position` for the drag are
/// then recomputed each frame from the pointer's total displacement since the drag started
/// (`pointer_pos - drag_pivot`) applied to those initial values, rather than from a per-frame
/// delta.
#[derive(Clone, Copy)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
struct ResizeDragState {
    left: bool,
    right: bool,
    top: bool,
    bottom: bool,
    drag_pivot: Pos2,
    initial_size: Vec2,
    initial_position: Pos2,
}

#[derive(Clone)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
struct ToolWindowState {
    collapsed: bool,
    position: Pos2,
    size: Vec2,

    drag_state: Option<DragState>,

    /// If false, we are no enabled
    resizable: Vec2b,

    /// The best-known minimum size the content needs, as measured by a one-off "sizing pass"
    /// (see `show`) the last time the window was resized while expanded. Stays set once that
    /// drag ends, so it can still clamp a resize started while collapsed, when the content
    /// isn't being rendered and so can't be re-measured - though it will be stale if the
    /// content's requirements changed since it was last measured. Not persisted to disk: starts
    /// at `Vec2::ZERO` (no minimum enforced beyond the baseline) each time the app launches.
    #[cfg_attr(feature = "persistence", serde(skip))]
    content_min_size: Vec2,

    /// Whether `content_min_size` has already been (re-)measured for the resize drag currently
    /// in progress. `false` whenever no drag is in progress, so the next drag triggers exactly
    /// one fresh measurement. Not persisted to disk.
    #[cfg_attr(feature = "persistence", serde(skip))]
    measured_for_current_drag: bool,

    /// `None` unless a resize drag is currently in progress. Not persisted to disk.
    #[cfg_attr(feature = "persistence", serde(skip))]
    resize_drag_state: Option<ResizeDragState>,
}

impl Default for ToolWindowState {
    fn default() -> Self {
        Self {
            resizable: Vec2b::TRUE,
            collapsed: false,
            position: Pos2::ZERO,
            size: vec2(300.0, 200.0),
            drag_state: None,
            content_min_size: Vec2::ZERO,
            measured_for_current_drag: false,
            resize_drag_state: None,
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

pub struct ToolWindows {
    scrollable: bool,
}

pub struct ToolWindowsStatePersistence {
    id: Id,
    state: ToolWindowsState,
}

#[derive(Default, Clone)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct ToolWindowsState {
    /// The order in which windows are rendered, the LAST one appears on TOP, the FIRST one on BOTTOM.
    rendering_stack: Vec<Id>,

    /// While any window is being dragged or resized, the union (in content space) of every
    /// window's extent seen so far during that drag - so it can only grow, never shrink, no
    /// matter how the window(s) responsible for the previous extent move or shrink in the
    /// meantime. `None` whenever no window is currently being dragged/resized, at which point the
    /// container is reported its true (possibly smaller) natural extent again.
    ///
    /// This exists because the reported extent feeds an enclosing `ScrollArea`'s content size,
    /// which determines its maximum scroll offset. If the extent were allowed to shrink
    /// mid-drag, a `ScrollArea` already scrolled near that max would immediately clamp its
    /// offset down to the new, smaller max - and since window positions are anchored relative to
    /// that same offset, the window being dragged would appear to stay glued to the same screen
    /// position, no matter how far the pointer moves, until the offset bottomed out at zero.
    #[cfg_attr(feature = "persistence", serde(skip))]
    sticky_content_extent: Option<Rect>,

    /// An in-progress ease from `sticky_content_extent`'s last value down to the natural extent,
    /// started the moment a drag/resize ends (see `sticky_content_extent`) if that leaves the
    /// container smaller than it was. `None` outside of that transition, in which case the
    /// natural extent is reported directly.
    #[cfg_attr(feature = "persistence", serde(skip))]
    settling_extent: Option<SettlingExtent>,
}

/// How long `ToolWindowsState::settling_extent` takes to ease down to the natural extent.
const CONTENT_EXTENT_SETTLE_DURATION: f32 = 0.2;

#[derive(Clone, Copy)]
struct SettlingExtent {
    from: Rect,
    to: Rect,
    start_time: f64,
}

impl ToolWindowsState {
    pub fn is_topmost(&self, id: Id) -> bool {
        self.rendering_stack.last() == Some(&id)
    }
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
        Self {
            scrollable: false,
        }
    }

    /// When `true`, windows are anchored to the container's scrollable content space (rather than
    /// its currently visible viewport) and their full extent - including any part currently
    /// clipped - is registered with the container's `Ui`. This lets an enclosing `ScrollArea` grow
    /// its content size and scroll to reveal a window that doesn't fully fit, instead of the
    /// window being clamped to always stay fully inside the visible viewport. Only meaningful when
    /// the container is actually scrollable; leave this `false` (the default) for containers like
    /// `Frame` or a resizable panel that can't scroll.
    pub fn scrollable(mut self, scrollable: bool) -> Self {
        self.scrollable = scrollable;
        self
    }

    pub fn windows<F>(self, ui: &mut Ui, mut collect_windows: F) -> HashMap<Id, Vec<ToolWindowAction>>
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
            for (id, _) in builder.windows.iter() {
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
        let mut windows_map: std::collections::HashMap<Id, ToolWindowParameters> = builder
            .windows
            .drain(..)
            .map(|(id, params)| (id, params))
            .collect();

        // The container's content-space origin - i.e. its top-left corner, not wherever the
        // cursor happens to be after any content already drawn in `ui` before this call. Windows
        // float on top of that content and use the entire container, so they must not be pushed
        // down/right by it. `max_rect().min` stays fixed regardless of what's drawn afterwards
        // (including the windows themselves, which - unlike ordinary widgets - only ever grow
        // `max_rect` outward from this corner), and, inside a `ScrollArea`, moves with the scroll
        // offset, which is what lets windows scroll together with the rest of the content.
        let content_origin = ui.max_rect().min;

        let mut actions: HashMap<Id, Vec<ToolWindowAction>> = HashMap::new();
        // Every window's extent and drag status this frame, gathered so they can be aggregated
        // into a single reported extent once every window has been processed - see
        // `ToolWindowsState::sticky_content_extent` for why this can't be done per-window.
        let mut window_results: Vec<(Rect, bool)> = Vec::new();
        // Render windows in the stored order
        let rendering_order = state_persistence
            .state
            .rendering_stack
            .clone();
        for id in rendering_order {
            if let Some(params) = windows_map.remove(&id) {
                trace!("rendering window: {:?}", id);

                let ctx = ui.ctx().clone();
                let mut tool_window = ToolWindow::load_or_create_from_params(&ctx, id, &params);
                ui.push_id(id.with("__tool_window"), |ui| {
                    let result = tool_window.show(
                        ui,
                        params,
                        &mut state_persistence.state,
                        self.scrollable,
                        content_origin,
                    );
                    if !result.actions.is_empty() {
                        actions.insert(id, result.actions);
                    }
                    if self.scrollable {
                        window_results.push((result.content_space_rect, result.dragging));
                    }
                });
                tool_window.store(&ctx);
            }
        }

        if self.scrollable {
            let any_dragging = window_results
                .iter()
                .any(|(_, dragging)| *dragging);
            let natural_extent = window_results
                .into_iter()
                .map(|(rect, _)| rect)
                .reduce(Rect::union);

            // While dragging, union with whatever extent was reported last frame so it can only
            // grow; once nothing is dragging, drop straight back to the natural extent - eased,
            // rather than snapped to, if a drag/resize just ended (see `settling_extent`).
            let reported_extent = if any_dragging {
                let merged = match (state_persistence.state.sticky_content_extent, natural_extent) {
                    (Some(sticky), Some(natural)) => Some(sticky.union(natural)),
                    (Some(sticky), None) => Some(sticky),
                    (None, natural) => natural,
                };
                state_persistence.state.sticky_content_extent = merged;
                state_persistence.state.settling_extent = None;
                merged
            } else {
                if let Some(frozen) = state_persistence.state.sticky_content_extent.take() {
                    if natural_extent != Some(frozen) {
                        state_persistence.state.settling_extent = Some(SettlingExtent {
                            from: frozen,
                            to: natural_extent.unwrap_or(frozen),
                            start_time: ui.ctx().input(|i| i.time),
                        });
                    }
                }

                if let Some(mut settling) = state_persistence.state.settling_extent {
                    // Keep tracking the natural extent in case it changes while settling (e.g. a
                    // window is closed), so this eases towards the latest target rather than a
                    // stale one.
                    if let Some(natural_extent) = natural_extent {
                        settling.to = natural_extent;
                    }

                    let now = ui.ctx().input(|i| i.time);
                    let t = ((now - settling.start_time) / CONTENT_EXTENT_SETTLE_DURATION as f64).clamp(0.0, 1.0)
                        as f32;

                    if t >= 1.0 {
                        state_persistence.state.settling_extent = None;
                        Some(settling.to)
                    } else {
                        state_persistence.state.settling_extent = Some(settling);
                        ui.ctx().request_repaint();
                        Some(
                            settling
                                .from
                                .lerp_towards(&settling.to, easing::cubic_out(t)),
                        )
                    }
                } else {
                    natural_extent
                }
            };

            if let Some(extent) = reported_extent {
                ui.advance_cursor_after_rect(extent.translate(content_origin.to_vec2()));
            }
        }

        state_persistence.store(&ctx);

        actions
    }
}

#[derive(Default)]
pub struct ToolWindowsBuilder {
    windows: Vec<(Id, ToolWindowParameters)>,
}

impl ToolWindowsBuilder {
    pub fn add_window(&mut self, id: Id) -> ToolWindowInstanceBuilder<'_> {
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

#[derive(Default)]
pub struct ToolWindowParameters {
    title: String,
    closable: bool,
    default_pos: Pos2,
    default_size: Vec2,
    titlebar_content_fn: Option<Box<dyn FnOnce(&mut Ui)>>,
    content_fn: Option<Box<dyn FnOnce(&mut Ui)>>,
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

    #[inline]
    pub fn closable(mut self, closable: bool) -> Self {
        self.params.closable = closable;
        self
    }

    pub fn titlebar_content<F>(mut self, content_fn: F) -> Self
    where
        F: FnOnce(&mut Ui) + 'static,
    {
        self.params.titlebar_content_fn = Some(Box::new(content_fn));

        self
    }

    pub fn show<F>(mut self, title: String, content_fn: F)
    where
        F: FnOnce(&mut Ui) + 'static,
    {
        self.params.title = title;
        self.params.content_fn = Some(Box::new(content_fn));
        self.builder
            .windows
            .push((self.id, self.params));
    }
}
