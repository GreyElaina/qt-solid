use qt_widget_derive::{Qt, qt_entity, qt_methods};

use super::shared::{CheckBox, QCheckBoxTextHost};

#[derive(Qt, Debug, Clone)]
#[qt_entity(widget, export = "check")]
pub struct CheckWidget;

#[qt_methods]
#[qt(host)]
impl QCheckBoxTextHost for CheckWidget {}

#[qt_methods]
#[qt(host)]
impl CheckBox for CheckWidget {}
