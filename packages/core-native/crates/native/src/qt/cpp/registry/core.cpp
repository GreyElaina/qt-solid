#include <cmath>
#include <unordered_set>

#include "qt_taffy_layout.h"

struct WidgetEntry;
class QtRegistry;

struct ContainerOps {
  const char *debug_name = "none";
  void (*insert_child)(WidgetEntry &parent, QWidget *child, int index,
                       const WidgetEntry &child_entry) = nullptr;
  void (*remove_child)(WidgetEntry &parent, QWidget *child) = nullptr;
};

static constexpr std::uint32_t kNoTaffyHandle = UINT32_MAX;

struct ChildLayoutData {
  int flex_grow = 0;
  int flex_shrink = 0;
  int flex_basis = -1; // -1 = auto
  int max_width = -1;  // -1 = none
  int max_height = -1; // -1 = none
  int margin = 0;
  std::uint8_t align_self_tag = static_cast<std::uint8_t>(AlignSelfKind::Auto);
  float aspect_ratio = 0.0f; // 0 = none
  int grid_row = -1;
  int grid_col = -1;
  int grid_row_span = 1;
  int grid_col_span = 1;
  std::uint32_t taffy_child_handle = kNoTaffyHandle;
};

struct WidgetStyleData {
  int min_width = 0;
  int min_height = 0;
};

struct WidgetEntry {
  std::uint32_t node_id = 0;
  WidgetKind kind;
  QWidget *widget = nullptr;
  QLayout *layout = nullptr;

  const ContainerOps *container_ops = nullptr;

  WidgetStyleData style;
  ChildLayoutData child_layout;
  std::uint32_t wired_event_mask = 0;

  WidgetEntry() = default;
  WidgetEntry(const WidgetEntry &) = delete;
  WidgetEntry &operator=(const WidgetEntry &) = delete;
  WidgetEntry(WidgetEntry &&) = default;
  WidgetEntry &operator=(WidgetEntry &&) = default;
};

static const ContainerOps kFlexContainerOps = {
    .debug_name = "TaffyFlexLayout",
    .insert_child =
        [](WidgetEntry &parent, QWidget *child, int index,
           const WidgetEntry &) {
          auto *taffy = static_cast<QTaffyLayout *>(parent.layout);
          if (!taffy) {
            throw_error("TaffyFlexLayout insert_child requires QTaffyLayout");
          }
          taffy->insertWidget(index, child);
        },
    .remove_child =
        [](WidgetEntry &parent, QWidget *child) {
          if (parent.layout) {
            parent.layout->removeWidget(child);
          }
        },
};

struct GridContainerData {
  int row_gap = 0;
  int col_gap = 0;
};

static const ContainerOps kGridContainerOps = {
    .debug_name = "GridLayout",
    .insert_child =
        [](WidgetEntry &parent, QWidget *child, int,
           const WidgetEntry &child_entry) {
          auto *grid = qobject_cast<QGridLayout *>(parent.layout);
          if (!grid) {
            throw_error("GridLayout insert_child requires QGridLayout");
          }
          const auto &cl = child_entry.child_layout;
          const int row = cl.grid_row < 0 ? grid->rowCount() : cl.grid_row;
          const int col = cl.grid_col < 0 ? 0 : cl.grid_col;
          grid->addWidget(child, row, col, cl.grid_row_span, cl.grid_col_span);
        },
    .remove_child =
        [](WidgetEntry &parent, QWidget *child) {
          if (parent.layout) {
            parent.layout->removeWidget(child);
          }
        },
};

struct StackedContainerData {
  int current_index = 0;
};

static const ContainerOps kStackedContainerOps = {
    .debug_name = "StackedLayout",
    .insert_child =
        [](WidgetEntry &parent, QWidget *child, int index,
           const WidgetEntry &) {
          auto *stacked = qobject_cast<QStackedLayout *>(parent.layout);
          if (!stacked) {
            throw_error("StackedLayout insert_child requires QStackedLayout");
          }
          stacked->insertWidget(index, child);
        },
    .remove_child =
        [](WidgetEntry &parent, QWidget *child) {
          if (parent.layout) {
            parent.layout->removeWidget(child);
          }
        },
};

static QWidget *deepest_child_at_excluding(
    QWidget *root, const QPoint &point_in_root,
    const std::unordered_set<const QWidget *> &skipped_roots) {
  if (root == nullptr || !root->isVisible() ||
      skipped_roots.find(root) != skipped_roots.end() ||
      !root->rect().contains(point_in_root)) {
    return nullptr;
  }

  const auto children =
      root->findChildren<QWidget *>(QString(), Qt::FindDirectChildrenOnly);
  for (auto it = children.rbegin(); it != children.rend(); ++it) {
    QWidget *child = *it;
    if (child == nullptr || !child->isVisible()) {
      continue;
    }
    const QPoint point_in_child = child->mapFrom(root, point_in_root);
    if (QWidget *deepest =
            deepest_child_at_excluding(child, point_in_child, skipped_roots)) {
      return deepest;
    }
  }

  return root;
}

class WindowFrameTracker final : public QObject {
public:
  explicit WindowFrameTracker(QWidget *target) : QObject(target), target_(target) {
    target_->installEventFilter(this);
  }

protected:
  bool eventFilter(QObject *watched, QEvent *event) override {
#if !defined(Q_OS_MACOS)
    if (watched == target_ && event != nullptr && event->type() == QEvent::Paint) {
      if (auto *host = dynamic_cast<HostWindowWidget *>(target_->window())) {
        host->tick_frame();
      }
    }
#else
    Q_UNUSED(watched);
    Q_UNUSED(event);
#endif

    return QObject::eventFilter(watched, event);
  }

private:
  QWidget *target_ = nullptr;
};

class MotionMouseEventFilter final : public QObject {
public:
  using MouseEventHandler = std::function<bool(QObject *, QMouseEvent *)>;

  explicit MotionMouseEventFilter(MouseEventHandler handler,
                                  QObject *parent = nullptr)
      : QObject(parent), handler_(std::move(handler)) {}

protected:
  bool eventFilter(QObject *watched, QEvent *event) override {
    switch (event->type()) {
    case QEvent::MouseButtonPress:
    case QEvent::MouseButtonRelease:
    case QEvent::MouseButtonDblClick:
    case QEvent::MouseMove:
      return handler_(watched, static_cast<QMouseEvent *>(event));
    default:
      return QObject::eventFilter(watched, event);
    }
  }

private:
  MouseEventHandler handler_;
};

