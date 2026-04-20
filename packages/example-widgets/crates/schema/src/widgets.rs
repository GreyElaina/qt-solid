use std::sync::OnceLock;

use qt_solid_widget_core::runtime::{
    QtWidgetNativeDecl, WidgetPropDecl, collect_widget_prop_decls,
};
use qt_solid_widget_core::schema::*;

pub use banner::BannerWidget;
pub use spin_triangle::SpinTriangleWidget;

mod banner {
    use qt_widget_derive::{Qt, qt_entity, qt_methods};

    use crate::core_widgets::QLabelTextHost;

    #[derive(Qt, Debug, Clone)]
    #[qt_entity(widget, export = "banner", children = Text)]
    pub struct BannerWidget;

    #[qt_methods]
    #[qt(host)]
    impl QLabelTextHost for BannerWidget {}
}

mod spin_triangle {
    use std::f64::consts::{FRAC_PI_2, TAU};

    use qt_solid_widget_core::vello::peniko::{
        Color as VelloColor, Fill,
        kurbo::{Affine, BezPath, Point, Rect, Stroke},
    };
    use qt_widget_derive::{Qt, qt_entity, qt_methods};

    use crate::core_widgets::{Paint, PaintSceneFrame, TexturePaintHost};
    use qt_solid_widget_core::vello::PaintScene;

    #[derive(Debug, Clone, Copy, PartialEq)]
    struct SpinTriangleDirtyBounds {
        x: f64,
        y: f64,
        width: f64,
        height: f64,
    }

    impl SpinTriangleDirtyBounds {
        fn new(x0: f64, y0: f64, x1: f64, y1: f64) -> Option<Self> {
            let width = x1 - x0;
            let height = y1 - y0;
            if !x0.is_finite()
                || !y0.is_finite()
                || !x1.is_finite()
                || !y1.is_finite()
                || width <= 0.0
                || height <= 0.0
            {
                return None;
            }

            Some(Self {
                x: x0,
                y: y0,
                width,
                height,
            })
        }

        fn clamp_to(self, width: f64, height: f64) -> Option<Self> {
            let x0 = self.x.max(0.0).min(width);
            let y0 = self.y.max(0.0).min(height);
            let x1 = (self.x + self.width).max(0.0).min(width);
            let y1 = (self.y + self.height).max(0.0).min(height);
            Self::new(x0, y0, x1, y1)
        }

        fn union(self, other: Self) -> Option<Self> {
            Self::new(
                self.x.min(other.x),
                self.y.min(other.y),
                (self.x + self.width).max(other.x + other.width),
                (self.y + self.height).max(other.y + other.height),
            )
        }
    }

    fn dynamic_bounds(
        width: f64,
        height: f64,
        vertices: &[Point],
        marker_radius: f64,
        stroke_width: f64,
    ) -> Option<SpinTriangleDirtyBounds> {
        let padding = marker_radius + stroke_width + 3.0;
        let mut min_x = f64::INFINITY;
        let mut min_y = f64::INFINITY;
        let mut max_x = f64::NEG_INFINITY;
        let mut max_y = f64::NEG_INFINITY;

        for vertex in vertices {
            min_x = min_x.min(vertex.x);
            min_y = min_y.min(vertex.y);
            max_x = max_x.max(vertex.x);
            max_y = max_y.max(vertex.y);
        }

        SpinTriangleDirtyBounds::new(
            min_x - padding,
            min_y - padding,
            max_x + padding,
            max_y + padding,
        )?
        .clamp_to(width, height)
    }

    #[derive(Qt, Debug, Clone)]
    #[qt_entity(widget, export = "spinTriangle")]
    pub struct SpinTriangleWidget {
        #[qt(default)]
        previous_vertices: Option<[Point; 3]>,
    }

    #[qt_methods]
    #[qt(host)]
    impl TexturePaintHost for SpinTriangleWidget {}

