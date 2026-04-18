use qt_widget_derive::{Qt, qt_entity, qt_methods};

use super::shared::QWidgetBoxHost;

#[derive(Qt, Debug, Clone)]
#[qt_entity(widget, export = "view", children = Nodes)]
pub struct ViewWidget;

#[qt_methods]
#[qt(host)]
impl QWidgetBoxHost for ViewWidget {}