static void replay_child_taffy_style(WidgetEntry &child) {
  if (child.child_layout.taffy_child_handle == kNoTaffyHandle) return;
  auto *parent = child.widget->parentWidget();
  if (!parent) return;
  auto *taffy = dynamic_cast<QTaffyLayout *>(parent->layout());
  if (!taffy) return;
  auto eid = taffy->engine_id();
  auto node = child.child_layout.taffy_child_handle;
  qt_taffy::engine_set_flex_grow(eid, node, static_cast<float>(child.child_layout.flex_grow));
  qt_taffy::engine_set_flex_shrink(eid, node, static_cast<float>(child.child_layout.flex_shrink));
  if (child.child_layout.flex_basis >= 0) {
    qt_taffy::engine_set_flex_basis_px(eid, node, static_cast<float>(child.child_layout.flex_basis));
  } else {
    qt_taffy::engine_set_flex_basis_auto(eid, node);
  }
  qt_taffy::engine_set_min_width_px(eid, node, static_cast<float>(child.style.min_width));
  qt_taffy::engine_set_min_height_px(eid, node, static_cast<float>(child.style.min_height));
  if (child.child_layout.max_width >= 0) {
    qt_taffy::engine_set_max_width_px(eid, node, static_cast<float>(child.child_layout.max_width));
  }
  if (child.child_layout.max_height >= 0) {
    qt_taffy::engine_set_max_height_px(eid, node, static_cast<float>(child.child_layout.max_height));
  }
  qt_taffy::engine_set_align_self(eid, node, child.child_layout.align_self_tag);
  qt_taffy::engine_set_margin_px(eid, node,
                        static_cast<float>(child.child_layout.margin),
                        static_cast<float>(child.child_layout.margin),
                        static_cast<float>(child.child_layout.margin),
                        static_cast<float>(child.child_layout.margin));
  qt_taffy::engine_set_aspect_ratio(eid, node, child.child_layout.aspect_ratio);
  taffy->invalidate();
}

static void apply_widget_style(WidgetEntry &widget) {
  widget.widget->setMinimumWidth(widget.style.min_width);
  widget.widget->setMinimumHeight(widget.style.min_height);
  replay_child_taffy_style(widget);
}

static void mark_window_scene_dirty(QWidget *widget) {
  if (widget == nullptr) {
    return;
  }

  if (auto *host = dynamic_cast<HostWindowWidget *>(widget->window())) {
    host->mark_compositor_scene_dirty();
    return;
  }

  widget->update();
}

static bool copy_widget_backingstore_into(QWidget *widget,
                                          std::uint32_t width_px,
                                          std::uint32_t height_px,
                                          std::size_t stride,
                                          qreal scale_factor,
                                          QImage *target) {
  const bool trace_enabled =
      qEnvironmentVariableIsSet("QT_SOLID_DEBUG_BACKINGSTORE_CAPTURE");
  if (widget == nullptr || target == nullptr || !widget->isWindow()) {
    if (trace_enabled) {
      qInfo("qt backingstore capture skipped: widget missing/target missing/not window");
    }
    return false;
  }

  QBackingStore *store = widget->backingStore();
  if (store == nullptr) {
    if (trace_enabled) {
      qInfo("qt backingstore capture skipped: no backingstore");
    }
    return false;
  }

  QPlatformBackingStore *platform_store = store->handle();
  if (platform_store == nullptr) {
    if (trace_enabled) {
      qInfo("qt backingstore capture skipped: no platform backingstore");
    }
    return false;
  }

  const QSize expected_size(static_cast<int>(width_px),
                            static_cast<int>(height_px));
  const auto copy_source_image = [&](QImage source) -> bool {
    if (source.isNull()) {
      return false;
    }

    if (source.format() != QImage::Format_ARGB32_Premultiplied) {
      source = source.convertToFormat(QImage::Format_ARGB32_Premultiplied);
    }

    if (source.size() != expected_size) {
      if (trace_enabled) {
        qInfo().noquote()
            << QString::fromLatin1(
                   "qt backingstore capture size mismatch before scale: got=%1x%2 expected=%3x%4")
                   .arg(source.width())
                   .arg(source.height())
                   .arg(expected_size.width())
                   .arg(expected_size.height());
      }
      source = source.scaled(expected_size, Qt::IgnoreAspectRatio,
                             Qt::FastTransformation);
    }
    if (source.size() != expected_size) {
      if (trace_enabled) {
        qInfo("qt backingstore capture skipped: scale to expected size failed");
      }
      return false;
    }

    source.setDevicePixelRatio(scale_factor);
    for (std::uint32_t row = 0; row < height_px; ++row) {
      const auto *src = source.constScanLine(static_cast<int>(row));
      auto *dst = target->scanLine(static_cast<int>(row));
      std::memcpy(dst, src, stride);
    }

    if (trace_enabled) {
      qInfo().noquote()
          << QString::fromLatin1("qt backingstore capture copied image=%1x%2 dpr=%3")
                 .arg(source.width())
                 .arg(source.height())
                 .arg(source.devicePixelRatio());
    }

    return true;
  };

  if (QPlatformGraphicsBuffer *graphics_buffer = platform_store->graphicsBuffer();
      graphics_buffer != nullptr) {
    if (graphics_buffer->lock(QPlatformGraphicsBuffer::SWReadAccess)) {
      const QImage::Format source_format =
          QImage::toImageFormat(graphics_buffer->format());
      if (source_format != QImage::Format_Invalid) {
        const QSize source_size = graphics_buffer->size();
        QImage source(graphics_buffer->data(), source_size.width(),
                      source_size.height(), graphics_buffer->bytesPerLine(),
                      source_format);
        if (graphics_buffer->origin() ==
            QPlatformGraphicsBuffer::OriginBottomLeft) {
          source = source.flipped(Qt::Vertical);
        }
        const bool copied = copy_source_image(source);
        graphics_buffer->unlock();
        if (copied) {
          return true;
        }
      } else {
        graphics_buffer->unlock();
      }
    }

    if (trace_enabled) {
      qInfo("qt backingstore capture graphicsBuffer unavailable/fallback");
    }
  }

  QImage source = platform_store->toImage();
  if (source.isNull()) {
    if (trace_enabled) {
      qInfo("qt backingstore capture skipped: toImage null");
    }
    return false;
  }

  return copy_source_image(source);
}

