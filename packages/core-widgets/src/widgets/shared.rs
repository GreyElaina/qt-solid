mod layout {
    use qt_widget_derive::qt_methods;

    pub trait LayoutKind {
        const LAYOUT: qt::schema::WidgetLayoutKind;
    }

    pub enum BoxKind {}

    impl LayoutKind for BoxKind {
        const LAYOUT: qt::schema::WidgetLayoutKind = qt::schema::WidgetLayoutKind::Box;
    }

    #[qt_methods]
    #[qt(host)]
    pub trait Geometry {
        #[qt(prop = width, setter)]
        fn set_width(&mut self, value: i32) {
            qt::cpp! {
                const int width = clamp_non_negative(value);
                self.resize(width, self.height());
            }
        }

        #[qt(prop = width, getter)]
        fn width(&self) -> i32 {
            qt::cpp! {
                return self.width();
            }
        }

        #[qt(prop = height, setter)]
        fn set_height(&mut self, value: i32) {
            qt::cpp! {
                const int height = clamp_non_negative(value);
                self.resize(self.width(), height);
            }
        }

        #[qt(prop = height, getter)]
        fn height(&self) -> i32 {
            qt::cpp! {
                return self.height();
            }
        }

        #[qt(prop = min_width, default = 0, setter)]
        fn set_min_width(&mut self, value: u32) {
            qt::cpp! {
                widget_entry.min_width = clamp_non_negative(value);
                apply_widget_style(widget_entry);
            }
        }

        #[qt(prop = min_width, getter)]
        fn min_width(&self) -> u32 {
            qt::cpp! {
                return self.minimumWidth();
            }
        }

        #[qt(prop = min_height, default = 0, setter)]
        fn set_min_height(&mut self, value: u32) {
            qt::cpp! {
                widget_entry.min_height = clamp_non_negative(value);
                apply_widget_style(widget_entry);
            }
        }

        #[qt(prop = min_height, getter)]
        fn min_height(&self) -> u32 {
            qt::cpp! {
                return self.minimumHeight();
            }
        }
    }

    #[qt_methods]
    #[qt(host)]
    pub trait PreferredSizeGeometry {
        #[qt(prop = width, setter)]
        fn set_width(&mut self, value: i32) {
            qt::cpp! {
                self.set_preferred_width(clamp_non_negative(value));
            }
        }

        #[qt(prop = width, getter)]
        fn width(&self) -> i32 {
            qt::cpp! {
                return self.width();
            }
        }

        #[qt(prop = height, setter)]
        fn set_height(&mut self, value: i32) {
            qt::cpp! {
                self.set_preferred_height(clamp_non_negative(value));
            }
        }

        #[qt(prop = height, getter)]
        fn height(&self) -> i32 {
            qt::cpp! {
                return self.height();
            }
        }

        #[qt(prop = min_width, default = 0, setter)]
        fn set_min_width(&mut self, value: u32) {
            qt::cpp! {
                widget_entry.min_width = clamp_non_negative(value);
                apply_widget_style(widget_entry);
                self.updateGeometry();
            }
        }

        #[qt(prop = min_width, getter)]
        fn min_width(&self) -> u32 {
            qt::cpp! {
                return self.minimumWidth();
            }
        }

        #[qt(prop = min_height, default = 0, setter)]
        fn set_min_height(&mut self, value: u32) {
            qt::cpp! {
                widget_entry.min_height = clamp_non_negative(value);
                apply_widget_style(widget_entry);
                self.updateGeometry();
            }
        }

        #[qt(prop = min_height, getter)]
        fn min_height(&self) -> u32 {
            qt::cpp! {
                return self.minimumHeight();
            }
        }
    }

    #[qt_methods]
    #[qt(host)]
    pub trait Flex {
        #[qt(prop = grow, default = 0, setter)]
        fn set_flex_grow(&mut self, value: u32) {
            qt::cpp! {
                widget_entry.flex_grow = clamp_non_negative(value);
                apply_widget_style(widget_entry);
            }
        }

        #[qt(prop = grow, getter)]
        fn flex_grow(&self) -> u32 {
            qt::cpp! {
                return widget_entry.flex_grow;
            }
        }

        #[qt(prop = shrink, default = 1, setter)]
        fn set_flex_shrink(&mut self, value: u32) {
            qt::cpp! {
                widget_entry.flex_shrink = clamp_non_negative(value);
                apply_widget_style(widget_entry);
            }
        }

        #[qt(prop = shrink, getter)]
        fn flex_shrink(&self) -> u32 {
            qt::cpp! {
                return widget_entry.flex_shrink;
            }
        }
    }

    #[qt_methods]
    #[qt(host)]
    pub trait Layout<K: LayoutKind> {
        #[qt(prop = direction, default = qt::decl::FlexDirection::Column, setter)]
        fn set_flex_direction(&mut self, value: qt::decl::FlexDirection) {
            qt::cpp! {
                if (widget_entry.layout == nullptr) {
                    throw_error("flex_direction requires a layout-backed widget");
                }
                if (value < 0 || value > 255) {
                    throw_error("Qt host enum prop received out-of-range tag");
                }
                widget_entry.flex_direction = flex_direction_from_tag(static_cast<std::uint8_t>(value));
                apply_layout_style(widget_entry);
            }
        }

        #[qt(prop = direction, getter)]
        fn flex_direction(&self) -> qt::decl::FlexDirection {
            qt::cpp! {
                return static_cast<std::int32_t>(widget_entry.flex_direction);
            }
        }

        #[qt(prop = align_items, default = qt::decl::AlignItems::Stretch, setter)]
        fn set_align_items(&mut self, value: qt::decl::AlignItems) {
            qt::cpp! {
                if (widget_entry.layout == nullptr) {
                    throw_error("align_items requires a layout-backed widget");
                }
                if (value < 0 || value > 255) {
                    throw_error("Qt host enum prop received out-of-range tag");
                }
                widget_entry.align_items = align_items_from_tag(static_cast<std::uint8_t>(value));
                apply_layout_style(widget_entry);
            }
        }

        #[qt(prop = align_items, getter)]
        fn align_items(&self) -> qt::decl::AlignItems {
            qt::cpp! {
                return static_cast<std::int32_t>(widget_entry.align_items);
            }
        }

        #[qt(prop = justify_content, default = qt::decl::JustifyContent::FlexStart, setter)]
        fn set_justify_content(&mut self, value: qt::decl::JustifyContent) {
            qt::cpp! {
                if (widget_entry.layout == nullptr) {
                    throw_error("justify_content requires a layout-backed widget");
                }
                if (value < 0 || value > 255) {
                    throw_error("Qt host enum prop received out-of-range tag");
                }
                widget_entry.justify_content = justify_content_from_tag(static_cast<std::uint8_t>(value));
                apply_layout_style(widget_entry);
            }
        }

        #[qt(prop = justify_content, getter)]
        fn justify_content(&self) -> qt::decl::JustifyContent {
            qt::cpp! {
                return static_cast<std::int32_t>(widget_entry.justify_content);
            }
        }

        #[qt(prop = gap, default = 0, setter)]
        fn set_gap(&mut self, value: u32) {
            qt::cpp! {
                if (widget_entry.layout == nullptr) {
                    throw_error("gap requires a layout-backed widget");
                }
                widget_entry.gap = clamp_non_negative(value);
                apply_layout_style(widget_entry);
            }
        }

        #[qt(prop = gap, getter)]
        fn gap(&self) -> u32 {
            qt::cpp! {
                return widget_entry.gap;
            }
        }

        #[qt(prop = padding, default = 0, setter)]
        fn set_padding(&mut self, value: u32) {
            qt::cpp! {
                if (widget_entry.layout == nullptr) {
                    throw_error("padding requires a layout-backed widget");
                }
                widget_entry.padding = clamp_non_negative(value);
                apply_layout_style(widget_entry);
            }
        }

        #[qt(prop = padding, getter)]
        fn padding(&self) -> u32 {
            qt::cpp! {
                return widget_entry.padding;
            }
        }
    }
}

