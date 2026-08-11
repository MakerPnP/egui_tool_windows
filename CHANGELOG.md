# Changelog

## 0.6.0
- Fix interactivity issues.  Now there is no bleed-though of hover and drag events to
  widgets that appear beneath the tool window.
- Allow resizing of tool windows without having to bring them to the front first.
- Add support for closable tool windows. See `closable`.
- Add support for custom content in the titlebar of the tool window. See `titlebar_content`.
  This is useful for adding menu buttons, settings buttons, indicators, etc.
- Id's must be created manually, instead of via salt.
- Tool windows return actions which should be processed, e.g. `ToolWindowAction::CloseRequested`.
- `Simple` demo updated to show how to use the `closable` and `titlebar_content` features.
- Long window titles now show ellipsis if they are too long to fit.
- Add support for a 'scrollable' mode (disabled by default).
- Add easing when dragging/resizing windows in a scroll area. 
- Shrinking and then growing a container will restore tool windows to their last-placed position.

## 0.5.0

- Add support for egui 0.35.0.

## 0.4.0

- Add support for egui 0.34.0.

## 0.3.0

- Add support for egui 0.33.0.

## 0.2.0

- Add support for egui 0.32.0.
- Update `inside_dock` example to use egui_dock 0.17

## 0.1.3

- New - Added a `persistence` feature, for use with the `egui` feature of the same name.

## 0.1.2

- Fixed - Added support for rendering inside an `egui::Window`.
- New - Added a an `inside_dock` example.

## 0.1.1

- Changed - Usability improvements.
- New - Tool windows have visual shadows.
- New - API improvements for `default_size` and `default_pos`.
- Fixed - Not being able to bring a tool window to the front by clicking in its title-bar.

## 0.1.0

First release