class QtRegistry {
public:
  void create_widget(std::uint32_t id, std::uint8_t kind_tag) {
    if (entries_.find(id) != entries_.end()) {
      throw_error("qt registry already contains widget id");
    }

    const auto kind = widget_kind_from_tag(kind_tag);
    WidgetEntry entry{.node_id = id, .kind = kind};

    switch (kind) {
#include "qt_widget_create_cases.inc"
    }

    if (auto *callback_host = dynamic_cast<RustWidgetBindingHost *>(entry.widget)) {
      callback_host->bind_rust_widget(id, kind_tag);
    }

#if !defined(Q_OS_MACOS)
    new WindowFrameTracker(entry.widget);
#endif
    apply_widget_style(entry);
    qt_taffy::child_layout_register(id);
    widget_to_id_.emplace(entry.widget, id);
    entries_.emplace(id, std::move(entry));
  }

  void insert_child(std::uint32_t parent_id, std::uint32_t child_id,
                    std::uint32_t anchor_id_or_zero) {
    auto &child = entry(child_id);
    QWidget *child_widget = child.widget;
    detach_widget(child_widget);

    if (parent_id == kRootNodeId) {
      if (!widget_kind_is_top_level(child.kind)) {
        throw_error("root node only accepts top-level widget children");
      }
      child_widget->setParent(nullptr);
      apply_widget_style(child);
      if (child_widget->isVisible()) {
        child_widget->show();
        child_widget->raise();
        if (auto *host = dynamic_cast<HostWindowWidget *>(child_widget)) {
          if (host->window_kind() == 0) {
            child_widget->activateWindow();
          }
        } else {
          child_widget->activateWindow();
        }
      }
      return;
    }

    auto &parent = entry(parent_id);
    if (!parent.container_ops || !parent.container_ops->insert_child) {
      throw_error("parent node does not support child insertion");
    }

    const int insert_at =
        anchor_id_or_zero == 0
            ? (parent.layout ? parent.layout->count() : 0)
            : find_layout_index(parent.layout, entry(anchor_id_or_zero).widget);
    if (insert_at < 0) {
      throw_error("anchor widget is not attached to parent layout");
    }

    parent.container_ops->insert_child(parent, child_widget, insert_at, child);

    if (auto *taffy = dynamic_cast<QTaffyLayout *>(parent.layout)) {
      int child_index = taffy->indexOf(child_widget);
      if (child_index >= 0) {
        child.child_layout.taffy_child_handle = taffy->child_node(child_index);
        qt_taffy::child_layout_set_taffy_handle(child_id, taffy->child_node(child_index), taffy->engine_id());
      }
    }

    apply_widget_style(child);
    child_widget->show();
    mark_window_scene_dirty(parent.widget);
  }

  void remove_child(std::uint32_t parent_id, std::uint32_t child_id) {
    auto &child = entry(child_id);
    child.child_layout.taffy_child_handle = kNoTaffyHandle;
    qt_taffy::child_layout_clear_taffy_handle(child_id);

    if (parent_id == kRootNodeId) {
      child.widget->hide();
      child.widget->setParent(nullptr);
      return;
    }

    auto &parent = entry(parent_id);
    if (parent.container_ops && parent.container_ops->remove_child) {
      parent.container_ops->remove_child(parent, child.widget);
    } else if (parent.layout) {
      parent.layout->removeWidget(child.widget);
    }
    child.widget->hide();
    child.widget->setParent(nullptr);
    mark_window_scene_dirty(parent.widget);
  }

  void destroy_widget(std::uint32_t id,
                      ::rust::Slice<const std::uint32_t> subtree_ids) {
    auto it = entries_.find(id);
    if (it == entries_.end()) {
      return;
    }

    QWidget *widget = it->second.widget;
    if (highlighted_widget_ && (highlighted_widget_ == widget ||
                                is_descendant(highlighted_widget_, widget))) {
      clear_highlight();
    }

    auto *owning_host = dynamic_cast<HostWindowWidget *>(widget->window());
    const bool defer_top_level_delete = widget->parentWidget() == nullptr;

    if (!defer_top_level_delete) {
      detach_widget(widget);
    }

    for (const auto remove_id : subtree_ids) {
      auto eit = entries_.find(remove_id);
      if (eit != entries_.end()) {
        widget_to_id_.erase(eit->second.widget);
        entries_.erase(eit);
      }
      qt_taffy::child_layout_unregister(remove_id);
    }

    if (defer_top_level_delete) {
      prune_pending_top_level_deletes();
      const auto pending_it =
          std::find(pending_top_level_deletes_.begin(),
                    pending_top_level_deletes_.end(), widget);
      if (pending_it == pending_top_level_deletes_.end()) {
        pending_top_level_deletes_.push_back(widget);
      }

      if (auto *context = QCoreApplication::instance()) {
        QPointer<QWidget> deferred_widget = widget;
        QTimer::singleShot(0, context, [deferred_widget]() {
          if (!deferred_widget) {
            return;
          }
          deferred_widget->hide();
          deferred_widget->deleteLater();
        });
      }
      return;
    }

    if (owning_host != nullptr) {
      owning_host->mark_compositor_scene_dirty();
    }
    widget->deleteLater();
  }

  void request_repaint(std::uint32_t id) {
    auto &widget = entry(id);
    widget.widget->update();
  }

  bool request_window_compositor_frame(std::uint32_t id) {
    auto &widget = entry(id);
    auto *host = dynamic_cast<HostWindowWidget *>(widget.widget);
    if (host == nullptr) {
      return false;
    }
    return host->request_compositor_frame();
  }

  void notify_window_compositor_frame_complete(std::uint32_t id) {
    auto it = entries_.find(id);
    if (it == entries_.end()) {
      return;
    }
    auto *host = dynamic_cast<HostWindowWidget *>(it->second.widget);
    if (host == nullptr) {
      return;
    }
    host->notify_compositor_frame_complete();
  }

  QtWidgetCaptureLayout capture_widget_layout(std::uint32_t id) {
    auto &widget_entry = entry(id);
    auto *widget = widget_entry.widget;
    const bool rgba_capture = false;
    const qreal scale_factor = widget->windowHandle() != nullptr
                                   ? widget->windowHandle()->devicePixelRatio()
                                   : widget->devicePixelRatioF();
    const QSize logical_size = widget->size();
    const auto width_px = static_cast<std::uint32_t>(
        std::max(0, qRound(static_cast<qreal>(logical_size.width()) * scale_factor)));
    const auto height_px = static_cast<std::uint32_t>(
        std::max(0, qRound(static_cast<qreal>(logical_size.height()) * scale_factor)));
    const auto stride = static_cast<std::size_t>(width_px) * 4;

    return QtWidgetCaptureLayout{
        .format_tag = static_cast<std::uint8_t>(rgba_capture ? 2 : 1),
        .width_px = width_px,
        .height_px = height_px,
        .stride = stride,
        .scale_factor = scale_factor,
    };
  }