mod text {
    use qt_widget_derive::qt_methods;

    pub trait FontKind {}

    pub enum TextKind {}

    impl FontKind for TextKind {}

    #[qt_methods]
    #[qt(host)]
    pub trait Font<K: FontKind> {
        #[qt(prop = family, setter)]
        fn set_family(&mut self, value: String) {
            qt::cpp! {
                QFont font = self.font();
                const QString family = to_qstring(value);
                if (font.family() == family) {
                    return true;
                }
                font.setFamily(family);
                self.setFont(font);
            }
        }

        #[qt(prop = family, getter)]
        fn family(&self) -> String {
            qt::cpp! {
                return to_rust_string(self.font().family());
            }
        }

        #[qt(prop = point_size, default = qt::runtime::NonNegativeF64(12.0), setter)]
        fn set_point_size(&mut self, value: qt::runtime::NonNegativeF64) {
            qt::cpp! {
                QFont font = self.font();
                const double point_size = std::max(1.0, value < 0.0 ? 0.0 : value);
                if (font.pointSizeF() == point_size) {
                    return true;
                }
                font.setPointSizeF(point_size);
                self.setFont(font);
            }
        }

        #[qt(prop = point_size, getter)]
        fn point_size(&self) -> qt::runtime::NonNegativeF64 {
            qt::cpp! {
                return self.font().pointSizeF();
            }
        }

