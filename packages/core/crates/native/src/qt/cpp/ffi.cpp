#include "qt/ffi.h"
#include "qt_wgpu_platform.h"
#include "native/src/qt/ffi.rs.h"
#include "qt/widget_host.h"
#include "qt_host/host.h"

#include <array>
#include <cstring>
#include <limits>
#include <unordered_map>

#include <QtCore/QCoreApplication>
#include <QtCore/QDebug>
#include <QtCore/QEvent>
#include <QtCore/QEventLoop>
#include <QtCore/QMetaMethod>
#include <QtCore/QMetaProperty>
#include <QtCore/QMetaType>
#include <QtCore/QObject>
#include <QtCore/QPoint>
#include <QtCore/QPointer>
#include <QtCore/QSignalBlocker>
#include <QtCore/QTimer>
#include <QtCore/QThread>
#include <QtCore/QMimeData>
#include <QtCore/QVariant>
#include <QtCore/QVersionNumber>
#include <QtGui/QCloseEvent>
#include <QtGui/QClipboard>
#include <QtGui/QCursor>
#include <QtGui/QExposeEvent>
#include <QtGui/QFont>
#include <QtGui/QFontMetricsF>
#include <QtGui/QGlyphRun>
#include <QtGui/QGuiApplication>
#include <QtGui/QBackingStore>
#include <QtGui/QImage>
#include <QtGui/QKeyEvent>
#include <QtGui/QMouseEvent>
#include <QtGui/QPaintEvent>
#include <QtGui/QPainter>
#include <QtGui/QPainterPath>
#include <QtGui/QPlatformSurfaceEvent>
#include <QtGui/QRawFont>
#include <QtGui/QStyleHints>
#include <QtGui/QTextLayout>
#include <QtGui/QWheelEvent>
#include <QtGui/QWindow>
#include <private/qbackingstorerhisupport_p.h>
#include <QtWidgets/QAbstractButton>
#include <QtWidgets/QApplication>
#include <QtWidgets/QBoxLayout>
#include <QtWidgets/QCheckBox>
#include <QtWidgets/QDoubleSpinBox>
#include <QtWidgets/QGroupBox>
#include <QtWidgets/QLabel>
#include <QtWidgets/QLayout>
#include <QtWidgets/QLineEdit>
#include <QtWidgets/QPushButton>
#include <QtWidgets/QSizePolicy>
#include <QtWidgets/QSlider>
#include <QtWidgets/QStackedLayout>
#include <QtWidgets/QWidget>
#include <QtWidgets/QFileDialog>
#include <qpa/qplatformbackingstore.h>
#include <qpa/qplatformgraphicsbuffer.h>
#include <rhi/qrhi.h>
#include <rhi/qshader.h>

#include <uv.h>

// -- Platform-specific includes --

#if defined(Q_OS_WIN)
#ifndef WIN32_LEAN_AND_MEAN
#define WIN32_LEAN_AND_MEAN
#endif
#include <windows.h>
#endif

#if defined(Q_OS_LINUX)
#include <dlfcn.h>
#endif

#if defined(Q_OS_MACOS)
#include "qt/macos/display_link.h"
#include <CoreFoundation/CoreFoundation.h>
#include <pthread.h>
#endif

#include <algorithm>
#include <atomic>
#include <array>
#include <cmath>
#include <cstdio>
#include <cstdint>
#include <cstdlib>
#include <cstring>
#include <functional>
#include <memory>
#include <optional>
#include <stdexcept>
#include <string>
#include <string_view>
#include <unordered_map>
#include <unordered_set>
#include <vector>

#include "qt/layout.h"

#include <private/qwidgetlinecontrol_p.h>