  ::rust::Vec<QtRect> capture_widget_visible_rects(std::uint32_t id) {
    auto &widget_entry = entry(id);
    auto *widget = widget_entry.widget;
    ::rust::Vec<QtRect> rects;
    const auto region = widget->visibleRegion();
    for (auto it = region.begin(); it != region.end(); ++it) {
      const QRect rect = *it;
      if (!rect.isValid() || rect.width() <= 0 || rect.height() <= 0) {
        continue;
      }

      rects.push_back(QtRect{
          .x = rect.x(),
          .y = rect.y(),
          .width = rect.width(),
          .height = rect.height(),
      });
    }
    return rects;
  }

  void set_widget_mouse_transparent(std::uint32_t id, bool transparent) {
    auto &widget = entry(id);
    widget.widget->setAttribute(Qt::WA_TransparentForMouseEvents, transparent);
  }

  void capture_widget_into(std::uint32_t id, std::uint32_t width_px,
                           std::uint32_t height_px, std::size_t stride,
                           bool include_children,
                           ::rust::Slice<std::uint8_t> bytes) {
    auto &widget_entry = entry(id);
    auto *widget = widget_entry.widget;
    const auto layout = capture_widget_layout(id);

    if (layout.width_px != width_px || layout.height_px != height_px ||
        layout.stride != stride) {
      throw_error("widget capture layout changed between prepare and render");
    }

    const auto required_len = stride * static_cast<std::size_t>(height_px);
    if (bytes.size() < required_len) {
      throw_error("widget capture target buffer is smaller than required");
    }

    if (width_px == 0 || height_px == 0) {
      return;
    }

    auto *raw = reinterpret_cast<uchar *>(bytes.data());
    QImage image(raw, static_cast<int>(width_px), static_cast<int>(height_px),
                 static_cast<qsizetype>(stride),
                 QImage::Format_ARGB32_Premultiplied);
    image.setDevicePixelRatio(layout.scale_factor);
    image.fill(Qt::transparent);

    if (auto *window = widget->window()) {
      window->ensurePolished();
    }
    widget->ensurePolished();
    QCoreApplication::sendPostedEvents(nullptr, QEvent::PolishRequest);

    QWidget::RenderFlags flags = QWidget::DrawWindowBackground;
    if (include_children) {
      flags |= QWidget::DrawChildren;
    }

    if (include_children &&
        copy_widget_backingstore_into(widget, width_px, height_px, stride,
                                      layout.scale_factor, &image)) {
      return;
    }

    widget->render(&image, QPoint(), QRegion(), flags);
  }

  void capture_widget_region_into(std::uint32_t id, std::uint32_t width_px,
                                  std::uint32_t height_px, std::size_t stride,
                                  bool include_children, QtRect rect,
                                  ::rust::Slice<std::uint8_t> bytes) {
    auto &widget_entry = entry(id);
    auto *widget = widget_entry.widget;
    const auto layout = capture_widget_layout(id);

    if (layout.width_px != width_px || layout.height_px != height_px ||
        layout.stride != stride) {
      throw_error("widget capture layout changed between prepare and render");
    }

    const auto required_len = stride * static_cast<std::size_t>(height_px);
    if (bytes.size() < required_len) {
      throw_error("widget capture target buffer is smaller than required");
    }

    if (width_px == 0 || height_px == 0 || rect.width <= 0 || rect.height <= 0) {
      return;
    }

    auto *raw = reinterpret_cast<uchar *>(bytes.data());
    QImage image(raw, static_cast<int>(width_px), static_cast<int>(height_px),
                 static_cast<qsizetype>(stride),
                 QImage::Format_ARGB32_Premultiplied);
    image.setDevicePixelRatio(layout.scale_factor);

    if (auto *window = widget->window()) {
      window->ensurePolished();
    }
    widget->ensurePolished();
    QCoreApplication::sendPostedEvents(nullptr, QEvent::PolishRequest);

    QWidget::RenderFlags flags = QWidget::DrawWindowBackground;
    if (include_children) {
      flags |= QWidget::DrawChildren;
    }

    if (include_children &&
        copy_widget_backingstore_into(widget, width_px, height_px, stride,
                                      layout.scale_factor, &image)) {
      return;
    }

    widget->render(&image, QPoint(),
                   QRegion(QRect(rect.x, rect.y, rect.width, rect.height)),
                   flags);
  }
//
//  void set_font_family(std::uint32_t id, rust::Str value) {
//    auto &widget = entry(id);
//    QFont font = widget.widget->font();
//    const QString family = to_qstring(value);
//    if (font.family() == family) {
//      return;
//    }
//    font.setFamily(family);
//    widget.widget->setFont(font);
//  }
//
//  void set_font_point_size(std::uint32_t id, double value) {
//    auto &widget = entry(id);
//    QFont font = widget.widget->font();
//    const double point_size = std::max(1.0, value < 0.0 ? 0.0 : value);
//    if (font.pointSizeF() == point_size) {
//      return;
//    }
//    font.setPointSizeF(point_size);
//    widget.widget->setFont(font);
//  }
//
//  void set_font_weight(std::uint32_t id, std::int32_t value) {
//    auto &widget = entry(id);
//    QFont font = widget.widget->font();
//    const int weight = std::min(1000, clamp_non_negative(value));
//    if (font.weight() == weight) {
//      return;
//    }
//    font.setWeight(static_cast<QFont::Weight>(weight));
//    widget.widget->setFont(font);
//  }
//
//  void set_font_italic(std::uint32_t id, bool value) {
//    auto &widget = entry(id);
//    QFont font = widget.widget->font();
//    if (font.italic() == value) {
//      return;
//    }
//    font.setItalic(value);
//    widget.widget->setFont(font);
//  }

  void set_focus_policy(std::uint32_t id, std::uint8_t focus_policy_tag) {
    auto &widget = entry(id);
    widget.widget->setFocusPolicy(focus_policy_from_tag(focus_policy_tag));
  }

