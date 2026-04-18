use qt_widget_derive::{Qt, qt_entity, qt_methods};

use super::shared::{FocusEvents, QSliderRangeHost, SliderControl};

#[derive(Qt, Debug, Clone)]
#[qt_entity(widget, export = "slider")]
pub struct SliderWidget;

#[qt_methods]
#[qt(host)]
impl QSliderRangeHost for SliderWidget {}

#[qt_methods]
#[qt(host)]
impl SliderControl for SliderWidget {}

#[qt_methods]
#[qt(host)]
impl FocusEvents for SliderWidget {}

#[qt_methods]
#[qt(host)]
impl SliderWidget {
    #[qt(prop = value, default = 0, setter)]
    fn set_range_value(&mut self, value: i32) {
        qt::cpp! {
            self.setValue(value);
        }
    }

    #[qt(prop = value, getter)]
    fn range_value(&self) -> i32 {
        qt::cpp! {
            return self.value();
        }
    }

    #[qt(prop = minimum, default = 0, setter)]
    fn set_range_minimum(&mut self, value: i32) {
        qt::cpp! {
            self.setMinimum(value);
        }
    }

    #[qt(prop = minimum, getter)]
    fn range_minimum(&self) -> i32 {
        qt::cpp! {
            return self.minimum();
        }
    }

    #[qt(prop = maximum, default = 100, setter)]
    fn set_range_maximum(&mut self, value: i32) {
        qt::cpp! {
            self.setMaximum(value);
        }
    }

    #[qt(prop = maximum, getter)]
    fn range_maximum(&self) -> i32 {
        qt::cpp! {
            return self.maximum();
        }
    }

    #[qt(prop = step, default = 1, setter)]
    fn set_range_step(&mut self, value: u32) {
        qt::cpp! {
            self.setSingleStep(value);
        }
    }

    #[qt(prop = step, getter)]
    fn range_step(&self) -> u32 {
        qt::cpp! {
            return self.singleStep();
        }
    }

    #[qt(prop = page_step, default = 10, setter)]
    fn set_range_page_step(&mut self, value: u32) {
        qt::cpp! {
            self.setPageStep(value);
        }
    }

    #[qt(prop = page_step, getter)]
    fn range_page_step(&self) -> u32 {
        qt::cpp! {
            return self.pageStep();
        }
    }
}
