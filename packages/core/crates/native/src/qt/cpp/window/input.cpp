void HostWindowWidget::mousePressEvent(QMouseEvent *event) {
  if (rust_node_id_ == 0) {
    QWidget::mousePressEvent(event);
    return;
  }
  if (event->button() == Qt::LeftButton) {
    const QPointF pos = event->position();
    qt_solid_spike::qt::qt_canvas_pointer_event(rust_node_id_, 1, pos.x(), pos.y());
  } else if (event->button() == Qt::RightButton) {
    // Accept the press so that mouseReleaseEvent receives the matching release.
    event->accept();
  } else {
    QWidget::mousePressEvent(event);
  }
}

void HostWindowWidget::mouseReleaseEvent(QMouseEvent *event) {
  if (rust_node_id_ == 0) {
    QWidget::mouseReleaseEvent(event);
    return;
  }

  // Right-click release (or Ctrl+Click on macOS): emit context menu directly.
  // On macOS, Qt may not synthesize QContextMenuEvent reliably when the
  // release handler does not call the base class, so handle it here.
  const bool context_click =
      event->button() == Qt::RightButton ||
      (event->button() == Qt::LeftButton &&
       (event->modifiers() & Qt::ControlModifier));
  if (context_click) {
    const QPointF local = event->position();
    const QPoint global = event->globalPosition().toPoint();
    qt_solid_spike::qt::qt_canvas_context_menu_event(
        rust_node_id_,
        local.x(), local.y(),
        static_cast<double>(global.x()),
        static_cast<double>(global.y()));
    event->accept();
    return;
  }

  if (event->button() != Qt::LeftButton) {
    QWidget::mouseReleaseEvent(event);
    return;
  }

  const QPointF pos = event->position();
  qt_solid_spike::qt::qt_canvas_pointer_event(rust_node_id_, 2, pos.x(), pos.y());
}

void HostWindowWidget::mouseMoveEvent(QMouseEvent *event) {
  if (rust_node_id_ == 0) {
    QWidget::mouseMoveEvent(event);
    return;
  }
  const QPointF pos = event->position();
  if (text_edit_session_.active() && (event->buttons() & Qt::LeftButton)) {
    qt_solid_spike::qt::qt_canvas_pointer_event(rust_node_id_, 4, pos.x(), pos.y());
  } else {
    qt_solid_spike::qt::qt_canvas_pointer_event(rust_node_id_, 3, pos.x(), pos.y());
  }
}

void HostWindowWidget::keyPressEvent(QKeyEvent *event) {
  if (rust_node_id_ == 0) {
    QWidget::keyPressEvent(event);
    return;
  }
  if (text_edit_session_.process_key_event(event)) {
    return;
  }
  // Forward to fragment dispatch (event_tag 1 = keydown).
  forward_key_event(event, 1);
}

void HostWindowWidget::keyReleaseEvent(QKeyEvent *event) {
  if (rust_node_id_ == 0) {
    QWidget::keyReleaseEvent(event);
    return;
  }
  // Forward to fragment dispatch (event_tag 2 = keyup).
  forward_key_event(event, 2);
}

void HostWindowWidget::wheelEvent(QWheelEvent *event) {
  if (rust_node_id_ == 0) {
    QWidget::wheelEvent(event);
    return;
  }
  const QPoint angle = event->angleDelta();
  const QPoint pixel = event->pixelDelta();
  const QPointF pos = event->position();
  const auto mods = static_cast<std::uint32_t>(event->modifiers().toInt());
  // Phase: 0=NoScroll, 1=Begin, 2=Update, 3=End, 4=Momentum
  uint32_t phase = 0;
  switch (event->phase()) {
    case Qt::ScrollBegin:   phase = 1; break;
    case Qt::ScrollUpdate:  phase = 2; break;
    case Qt::ScrollEnd:     phase = 3; break;
    case Qt::ScrollMomentum: phase = 4; break;
    default: phase = 0; break;
  }
  qt_solid_spike::qt::qt_canvas_wheel_event(
      rust_node_id_,
      static_cast<double>(angle.x()),
      static_cast<double>(angle.y()),
      static_cast<double>(pixel.x()),
      static_cast<double>(pixel.y()),
      pos.x(), pos.y(), mods, phase);
}

void HostWindowWidget::mouseDoubleClickEvent(QMouseEvent *event) {
  if (rust_node_id_ == 0) {
    QWidget::mouseDoubleClickEvent(event);
    return;
  }
  const QPointF pos = event->position();
  // Qt's double-click replaces the second press; forward tag 1 to restore
  // pointer down/up symmetry, then tag 5 for the double-click itself.
  qt_solid_spike::qt::qt_canvas_pointer_event(rust_node_id_, 1, pos.x(), pos.y());
  qt_solid_spike::qt::qt_canvas_pointer_event(rust_node_id_, 5, pos.x(), pos.y());
}

void HostWindowWidget::inputMethodEvent(QInputMethodEvent *event) {
  if (text_edit_session_.process_input_method_event(event)) {
    return;
  }
  QWidget::inputMethodEvent(event);
}

QVariant HostWindowWidget::inputMethodQuery(Qt::InputMethodQuery query) const {
  if (text_edit_session_.active()) {
    return text_edit_session_.input_method_query(query);
  }
  return QWidget::inputMethodQuery(query);
}

void HostWindowWidget::forward_key_event(QKeyEvent *event, std::uint8_t event_tag) {
  const auto qt_key = static_cast<std::int32_t>(event->key());
  const auto mods = static_cast<std::uint32_t>(event->modifiers().toInt());
  const QByteArray text_utf8 = event->text().toUtf8();
  const bool repeat = event->isAutoRepeat();
  const auto scan_code = static_cast<std::uint32_t>(event->nativeScanCode());
  const auto virtual_key = static_cast<std::uint32_t>(event->nativeVirtualKey());
  qt_solid_spike::qt::qt_canvas_key_event(
      rust_node_id_, event_tag, qt_key, mods,
      rust::Str(text_utf8.constData(), text_utf8.size()),
      repeat, scan_code, virtual_key);
}

void HostWindowWidget::contextMenuEvent(QContextMenuEvent *event) {
  if (rust_node_id_ == 0) {
    QWidget::contextMenuEvent(event);
    return;
  }
  // Keyboard-triggered context menu (Menu key, Shift+F10).
  // Mouse-triggered context menus are handled in mouseReleaseEvent.
  if (event->reason() == QContextMenuEvent::Keyboard) {
    const QPoint local = event->pos();
    const QPoint global = event->globalPos();
    qt_solid_spike::qt::qt_canvas_context_menu_event(
        rust_node_id_,
        static_cast<double>(local.x()),
        static_cast<double>(local.y()),
        static_cast<double>(global.x()),
        static_cast<double>(global.y()));
  }
  event->accept();
}