  void set_auto_focus(std::uint32_t id, bool value) {
    if (!value) {
      return;
    }

    auto &widget = entry(id);
    if (QWidget *window = widget.widget->window()) {
      window->raise();
      window->activateWindow();
    }
    widget.widget->setFocus(Qt::OtherFocusReason);
    if (auto *context = QCoreApplication::instance()) {
      QPointer<QWidget> deferred_widget = widget.widget;
      QTimer::singleShot(0, context, [deferred_widget]() {
        if (deferred_widget != nullptr) {
          deferred_widget->setFocus(Qt::OtherFocusReason);
        }
      });
      const int repaint_delay_ms = std::max(200, QApplication::cursorFlashTime() / 2);
      QTimer::singleShot(repaint_delay_ms, context, [deferred_widget]() {
        if (auto *line_edit = qobject_cast<QLineEdit *>(deferred_widget.data())) {
          if (line_edit->hasFocus()) {
            line_edit->update();
          }
        }
      });
    }
  }

  void debug_click_node(std::uint32_t id) {
    auto &widget = entry(id);
    if (auto *button = qobject_cast<QAbstractButton *>(widget.widget)) {
      button->click();
      return;
    }

    throw_error("debug_click_node requires a Qt abstract button widget");
  }

  void debug_close_node(std::uint32_t id) {
    auto &widget = entry(id);
    if (auto *window = dynamic_cast<HostWindowWidget *>(widget.widget)) {
      window->close();
      return;
    }

    throw_error("debug_close_node requires a host window widget");
  }

  void debug_input_insert_text(std::uint32_t id, ::rust::Str value) {
    auto &widget = entry(id);
    if (auto *input = qobject_cast<QLineEdit *>(widget.widget)) {
      input->insert(to_qstring(value));
      return;
    }

    throw_error("debug_input_insert_text requires a Qt line edit widget");
  }

  void debug_highlight_node(std::uint32_t id) {
    auto &widget = entry(id);
    set_highlighted_widget(widget.widget);
  }

  QtNodeBounds debug_node_bounds(std::uint32_t id) {
    auto &widget = entry(id);
    return bounds_for_widget(widget.widget);
  }

  QtScreenGeometry get_screen_geometry(std::uint32_t id) {
    auto &widget = entry(id);
    QScreen *screen = widget.widget->screen();
    if (!screen) {
      screen = QGuiApplication::primaryScreen();
    }
    if (!screen) {
      return QtScreenGeometry{0, 0, 0, 0};
    }
    QRect avail = screen->availableGeometry();
    return QtScreenGeometry{avail.x(), avail.y(), avail.width(), avail.height()};
  }

  void focus_widget(std::uint32_t id) {
    auto &widget = entry(id);
    if (widget.widget) {
      widget.widget->setFocus(Qt::OtherFocusReason);
    }
  }

  QtScreenGeometry get_widget_size_hint(std::uint32_t id) {
    auto &widget = entry(id);
    if (!widget.widget) {
      return QtScreenGeometry{0, 0, 0, 0};
    }
    QSize hint = widget.widget->sizeHint();
    if (!hint.isValid()) {
      hint = widget.widget->size();
    }
    return QtScreenGeometry{0, 0, hint.width(), hint.height()};
  }

  std::uint32_t debug_node_at_point(std::int32_t screen_x,
                                    std::int32_t screen_y) {
    return widget_id_at_point(QPoint(screen_x, screen_y));
  }

  void debug_set_inspect_mode(bool enabled) {
    inspect_mode_enabled_ = enabled;
    inspect_press_active_ = false;

    if (!enabled) {
      if (inspect_poll_timer_) {
        inspect_poll_timer_->stop();
      }
      clear_highlight();
      return;
    }

    auto *app = qobject_cast<QApplication *>(QCoreApplication::instance());
    if (!app) {
      throw_error("debug_set_inspect_mode requires a QApplication instance");
    }

    if (!inspect_event_filter_) {
      inspect_event_filter_ = new InspectModeEventFilter(
          [this](QMouseEvent *event) {
            return handle_inspect_mouse_event(event);
          },
          app);
      app->installEventFilter(inspect_event_filter_);
    }

    if (!inspect_poll_timer_) {
      inspect_poll_timer_ = new QTimer(app);
      inspect_poll_timer_->setInterval(16);
      QObject::connect(inspect_poll_timer_, &QTimer::timeout, app,
                       [this]() { update_inspect_hover(); });
    }

    inspect_poll_timer_->start();
    update_inspect_hover();
  }

  void install_motion_mouse_filter(QApplication *app) {
    if (motion_mouse_event_filter_ || app == nullptr) {
      return;
    }

    motion_mouse_event_filter_ = new MotionMouseEventFilter(
        [this](QObject *watched, QMouseEvent *event) {
          return handle_motion_mouse_event(watched, event);
        },
        app);
    app->installEventFilter(motion_mouse_event_filter_);
  }

  void debug_clear_highlight() { clear_highlight(); }

  QtRealizedNodeState debug_node_state(std::uint32_t id) {
    auto &widget = entry(id);
    QtRealizedNodeState state{};

    state.has_title = true;
    if (auto *group = qobject_cast<QGroupBox *>(widget.widget)) {
      state.title = to_rust_string(group->title());
    } else {
      state.title = to_rust_string(widget.widget->windowTitle());
    }

    state.has_width = true;
    state.width = widget.widget->width();
    state.has_height = true;
    state.height = widget.widget->height();
    state.has_min_width = true;
    state.min_width = widget.widget->minimumWidth();
    state.has_min_height = true;
    state.min_height = widget.widget->minimumHeight();
    state.has_flex_grow = true;
    state.flex_grow = widget.child_layout.flex_grow;
    state.has_flex_shrink = true;
    state.flex_shrink = widget.child_layout.flex_shrink;
    state.has_enabled = true;
    state.enabled = widget.widget->isEnabled();

    if (auto *label = qobject_cast<QLabel *>(widget.widget)) {
      state.has_text = true;
      state.text = to_rust_string(label->text());
    } else if (auto *button = qobject_cast<QPushButton *>(widget.widget)) {
      state.has_text = true;
      state.text = to_rust_string(button->text());
    } else if (auto *input = qobject_cast<QLineEdit *>(widget.widget)) {
      state.has_text = true;
      state.text = to_rust_string(input->text());
      state.has_placeholder = true;
      state.placeholder = to_rust_string(input->placeholderText());
    } else if (auto *check = qobject_cast<QCheckBox *>(widget.widget)) {
      state.has_text = true;
      state.text = to_rust_string(check->text());
      state.has_checked = true;
      state.checked = check->isChecked();
    } else if (auto *slider = qobject_cast<QSlider *>(widget.widget)) {
      state.has_value = true;
      state.value = static_cast<double>(slider->value());
    } else if (auto *spin = qobject_cast<QDoubleSpinBox *>(widget.widget)) {
      state.has_value = true;
      state.value = spin->value();
    }

    return state;
  }

