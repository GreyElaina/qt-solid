use qt_widget_derive::{Qt, qt_entity, qt_methods};

use super::shared::QLabelTextHost;

#[derive(Qt, Debug, Clone)]
#[qt_entity(widget, export = "label", children = Text)]
pub struct LabelWidget;

#[qt_methods]
#[qt(host)]
impl QLabelTextHost for LabelWidget {}
