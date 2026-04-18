use qt_widget_derive::{Qt, qt_entity, qt_methods};

use super::shared::QGroupBoxBoxHost;

#[derive(Qt, Debug, Clone)]
#[qt_entity(widget, export = "group", children = Nodes)]
pub struct GroupWidget;

#[qt_methods]
#[qt(host)]
impl QGroupBoxBoxHost for GroupWidget {}
