std::atomic<std::uint32_t> next_dialog_request_id{1};

std::uint32_t qt_show_open_file_dialog(std::uint32_t window_id, rust::Str title, rust::Str filter, bool multiple) {
  std::uint32_t request_id = next_dialog_request_id.fetch_add(1);

  QWidget *parent = nullptr;
  if (window_id != 0 && g_host && g_host->started()) {
    parent = g_host->registry().widget_ptr(window_id);
  }

  QString qtitle = QString::fromUtf8(title.data(), static_cast<int>(title.size()));
  QString qfilter = QString::fromUtf8(filter.data(), static_cast<int>(filter.size()));

  auto *dialog = new QFileDialog(parent, qtitle, QString(), qfilter);
  dialog->setFileMode(multiple ? QFileDialog::ExistingFiles : QFileDialog::ExistingFile);
  dialog->setOption(QFileDialog::DontUseNativeDialog, false);
  dialog->setAttribute(Qt::WA_DeleteOnClose);

  QObject::connect(dialog, &QFileDialog::finished, dialog, [request_id, dialog](int result) {
    rust::Vec<rust::String> paths;
    if (result == QDialog::Accepted) {
      const auto selected = dialog->selectedFiles();
      for (const auto &f : selected) {
        paths.push_back(rust::String(f.toUtf8().constData(), f.toUtf8().size()));
      }
    }
    qt_solid_spike::qt::qt_file_dialog_result(request_id, paths);
    request_qt_pump();
  });

  dialog->open();
  return request_id;
}

std::uint32_t qt_show_save_file_dialog(std::uint32_t window_id, rust::Str title, rust::Str filter, rust::Str default_name) {
  std::uint32_t request_id = next_dialog_request_id.fetch_add(1);

  QWidget *parent = nullptr;
  if (window_id != 0 && g_host && g_host->started()) {
    parent = g_host->registry().widget_ptr(window_id);
  }

  QString qtitle = QString::fromUtf8(title.data(), static_cast<int>(title.size()));
  QString qfilter = QString::fromUtf8(filter.data(), static_cast<int>(filter.size()));
  QString qdefault = QString::fromUtf8(default_name.data(), static_cast<int>(default_name.size()));

  auto *dialog = new QFileDialog(parent, qtitle, qdefault, qfilter);
  dialog->setAcceptMode(QFileDialog::AcceptSave);
  dialog->setFileMode(QFileDialog::AnyFile);
  dialog->setOption(QFileDialog::DontUseNativeDialog, false);
  dialog->setAttribute(Qt::WA_DeleteOnClose);

  QObject::connect(dialog, &QFileDialog::finished, dialog, [request_id, dialog](int result) {
    rust::Vec<rust::String> paths;
    if (result == QDialog::Accepted) {
      const auto selected = dialog->selectedFiles();
      for (const auto &f : selected) {
        paths.push_back(rust::String(f.toUtf8().constData(), f.toUtf8().size()));
      }
    }
    qt_solid_spike::qt::qt_file_dialog_result(request_id, paths);
    request_qt_pump();
  });

  dialog->open();
  return request_id;
}