        #[qt(prop = weight, default = qt::runtime::FontWeight(400), setter)]
        fn set_weight(&mut self, value: qt::runtime::FontWeight) {
            qt::cpp! {
                QFont font = self.font();
                const int weight = std::min(1000, clamp_non_negative(value));
                if (font.weight() == weight) {
                    return true;
                }
                font.setWeight(static_cast<QFont::Weight>(weight));
                self.setFont(font);
            }
        }

        #[qt(prop = weight, getter)]
        fn weight(&self) -> qt::runtime::FontWeight {
            qt::cpp! {
                return self.font().weight();
            }
        }

        #[qt(prop = italic, default = false, setter)]
        fn set_italic(&mut self, value: bool) {
            qt::cpp! {
                QFont font = self.font();
                if (font.italic() == value) {
                    return true;
                }
                font.setItalic(value);
                self.setFont(font);
            }
        }

        #[qt(prop = italic, getter)]
        fn italic(&self) -> bool {
            qt::cpp! {
                return self.font().italic();
            }
        }
    }
}

mod focus {
    use qt_widget_derive::qt_methods;

    #[qt_methods]
    #[qt(host)]
    pub trait Focusable {
        #[qt(prop = focus_policy, setter)]
        fn set_focus_policy(&mut self, value: qt::decl::FocusPolicy) {
            qt::cpp! {
                if (value < 0 || value > 255) {
                    throw_error("Qt host enum prop received out-of-range tag");
                }
                self.setFocusPolicy(focus_policy_from_tag(static_cast<std::uint8_t>(value)));
            }
        }

        #[qt(prop = focus_policy, getter)]
        fn focus_policy(&self) -> qt::decl::FocusPolicy {
            qt::cpp! {
                switch (self.focusPolicy()) {
                case Qt::NoFocus:
                    return static_cast<std::int32_t>(FocusPolicyKind::NoFocus);
                case Qt::TabFocus:
                    return static_cast<std::int32_t>(FocusPolicyKind::TabFocus);
                case Qt::ClickFocus:
                    return static_cast<std::int32_t>(FocusPolicyKind::ClickFocus);
                case Qt::StrongFocus:
                    return static_cast<std::int32_t>(FocusPolicyKind::StrongFocus);
                default:
                    throw_error("Qt focus policy not supported by Rust schema");
                }
            }
        }

        #[qt(prop = auto_focus, default = false, const, setter)]
        fn set_auto_focus(&mut self, value: bool) {
            qt::cpp! {
                if (!value) {
                    return true;
                }
                self.setFocus(Qt::OtherFocusReason);
            }
        }
    }
}

mod props {
    use qt_widget_derive::qt_methods;

    #[qt_methods]
    #[qt(host)]
    pub trait Enabled {
        #[qt(prop = enabled, default = true, setter)]
        fn set_enabled(&mut self, value: bool) {
            qt::cpp! {
                self.setEnabled(value);
            }
        }

        #[qt(prop = enabled, getter)]
        fn enabled(&self) -> bool {
            qt::cpp! {
                return self.isEnabled();
            }
        }
    }

    #[qt_methods]
    #[qt(host)]
    pub trait Visible {
        #[qt(prop = visible, default = true, setter)]
        fn set_visible(&mut self, value: bool) {
            qt::cpp! {
                self.setVisible(value);
            }
        }

        #[qt(prop = visible, getter)]
        fn visible(&self) -> bool {
            qt::cpp! {
                return self.isVisible();
            }
        }
    }

    #[qt_methods]
    #[qt(host)]
    pub trait Text {
        #[qt(prop = text, setter)]
        fn set_text(&mut self, value: String) {
            qt::cpp! {
                self.setText(to_qstring(value));
            }
        }

        #[qt(prop = text, getter)]
        fn text(&self) -> String {
            qt::cpp! {
                return to_rust_string(self.text());
            }
        }
    }

    #[qt_methods]
    #[qt(host)]
    pub trait Placeholder {
        #[qt(prop = placeholder, setter)]
        fn set_placeholder(&mut self, value: String) {
            qt::cpp! {
                self.setPlaceholderText(to_qstring(value));
            }
        }

