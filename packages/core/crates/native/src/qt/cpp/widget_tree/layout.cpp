struct WidgetEntry;

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
