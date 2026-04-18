use qt_widget_derive::{Qt, qt_entity, qt_methods};

use super::shared::{PushButton, QPushButtonTextHost};

#[derive(Qt, Debug, Clone)]
#[qt_entity(widget, export = "button", children = Text)]
pub struct ButtonWidget;

#[qt_methods]
#[qt(host)]
impl QPushButtonTextHost for ButtonWidget {}

#[qt_methods]
#[qt(host)]
impl PushButton for ButtonWidget {}
