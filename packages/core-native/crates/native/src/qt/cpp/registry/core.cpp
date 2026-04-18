struct WidgetEntry {
  WidgetKind kind;
  QWidget *widget = nullptr;
  QLayout *layout = nullptr;
  FlexDirectionKind flex_direction = FlexDirectionKind::Column;
  AlignItemsKind align_items = AlignItemsKind::Stretch;
  JustifyContentKind justify_content = JustifyContentKind::FlexStart;
  int gap = 0;
  int padding = 0;
  int min_width = 0;
  int min_height = 0;
  int flex_grow = 0;
  int flex_shrink = 1;
};

static int clamp_non_negative(int value) { return std::max(0, value); }

static int clamp_stretch(int value) { return std::clamp(clamp_non_negative(value), 0, 255); }

static QBoxLayout::Direction to_box_direction(FlexDirectionKind direction) {
  switch (direction) {
  case FlexDirectionKind::Column:
    return QBoxLayout::TopToBottom;
  case FlexDirectionKind::Row:
    return QBoxLayout::LeftToRight;
  }

  throw_error("unsupported flex direction");
}

static Qt::Alignment layout_alignment(FlexDirectionKind direction,
                                      JustifyContentKind justify_content,
                                      AlignItemsKind align_items) {
  Qt::Alignment alignment = {};
  const bool row = direction == FlexDirectionKind::Row;

  switch (justify_content) {
  case JustifyContentKind::FlexStart:
    alignment |= row ? Qt::AlignLeft : Qt::AlignTop;
    break;
  case JustifyContentKind::Center:
    alignment |= row ? Qt::AlignHCenter : Qt::AlignVCenter;
    break;
  case JustifyContentKind::FlexEnd:
    alignment |= row ? Qt::AlignRight : Qt::AlignBottom;
    break;
  }

  switch (align_items) {
  case AlignItemsKind::FlexStart:
    alignment |= row ? Qt::AlignTop : Qt::AlignLeft;
    break;
  case AlignItemsKind::Center:
    alignment |= row ? Qt::AlignVCenter : Qt::AlignHCenter;
    break;
  case AlignItemsKind::FlexEnd:
    alignment |= row ? Qt::AlignBottom : Qt::AlignRight;
    break;
  case AlignItemsKind::Stretch:
    break;
  }

  return alignment;
}

static QSizePolicy::Policy item_size_policy(int flex_grow, int flex_shrink) {
  if (flex_grow > 0) {
    return QSizePolicy::Expanding;
  }
  if (flex_shrink == 0) {
    return QSizePolicy::Fixed;
  }
  return QSizePolicy::Preferred;
}

class WindowFrameTracker final : public QObject {
public:
  explicit WindowFrameTracker(QWidget *target) : QObject(target), target_(target) {
    target_->installEventFilter(this);
  }

protected:
  bool eventFilter(QObject *watched, QEvent *event) override {
    if (watched == target_ && event != nullptr && event->type() == QEvent::Paint) {
      if (auto *host = dynamic_cast<HostWindowWidget *>(target_->window())) {
        host->tick_frame();
      }
    }

    return QObject::eventFilter(watched, event);
  }

private:
  QWidget *target_ = nullptr;
};

static std::unique_ptr<QWidget> create_probe_widget(WidgetKind kind) {
  switch (kind) {
#include "qt_widget_probe_cases.inc"
  }

  throw_error("unsupported widget probe kind");
}

static bool meta_type_matches(const QMetaProperty &property,
                              PropPayloadKind payload_kind) {
  const QMetaType meta_type = property.metaType();
  switch (payload_kind) {
  case PropPayloadKind::String:
    return meta_type.id() == QMetaType::QString;
  case PropPayloadKind::Bool:
    return meta_type.id() == QMetaType::Bool;
  case PropPayloadKind::I32:
  case PropPayloadKind::Enum:
    return meta_type.id() == QMetaType::Int;
  case PropPayloadKind::F64:
    return meta_type.id() == QMetaType::Double;
  }

  return false;
}

