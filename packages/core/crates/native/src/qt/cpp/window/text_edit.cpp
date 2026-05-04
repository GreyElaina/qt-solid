class TextEditSession {
public:
  TextEditSession() = default;
  ~TextEditSession() { deactivate(); }

  bool active() const { return control_ != nullptr; }
  std::uint32_t canvas_node_id() const { return canvas_node_id_; }
  std::uint32_t fragment_id() const { return fragment_id_; }

  void activate(std::uint32_t canvas_node_id, std::uint32_t fragment_id,
                const QString &text, double font_size, int cursor_pos,
                int sel_start, int sel_end) {
    if (control_ != nullptr && canvas_node_id_ == canvas_node_id &&
        fragment_id_ == fragment_id) {
      return;
    }
    deactivate();

    canvas_node_id_ = canvas_node_id;
    fragment_id_ = fragment_id;
    font_size_ = font_size;

    control_ = new QWidgetLineControl(text);
    control_->setCursorPosition(cursor_pos);
    if (sel_start >= 0 && sel_end > sel_start) {
      control_->setSelection(sel_start, sel_end - sel_start);
    }

    QObject::connect(control_, &QWidgetLineControl::updateNeeded, [this](const QRect &) {
      if (control_ == nullptr) {
        return;
      }
      const bool visible = control_->cursorBlinkStatus();
      qt_solid_spike::qt::qt_text_edit_set_caret_visible(
          canvas_node_id_, fragment_id_, visible);
      request_qt_pump();
    });

    control_->setBlinkingCursorEnabled(true);
  }

  void deactivate() {
    if (control_ == nullptr) {
      return;
    }
    delete control_;
    control_ = nullptr;
    canvas_node_id_ = 0;
    fragment_id_ = 0;
  }

  bool process_key_event(QKeyEvent *event) {
    if (control_ == nullptr) {
      return false;
    }
    if (process_grapheme_backspace(event)) {
      sync_to_rust();
      return true;
    }
    control_->processKeyEvent(event);
    sync_to_rust();
    return true;
  }

  bool process_input_method_event(QInputMethodEvent *event) {
    if (control_ == nullptr) {
      return false;
    }
    control_->processInputMethodEvent(event);
    sync_to_rust();
    return true;
  }

  QVariant input_method_query(Qt::InputMethodQuery query) const {
    if (control_ == nullptr) {
      return QVariant();
    }
    switch (query) {
    case Qt::ImCursorRectangle: {
      const qreal x = control_->cursorToX();
      return QRectF(x, 0.0, 1.0, font_size_);
    }
    case Qt::ImCursorPosition:
      return control_->cursor();
    case Qt::ImSurroundingText:
      return control_->text();
    case Qt::ImCurrentSelection:
      return control_->selectedText();
    case Qt::ImAnchorPosition: {
      const int sel_start = control_->selectionStart();
      const int cursor = control_->cursor();
      if (sel_start >= 0) {
        return (sel_start == cursor) ? control_->selectionEnd() : sel_start;
      }
      return cursor;
    }
    case Qt::ImEnabled:
      return true;
    default:
      return QVariant();
    }
  }

  void click_to_cursor(double local_x) {
    if (control_ == nullptr) {
      return;
    }
    const QTextLine line = control_->textLayout()->lineAt(0);
    if (!line.isValid()) {
      return;
    }
    const int pos = line.xToCursor(local_x, QTextLine::CursorBetweenCharacters);
    control_->setCursorPosition(pos);
    sync_to_rust();
  }

  void drag_to_cursor(double local_x) {
    if (control_ == nullptr) {
      return;
    }
    const QTextLine line = control_->textLayout()->lineAt(0);
    if (!line.isValid()) {
      return;
    }
    const int pos = line.xToCursor(local_x, QTextLine::CursorBetweenCharacters);
    control_->moveCursor(pos, true);
    sync_to_rust();
  }

private:
  bool process_grapheme_backspace(QKeyEvent *event) {
    if (event->key() != Qt::Key_Backspace) {
      return false;
    }
    const Qt::KeyboardModifiers modifiers =
        event->modifiers() & ~(Qt::ShiftModifier | Qt::KeypadModifier);
    if (modifiers != Qt::NoModifier) {
      return false;
    }

    if (control_->hasSelectedText()) {
      control_->del();
      event->accept();
      return true;
    }

    const int cursor = control_->cursor();
    if (cursor <= 0) {
      event->accept();
      return true;
    }

    const int previous = control_->textLayout()->previousCursorPosition(cursor);
    if (previous < cursor) {
      control_->setSelection(previous, cursor - previous);
      control_->del();
    }
    event->accept();
    return true;
  }

  void sync_to_rust() {
    if (control_ == nullptr) {
      return;
    }

    const QString text = control_->text();
    const QByteArray utf8 = text.toUtf8();

    // Re-shape text via existing Qt shaping FFI.
    auto shaped = qt_solid_spike::qt::qt_shape_text_with_cursors(
        rust::Str(utf8.constData(), utf8.size()), font_size_, rust::Str("", 0), 0, false);

    const int cursor = control_->cursor();
    const int sel_start = control_->selectionStart();
    const int sel_end = control_->selectionEnd();

    rust::Slice<const qt_solid_spike::qt::QtShapedPathEl> elements(
        shaped.elements.data(), shaped.elements.size());
    rust::Slice<const double> cursor_positions(
        shaped.cursor_x_positions.data(), shaped.cursor_x_positions.size());
    rust::Slice<const qt_solid_spike::qt::QtRasterizedGlyph> rasterized_glyphs(
        shaped.rasterized_glyphs.data(), shaped.rasterized_glyphs.size());

    qt_solid_spike::qt::qt_text_edit_sync(
        canvas_node_id_, fragment_id_,
        rust::Str(utf8.constData(), utf8.size()),
        cursor, sel_start, sel_end,
        elements, cursor_positions,
        shaped.ascent, shaped.descent, shaped.total_width,
        rasterized_glyphs);
  }

  QWidgetLineControl *control_ = nullptr;
  std::uint32_t canvas_node_id_ = 0;
  std::uint32_t fragment_id_ = 0;
  double font_size_ = 14.0;
};