namespace qt_solid_spike::qt {
namespace {

using qt_solid::host::throw_error;
using qt_solid::host::throw_uv_error;
using qt_solid::host::request_qt_pump;
using qt_solid::host::on_required_qt_host_thread;

// Trace helpers (duplicated here for compositor.cpp and other ffi-layer users)
static bool qt_solid_wgpu_trace_enabled() {
  static const bool enabled = qEnvironmentVariableIsSet("QT_SOLID_WGPU_TRACE");
  return enabled;
}

template <typename... Args>
static void qt_solid_wgpu_trace(const char *fmt, Args... args) {
  if (!qt_solid_wgpu_trace_enabled()) {
    return;
  }
  std::fprintf(stdout, "[qt-host] ");
  std::fprintf(stdout, fmt, args...);
  std::fprintf(stdout, "\n");
  std::fflush(stdout);
}

#include "util.cpp"
#include "inspector.cpp"
#include "text.cpp"
#include "window/text_edit.cpp"
#include "window/widget.cpp"
#include "window/input.cpp"
#include "window/compositor.cpp"
#include "widget_tree/layout.cpp"
#include "widget_tree/mouse.cpp"
#include "widget_tree/tree.cpp"

} // namespace

static QtRegistry *g_registry = nullptr;

// ---------------------------------------------------------------------------
// FFI routing — thin wrappers that delegate to g_host / registry
// ---------------------------------------------------------------------------

bool qt_host_started() { return qt_solid::host::qt_host_started_impl(); }

void qt_host_start(std::uintptr_t uv_loop_ptr) {
  if (uv_loop_ptr == 0) {
    throw_error("received null libuv loop pointer from N-API");
  }

  if (!g_registry) {
    g_registry = new QtRegistry();
  }

  qt_solid::host::QtHostCallbacks callbacks;
  callbacks.on_pre_start = []() {
    qt_wgpu_renderer::register_static_platform_plugins();
    qt_wgpu_renderer::configure_unified_compositor_platform();
  };
  callbacks.on_started = [](QApplication *app) {
    qt_wgpu_renderer::sync_unified_compositor_active_state();
    g_registry->install_motion_mouse_filter(app);
  };
  callbacks.on_shutdown = []() {
    g_registry->clear();
  };
  callbacks.on_app_activate = []() {
    qt_solid_spike::qt::emit_app_event(::rust::Str("activate"));
  };

  qt_solid::host::qt_host_start_impl(
      reinterpret_cast<uv_loop_t *>(uv_loop_ptr), callbacks);

#if QT_VERSION >= QT_VERSION_CHECK(6, 5, 0)
  QObject::connect(
      QGuiApplication::styleHints(), &QStyleHints::colorSchemeChanged,
      QCoreApplication::instance(), [](Qt::ColorScheme scheme) {
        std::uint8_t tag = 0;
        switch (scheme) {
        case Qt::ColorScheme::Light:
          tag = 1;
          break;
        case Qt::ColorScheme::Dark:
          tag = 2;
          break;
        default:
          break;
        }
        qt_solid_spike::qt::qt_system_color_scheme_changed(tag);
        request_qt_pump();
      });
#endif

  auto *primary_screen = QGuiApplication::primaryScreen();
  if (primary_screen) {
    QObject::connect(
        primary_screen, &QScreen::logicalDotsPerInchChanged,
        QCoreApplication::instance(), [](qreal dpi) {
          qt_solid_spike::qt::qt_screen_dpi_changed(dpi);
          request_qt_pump();
        });
  }
}

void qt_host_shutdown() {
  qt_solid::host::qt_host_shutdown_impl();
}

void qt_create_widget(std::uint32_t id, std::uint8_t kind_tag) {
  if (!qt_solid::host::qt_host_started_impl()) {
    throw_error("call QtApp.start before creating Qt widgets");
  }

  g_registry->create_widget(id, kind_tag);
  request_qt_pump();
}

void qt_insert_child(std::uint32_t parent_id, std::uint32_t child_id,
                     std::uint32_t anchor_id_or_zero) {
  if (!qt_solid::host::qt_host_started_impl()) {
    throw_error("call QtApp.start before inserting Qt widgets");
  }

  g_registry->insert_child(parent_id, child_id, anchor_id_or_zero);
  request_qt_pump();
}

void qt_remove_child(std::uint32_t parent_id, std::uint32_t child_id) {
  if (!qt_solid::host::qt_host_started_impl()) {
    throw_error("call QtApp.start before removing Qt widgets");
  }

  g_registry->remove_child(parent_id, child_id);
  request_qt_pump();
}

void qt_destroy_widget(std::uint32_t id,
                       rust::Slice<const std::uint32_t> subtree_ids) {
  if (!qt_solid::host::qt_host_started_impl()) {
    return;
  }

  g_registry->destroy_widget(id, subtree_ids);
  request_qt_pump();
}

void qt_request_repaint(std::uint32_t id) {
  if (!qt_solid::host::qt_host_started_impl()) {
    throw_error("call QtApp.start before requesting a repaint");
  }

  g_registry->request_repaint(id);
  request_qt_pump();
}

bool qt_request_window_compositor_frame(std::uint32_t id) {
  if (!qt_solid::host::qt_host_started_impl()) {
    throw_error("call QtApp.start before requesting a compositor frame");
  }

  const bool requested = g_registry->request_window_compositor_frame(id);
#if !defined(Q_OS_MACOS)
  request_qt_pump();
#endif
  return requested;
}

void notify_window_compositor_present_complete(std::uint32_t id) {
  if (!qt_solid::host::qt_host_started_impl()) {
    return;
  }

  auto *context = QCoreApplication::instance();
  if (context == nullptr) {
    return;
  }

  QTimer::singleShot(0, context, [id]() {
    if (!qt_solid::host::qt_host_started_impl()) {
      return;
    }

    g_registry->notify_window_compositor_frame_complete(id);
  });
  request_qt_pump();
}

QtWidgetCaptureLayout qt_capture_widget_layout(std::uint32_t id) {
  if (!qt_solid::host::qt_host_started_impl()) {
    throw_error("call QtApp.start before capturing a Qt widget");
  }

  return g_registry->capture_widget_layout(id);
}

void qt_set_window_transient_owner(std::uint32_t window_id, std::uint32_t owner_id) {
  if (!qt_solid::host::qt_host_started_impl()) {
    throw_error("call QtApp.start before setting transient owner");
  }

  g_registry->set_window_transient_owner(window_id, owner_id);
}

rust::Vec<QtRect> qt_capture_widget_visible_rects(std::uint32_t id) {
  if (!qt_solid::host::qt_host_started_impl()) {
    throw_error("call QtApp.start before reading a Qt widget visible region");
  }

  return g_registry->capture_widget_visible_rects(id);
}

void qt_set_widget_mouse_transparent(std::uint32_t id, bool transparent) {
  if (!qt_solid::host::qt_host_started_impl()) {
    throw_error("call QtApp.start before changing a Qt widget mouse transparency");
  }

  g_registry->set_widget_mouse_transparent(id, transparent);
}

void qt_capture_widget_into(std::uint32_t id, std::uint32_t width_px,
                            std::uint32_t height_px, std::size_t stride,
                            bool include_children,
                            rust::Slice<std::uint8_t> bytes) {
  if (!qt_solid::host::qt_host_started_impl()) {
    throw_error("call QtApp.start before capturing a Qt widget");
  }

  g_registry->capture_widget_into(id, width_px, height_px, stride,
                                         include_children, bytes);
}

void qt_capture_widget_region_into(std::uint32_t id, std::uint32_t width_px,
                                   std::uint32_t height_px, std::size_t stride,
                                   bool include_children, QtRect rect,
                                   rust::Slice<std::uint8_t> bytes) {
  if (!qt_solid::host::qt_host_started_impl()) {
    throw_error("call QtApp.start before capturing a Qt widget");
  }

  g_registry->capture_widget_region_into(id, width_px, height_px, stride,
                                                include_children, rect, bytes);
}

QtRealizedNodeState qt_debug_node_state(std::uint32_t id) {
  if (!qt_solid::host::qt_host_started_impl()) {
    throw_error("call QtApp.start before reading a Qt debug snapshot");
  }

  return g_registry->debug_node_state(id);
}

QtNodeBounds debug_node_bounds(std::uint32_t id) {
  if (!qt_solid::host::qt_host_started_impl()) {
    throw_error("call QtApp.start before reading debug node bounds");
  }

  return g_registry->debug_node_bounds(id);
}

QtScreenGeometry get_screen_geometry(std::uint32_t id) {
  if (!qt_solid::host::qt_host_started_impl()) {
    throw_error("call QtApp.start before reading screen geometry");
  }

  return g_registry->get_screen_geometry(id);
}

void focus_widget(std::uint32_t id) {
  if (!qt_solid::host::qt_host_started_impl()) {
    throw_error("call QtApp.start before focusing widget");
  }

  g_registry->focus_widget(id);
}

QtScreenGeometry get_widget_size_hint(std::uint32_t id) {
  if (!qt_solid::host::qt_host_started_impl()) {
    throw_error("call QtApp.start before reading widget size hint");
  }

  return g_registry->get_widget_size_hint(id);
}

std::uint32_t debug_node_at_point(std::int32_t screen_x,
                                  std::int32_t screen_y) {
  if (!qt_solid::host::qt_host_started_impl()) {
    throw_error("call QtApp.start before reading debug node at point");
  }

  return g_registry->debug_node_at_point(screen_x, screen_y);
}

void debug_set_inspect_mode(bool enabled) {
  if (!qt_solid::host::qt_host_started_impl()) {
    throw_error("call QtApp.start before toggling debug inspect mode");
  }

  g_registry->debug_set_inspect_mode(enabled);
  request_qt_pump();
}

void debug_click_node(std::uint32_t id) {
  if (!qt_solid::host::qt_host_started_impl()) {
    throw_error("call QtApp.start before triggering debug clicks");
  }

  g_registry->debug_click_node(id);
  request_qt_pump();
}

void debug_close_node(std::uint32_t id) {
  if (!qt_solid::host::qt_host_started_impl()) {
    throw_error("call QtApp.start before triggering debug close requests");
  }

  g_registry->debug_close_node(id);
  request_qt_pump();
}

void debug_input_insert_text(std::uint32_t id, rust::Str value) {
  if (!qt_solid::host::qt_host_started_impl()) {
    throw_error("call QtApp.start before triggering debug text input");
  }

  g_registry->debug_input_insert_text(id, value);
  request_qt_pump();
}

void debug_highlight_node(std::uint32_t id) {
  if (!qt_solid::host::qt_host_started_impl()) {
    throw_error("call QtApp.start before triggering debug highlight");
  }

  g_registry->debug_highlight_node(id);
  request_qt_pump();
}

void debug_clear_highlight() {
  if (!qt_solid::host::qt_host_started_impl()) {
    throw_error("call QtApp.start before clearing debug highlight");
  }

  g_registry->debug_clear_highlight();
  request_qt_pump();
}

void schedule_debug_event(std::uint32_t delay_ms, rust::Str name) {
  if (!qt_solid::host::qt_host_started_impl()) {
    throw_error("call QtApp.start before scheduling a debug event");
  }

  std::string event_name(name);
  auto *context = QCoreApplication::instance();
  QTimer::singleShot(static_cast<int>(delay_ms), context,
                     [event_name = std::move(event_name)]() {
                       ::qt_solid_spike::qt::emit_debug_event(
                           rust::Str(event_name));
                     });
  request_qt_pump();
}

// ---------------------------------------------------------------------------
// Clipboard
// ---------------------------------------------------------------------------

#include "platform/clipboard.cpp"

// ---------------------------------------------------------------------------
// Trace timestamp
// ---------------------------------------------------------------------------

std::uint64_t trace_now_ns() { return uv_hrtime(); }

// ---------------------------------------------------------------------------
// Text shaping FFI functions
// ---------------------------------------------------------------------------

QtShapedTextResult qt_shape_text_to_path(rust::Str text, double font_size, rust::Str font_family, std::int32_t font_weight, bool font_italic, double max_width, std::uint8_t elide_mode) {
  QFont font = make_qfont(font_family, font_size, font_weight, font_italic);
  QString qtext = QString::fromUtf8(text.data(), static_cast<int>(text.size()));

  // Elide text if requested: 1=clip (just no-wrap), 2=ellipsis (elide right).
  if (elide_mode == 2 && max_width > 0.0) {
    QFontMetricsF fm(font);
    qtext = fm.elidedText(qtext, Qt::ElideRight, max_width);
  }

  QTextLayout layout(qtext, font);
  layout.beginLayout();
  double y_offset = 0.0;
  while (true) {
    QTextLine line = layout.createLine();
    if (!line.isValid()) break;
    if (max_width > 0.0) {
      line.setLineWidth(max_width);
    } else {
      line.setLineWidth(std::numeric_limits<qreal>::max());
    }
    line.setPosition(QPointF(0.0, y_offset));
    y_offset += line.height();
    // Single-line mode for clip/ellipsis: stop after first line.
    if (elide_mode > 0) break;
  }
  layout.endLayout();

  auto glyph_result = collect_glyphs_with_color_fallback(layout);
  rust::Vec<QtShapedPathEl> elements = serialize_painter_path(glyph_result.outline_path);

  rust::Vec<QtRasterizedGlyph> rasterized_glyphs;
  fill_rasterized_glyph_wire(glyph_result, rasterized_glyphs);

  QFontMetricsF metrics(font);
  rust::Vec<QtShapedTextLine> lines;
  double total_width = 0.0;
  double total_height = 0.0;
  for (int i = 0; i < layout.lineCount(); ++i) {
    QTextLine ln = layout.lineAt(i);
    QtShapedTextLine line_info;
    line_info.y_offset = ln.position().y();
    line_info.width = ln.naturalTextWidth();
    line_info.height = ln.height();
    line_info.ascent = ln.ascent();
    line_info.descent = ln.descent();
    lines.push_back(line_info);
    if (ln.naturalTextWidth() > total_width) total_width = ln.naturalTextWidth();
    total_height = ln.position().y() + ln.height();
  }
  if (lines.empty()) {
    total_width = 0.0;
    total_height = metrics.ascent() + metrics.descent();
  }

  return QtShapedTextResult{
      std::move(elements),
      std::move(lines),
      metrics.ascent(),
      metrics.descent(),
      total_width,
      total_height,
      std::move(rasterized_glyphs),
  };
}

QtShapedTextWithCursorsResult qt_shape_text_with_cursors(rust::Str text, double font_size, rust::Str font_family, std::int32_t font_weight, bool font_italic) {
  QFont font = make_qfont(font_family, font_size, font_weight, font_italic);
  QString qtext = QString::fromUtf8(text.data(), static_cast<int>(text.size()));

  QTextLayout layout(qtext, font);
  layout.beginLayout();
  QTextLine line = layout.createLine();
  if (line.isValid()) {
    line.setLineWidth(std::numeric_limits<qreal>::max());
  }
  layout.endLayout();

  QPainterPath combined_path = collect_glyph_path(layout);
  rust::Vec<QtShapedPathEl> elements = serialize_painter_path(combined_path);

  QFontMetricsF metrics(font);
  double total_width =
      line.isValid() ? line.naturalTextWidth() : metrics.horizontalAdvance(qtext);

  rust::Vec<double> cursor_x_positions;
  const int text_len = qtext.length();
  if (line.isValid()) {
    for (int pos = 0; pos <= text_len; ++pos) {
      cursor_x_positions.push_back(line.cursorToX(pos));
    }
  } else {
    for (int pos = 0; pos <= text_len; ++pos) {
      cursor_x_positions.push_back(
          text_len > 0 ? total_width * pos / text_len : 0.0);
    }
  }

  return QtShapedTextWithCursorsResult{
      std::move(elements),
      std::move(cursor_x_positions),
      metrics.ascent(),
      metrics.descent(),
      total_width,
  };
}

QtStyledShapedTextResult qt_shape_styled_text_to_path(
    rust::Str text,
    double default_font_size,
    rust::Str default_font_family,
    double max_width,
    std::uint8_t elide_mode,
    rust::Slice<const QtTextStyleRun> style_runs) {

  QFont default_font = make_qfont(default_font_family, default_font_size, 0, false);
  QString qtext = QString::fromUtf8(text.data(), static_cast<int>(text.size()));

  QTextLayout layout(qtext, default_font);

  // Apply per-run formatting via QTextLayout::FormatRange.
  QList<QTextLayout::FormatRange> formats;
  for (std::size_t idx = 0; idx < style_runs.size(); ++idx) {
    const auto &run = style_runs[idx];
    QTextLayout::FormatRange range;
    range.start = run.start;
    range.length = run.length;

    rust::Str run_family = run.font_family.size() > 0
        ? rust::Str(run.font_family)
        : default_font_family;
    double run_size = run.font_size > 0 ? run.font_size : default_font_size;
    QFont run_font = make_qfont(run_family, run_size, run.font_weight, run.font_italic);
    range.format.setFont(run_font);
    formats.append(range);
  }
  layout.setFormats(formats);

  layout.beginLayout();
  double y_offset = 0.0;
  while (true) {
    QTextLine line = layout.createLine();
    if (!line.isValid()) break;
    if (max_width > 0.0) {
      line.setLineWidth(max_width);
    } else {
      line.setLineWidth(std::numeric_limits<qreal>::max());
    }
    line.setPosition(QPointF(0.0, y_offset));
    y_offset += line.height();
    if (elide_mode > 0) break;
  }
  layout.endLayout();

  // Build per-run glyph paths by attributing glyphs via string indexes.
  // Color glyphs (empty pathForGlyph) are collected separately.
  const int num_runs = static_cast<int>(style_runs.size());
  std::vector<QPainterPath> run_paths(num_runs);
  GlyphCollectionResult color_result;

  auto resolve_target_run = [&](int char_idx) -> int {
    for (int r = 0; r < num_runs; ++r) {
      if (char_idx >= style_runs[r].start &&
          char_idx < style_runs[r].start + style_runs[r].length) {
        return r;
      }
    }
    return num_runs > 0 ? 0 : -1;
  };

#if QT_VERSION >= QT_VERSION_CHECK(6, 5, 0)
  const auto glyph_runs = layout.glyphRuns(-1, -1,
      QTextLayout::GlyphRunRetrievalFlag::RetrieveGlyphIndexes
      | QTextLayout::GlyphRunRetrievalFlag::RetrieveGlyphPositions
      | QTextLayout::GlyphRunRetrievalFlag::RetrieveStringIndexes);
#else
  const auto glyph_runs = layout.glyphRuns();
#endif
  for (const QGlyphRun &gr : glyph_runs) {
    QRawFont raw_font = gr.rawFont();
    const auto indexes = gr.glyphIndexes();
    const auto positions = gr.positions();
    const auto string_indexes = gr.stringIndexes();

    // If string indexes unavailable, fall back to combined single-run path.
    if (string_indexes.size() != indexes.size()) {
      for (int gi = 0; gi < indexes.size(); ++gi) {
        QPainterPath gp = raw_font.pathForGlyph(indexes[gi]);
        if (!gp.isEmpty()) {
          gp.translate(positions[gi]);
          if (num_runs > 0) run_paths[0].addPath(gp);
        } else {
          auto rasterized = rasterize_color_glyph(raw_font, indexes[gi], positions[gi], 0);
          if (rasterized.width > 0 && rasterized.height > 0) {
            color_result.rasterized_glyphs.push_back(std::move(rasterized));
          }
        }
      }
      continue;
    }

    for (int i = 0; i < indexes.size(); ++i) {
      int char_idx = (i < string_indexes.size()) ? string_indexes[i] : -1;
      int target_run = resolve_target_run(char_idx);

      QPainterPath glyph_path = raw_font.pathForGlyph(indexes[i]);
      if (!glyph_path.isEmpty()) {
        glyph_path.translate(positions[i]);
        if (target_run >= 0) {
          run_paths[target_run].addPath(glyph_path);
        }
      } else {
        // Color glyph — rasterize with run attribution.
        int run_idx = target_run >= 0 ? target_run : 0;
        auto rasterized = rasterize_color_glyph(raw_font, indexes[i], positions[i], run_idx);
        if (rasterized.width > 0 && rasterized.height > 0) {
          color_result.rasterized_glyphs.push_back(std::move(rasterized));
        }
      }
    }
  }

  // Serialize per-run paths.
  rust::Vec<QtStyledShapedRun> result_runs;
  for (int r = 0; r < num_runs; ++r) {
    QtStyledShapedRun shaped_run;
    shaped_run.elements = serialize_painter_path(run_paths[r]);
    result_runs.push_back(std::move(shaped_run));
  }

  // Convert rasterized glyphs to wire format.
  rust::Vec<QtRasterizedGlyph> rasterized_glyphs;
  fill_rasterized_glyph_wire(color_result, rasterized_glyphs);

  // Line metrics.
  QFontMetricsF metrics(default_font);
  rust::Vec<QtShapedTextLine> lines;
  double total_width = 0.0;
  double total_height = 0.0;
  for (int i = 0; i < layout.lineCount(); ++i) {
    QTextLine ln = layout.lineAt(i);
    QtShapedTextLine line_info;
    line_info.y_offset = ln.position().y();
    line_info.width = ln.naturalTextWidth();
    line_info.height = ln.height();
    line_info.ascent = ln.ascent();
    line_info.descent = ln.descent();
    lines.push_back(line_info);
    if (ln.naturalTextWidth() > total_width) total_width = ln.naturalTextWidth();
    total_height = ln.position().y() + ln.height();
  }
  if (lines.empty()) {
    total_width = 0.0;
    total_height = metrics.ascent() + metrics.descent();
  }

  // Combined path for measurement.
  QPainterPath combined;
  for (int r = 0; r < num_runs; ++r) {
    combined.addPath(run_paths[r]);
  }
  rust::Vec<QtShapedPathEl> combined_elements = serialize_painter_path(combined);

  return QtStyledShapedTextResult{
      std::move(combined_elements),
      std::move(result_runs),
      std::move(lines),
      metrics.ascent(),
      metrics.descent(),
      total_width,
      total_height,
      std::move(rasterized_glyphs),
  };
}

// ---------------------------------------------------------------------------
// Appearance
// ---------------------------------------------------------------------------

#include "platform/appearance.cpp"

// ---------------------------------------------------------------------------
// Window property FFI
// ---------------------------------------------------------------------------

static HostWindowWidget *require_host_window(std::uint32_t id) {
  if (!qt_solid::host::qt_host_started_impl()) {
    throw_error("call QtApp.start before modifying window properties");
  }
  auto *widget = g_registry->widget_ptr(id);
  auto *host = dynamic_cast<HostWindowWidget *>(widget);
  if (!host) {
    throw_error("target widget is not a window");
  }
  return host;
}

void qt_window_set_title(std::uint32_t id, rust::Str value) {
  auto *host = require_host_window(id);
  host->setWindowTitle(QString::fromUtf8(value.data(), static_cast<int>(value.size())));
  request_qt_pump();
}

void qt_window_set_width(std::uint32_t id, std::int32_t value) {
  auto *host = require_host_window(id);
  host->resize(value, host->height());
  request_qt_pump();
}

void qt_window_set_height(std::uint32_t id, std::int32_t value) {
  auto *host = require_host_window(id);
  host->resize(host->width(), value);
  request_qt_pump();
}

void qt_window_set_min_width(std::uint32_t id, std::int32_t value) {
  auto *host = require_host_window(id);
  host->setMinimumWidth(value);
  request_qt_pump();
}

void qt_window_set_min_height(std::uint32_t id, std::int32_t value) {
  auto *host = require_host_window(id);
  host->setMinimumHeight(value);
  request_qt_pump();
}

void qt_window_set_visible(std::uint32_t id, bool value) {
  auto *host = require_host_window(id);
  host->setVisible(value);
  request_qt_pump();
}

void qt_window_set_enabled(std::uint32_t id, bool value) {
  auto *host = require_host_window(id);
  host->setEnabled(value);
  request_qt_pump();
}

void qt_window_set_frameless(std::uint32_t id, bool value) {
  auto *host = require_host_window(id);
  host->set_frameless(value);
  request_qt_pump();
}

void qt_window_set_transparent_background(std::uint32_t id, bool value) {
  auto *host = require_host_window(id);
  host->set_transparent_background(value);
  request_qt_pump();
}

void qt_window_set_always_on_top(std::uint32_t id, bool value) {
  auto *host = require_host_window(id);
  host->set_always_on_top(value);
  request_qt_pump();
}

void qt_window_set_window_kind(std::uint32_t id, std::uint8_t value) {
  auto *host = require_host_window(id);
  host->set_window_kind(value);
  request_qt_pump();
}

void qt_window_set_screen_position(std::uint32_t id, std::int32_t x,
                                   std::int32_t y) {
  auto *host = require_host_window(id);
  host->set_screen_position(x, y);
  request_qt_pump();
}

void qt_window_wire_close_requested(std::uint32_t id) {
  auto *host = require_host_window(id);
  host->add_close_requested_handler([id]() {
    qt_solid_spike::qt::qt_window_event_close_requested(id);
  });
}

void qt_window_wire_hover_enter(std::uint32_t id) {
  auto *host = require_host_window(id);
  host->setAttribute(Qt::WA_Hover, true);
  new WidgetEventForwarder(host, QEvent::HoverEnter, [id]() {
    qt_solid_spike::qt::qt_window_event_hover_enter(id);
  });
}

void qt_window_wire_hover_leave(std::uint32_t id) {
  auto *host = require_host_window(id);
  host->setAttribute(Qt::WA_Hover, true);
  new WidgetEventForwarder(host, QEvent::HoverLeave, [id]() {
    qt_solid_spike::qt::qt_window_event_hover_leave(id);
  });
}

void qt_canvas_set_cursor(std::uint32_t node_id, std::uint8_t cursor_tag) {
  auto *host = require_host_window(node_id);
  Qt::CursorShape shape;
  switch (cursor_tag) {
  case 1: shape = Qt::PointingHandCursor; break;
  case 2: shape = Qt::IBeamCursor; break;
  case 3: shape = Qt::CrossCursor; break;
  case 4: shape = Qt::SizeAllCursor; break;
  case 5: shape = Qt::WaitCursor; break;
  case 6: shape = Qt::ForbiddenCursor; break;
  case 7: shape = Qt::OpenHandCursor; break;
  case 8: shape = Qt::ClosedHandCursor; break;
  default: shape = Qt::ArrowCursor; break;
  }
  host->setCursor(shape);
}

void qt_text_edit_activate(std::uint32_t window_id, std::uint32_t canvas_node_id,
                           std::uint32_t fragment_id, rust::Str text,
                           double font_size, std::int32_t cursor_pos,
                           std::int32_t sel_start, std::int32_t sel_end) {
  auto *host = require_host_window(window_id);
  QString qtext = QString::fromUtf8(text.data(), static_cast<int>(text.size()));
  host->text_edit_session().activate(canvas_node_id, fragment_id, qtext,
                                    font_size, cursor_pos, sel_start, sel_end);
  host->setAttribute(Qt::WA_InputMethodEnabled, true);
  QGuiApplication::inputMethod()->update(Qt::ImEnabled);
}

void qt_text_edit_deactivate(std::uint32_t window_id) {
  auto *host = require_host_window(window_id);
  host->text_edit_session().deactivate();
  host->setAttribute(Qt::WA_InputMethodEnabled, false);
  QGuiApplication::inputMethod()->update(Qt::ImEnabled);
}

void qt_text_edit_click_to_cursor(std::uint32_t window_id, double local_x) {
  auto *host = require_host_window(window_id);
  host->text_edit_session().click_to_cursor(local_x);
}

void qt_text_edit_drag_to_cursor(std::uint32_t window_id, double local_x) {
  auto *host = require_host_window(window_id);
  host->text_edit_session().drag_to_cursor(local_x);
}

QtTextMeasurement qt_measure_text(rust::Str text, double font_size, rust::Str font_family, std::int32_t font_weight, bool font_italic, double max_width) {
  QFont font;
  if (font_family.size() > 0) {
    font = QFont(QString::fromUtf8(font_family.data(), static_cast<int>(font_family.size())));
  } else {
    font = QGuiApplication::font();
  }
  font.setStyleStrategy(QFont::PreferOutline);
  font.setPointSizeF(font_size);
  if (font_weight > 0) {
    font.setWeight(static_cast<QFont::Weight>(font_weight));
  }
  if (font_italic) {
    font.setItalic(true);
  }

  QString qtext = QString::fromUtf8(text.data(), static_cast<int>(text.size()));

  QTextLayout layout(qtext, font);
  layout.beginLayout();
  double y_offset = 0.0;
  while (true) {
    QTextLine line = layout.createLine();
    if (!line.isValid()) break;
    if (max_width > 0.0) {
      line.setLineWidth(max_width);
    } else {
      line.setLineWidth(std::numeric_limits<qreal>::max());
    }
    line.setPosition(QPointF(0.0, y_offset));
    y_offset += line.height();
  }
  layout.endLayout();

  QFontMetricsF metrics(font);
  double total_width = 0.0;
  double total_height = 0.0;
  std::int32_t line_count = layout.lineCount();

  for (int i = 0; i < line_count; ++i) {
    QTextLine ln = layout.lineAt(i);
    double w = ln.naturalTextWidth();
    if (w > total_width) total_width = w;
    total_height = ln.position().y() + ln.height();
  }

  if (line_count == 0) {
    total_height = metrics.ascent() + metrics.descent();
  }

  return QtTextMeasurement{
      total_width,
      total_height,
      metrics.ascent(),
      metrics.descent(),
      line_count,
  };
}

void qt_window_minimize(std::uint32_t id) {
  auto *host = require_host_window(id);
  host->showMinimized();
  request_qt_pump();
}

void qt_window_maximize(std::uint32_t id) {
  auto *host = require_host_window(id);
  host->showMaximized();
  request_qt_pump();
}

void qt_window_restore(std::uint32_t id) {
  auto *host = require_host_window(id);
  host->showNormal();
  request_qt_pump();
}

void qt_window_fullscreen(std::uint32_t id, bool enter) {
  auto *host = require_host_window(id);
  if (enter) {
    host->showFullScreen();
  } else {
    host->showNormal();
  }
  request_qt_pump();
}

bool qt_window_is_minimized(std::uint32_t id) {
  auto *host = require_host_window(id);
  return host->isMinimized();
}

bool qt_window_is_maximized(std::uint32_t id) {
  auto *host = require_host_window(id);
  return host->isMaximized();
}

bool qt_window_is_fullscreen(std::uint32_t id) {
  auto *host = require_host_window(id);
  return host->isFullScreen();
}

void qt_window_present_cpu_frame(std::uint32_t node_id,
                                  rust::Slice<const std::uint8_t> pixels,
                                  std::uint32_t width,
                                  std::uint32_t height,
                                  std::uint32_t stride) {
    auto *host = require_host_window(node_id);
    host->present_cpu_frame(pixels.data(),
                            static_cast<int>(width),
                            static_cast<int>(height), static_cast<int>(stride));
}

// ---------------------------------------------------------------------------
// File dialogs
// ---------------------------------------------------------------------------

#include "platform/file_dialogs.cpp"

// ---------------------------------------------------------------------------
// Non-macOS frame signal
// ---------------------------------------------------------------------------

#if !defined(Q_OS_MACOS)
void post_frame_signal_for_node(std::uint32_t node_id) {
  auto *context = QCoreApplication::instance();
  if (context == nullptr) {
    return;
  }
  QMetaObject::invokeMethod(
      context,
      [node_id]() {
        if (!qt_solid::host::qt_host_started_impl()) {
          return;
        }
        auto *widget = g_registry->widget_ptr(node_id);
        auto *host = dynamic_cast<HostWindowWidget *>(widget);
        if (host != nullptr) {
          host->drive_compositor_frame_from_signal();
        }
      },
      Qt::QueuedConnection);
  request_qt_pump();
}
#endif

void qt_macos_set_display_link_frame_rate(std::uint32_t node_id, float fps) {
#if defined(Q_OS_MACOS)
  if (!qt_solid::host::qt_host_started_impl()) {
    return;
  }
  auto *widget = g_registry->widget_ptr(node_id);
  auto *host = dynamic_cast<HostWindowWidget *>(widget);
  if (host) {
    host->set_display_link_frame_rate(fps);
  }
#else
  (void)node_id;
  (void)fps;
#endif
}

} // namespace qt_solid_spike::qt

extern "C" void qt_solid_notify_window_compositor_present_complete(
    std::uint32_t id) {
  qt_solid_spike::qt::notify_window_compositor_present_complete(id);
}

#if !defined(Q_OS_MACOS)
extern "C" void qt_solid_post_frame_signal_for_node(std::uint32_t node_id) {
  qt_solid_spike::qt::post_frame_signal_for_node(node_id);
}
#endif