        #[qt(prop = placeholder, getter)]
        fn placeholder(&self) -> String {
            qt::cpp! {
                return to_rust_string(self.placeholderText());
            }
        }
    }

    #[qt_methods]
    #[qt(host)]
    pub trait Checkable {
        #[qt(prop = checked, default = false, setter)]
        fn set_checked(&mut self, value: bool) {
            qt::cpp! {
                self.setChecked(value);
            }
        }

        #[qt(prop = checked, getter)]
        fn checked(&self) -> bool {
            qt::cpp! {
                return self.isChecked();
            }
        }
    }

    #[qt_methods]
    #[qt(host)]
    pub trait GroupTitle {
        #[qt(prop = title, setter)]
        fn set_title(&mut self, value: String) {
            qt::cpp! {
                self.setTitle(to_qstring(value));
            }
        }

        #[qt(prop = title, getter)]
        fn title(&self) -> String {
            qt::cpp! {
                return to_rust_string(self.title());
            }
        }
    }

    #[qt_methods]
    #[qt(host)]
    pub trait WindowTitle {
        #[qt(prop = title, setter)]
        fn set_title(&mut self, value: String) {
            qt::cpp! {
                self.setWindowTitle(to_qstring(value));
            }
        }

        #[qt(prop = title, getter)]
        fn title(&self) -> String {
            qt::cpp! {
                return to_rust_string(self.windowTitle());
            }
        }
    }
}

mod controls {
    use qt_widget_derive::qt_methods;

    #[qt_methods]
    #[qt(host)]
    pub trait FocusEvents {
        #[qt(notify = focus::focus_in, export = "onFocusIn")]
        fn focus_in(&mut self) {
            qt::cpp! {
                new WidgetEventForwarder(&self, QEvent::FocusIn, [dispatch_event]() {
                    dispatch_event();
                });
            }
        }

        #[qt(notify = focus::focus_out, export = "onFocusOut")]
        fn focus_out(&mut self) {
            qt::cpp! {
                new WidgetEventForwarder(&self, QEvent::FocusOut, [dispatch_event]() {
                    dispatch_event();
                });
            }
        }
    }

    #[qt_methods]
    #[qt(host)]
    pub trait PushButton {
        #[qt(notify = widget::clicked, qt_signal = "clicked", export = "onClicked")]
        fn clicked(&mut self);
    }

    #[qt_methods]
    #[qt(host)]
    pub trait CheckBox {
        #[qt(notify = widget::toggled, qt_signal = "toggled", export = "onToggled")]
        fn toggled(&mut self, #[qt(echo)] checked: bool);
    }