static const char *event_value_cpp_type(EventValueKind kind) {
  switch (kind) {
  case EventValueKind::String:
    return "QString";
  case EventValueKind::Bool:
    return "bool";
  case EventValueKind::I32:
    return "int";
  case EventValueKind::F64:
    return "double";
  }

  throw_error("unsupported event value kind");
}

static QByteArray signal_signature(const std::string &lower_name,
                                   const CompiledEventBinding &binding) {
  QByteArray signature(lower_name.c_str(), static_cast<qsizetype>(lower_name.size()));
  signature += '(';

  bool first = true;
  auto append_type = [&](const char *type_name) {
    if (!first) {
      signature += ',';
    }
    signature += type_name;
    first = false;
  };

  switch (binding.payload_kind) {
  case EventPayloadKind::Unit:
    break;
  case EventPayloadKind::Scalar:
    append_type(event_value_cpp_type(binding.scalar_kind));
    break;
  case EventPayloadKind::Object:
    for (const auto &field : binding.payload_fields) {
      append_type(event_value_cpp_type(field.kind));
    }
    break;
  }

  signature += ')';
  return signature;
}

static void apply_layout_style(WidgetEntry &widget) {
  auto *box_layout = qobject_cast<QBoxLayout *>(widget.layout);
  if (!box_layout) {
    throw_error("expected QBoxLayout for layout-backed widget");
  }

  box_layout->setDirection(to_box_direction(widget.flex_direction));
  box_layout->setSpacing(widget.gap);
  box_layout->setContentsMargins(widget.padding, widget.padding,
                                 widget.padding, widget.padding);

  const auto alignment = layout_alignment(
      widget.flex_direction, widget.justify_content, widget.align_items);
  box_layout->setAlignment(alignment);
}

