QtNodeBounds bounds_for_widget(QWidget *widget) {
  if (!widget) {
    return QtNodeBounds{false, 0, 0, 0, 0};
  }

  const QPoint top_left = widget->mapToGlobal(QPoint(0, 0));
  return QtNodeBounds{
      widget->isVisible(), top_left.x(),     top_left.y(),
      widget->width(),     widget->height(),
  };
}

class DebugHighlightOverlay final : public QWidget {
public:
  explicit DebugHighlightOverlay(QWidget *parent = nullptr) : QWidget(parent) {
    setAttribute(Qt::WA_TransparentForMouseEvents, true);
    setAttribute(Qt::WA_NoSystemBackground, true);
    setAttribute(Qt::WA_TranslucentBackground, true);
    setFocusPolicy(Qt::NoFocus);
    hide();
  }

  void highlight_widget(QWidget *target) {
    if (target_widget_ == target) {
      update_geometry();
      return;
    }

    detach_target();
    if (!target) {
      hide();
      return;
    }

    target_widget_ = target;
    target_widget_->installEventFilter(this);
    update_geometry();
  }

  void clear_highlight() {
    detach_target();
    hide();
    if (parentWidget()) {
      setParent(nullptr);
    }
  }

protected:
  bool eventFilter(QObject *watched, QEvent *event) override {
    if (watched == target_widget_) {
      switch (event->type()) {
      case QEvent::Move:
      case QEvent::Resize:
      case QEvent::Show:
      case QEvent::Hide:
      case QEvent::LayoutRequest:
      case QEvent::WindowStateChange:
        update_geometry();
        break;
      case QEvent::Destroy:
        target_widget_.clear();
        hide();
        if (parentWidget()) {
          setParent(nullptr);
        }
        break;
      default:
        break;
      }
    }

    return QWidget::eventFilter(watched, event);
  }

  void paintEvent(QPaintEvent *event) override {
    QWidget::paintEvent(event);

    QPainter painter(this);
    painter.setRenderHint(QPainter::Antialiasing, false);
    painter.fillRect(rect().adjusted(3, 3, -3, -3), QColor(255, 191, 0, 32));

    QPen border(QColor(255, 191, 0, 220));
    border.setWidth(2);
    painter.setPen(border);
    painter.setBrush(Qt::NoBrush);
    painter.drawRect(rect().adjusted(1, 1, -2, -2));
  }

private:
  static QWidget *overlay_parent_for(QWidget *target) {
    return target && target->parentWidget() == nullptr ? target
                                                       : target->window();
  }

  void detach_target() {
    if (target_widget_) {
      target_widget_->removeEventFilter(this);
    }
    target_widget_.clear();
  }

  void update_geometry() {
    if (!target_widget_) {
      hide();
      return;
    }

    QWidget *overlay_parent = overlay_parent_for(target_widget_);
    if (!overlay_parent || target_widget_->width() <= 0 ||
        target_widget_->height() <= 0) {
      hide();
      return;
    }
    if (overlay_parent->testAttribute(Qt::WA_DontShowOnScreen)) {
      hide();
      return;
    }

    if (parentWidget() != overlay_parent) {
      setParent(overlay_parent);
    }

    const QRect target_rect =
        target_widget_ == overlay_parent
            ? overlay_parent->rect()
            : QRect(target_widget_->mapTo(overlay_parent, QPoint(0, 0)),
                    target_widget_->size());
    setGeometry(target_rect.adjusted(-1, -1, 1, 1));
    setVisible(target_widget_->isVisible());
    raise();
    update();
  }

  QPointer<QWidget> target_widget_;
};

class InspectModeEventFilter final : public QObject {
public:
  using MouseEventHandler = std::function<bool(QMouseEvent *)>;

  explicit InspectModeEventFilter(MouseEventHandler handler,
                                  QObject *parent = nullptr)
      : QObject(parent), handler_(std::move(handler)) {}

protected:
  bool eventFilter(QObject *watched, QEvent *event) override {
    (void)watched;
    switch (event->type()) {
    case QEvent::MouseButtonPress:
    case QEvent::MouseButtonRelease:
    case QEvent::MouseButtonDblClick:
      return handler_(static_cast<QMouseEvent *>(event));
    default:
      return QObject::eventFilter(watched, event);
    }
  }

private:
  MouseEventHandler handler_;
};
