use qt_widget_derive::{Qt, qt_entity, qt_methods};

use super::shared::QLabelTextHost;

#[derive(Qt, Debug, Clone)]
#[qt_entity(widget, export = "text", children = Text)]
pub struct TextWidget;

#[qt_methods]
#[qt(host)]
impl QLabelTextHost for TextWidget {}
