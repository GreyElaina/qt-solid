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

class WidgetEventForwarder final : public QObject {
public:
  using EventHandler = std::function<void()>;

  WidgetEventForwarder(QWidget *target, QEvent::Type event_type,
                       EventHandler handler)
      : QObject(target), target_(target), event_type_(event_type),
        handler_(std::move(handler)) {
    target_->installEventFilter(this);
  }

protected:
  bool eventFilter(QObject *watched, QEvent *event) override {
    if (watched == target_ && event && event->type() == event_type_) {
      handler_();
    }

    return QObject::eventFilter(watched, event);
  }

private:
  QWidget *target_ = nullptr;
  QEvent::Type event_type_;
  EventHandler handler_;
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
