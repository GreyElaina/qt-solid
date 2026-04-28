#pragma once

#include <cmath>
#include <vector>

#include <QtWidgets/QLayout>
#include <QtWidgets/QWidget>

#include "native/src/layout/registry_ffi.rs.h"

class QTaffyLayout : public QLayout {
public:
  explicit QTaffyLayout(QWidget *parent = nullptr)
      : QLayout(parent), dirty_(true) {
    auto handle = qt_taffy::engine_create();
    engine_id_ = handle.engine_id;
    root_node_ = handle.root_node;
    setContentsMargins(0, 0, 0, 0);
  }

  ~QTaffyLayout() override {
    while (QLayoutItem *item = takeAt(0))
      delete item;
    qt_taffy::engine_remove_node(engine_id_, root_node_);
    qt_taffy::engine_destroy(engine_id_);
  }

  void addItem(QLayoutItem *item) override {
    auto node = qt_taffy::engine_create_node(engine_id_);
    items_.push_back(item);
    child_nodes_.push_back(node);
    sync_children();
    invalidate();
  }

  void insertWidget(int index, QWidget *widget) {
    addChildWidget(widget);
    auto *item = new QWidgetItem(widget);
    auto node = qt_taffy::engine_create_node(engine_id_);
    if (index < 0 || index >= static_cast<int>(items_.size())) {
      items_.push_back(item);
      child_nodes_.push_back(node);
    } else {
      items_.insert(items_.begin() + index, item);
      child_nodes_.insert(child_nodes_.begin() + index, node);
    }
    sync_children();
    invalidate();
  }

  int count() const override { return static_cast<int>(items_.size()); }

  QLayoutItem *itemAt(int index) const override {
    if (index >= 0 && index < static_cast<int>(items_.size()))
      return items_[index];
    return nullptr;
  }

  QLayoutItem *takeAt(int index) override {
    if (index < 0 || index >= static_cast<int>(items_.size()))
      return nullptr;
    QLayoutItem *item = items_[index];
    auto node = child_nodes_[index];
    items_.erase(items_.begin() + index);
    child_nodes_.erase(child_nodes_.begin() + index);
    qt_taffy::engine_remove_node(engine_id_, node);
    sync_children();
    invalidate();
    return item;
  }

  void setGeometry(const QRect &rect) override {
    QLayout::setGeometry(rect);

    if (items_.empty())
      return;

    const QRect inner = contentsRect();

    qt_taffy::engine_set_width_px(engine_id_, root_node_,
                                  static_cast<float>(inner.width()));
    qt_taffy::engine_set_height_px(engine_id_, root_node_,
                                   static_cast<float>(inner.height()));

    for (std::size_t i = 0; i < items_.size(); ++i) {
      const QSize hint = measure_child_hint(items_[i]);
      qt_taffy::engine_set_fixed_measure(engine_id_, child_nodes_[i],
                                         static_cast<float>(hint.width()),
                                         static_cast<float>(hint.height()));
    }

    qt_taffy::engine_compute_layout(engine_id_, root_node_,
                                    static_cast<float>(inner.width()),
                                    static_cast<float>(inner.height()));

    for (std::size_t i = 0; i < items_.size(); ++i) {
      auto layout = qt_taffy::engine_get_layout(engine_id_, child_nodes_[i]);
      items_[i]->setGeometry(QRect(
          inner.x() + static_cast<int>(std::round(layout.x)),
          inner.y() + static_cast<int>(std::round(layout.y)),
          static_cast<int>(std::round(layout.width)),
          static_cast<int>(std::round(layout.height))));
    }

    dirty_ = false;
  }

  QSize sizeHint() const override {
    if (!dirty_ && cached_size_hint_.isValid())
      return cached_size_hint_;
    cached_size_hint_ = compute_hint(std::numeric_limits<float>::infinity(),
                                     std::numeric_limits<float>::infinity());
    return cached_size_hint_;
  }

  QSize minimumSize() const override {
    if (!dirty_ && cached_min_size_.isValid())
      return cached_min_size_;

    qt_taffy::engine_set_width_auto(engine_id_, root_node_);
    qt_taffy::engine_set_height_auto(engine_id_, root_node_);
    for (std::size_t i = 0; i < items_.size(); ++i) {
      const QSize min = measure_child_min(items_[i]);
      qt_taffy::engine_set_fixed_measure(engine_id_, child_nodes_[i],
                                         static_cast<float>(min.width()),
                                         static_cast<float>(min.height()));
    }
    qt_taffy::engine_compute_layout(engine_id_, root_node_,
                                    std::numeric_limits<float>::infinity(),
                                    std::numeric_limits<float>::infinity());
    auto root_layout = qt_taffy::engine_get_layout(engine_id_, root_node_);
    cached_min_size_ = QSize(static_cast<int>(std::ceil(root_layout.width)),
                             static_cast<int>(std::ceil(root_layout.height)));
    return cached_min_size_;
  }

  void invalidate() override {
    dirty_ = true;
    cached_size_hint_ = QSize();
    cached_min_size_ = QSize();
    for (auto *item : items_) {
      if (item) item->invalidate();
    }
    QLayout::invalidate();
  }

  Qt::Orientations expandingDirections() const override { return {}; }

  uint32_t engine_id() const { return engine_id_; }
  uint32_t root_node() const { return root_node_; }
  uint32_t child_node(int index) const { return child_nodes_[index]; }

private:
  void sync_children() {
    ::rust::Slice<const uint32_t> slice(child_nodes_.data(),
                                      child_nodes_.size());
    qt_taffy::engine_set_children(engine_id_, root_node_, slice);
  }

  static QSize measure_child_hint(QLayoutItem *item) {
    if (QWidget *w = item->widget()) {
      return w->sizeHint()
          .expandedTo(w->minimumSizeHint())
          .expandedTo(w->minimumSize());
    }
    return item->sizeHint();
  }

  static QSize measure_child_min(QLayoutItem *item) {
    if (QWidget *w = item->widget()) {
      return w->minimumSizeHint().expandedTo(w->minimumSize());
    }
    return item->minimumSize();
  }

  QSize compute_hint(float avail_w, float avail_h) const {
    qt_taffy::engine_set_width_auto(engine_id_, root_node_);
    qt_taffy::engine_set_height_auto(engine_id_, root_node_);
    for (std::size_t i = 0; i < items_.size(); ++i) {
      const QSize hint = measure_child_hint(items_[i]);
      qt_taffy::engine_set_fixed_measure(engine_id_, child_nodes_[i],
                                         static_cast<float>(hint.width()),
                                         static_cast<float>(hint.height()));
    }
    qt_taffy::engine_compute_layout(engine_id_, root_node_, avail_w, avail_h);
    auto root_layout = qt_taffy::engine_get_layout(engine_id_, root_node_);
    return QSize(static_cast<int>(std::ceil(root_layout.width)),
                 static_cast<int>(std::ceil(root_layout.height)));
  }

  uint32_t engine_id_;
  uint32_t root_node_;
  std::vector<QLayoutItem *> items_;
  std::vector<uint32_t> child_nodes_;
  bool dirty_;
  mutable QSize cached_size_hint_;
  mutable QSize cached_min_size_;
};