  void clear() {
    inspect_mode_enabled_ = false;
    inspect_press_active_ = false;
    clear_active_motion_mouse_target();
    if (inspect_poll_timer_) {
      inspect_poll_timer_->stop();
    }
    clear_highlight();

    // Avoid close()/closeAllWindows() on macOS here. In embedded Node + Qt,
    // explicit top-level close can crash in
    // QCocoaEventDispatcherPrivate::endModalSession. Hiding and draining
    // DeferredDelete has been stable under stress.
    std::vector<QWidget *> delete_roots;
    delete_roots.reserve(entries_.size() + pending_top_level_deletes_.size());

    for (const auto &widget : pending_top_level_deletes_) {
      if (widget) {
        delete_roots.push_back(widget);
      }
    }
    pending_top_level_deletes_.clear();

    for (const auto &[id, entry] : entries_) {
      (void)id;
      QWidget *widget = entry.widget;
      if (!widget) {
        continue;
      }
      if (widget->parentWidget() == nullptr) {
        delete_roots.push_back(widget);
      }
    }

    entries_.clear();

    if (highlight_overlay_) {
      highlight_overlay_->deleteLater();
      highlight_overlay_.clear();
    }

    for (auto *widget : delete_roots) {
      widget->hide();
      widget->deleteLater();
    }
  }

  void set_window_transient_owner(std::uint32_t window_id, std::uint32_t owner_id) {
    auto &window_entry = entry(window_id);
    auto *window_host = dynamic_cast<HostWindowWidget *>(window_entry.widget);
    if (!window_host) {
      throw_error("set_window_transient_owner: target is not a window widget");
    }

    QWidget *owner_widget = nullptr;
    if (owner_id != 0) {
      auto &owner_entry = entry(owner_id);
      owner_widget = owner_entry.widget;
      if (owner_widget) {
        owner_widget = owner_widget->window();
      }
    }

    if (owner_widget && owner_widget->windowHandle() && window_host->windowHandle()) {
      window_host->windowHandle()->setTransientParent(owner_widget->windowHandle());
    }
  }

  QWidget *widget_ptr(std::uint32_t id) {
    return entry(id).widget;
  }

private:
  WidgetEntry &entry(std::uint32_t id) {
    auto it = entries_.find(id);
    if (it == entries_.end()) {
      throw_error("qt registry does not contain requested widget id");
    }
    return it->second;
  }

  WidgetEntry &layout_widget_entry(std::uint32_t id,
                                   const char *error_message) {
    auto &widget = entry(id);
    if (!widget.layout) {
      throw_error(error_message);
    }
    return widget;
  }

  std::uint32_t widget_id_for_widget(QWidget *widget) const {
    for (auto *current = widget; current != nullptr;
         current = current->parentWidget()) {
      auto it = widget_to_id_.find(current);
      if (it != widget_to_id_.end()) {
        return it->second;
      }
    }

    return 0;
  }

  std::uint32_t widget_id_at_point(const QPoint &global_pos) const {
    return widget_id_for_widget(QApplication::widgetAt(global_pos));
  }

  void clear_active_motion_mouse_target() {
    active_motion_target_widget_.clear();
    active_motion_root_id_ = 0;
    active_motion_window_id_ = 0;
  }

  std::uint32_t motion_root_id_for_widget(
      QWidget *widget, const ::rust::Vec<std::uint32_t> &motion_root_ids) const {
    if (widget == nullptr || motion_root_ids.empty()) {
      return 0;
    }

    const std::unordered_set<std::uint32_t> motion_root_set(motion_root_ids.begin(),
                                                            motion_root_ids.end());
    for (QWidget *current = widget; current != nullptr;
         current = current->parentWidget()) {
      const std::uint32_t node_id = widget_id_for_widget(current);
      if (node_id != 0 && motion_root_set.find(node_id) != motion_root_set.end()) {
        return node_id;
      }
    }

    return 0;
  }

  bool dispatch_mouse_event_to_widget(QWidget *target_widget,
                                      const QPointF &target_local_pos,
                                      QMouseEvent *event) {
    if (target_widget == nullptr || event == nullptr) {
      return false;
    }

    QWidget *target_window = target_widget->window();
    const QPoint global_pos = event->globalPosition().toPoint();
    const QPointF window_pos =
        target_window != nullptr
            ? QPointF(target_window->mapFromGlobal(global_pos))
            : QPointF(global_pos);
    QMouseEvent cloned(event->type(), target_local_pos, window_pos,
                       event->globalPosition(), event->button(), event->buttons(),
                       event->modifiers(), event->pointingDevice());

    dispatching_motion_mouse_event_ = true;
    QCoreApplication::sendEvent(target_widget, &cloned);
    dispatching_motion_mouse_event_ = false;
    return true;
  }

  bool dispatch_motion_mouse_event(
      const qt_solid_spike::qt::QtMotionMouseTarget &target, QMouseEvent *event,
      QWidget *preferred_target_widget, bool keep_active_capture) {
    auto it = entries_.find(target.root_node_id);
    if (it == entries_.end() || it->second.widget == nullptr || event == nullptr) {
      return false;
    }

    QWidget *root_widget = it->second.widget;
    QWidget *target_widget = nullptr;
    const QPoint root_local_floor(static_cast<int>(std::floor(target.local_x)),
                                  static_cast<int>(std::floor(target.local_y)));
    if (preferred_target_widget != nullptr &&
        (preferred_target_widget == root_widget ||
         root_widget->isAncestorOf(preferred_target_widget))) {
      target_widget = preferred_target_widget;
    } else {
      target_widget = deepest_child_at(root_widget, root_local_floor);
    }
    if (target_widget == nullptr) {
      target_widget = root_widget;
    }

    const QPoint target_local_floor =
        target_widget->mapFrom(root_widget, root_local_floor);
    const QPointF target_local(
        static_cast<double>(target_local_floor.x()) +
            (target.local_x - std::floor(target.local_x)),
        static_cast<double>(target_local_floor.y()) +
            (target.local_y - std::floor(target.local_y)));
    const bool dispatched =
        dispatch_mouse_event_to_widget(target_widget, target_local, event);
    if (!dispatched) {
      return false;
    }

    if (keep_active_capture) {
      active_motion_target_widget_ = target_widget;
      active_motion_root_id_ = target.root_node_id;
      active_motion_window_id_ = widget_id_for_widget(root_widget->window());
    } else if (event->type() == QEvent::MouseButtonRelease) {
      clear_active_motion_mouse_target();
    }

    return true;
  }

