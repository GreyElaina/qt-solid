use qt_widget_derive::{Qt, qt_entity, qt_methods};

use super::shared::{DoubleSpinBoxControl, FocusEvents, QDoubleSpinBoxRangeHost};

#[derive(Qt, Debug, Clone)]
#[qt_entity(widget, export = "doubleSpinBox")]
pub struct DoubleSpinBoxWidget;

#[qt_methods]
#[qt(host)]
impl QDoubleSpinBoxRangeHost for DoubleSpinBoxWidget {}

#[qt_methods]
#[qt(host)]
impl DoubleSpinBoxControl for DoubleSpinBoxWidget {}

#[qt_methods]
#[qt(host)]
impl FocusEvents for DoubleSpinBoxWidget {}

#[qt_methods]
#[qt(host)]
impl DoubleSpinBoxWidget {
    #[qt(prop = value, default = 0.0, setter)]
    fn set_range_value(&mut self, value: f64) {
        qt::cpp! {
            self.setValue(value);
        }
    }

    #[qt(prop = value, getter)]
    fn range_value(&self) -> f64 {
        qt::cpp! {
            return self.value();
        }
    }

    #[qt(prop = minimum, default = 0.0, setter)]
    fn set_range_minimum(&mut self, value: f64) {
        qt::cpp! {
            self.setMinimum(value);
        }
    }

    #[qt(prop = minimum, getter)]
    fn range_minimum(&self) -> f64 {
        qt::cpp! {
            return self.minimum();
        }
    }

    #[qt(prop = maximum, default = 100.0, setter)]
    fn set_range_maximum(&mut self, value: f64) {
        qt::cpp! {
            self.setMaximum(value);
        }
    }

    #[qt(prop = maximum, getter)]
    fn range_maximum(&self) -> f64 {
        qt::cpp! {
            return self.maximum();
        }
    }

    #[qt(prop = step, default = qt::runtime::NonNegativeF64(1.0), setter)]
    fn set_range_step(&mut self, value: qt::runtime::NonNegativeF64) {
        qt::cpp! {
            self.setSingleStep(value);
        }
    }

    #[qt(prop = step, getter)]
    fn range_step(&self) -> qt::runtime::NonNegativeF64 {
        qt::cpp! {
            return self.singleStep();
        }
    }
}
