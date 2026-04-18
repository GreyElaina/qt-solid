use qt_widget_derive::{Qt, qt_entity, qt_methods};

use super::shared::{FocusEvents, LineEdit, QLineEditHost};

#[derive(Qt, Debug, Clone)]
#[qt_entity(widget, export = "input")]
pub struct InputWidget;

#[qt_methods]
#[qt(host)]
impl QLineEditHost for InputWidget {}

#[qt_methods]
#[qt(host)]
impl LineEdit for InputWidget {}

#[qt_methods]
#[qt(host)]
impl FocusEvents for InputWidget {}

#[qt_methods]
trait InputMethods {
    #[qt(host("setFocus"))]
    fn focus(&self);

    #[qt(host("selectAll"))]
    fn select_all(&self);
}

#[qt_methods]
impl InputMethods for InputWidget {}

#[qt_methods]
#[qt(host)]
impl InputWidget {
    #[qt(prop = cursor_position, setter)]
    fn set_selection_cursor_position(&mut self, value: i32) {
        qt::cpp! {
            const int length = static_cast<int>(self.text().size());
            self.setCursorPosition(std::clamp(value, 0, length));
        }
    }

    #[qt(prop = cursor_position, getter)]
    fn selection_cursor_position(&self) -> i32 {
        qt::cpp! {
            return self.cursorPosition();
        }
    }

    #[qt(prop = selection_start, setter)]
    fn set_selection_start(&mut self, value: i32) {
        qt::cpp! {
            const int length = static_cast<int>(self.text().size());
            const int current_end = self.hasSelectedText()
                ? self.selectionStart() + static_cast<int>(self.selectedText().size())
                : self.cursorPosition();
            const int start = std::clamp(value, 0, length);
            const int end = std::clamp(current_end, 0, length);
            self.setSelection(std::min(start, end), std::abs(end - start));
        }
    }

    #[qt(prop = selection_start, getter)]
    fn selection_start(&self) -> i32 {
        qt::cpp! {
            return self.hasSelectedText() ? self.selectionStart() : self.cursorPosition();
        }
    }

    #[qt(prop = selection_end, setter)]
    fn set_selection_end(&mut self, value: i32) {
        qt::cpp! {
            const int length = static_cast<int>(self.text().size());
            const int current_start = self.hasSelectedText()
                ? self.selectionStart()
                : self.cursorPosition();
            const int start = std::clamp(current_start, 0, length);
            const int end = std::clamp(value, 0, length);
            self.setSelection(std::min(start, end), std::abs(end - start));
        }
    }

    #[qt(prop = selection_end, getter)]
    fn selection_end(&self) -> i32 {
        qt::cpp! {
            return self.hasSelectedText()
                ? self.selectionStart() + static_cast<int>(self.selectedText().size())
                : self.cursorPosition();
        }
    }
}