  bool dispatch_underlying_mouse_event(
      QWidget *window_widget, const ::rust::Vec<std::uint32_t> &motion_root_ids,
      QMouseEvent *event) {
    if (window_widget == nullptr || event == nullptr) {
      return false;
    }

    std::unordered_set<const QWidget *> skipped_roots;
    skipped_roots.reserve(motion_root_ids.size());
    for (std::uint32_t root_id : motion_root_ids) {
      auto it = entries_.find(root_id);
      if (it != entries_.end() && it->second.widget != nullptr) {
        skipped_roots.insert(it->second.widget);
      }
    }

    const QPoint window_point =
        window_widget->mapFromGlobal(event->globalPosition().toPoint());
    QWidget *target_widget =
        deepest_child_at_excluding(window_widget, window_point, skipped_roots);
    if (target_widget == nullptr) {
      return false;
    }

    return dispatch_mouse_event_to_widget(target_widget,
                                          QPointF(target_widget->mapFromGlobal(
                                              event->globalPosition().toPoint())),
                                          event);
  }

  void update_inspect_hover() {
    if (!inspect_mode_enabled_) {
      return;
    }

    const std::uint32_t node_id = widget_id_at_point(QCursor::pos());
    if (node_id == 0) {
      clear_highlight();
      return;
    }

    auto it = entries_.find(node_id);
    if (it == entries_.end()) {
      clear_highlight();
      return;
    }

    set_highlighted_widget(it->second.widget);
  }

  bool handle_inspect_mouse_event(QMouseEvent *event) {
    if (!inspect_mode_enabled_ || !event) {
      return false;
    }

    switch (event->type()) {
    case QEvent::MouseButtonPress:
    case QEvent::MouseButtonDblClick: {
      if (event->button() != Qt::LeftButton) {
        return true;
      }

      inspect_press_active_ = true;
      const QPoint global_pos = event->globalPosition().toPoint();
      const std::uint32_t node_id = widget_id_at_point(global_pos);
      if (node_id != 0) {
        emit_inspect_event(node_id);
        debug_set_inspect_mode(false);
      }
      return true;
    }
    case QEvent::MouseButtonRelease:
      if (inspect_press_active_) {
        inspect_press_active_ = false;
        return true;
      }
      return false;
    default:
      return false;
    }
  }

  bool handle_motion_mouse_event(QObject *watched, QMouseEvent *event) {
    if (dispatching_motion_mouse_event_ || event == nullptr) {
      return false;
    }

    auto *watched_widget = qobject_cast<QWidget *>(watched);
    const QPoint global_pos = event->globalPosition().toPoint();
    if (active_motion_target_widget_ != nullptr && active_motion_root_id_ != 0 &&
        (event->type() == QEvent::MouseMove ||
         event->type() == QEvent::MouseButtonRelease)) {
      try {
        const auto mapped = qt_solid_spike::qt::qt_window_motion_map_point_to_root(
            active_motion_window_id_, active_motion_root_id_, global_pos.x(),
            global_pos.y());
        if (mapped.found) {
          return dispatch_motion_mouse_event(
              mapped, event, active_motion_target_widget_,
              event->type() != QEvent::MouseButtonRelease);
        }
      } catch (const rust::Error &error) {
        qWarning() << "motion mouse map failed:" << error.what();
      }
      clear_active_motion_mouse_target();
    }

    if (watched_widget == nullptr) {
      return false;
    }

    QWidget *window_widget = watched_widget->window();
    if (window_widget == nullptr) {
      return false;
    }
    const std::uint32_t window_id = widget_id_for_widget(window_widget);
    if (window_id == 0) {
      return false;
    }

    try {
      const auto hit = qt_solid_spike::qt::qt_window_motion_hit_test(
          window_id, global_pos.x(), global_pos.y());
      if (hit.found) {
        return dispatch_motion_mouse_event(
            hit, event, nullptr,
            event->type() == QEvent::MouseButtonPress ||
                event->type() == QEvent::MouseButtonDblClick);
      }

      const auto motion_root_ids =
          qt_solid_spike::qt::qt_window_motion_hit_root_ids(window_id);
      if (motion_root_ids.empty()) {
        return false;
      }

      if (motion_root_id_for_widget(watched_widget, motion_root_ids) == 0) {
        return false;
      }

      if (dispatch_underlying_mouse_event(window_widget, motion_root_ids, event)) {
        return true;
      }

      return true;
    } catch (const rust::Error &error) {
      qWarning() << "motion mouse dispatch failed:" << error.what();
      return false;
    }
  }
  static bool is_descendant(QWidget *candidate, QWidget *ancestor) {
    for (auto *parent = candidate->parentWidget(); parent != nullptr;
         parent = parent->parentWidget()) {
      if (parent == ancestor) {
        return true;
      }
    }
    return false;
  }

  static int find_layout_index(QLayout *layout, QWidget *widget) {
    for (int index = 0; index < layout->count(); ++index) {
      if (auto *item = layout->itemAt(index);
          item && item->widget() == widget) {
        return index;
      }
    }
    return -1;
  }

  static void detach_widget(QWidget *widget) {
    if (auto *parent = widget->parentWidget()) {
      if (auto *layout = parent->layout()) {
        layout->removeWidget(widget);
      }
      widget->hide();
      widget->setParent(nullptr);
      return;
    }

    widget->hide();
    widget->setParent(nullptr);
  }

  void prune_pending_top_level_deletes() {
    pending_top_level_deletes_.erase(
        std::remove_if(pending_top_level_deletes_.begin(),
                       pending_top_level_deletes_.end(),
                       [](const QPointer<QWidget> &widget) { return !widget; }),
        pending_top_level_deletes_.end());
  }

  DebugHighlightOverlay *ensure_highlight_overlay() {
    if (!highlight_overlay_) {
      highlight_overlay_ = new DebugHighlightOverlay();
    }

    return highlight_overlay_;
  }

  void set_highlighted_widget(QWidget *widget) {
    if (!widget) {
      clear_highlight();
      return;
    }

    highlighted_widget_ = widget;
    ensure_highlight_overlay()->highlight_widget(widget);
  }