    #[qt_methods]
    #[qt(host)]
    pub trait LineEdit {
        #[qt(
            notify = widget::text_changed,
            qt_signal = "textChanged",
            export = "onChanged",
            export = "onTextChanged"
        )]
        fn text_changed(&mut self, #[qt(echo)] text: String);

        #[qt(
            notify = selection::cursor_position_changed,
            qt_signal = "cursorPositionChanged",
            export = "onCursorPositionChanged"
        )]
        fn cursor_position_changed(&mut self, old_position: i32, #[qt(echo)] cursor_position: i32);

        #[qt(notify = selection::selection_changed, export = "onSelectionChanged")]
        fn selection_changed(
            &mut self,
            #[qt(echo)] selection_start: i32,
            #[qt(echo)] selection_end: i32,
        ) {
            qt::cpp! {
                QObject::connect(&self, &QLineEdit::selectionChanged, &self, [&self, dispatch_event]() {
                    const int cursor = self.cursorPosition();
                    const int start = self.hasSelectedText() ? self.selectionStart() : cursor;
                    const int end = self.hasSelectedText()
                        ? self.selectionStart() + self.selectedText().size()
                        : cursor;
                    dispatch_event(start, end);
                });
            }
        }
    }

    #[qt_methods]
    #[qt(host)]
    pub trait SliderControl {
        #[qt(notify = range::value_changed, qt_signal = "valueChanged", export = "onValueChanged")]
        fn value_changed(&mut self, #[qt(echo)] value: i32);
    }

    #[qt_methods]
    #[qt(host)]
    pub trait DoubleSpinBoxControl {
        #[qt(notify = range::value_changed, qt_signal = "valueChanged", export = "onValueChanged")]
        fn value_changed(&mut self, #[qt(echo)] value: f64);
    }
}

mod window {
    use qt_widget_derive::qt_methods;

    #[qt_methods]
    #[qt(host)]
    pub trait WindowFrame {
        #[qt(notify = window::close_requested, export = "onCloseRequested")]
        fn close_requested(&mut self) {
            qt::cpp! {
                self.add_close_requested_handler([dispatch_event]() {
                    dispatch_event();
                });
            }
        }

        #[qt(prop = frameless, default = false, setter)]
        fn set_frameless(&mut self, value: bool) {
            qt::cpp! {
                self.set_frameless(value);
            }
        }

        #[qt(prop = frameless, getter)]
        fn frameless(&self) -> bool {
            qt::cpp! {
                return self.frameless();
            }
        }

        #[qt(prop = transparent_background, default = false, setter)]
        fn set_transparent_background(&mut self, value: bool) {
            qt::cpp! {
                self.set_transparent_background(value);
            }
        }

        #[qt(prop = transparent_background, getter)]
        fn transparent_background(&self) -> bool {
            qt::cpp! {
                return self.transparent_background();
            }
        }

        #[qt(prop = always_on_top, default = false, setter)]
        fn set_always_on_top(&mut self, value: bool) {
            qt::cpp! {
                self.set_always_on_top(value);
            }
        }

        #[qt(prop = always_on_top, getter)]
        fn always_on_top(&self) -> bool {
            qt::cpp! {
                return self.always_on_top();
            }
        }
    }
}

mod hosts {
    use qt_widget_derive::qt_methods;

    use super::*;

    #[qt_methods]
    #[qt(host(class = "QWidget", include = "<QtWidgets/QWidget>"))]
    pub trait QWidgetBoxHost: Geometry + Flex + Layout<BoxKind> + Enabled {}

    #[qt_methods]
    #[qt(host(class = "QGroupBox", include = "<QtWidgets/QGroupBox>"))]
    pub trait QGroupBoxBoxHost: Geometry + Flex + Layout<BoxKind> + Enabled + GroupTitle {}

    #[qt_methods]
    #[qt(host(class = "QLabel", include = "<QtWidgets/QLabel>"))]
    pub trait QLabelTextHost: Geometry + Flex + Font<TextKind> + Enabled + Text {}

    #[qt_methods]
    #[qt(host(class = "QPushButton", include = "<QtWidgets/QPushButton>"))]
    pub trait QPushButtonTextHost: Geometry + Flex + Font<TextKind> + Enabled + Text {}

    #[qt_methods]
    #[qt(host(class = "QCheckBox", include = "<QtWidgets/QCheckBox>"))]
    pub trait QCheckBoxTextHost:
        Geometry + Flex + Font<TextKind> + Enabled + Text + Checkable
    {
    }

    #[qt_methods]
    #[qt(host(class = "QLineEdit", include = "<QtWidgets/QLineEdit>"))]
    pub trait QLineEditHost:
        Geometry + Flex + Font<TextKind> + Focusable + Enabled + Text + Placeholder
    {
    }

    #[qt_methods]
    #[qt(host(class = "QSlider", include = "<QtWidgets/QSlider>"))]
    pub trait QSliderRangeHost: Geometry + Flex + Focusable + Enabled {}

    #[qt_methods]
    #[qt(host(class = "QDoubleSpinBox", include = "<QtWidgets/QDoubleSpinBox>"))]
    pub trait QDoubleSpinBoxRangeHost: Geometry + Flex + Focusable + Enabled {}

    #[qt_methods]
    #[qt(host(
        class = "HostWindowWidget",
        include = "\"host_window_widget.h\"",
        factory = "window.host",
        top_level = true
    ))]
    pub trait WindowHost:
        Geometry + Flex + Layout<BoxKind> + Enabled + Visible + WindowTitle
    {
    }

    #[qt_methods]
    #[qt(host(
        class = "CustomPaintHostWidget",
        include = "\"custom_paint_host_widget.h\""
    ))]
    pub trait CustomPaintHost: PreferredSizeGeometry + Enabled {}

    #[qt_methods]
    #[qt(host(
        class = "TexturePaintHostWidget",
        include = "\"texture_paint_host_widget.h\""
    ))]
    pub trait TexturePaintHost: PreferredSizeGeometry + Enabled {}

    #[qt_methods]
    #[qt(host(
        class = "CustomPaintHostWidget",
        include = "\"custom_paint_host_widget.h\""
    ))]
    pub trait CustomPaintFlexHost: PreferredSizeGeometry + Flex + Enabled {}
}

pub use controls::*;
pub use focus::*;
pub use hosts::*;
pub use layout::*;
pub use props::*;
pub use text::*;
pub use window::*;