    #[qt_methods]
    #[qt(host)]
    impl Paint<PaintSceneFrame<'_>> for SpinTriangleWidget {
        fn paint(&mut self, frame: &mut PaintSceneFrame<'_>) {
            let width = frame.width();
            let height = frame.height();
            if width <= 16.0 || height <= 16.0 {
                self.previous_vertices = None;
                return;
            }

            frame.request_next_frame();

            let center = Point::new(width * 0.5, height * 0.5);
            let radius = width.min(height) * 0.34;
            let marker_radius = width.min(height) * 0.028;
            let triangle_stroke_width = 2.4;
            let angle = frame.elapsed().as_secs_f64() * TAU * 0.18;

            let mut vertices = Vec::with_capacity(3);
            for index in 0..3 {
                let theta = angle - FRAC_PI_2 + (index as f64) * TAU / 3.0;
                vertices.push(Point::new(
                    center.x + radius * theta.cos(),
                    center.y + radius * theta.sin(),
                ));
            }

            let current_vertices = [vertices[0], vertices[1], vertices[2]];
            let current_dynamic_bounds = dynamic_bounds(
                width,
                height,
                &vertices,
                marker_radius,
                triangle_stroke_width,
            );
            let previous_dynamic_bounds = self.previous_vertices.and_then(|previous_vertices| {
                dynamic_bounds(
                    width,
                    height,
                    &previous_vertices,
                    marker_radius,
                    triangle_stroke_width,
                )
            });
            let requested_dirty_bounds = match (previous_dynamic_bounds, current_dynamic_bounds) {
                (Some(previous), Some(current)) => previous.union(current),
                (Some(previous), None) => Some(previous),
                (None, Some(current)) => Some(current),
                (None, None) => None,
            };
            if let Some(bounds) = requested_dirty_bounds {
                frame.request_dirty_rect(bounds.x, bounds.y, bounds.width, bounds.height);
            } else {
                frame.request_dirty_rect(0.0, 0.0, width, height);
            }
            self.previous_vertices = Some(current_vertices);

            let scene = frame.scene();

            scene.set_solid_brush(VelloColor::from_rgba8(7, 12, 20, 255));
            scene.fill_path(
                Affine::IDENTITY,
                Fill::NonZero,
                &Rect::new(0.0, 0.0, width, height),
            );

            scene.set_solid_brush(VelloColor::from_rgba8(255, 255, 255, 18));
            scene.stroke_path(
                Affine::IDENTITY,
                &Stroke::new(1.5),
                &Rect::new(6.0, 6.0, width - 6.0, height - 6.0),
            );

            let mut triangle = BezPath::new();
            triangle.move_to(vertices[0]);
            triangle.line_to(vertices[1]);
            triangle.line_to(vertices[2]);
            triangle.close_path();

            scene.set_solid_brush(VelloColor::from_rgba8(76, 213, 255, 210));
            scene.fill_path(Affine::IDENTITY, Fill::NonZero, &triangle);

            scene.set_solid_brush(VelloColor::from_rgba8(255, 255, 255, 230));
            scene.stroke_path(Affine::IDENTITY, &Stroke::new(triangle_stroke_width), &triangle);

            let marker_colors = [
                VelloColor::from_rgba8(255, 99, 132, 255),
                VelloColor::from_rgba8(255, 205, 86, 255),
                VelloColor::from_rgba8(54, 162, 235, 255),
            ];
            for (vertex, color) in current_vertices.into_iter().zip(marker_colors) {
                scene.set_solid_brush(color);
                scene.fill_path(
                    Affine::IDENTITY,
                    Fill::NonZero,
                    &Rect::new(
                        vertex.x - marker_radius,
                        vertex.y - marker_radius,
                        vertex.x + marker_radius,
                        vertex.y + marker_radius,
                    ),
                );
            }
        }
    }
}

fn example_widgets_spec_bindings() -> &'static [&'static SpecWidgetBinding] {
    static ALL: OnceLock<Vec<&'static SpecWidgetBinding>> = OnceLock::new();
    ALL.get_or_init(|| vec![BannerWidget::spec(), SpinTriangleWidget::spec()])
        .as_slice()
}

fn example_widgets_prop_decls() -> &'static [&'static WidgetPropDecl] {
    static ALL: OnceLock<Vec<&'static WidgetPropDecl>> = OnceLock::new();
    ALL.get_or_init(|| collect_widget_prop_decls(example_widgets_spec_bindings()))
        .as_slice()
}

fn example_widgets_runtime_spec_bindings() -> &'static [&'static SpecWidgetBinding] {
    static ALL: OnceLock<Vec<&'static SpecWidgetBinding>> = OnceLock::new();
    ALL.get_or_init(|| vec![SpinTriangleWidget::spec()])
        .as_slice()
}

fn example_widgets_runtime_prop_decls() -> &'static [&'static WidgetPropDecl] {
    static ALL: OnceLock<Vec<&'static WidgetPropDecl>> = OnceLock::new();
    ALL.get_or_init(|| collect_widget_prop_decls(example_widgets_runtime_spec_bindings()))
        .as_slice()
}

pub fn example_widgets_library() -> &'static WidgetLibraryBindings {
    static LIBRARY: OnceLock<WidgetLibraryBindings> = OnceLock::new();
    LIBRARY.get_or_init(|| WidgetLibraryBindings {
        library_key: "@qt-solid/example-widgets",
        spec_bindings: example_widgets_spec_bindings(),
        opaque_decls: &[],
        opaque_codegen_decls: &[],
        widget_native_decls: &[&BannerWidget::NATIVE_DECL, &SpinTriangleWidget::NATIVE_DECL],
        widget_prop_decls: example_widgets_prop_decls(),
    })
}

pub fn example_widgets_runtime_library() -> &'static WidgetLibraryBindings {
    static LIBRARY: OnceLock<WidgetLibraryBindings> = OnceLock::new();
    LIBRARY.get_or_init(|| WidgetLibraryBindings {
        library_key: "@qt-solid/example-widgets/runtime",
        spec_bindings: example_widgets_runtime_spec_bindings(),
        opaque_decls: &[],
        opaque_codegen_decls: &[],
        widget_native_decls: &[&SpinTriangleWidget::NATIVE_DECL],
        widget_prop_decls: example_widgets_runtime_prop_decls(),
    })
}
