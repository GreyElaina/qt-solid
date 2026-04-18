use crate::prelude::*;
use std::sync::{Arc, Mutex};

use super::shared::CustomPaintFlexHost;

type PaintHandler = dyn for<'a> FnMut(&mut QtPainter<'a>) + Send + 'static;

#[qt_entity(
    opaque,
    borrow = "mut",
    host(class = "QPainter", include = "<QtGui/QPainter>")
)]
#[derive(Qt)]
pub struct QtPainter<'a> {}

#[qt_methods]
impl<'a> QtPainter<'a> {
    #[qt(host)]
    pub fn width(&mut self) -> f64 {
        qt::cpp! {
            return static_cast<double>(self.viewport().width());
        }
    }

    #[qt(host)]
    pub fn height(&mut self) -> f64 {
        qt::cpp! {
            return static_cast<double>(self.viewport().height());
        }
    }

    #[qt(host)]
    #[qt(include = "<QtCore/QRectF>")]
    #[qt(include = "<QtGui/QColor>")]
    fn fill_rect_rgba(
        &mut self,
        x: f64,
        y: f64,
        width: f64,
        height: f64,
        red: f64,
        green: f64,
        blue: f64,
        alpha: f64,
    ) {
        qt::cpp! {
            self.fillRect(
                QRectF(x, y, width, height),
                QColor::fromRgbF(red, green, blue, alpha)
            );
        }
    }

    #[qt(host)]
    #[qt(include = "<QtCore/QRectF>")]
    #[qt(include = "<QtGui/QColor>")]
    #[qt(include = "<QtGui/QPen>")]
    fn stroke_rect_rgba(
        &mut self,
        x: f64,
        y: f64,
        width: f64,
        height: f64,
        red: f64,
        green: f64,
        blue: f64,
        alpha: f64,
        line_width: f64,
    ) {
        qt::cpp! {
            self.save();
            QPen pen(QColor::fromRgbF(red, green, blue, alpha));
            pen.setWidthF(line_width);
            self.setPen(pen);
            self.setBrush(Qt::NoBrush);
            self.drawRect(QRectF(x, y, width, height));
            self.restore();
        }
    }

    pub fn fill_rect(&mut self, rect: QtRect, color: QtColor) {
        self.fill_rect_rgba(
            rect.x,
            rect.y,
            rect.width,
            rect.height,
            f64::from(color.red),
            f64::from(color.green),
            f64::from(color.blue),
            f64::from(color.alpha),
        );
    }

    pub fn stroke_rect(&mut self, rect: QtRect, color: QtColor, line_width: f64) {
        self.stroke_rect_rgba(
            rect.x,
            rect.y,
            rect.width,
            rect.height,
            f64::from(color.red),
            f64::from(color.green),
            f64::from(color.blue),
            f64::from(color.alpha),
            line_width,
        );
    }

    pub fn draw_demo_frame(&mut self) {
        let width = self.width();
        let height = self.height();
        if width <= 8.0 || height <= 8.0 {
            return;
        }

        self.fill_rect(
            QtRect {
                x: 3.0,
                y: 3.0,
                width: width - 6.0,
                height: height - 6.0,
            },
            QtColor {
                red: 1.0,
                green: 191.0 / 255.0,
                blue: 0.0,
                alpha: 32.0 / 255.0,
            },
        );
        self.stroke_rect(
            QtRect {
                x: 1.0,
                y: 1.0,
                width: width - 2.0,
                height: height - 2.0,
            },
            QtColor {
                red: 1.0,
                green: 191.0 / 255.0,
                blue: 0.0,
                alpha: 220.0 / 255.0,
            },
            2.0,
        );
    }
}

#[derive(Clone)]
pub struct CanvasPaintCallback(Arc<Mutex<Box<PaintHandler>>>);

impl CanvasPaintCallback {
    fn new(handler: impl for<'a> FnMut(&mut QtPainter<'a>) + Send + 'static) -> Self {
        Self(Arc::new(Mutex::new(Box::new(handler))))
    }

    fn call(&self, painter: &mut QtPainter<'_>) {
        let mut handler = self.0.lock().expect("canvas paint callback mutex poisoned");
        (*handler)(painter);
    }
}

impl Default for CanvasPaintCallback {
    fn default() -> Self {
        Self::new(|painter| painter.draw_demo_frame())
    }
}

impl std::fmt::Debug for CanvasPaintCallback {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("CanvasPaintCallback").finish()
    }
}

#[derive(Qt, Debug, Clone)]
#[qt_entity(widget, export = "canvas")]
pub struct CanvasWidget {
    #[qt(default)]
    paint_handler: CanvasPaintCallback,
}

#[qt_methods]
#[qt(host)]
impl CustomPaintFlexHost for CanvasWidget {}

#[qt_methods]
#[qt(host)]
impl Paint<QtPainter<'_>> for CanvasWidget {
    fn paint(&mut self, painter: &mut QtPainter<'_>) {
        self.paint_handler.call(painter);
    }
}