static void apply_widget_style(WidgetEntry &widget) {
  widget.widget->setMinimumWidth(widget.min_width);
  widget.widget->setMinimumHeight(widget.min_height);

  auto policy = widget.widget->sizePolicy();
  const auto item_policy = item_size_policy(widget.flex_grow, widget.flex_shrink);
  const auto stretch = clamp_stretch(widget.flex_grow);
  policy.setHorizontalPolicy(item_policy);
  policy.setVerticalPolicy(item_policy);
  policy.setHorizontalStretch(stretch);
  policy.setVerticalStretch(stretch);
  widget.widget->setSizePolicy(policy);

  if (auto *parent = widget.widget->parentWidget()) {
    if (auto *box_layout = qobject_cast<QBoxLayout *>(parent->layout())) {
      box_layout->setStretchFactor(widget.widget, widget.flex_grow);
    }
  }
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

#include "qt_widget_prop_dispatch.inc"
#include "qt_widget_host_methods.inc"

class QtRegistry {
public:
  void compile_meta_contracts() {
    contracts_.clear();

    for (const auto kind : kAllWidgetKinds) {
      auto probe = create_probe_widget(kind);
      CompiledWidgetContract contract;
      contract.kind_tag = static_cast<std::uint8_t>(kind);
      compile_prop_contract(kind, probe->metaObject(), contract);
      compile_event_contract(kind, probe->metaObject(), contract);
      contracts_.emplace(contract.kind_tag, std::move(contract));
    }
  }

  void create_widget(std::uint32_t id, std::uint8_t kind_tag) {
    if (entries_.find(id) != entries_.end()) {
      throw_error("qt registry already contains widget id");
    }

    const auto kind = widget_kind_from_tag(kind_tag);
    const auto &contract = compiled_contract(kind);
    WidgetEntry entry{.kind = kind};

    switch (kind) {
#include "qt_widget_create_cases.inc"
    }

    if (auto *callback_host = dynamic_cast<RustWidgetBindingHost *>(entry.widget)) {
      callback_host->bind_rust_widget(id, kind_tag);
    }

    new WindowFrameTracker(entry.widget);
    wire_widget_events(id, entry.widget, contract);
    apply_widget_style(entry);
    entries_.emplace(id, entry);
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
      child_widget->show();
      child_widget->raise();
      child_widget->activateWindow();
      return;
    }

    auto &parent = entry(parent_id);
    if (!parent.layout) {
      throw_error("parent node does not expose a Qt layout");
    }

    const int insert_at =
        anchor_id_or_zero == 0
            ? parent.layout->count()
            : find_layout_index(parent.layout, entry(anchor_id_or_zero).widget);
    if (insert_at < 0) {
      throw_error("anchor widget is not attached to parent layout");
    }

    if (auto *box_layout = qobject_cast<QBoxLayout *>(parent.layout)) {
      box_layout->insertWidget(insert_at, child_widget);
      apply_widget_style(child);
      child_widget->show();
      mark_window_scene_dirty(parent.widget);
      return;
    }

    throw_error("unsupported Qt layout type for insert_child");
  }

  void remove_child(std::uint32_t parent_id, std::uint32_t child_id) {
    auto &child = entry(child_id);
    if (parent_id == kRootNodeId) {
      child.widget->hide();
      child.widget->setParent(nullptr);
      return;
    }

    auto &parent = entry(parent_id);
    if (!parent.layout) {
      throw_error("parent node does not expose a Qt layout");
    }

    parent.layout->removeWidget(child.widget);
    child.widget->hide();
    child.widget->setParent(nullptr);
    mark_window_scene_dirty(parent.widget);
  }

  void destroy_widget(std::uint32_t id) {
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

    std::vector<std::uint32_t> remove_ids;
    remove_ids.reserve(entries_.size());
    for (const auto &[candidate_id, candidate] : entries_) {
      if (candidate.widget == widget ||
          is_descendant(candidate.widget, widget)) {
        remove_ids.push_back(candidate_id);
      }
    }

    for (const auto remove_id : remove_ids) {
      entries_.erase(remove_id);
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
    if (auto *texture_widget =
            dynamic_cast<TexturePaintHostWidget *>(widget.widget)) {
      texture_widget->mark_frame_dirty();
      return;
    }
    widget.widget->update();
  }

  QtWidgetCaptureLayout capture_widget_layout(std::uint32_t id) {
    auto &widget_entry = entry(id);
    auto *widget = widget_entry.widget;
    const bool rgba_capture =
        dynamic_cast<CustomPaintHostWidget *>(widget) != nullptr;
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

  rust::Vec<QtRect> capture_widget_visible_rects(std::uint32_t id) {
    auto &widget_entry = entry(id);
    auto *widget = widget_entry.widget;
    rust::Vec<QtRect> rects;
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

  void capture_widget_into(std::uint32_t id, std::uint32_t width_px,
                           std::uint32_t height_px, std::size_t stride,
                           bool include_children,
                           rust::Slice<std::uint8_t> bytes) {
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
                                  rust::Slice<std::uint8_t> bytes) {
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
      qt_solid_spike::qt::window_host_request_wake();
    }
  }

  void apply_string_prop(std::uint32_t id, std::uint16_t prop_id,
                         std::uint64_t trace_id, rust::Str value) {
    qt_solid_spike::qt::trace_cpp_stage(
        trace_id, rust::Str("cpp.apply_string_prop.enter"), id, prop_id,
        rust::Str(""));

    auto &widget = entry(id);
    const auto &binding = compiled_prop_binding(widget.kind, prop_id);
    if (binding.payload_kind != PropPayloadKind::String) {
      throw_error("Qt host prop slot payload mismatch for string prop apply");
    }

    if (binding.lower_kind == PropLowerKind::MetaProperty) {
      write_meta_property(widget, binding, QVariant(to_qstring(value)));
      qt_solid_spike::qt::trace_cpp_stage(
          trace_id, rust::Str("cpp.apply_string_prop.exit"), id, prop_id,
          rust::Str(binding.js_name));
      return;
    }

    {
      const QSignalBlocker blocker(widget.widget);
      if (apply_generated_string_prop(widget, binding, value)) {
        qt_solid_spike::qt::trace_cpp_stage(
            trace_id, rust::Str("cpp.apply_string_prop.exit"), id, prop_id,
            rust::Str(binding.js_name));
        return;
      }
    }

    throw_error("Qt host contract has unsupported custom string prop lowering");
  }

  void apply_i32_prop(std::uint32_t id, std::uint16_t prop_id,
                      std::uint64_t trace_id, std::int32_t value) {
    qt_solid_spike::qt::trace_cpp_stage(
        trace_id, rust::Str("cpp.apply_i32_prop.enter"), id, prop_id,
        rust::Str(""));

    auto &widget = entry(id);
    const auto &binding = compiled_prop_binding(widget.kind, prop_id);
    if (binding.payload_kind != PropPayloadKind::I32 &&
        binding.payload_kind != PropPayloadKind::Enum) {
      throw_error("Qt host prop slot payload mismatch for i32 prop apply");
    }

    if (binding.lower_kind == PropLowerKind::MetaProperty) {
      write_meta_property(widget, binding, QVariant(value));
      qt_solid_spike::qt::trace_cpp_stage(
          trace_id, rust::Str("cpp.apply_i32_prop.exit"), id, prop_id,
          rust::Str(binding.js_name));
      return;
    }

    {
      const QSignalBlocker blocker(widget.widget);
      if (apply_generated_i32_prop(widget, binding, value)) {
        qt_solid_spike::qt::trace_cpp_stage(
            trace_id, rust::Str("cpp.apply_i32_prop.exit"), id, prop_id,
            rust::Str(binding.js_name));
        return;
      }
    }

    throw_error("Qt host contract has unsupported custom i32 prop lowering");
  }

  void apply_bool_prop(std::uint32_t id, std::uint16_t prop_id,
                       std::uint64_t trace_id, bool value) {
    qt_solid_spike::qt::trace_cpp_stage(
        trace_id, rust::Str("cpp.apply_bool_prop.enter"), id, prop_id,
        rust::Str(""));

    auto &widget = entry(id);
    const auto &binding = compiled_prop_binding(widget.kind, prop_id);
    if (binding.payload_kind != PropPayloadKind::Bool) {
      throw_error("Qt host prop slot payload mismatch for bool prop apply");
    }

    if (binding.lower_kind == PropLowerKind::MetaProperty) {
      write_meta_property(widget, binding, QVariant(value));
      qt_solid_spike::qt::trace_cpp_stage(
          trace_id, rust::Str("cpp.apply_bool_prop.exit"), id, prop_id,
          rust::Str(binding.js_name));
      return;
    }

    {
      const QSignalBlocker blocker(widget.widget);
      if (apply_generated_bool_prop(widget, binding, value)) {
        qt_solid_spike::qt::trace_cpp_stage(
            trace_id, rust::Str("cpp.apply_bool_prop.exit"), id, prop_id,
            rust::Str(binding.js_name));
        return;
      }
    }

    throw_error("Qt host contract has unsupported custom bool prop lowering");
  }

  void apply_f64_prop(std::uint32_t id, std::uint16_t prop_id,
                      std::uint64_t trace_id, double value) {
    qt_solid_spike::qt::trace_cpp_stage(
        trace_id, rust::Str("cpp.apply_f64_prop.enter"), id, prop_id,
        rust::Str(""));

    auto &widget = entry(id);
    const auto &binding = compiled_prop_binding(widget.kind, prop_id);
    if (binding.payload_kind != PropPayloadKind::F64) {
      throw_error("Qt host prop slot payload mismatch for f64 prop apply");
    }

    if (binding.lower_kind == PropLowerKind::MetaProperty) {
      write_meta_property(widget, binding, QVariant(value));
      qt_solid_spike::qt::trace_cpp_stage(
          trace_id, rust::Str("cpp.apply_f64_prop.exit"), id, prop_id,
          rust::Str(binding.js_name));
      return;
    }

    {
      const QSignalBlocker blocker(widget.widget);
      if (apply_generated_f64_prop(widget, binding, value)) {
        qt_solid_spike::qt::trace_cpp_stage(
            trace_id, rust::Str("cpp.apply_f64_prop.exit"), id, prop_id,
            rust::Str(binding.js_name));
        return;
      }
    }

    throw_error("Qt host contract has unsupported custom f64 prop lowering");
  }

  rust::String read_string_prop(std::uint32_t id, std::uint16_t prop_id) {
    auto &widget = entry(id);
    const auto &binding = compiled_prop_binding(widget.kind, prop_id);
    if (binding.payload_kind != PropPayloadKind::String) {
      throw_error("Qt host prop slot payload mismatch for string prop read");
    }
    if (!binding.has_read_lowering) {
      throw_error("Qt host contract has no string read lowering for prop id");
    }

    if (binding.read_lower_kind == PropLowerKind::MetaProperty) {
      const QMetaProperty property =
          widget.widget->metaObject()->property(binding.read_property_index);
      const QVariant variant = property.read(widget.widget);
      if (!variant.isValid() || !variant.canConvert<QString>()) {
        throw_error("Qt host failed to read string property");
      }
      return to_rust_string(variant.toString());
    }

    rust::String generated;
    if (read_generated_string_prop(widget, binding, &generated)) {
      return generated;
    }

    throw_error("Qt host contract has unsupported custom string prop read lowering");
  }

  std::int32_t read_i32_prop(std::uint32_t id, std::uint16_t prop_id) {
    auto &widget = entry(id);
    const auto &binding = compiled_prop_binding(widget.kind, prop_id);
    if (binding.payload_kind != PropPayloadKind::I32 &&
        binding.payload_kind != PropPayloadKind::Enum) {
      throw_error("Qt host prop slot payload mismatch for i32 prop read");
    }
    if (!binding.has_read_lowering) {
      throw_error("Qt host contract has no i32 read lowering for prop id");
    }

    if (binding.read_lower_kind == PropLowerKind::MetaProperty) {
      const QMetaProperty property =
          widget.widget->metaObject()->property(binding.read_property_index);
      const QVariant variant = property.read(widget.widget);
      if (!variant.isValid()) {
        throw_error("Qt host failed to read i32 property");
      }
      return variant.toInt();
    }

    std::int32_t generated = 0;
    if (read_generated_i32_prop(widget, binding, &generated)) {
      return generated;
    }

    throw_error("Qt host contract has unsupported custom i32 prop read lowering");
  }

  double read_f64_prop(std::uint32_t id, std::uint16_t prop_id) {
    auto &widget = entry(id);
    const auto &binding = compiled_prop_binding(widget.kind, prop_id);
    if (binding.payload_kind != PropPayloadKind::F64) {
      throw_error("Qt host prop slot payload mismatch for f64 prop read");
    }
    if (!binding.has_read_lowering) {
      throw_error("Qt host contract has no f64 read lowering for prop id");
    }

    if (binding.read_lower_kind == PropLowerKind::MetaProperty) {
      const QMetaProperty property =
          widget.widget->metaObject()->property(binding.read_property_index);
      const QVariant variant = property.read(widget.widget);
      if (!variant.isValid()) {
        throw_error("Qt host failed to read f64 property");
      }
      return variant.toDouble();
    }

    double generated = 0.0;
    if (read_generated_f64_prop(widget, binding, &generated)) {
      return generated;
    }

    throw_error("Qt host contract has unsupported custom f64 prop read lowering");
  }

  bool read_bool_prop(std::uint32_t id, std::uint16_t prop_id) {
    auto &widget = entry(id);
    const auto &binding = compiled_prop_binding(widget.kind, prop_id);
    if (binding.payload_kind != PropPayloadKind::Bool) {
      throw_error("Qt host prop slot payload mismatch for bool prop read");
    }
    if (!binding.has_read_lowering) {
      throw_error("Qt host contract has no bool read lowering for prop id");
    }

    if (binding.read_lower_kind == PropLowerKind::MetaProperty) {
      const QMetaProperty property =
          widget.widget->metaObject()->property(binding.read_property_index);
      const QVariant variant = property.read(widget.widget);
      if (!variant.isValid()) {
        throw_error("Qt host failed to read bool property");
      }
      return variant.toBool();
    }

    bool generated = false;
    if (read_generated_bool_prop(widget, binding, &generated)) {
      return generated;
    }

    throw_error("Qt host contract has unsupported custom bool prop read lowering");
  }

  QtMethodValue call_host_slot(std::uint32_t id, std::uint16_t slot,
                               const rust::Vec<QtMethodValue> &args) {
    auto &widget = entry(id);
    return call_generated_host_slot(widget, slot, args);
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

  void debug_input_insert_text(std::uint32_t id, rust::Str value) {
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
    state.flex_grow = widget.flex_grow;
    state.has_flex_shrink = true;
    state.flex_shrink = widget.flex_shrink;
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

    if (widget.layout) {
      state.flex_direction_tag =
          static_cast<std::uint8_t>(widget.flex_direction);
      state.justify_content_tag =
          static_cast<std::uint8_t>(widget.justify_content);
      state.align_items_tag = static_cast<std::uint8_t>(widget.align_items);
      state.has_gap = true;
      state.gap = widget.gap;
      state.has_padding = true;
      state.padding = widget.padding;
    }

    return state;
  }

  void clear() {
    inspect_mode_enabled_ = false;
    inspect_press_active_ = false;
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
      for (const auto &[candidate_id, candidate] : entries_) {
        if (candidate.widget == current) {
          return candidate_id;
        }
      }
    }

    return 0;
  }

  std::uint32_t widget_id_at_point(const QPoint &global_pos) const {
    return widget_id_for_widget(QApplication::widgetAt(global_pos));
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

  const CompiledWidgetContract &compiled_contract(WidgetKind kind) const {
    const auto kind_tag = static_cast<std::uint8_t>(kind);
    auto it = contracts_.find(kind_tag);
    if (it == contracts_.end()) {
      throw_error("Qt host contract registry is not compiled for widget kind");
    }
    return it->second;
  }

  const CompiledPropBinding &compiled_prop_binding(WidgetKind kind,
                                                   const char *js_name) const {
    const auto &contract = compiled_contract(kind);
    auto it = std::find_if(contract.props.begin(), contract.props.end(),
                           [&](const CompiledPropBinding &binding) {
                             return binding.js_name == js_name;
                           });
    if (it == contract.props.end()) {
      throw_error("Qt host contract registry is missing prop binding");
    }
    return *it;
  }

  const CompiledPropBinding &
  compiled_prop_binding(WidgetKind kind, std::uint16_t prop_id) const {
    const auto &contract = compiled_contract(kind);
    auto it = std::find_if(contract.props.begin(), contract.props.end(),
                           [&](const CompiledPropBinding &binding) {
                             return binding.prop_id == prop_id;
                           });
    if (it == contract.props.end()) {
      throw_error("Qt host contract registry is missing prop id binding");
    }
    return *it;
  }

  void compile_prop_contract(WidgetKind kind, const QMetaObject *meta_object,
                             CompiledWidgetContract &contract) const {
    const auto kind_tag = static_cast<std::uint8_t>(kind);
    const std::size_t prop_count =
        qt_solid_spike::qt::qt_widget_prop_count(kind_tag);

    for (std::size_t index = 0; index < prop_count; ++index) {
      const std::uint16_t prop_id =
          qt_solid_spike::qt::qt_widget_prop_id(kind_tag, index);
      const std::string js_name(
          qt_solid_spike::qt::qt_widget_prop_js_name(kind_tag, index));
      const auto payload_kind = prop_payload_kind_from_tag(
          qt_solid_spike::qt::qt_widget_prop_payload_kind(kind_tag,
                                                               index));
      const bool non_negative =
          qt_solid_spike::qt::qt_widget_prop_non_negative(kind_tag, index);
      const auto lower_kind = prop_lower_kind_from_tag(
          qt_solid_spike::qt::qt_widget_prop_lower_kind(kind_tag, index));
      const std::string lower_name(
          qt_solid_spike::qt::qt_widget_prop_lower_name(kind_tag, index));
      const std::uint8_t read_lower_kind_tag =
          qt_solid_spike::qt::qt_widget_prop_read_lower_kind(kind_tag,
                                                                  index);
      const std::string read_lower_name(
          qt_solid_spike::qt::qt_widget_prop_read_lower_name(kind_tag,
                                                                  index));

      CompiledPropBinding binding;
      binding.prop_id = prop_id;
      binding.js_name = js_name;
      binding.payload_kind = payload_kind;
      binding.non_negative = non_negative;
      binding.lower_kind = lower_kind;
      binding.lower_name = lower_name;

      if (binding.lower_name.empty()) {
        throw_error("Rust prop lowering metadata is missing a target name");
      }

      if (lower_kind == PropLowerKind::MetaProperty) {
        const int property_index =
            meta_object->indexOfProperty(binding.lower_name.c_str());
        if (property_index < 0) {
          throw_error(
              "failed to resolve Qt property from Rust prop lowering metadata");
        }

        const QMetaProperty property = meta_object->property(property_index);
        if (!property.isValid() || !property.isWritable()) {
          throw_error(
              "Qt property from Rust prop lowering metadata is not writable");
        }
        if (!meta_type_matches(property, payload_kind)) {
          throw_error("Qt property type does not match Rust prop schema");
        }

        binding.property_index = property_index;
      } else if (!supports_generated_host_prop_lowering(kind, binding.lower_name,
                                                        payload_kind)) {
        throw_error("Qt host contract has unsupported custom prop lowering");
      }

      if (read_lower_kind_tag != 0) {
        binding.has_read_lowering = true;
        binding.read_lower_kind = prop_lower_kind_from_tag(read_lower_kind_tag);
        binding.read_lower_name = read_lower_name;

        if (binding.read_lower_name.empty()) {
          throw_error(
              "Rust prop read-lowering metadata is missing a target name");
        }

        if (binding.read_lower_kind == PropLowerKind::MetaProperty) {
          const int property_index =
              meta_object->indexOfProperty(binding.read_lower_name.c_str());
          if (property_index < 0) {
            throw_error("failed to resolve Qt property from Rust read-lowering "
                        "metadata");
          }

          const QMetaProperty property = meta_object->property(property_index);
          if (!property.isValid() || !property.isReadable()) {
            throw_error(
                "Qt property from Rust read-lowering metadata is not readable");
          }
          if (!meta_type_matches(property, payload_kind)) {
            throw_error("Qt property type does not match Rust read schema");
          }

          binding.read_property_index = property_index;
        } else if (!supports_generated_host_prop_reading(
                       kind, binding.read_lower_name, payload_kind)) {
          throw_error(
              "Qt host contract has unsupported custom prop read lowering");
        }
      }

      contract.props.push_back(std::move(binding));
    }
  }

  void compile_event_contract(WidgetKind kind, const QMetaObject *meta_object,
                              CompiledWidgetContract &contract) const {
    const auto kind_tag = static_cast<std::uint8_t>(kind);
    const std::size_t event_count =
        qt_solid_spike::qt::qt_widget_event_count(kind_tag);

    for (std::size_t index = 0; index < event_count; ++index) {
      const auto payload_kind = event_payload_kind_from_tag(
          qt_solid_spike::qt::qt_widget_event_payload_kind(kind_tag,
                                                                index));
      const std::size_t payload_field_count =
          qt_solid_spike::qt::qt_widget_event_payload_field_count(kind_tag,
                                                                       index);
      const auto lower_kind = event_lower_kind_from_tag(
          qt_solid_spike::qt::qt_widget_event_lower_kind(kind_tag, index));
      const std::string lower_name(
          qt_solid_spike::qt::qt_widget_event_lower_name(kind_tag, index));

      CompiledEventBinding binding;
      binding.event_index = static_cast<std::uint8_t>(index);
      binding.payload_kind = payload_kind;
      binding.lower_kind = lower_kind;
      binding.lower_name = lower_name;
      binding.has_scalar_kind = false;
      if (binding.payload_kind == EventPayloadKind::Scalar) {
        binding.has_scalar_kind = true;
        binding.scalar_kind = event_value_kind_from_tag(
            qt_solid_spike::qt::qt_widget_event_payload_scalar_kind(
                kind_tag, index));
      }
      for (std::size_t field_index = 0; field_index < payload_field_count;
           ++field_index) {
        binding.payload_fields.push_back(CompiledEventBinding::PayloadField{
            std::string(
                qt_solid_spike::qt::qt_widget_event_payload_field_name(
                    kind_tag, index, field_index)),
            event_value_kind_from_tag(
                qt_solid_spike::qt::qt_widget_event_payload_field_kind(
                    kind_tag, index, field_index)),
        });
      }

      if (binding.lower_name.empty()) {
        throw_error("Rust event lowering metadata is missing a target name");
      }

      if (binding.payload_kind == EventPayloadKind::Object &&
          binding.payload_fields.empty()) {
        throw_error("Qt host event object payload is missing fields");
      }

      if (binding.payload_kind != EventPayloadKind::Object &&
          !binding.payload_fields.empty()) {
        throw_error(
            "Qt host scalar event payload unexpectedly declared fields");
      }

      if (binding.payload_kind == EventPayloadKind::Scalar &&
          !binding.has_scalar_kind) {
        throw_error("Qt host scalar event payload is missing scalar kind");
      }

      if (binding.lower_kind == EventLowerKind::Custom) {
        contract.events.push_back(binding);
        continue;
      }

      const QByteArray signature =
          signal_signature(binding.lower_name, binding);
      const int signal_index =
          meta_object->indexOfSignal(signature.constData());
      if (signal_index < 0) {
        throw_error("failed to resolve Qt signal from Rust event metadata");
      }

      binding.signal_method_index = signal_index;
      contract.events.push_back(binding);
    }
  }

  void write_meta_property(WidgetEntry &widget,
                           const CompiledPropBinding &binding,
                           const QVariant &value) {
    if (binding.lower_kind != PropLowerKind::MetaProperty ||
        binding.property_index < 0) {
      throw_error("requested Qt meta-property write for non-meta prop binding");
    }

    const QMetaProperty property =
        widget.widget->metaObject()->property(binding.property_index);
    if (!property.isValid()) {
      throw_error(
          "failed to resolve Qt meta-property from compiled host contract");
    }

    const QVariant current = property.read(widget.widget);
    if (current == value) {
      return;
    }

    const QSignalBlocker blocker(widget.widget);
    if (!property.write(widget.widget, value)) {
      throw_error(
          "failed to write Qt meta-property from compiled host contract");
    }
  }

  void write_meta_property(WidgetEntry &widget, const char *js_name,
                           const QVariant &value) {
    const auto &binding = compiled_prop_binding(widget.kind, js_name);
    write_meta_property(widget, binding, value);
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

  std::unordered_map<std::uint8_t, CompiledWidgetContract> contracts_;
  std::unordered_map<std::uint32_t, WidgetEntry> entries_;
  std::vector<QPointer<QWidget>> pending_top_level_deletes_;
  QPointer<QWidget> highlighted_widget_;
  QPointer<DebugHighlightOverlay> highlight_overlay_;
  QPointer<QTimer> inspect_poll_timer_;
  QPointer<InspectModeEventFilter> inspect_event_filter_;
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
      QObject::connect(app_.get(), &QGuiApplication::applicationStateChanged,
                       app_.get(), [](Qt::ApplicationState state) {
                         if (state == Qt::ApplicationActive) {
                           qt_solid_spike::qt::emit_app_event(
                               rust::Str("activate"));
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
      registry_.compile_meta_contracts();
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

    if (pump_) {
      pump_->abandon_for_process_exit();
      pump_.release();
    }

#if defined(__APPLE__)
    wait_bridge_.reset();
#endif
    registry_.clear();
    for (int index = 0; index < 4; ++index) {
      QCoreApplication::sendPostedEvents(nullptr, QEvent::DeferredDelete);
      QCoreApplication::processEvents(QEventLoop::AllEvents);
      QCoreApplication::sendPostedEvents(nullptr);
    }

    app_.reset();
    started_ = false;
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

std::optional<std::uint64_t> current_runtime_wait_bridge_timer_delay_ms()
    noexcept {
  if (!g_host) {
    return std::nullopt;
  }

  return g_host->runtime_wait_bridge_timer_delay_ms();
}