  void clear_highlight() {
    if (highlight_overlay_) {
      highlight_overlay_->clear_highlight();
    }
    highlighted_widget_.clear();
  }

  std::unordered_map<std::uint32_t, WidgetEntry> entries_;
  std::unordered_map<const QWidget *, std::uint32_t> widget_to_id_;
  std::vector<QPointer<QWidget>> pending_top_level_deletes_;
  QPointer<QWidget> highlighted_widget_;
  QPointer<DebugHighlightOverlay> highlight_overlay_;
  QPointer<QTimer> inspect_poll_timer_;
  QPointer<InspectModeEventFilter> inspect_event_filter_;
  QPointer<MotionMouseEventFilter> motion_mouse_event_filter_;
  QPointer<QWidget> active_motion_target_widget_;
  std::uint32_t active_motion_root_id_ = 0;
  std::uint32_t active_motion_window_id_ = 0;
  bool dispatching_motion_mouse_event_ = false;
  bool inspect_mode_enabled_ = false;
  bool inspect_press_active_ = false;
};

class QtHostState {
public:
  explicit QtHostState(uv_loop_t *loop) : loop_(loop) {}

  bool started() const { return started_; }

  void start() {
    if (started_) {
      return;
    }

    qt_wgpu_renderer::register_static_platform_plugins();
    qt_wgpu_renderer::configure_unified_compositor_platform();

    argv_storage_ = "qt-solid-spike";
    argv_[0] = argv_storage_.data();
    argv_[1] = nullptr;

    if (!app_) {
      if (QCoreApplication::instance() != nullptr) {
        throw_error(
            "QCoreApplication already exists before qt-solid host startup");
      }

      app_ = std::make_unique<QApplication>(argc_, argv_);
      app_->setApplicationName(QStringLiteral("qt-solid-spike"));
      app_->setQuitOnLastWindowClosed(false);
      qt_wgpu_renderer::sync_unified_compositor_active_state();
      QObject::connect(app_.get(), &QGuiApplication::applicationStateChanged,
                       app_.get(), [](Qt::ApplicationState state) {
                         if (state == Qt::ApplicationActive) {
                           qt_solid_spike::qt::emit_app_event(
                               ::rust::Str("activate"));
                         }
                       });
#if defined(__APPLE__)
      wait_bridge_ = std::make_unique<MacosEventBufferBridge>();
      const auto dispatcher_probe = probe_cocoa_dispatcher_private_prefix();
      if (dispatcher_probe.dispatcher_private == nullptr) {
        throw_error(dispatcher_probe.error_message);
      }
#endif
    } else {
      throw_error("Qt host cannot restart in the same process yet");
    }

    try {
      pump_ = std::make_unique<LibuvQtPump>(loop_);
      pump_->start();
      pump_->pump_events();
      registry_.install_motion_mouse_filter(app_.get());
      started_ = true;
    } catch (...) {
      if (pump_) {
        if (pump_->close_async()) {
          pump_.release();
        } else {
          pump_.reset();
        }
      }
#if defined(__APPLE__)
      wait_bridge_.reset();
#endif
      app_.reset();
      throw;
    }
  }

  void request_pump() {
    if (!started_ || !pump_) {
      return;
    }

    pump_->request_pump();
  }
  void shutdown() {
    if (!started_) {
      return;
    }

    started_ = false;

    if (pump_) {
      pump_->request_shutdown([this]() {
        execute_teardown();
      });
    } else {
      execute_teardown();
    }
  }

  QtRegistry &registry() {
    if (!started_) {
      throw_error("Qt host is not started");
    }
    return registry_;
  }

  int runtime_wait_bridge_unix_fd() const noexcept {
#if defined(__APPLE__)
    if (wait_bridge_) {
      return wait_bridge_->read_fd();
    }
#endif
    return -1;
  }

  void drain_runtime_wait_bridge() noexcept {
#if defined(__APPLE__)
    if (wait_bridge_) {
      wait_bridge_->drain();
    }
#endif
  }

  std::optional<std::uint64_t> runtime_wait_bridge_timer_delay_ms()
      noexcept {
#if defined(__APPLE__)
    if (!app_) {
      return std::nullopt;
    }

    const auto dispatcher_probe = probe_cocoa_dispatcher_private_prefix();
    if (dispatcher_probe.dispatcher_private == nullptr) {
      return std::nullopt;
    }

    if (auto delay = dispatcher_probe.dispatcher_private->timerInfoList.timerWait()) {
      const auto delay_ms = std::chrono::duration_cast<std::chrono::milliseconds>(
          *delay);
      return delay_ms.count() > 0
                 ? static_cast<std::uint64_t>(delay_ms.count())
                 : std::uint64_t{0};
    }

    return std::nullopt;
#else
    return std::nullopt;
#endif
  }

private:
  void execute_teardown() {
#if defined(__APPLE__)
    wait_bridge_.reset();
#endif
    registry_.clear();
    if (QCoreApplication::instance() != nullptr) {
      for (int index = 0; index < 4; ++index) {
        QCoreApplication::sendPostedEvents(nullptr, QEvent::DeferredDelete);
        QCoreApplication::processEvents(QEventLoop::AllEvents);
        QCoreApplication::sendPostedEvents(nullptr);
      }
    }
    app_.reset();
  }

  uv_loop_t *loop_ = nullptr;
  int argc_ = 1;
  std::string argv_storage_;
  char *argv_[2] = {nullptr, nullptr};
  std::unique_ptr<QApplication> app_;
  std::unique_ptr<LibuvQtPump> pump_;
#if defined(__APPLE__)
  std::unique_ptr<MacosEventBufferBridge> wait_bridge_;
#endif
  QtRegistry registry_;
  bool started_ = false;
};

QtHostState *g_host = nullptr;

void request_qt_pump() {
  if (!g_host || !g_host->started()) {
    return;
  }

  record_request_qt_pump();
  g_host->request_pump();
}
int current_runtime_wait_bridge_unix_fd() noexcept {
  if (!g_host) {
    return -1;
  }

  return g_host->runtime_wait_bridge_unix_fd();
}

void drain_runtime_wait_bridge_notifications() noexcept {
  if (!g_host) {
    return;
  }

  g_host->drain_runtime_wait_bridge();
}

#if !defined(__APPLE__)
std::optional<std::uint64_t> current_runtime_wait_bridge_timer_delay_ms()
    noexcept {
  if (!g_host) {
    return std::nullopt;
  }

  return g_host->runtime_wait_bridge_timer_delay_ms();
}
#endif