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
