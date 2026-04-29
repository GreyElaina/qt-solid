rust::String qt_clipboard_get_text() {
  auto *clipboard = QGuiApplication::clipboard();
  const QByteArray utf8 = clipboard->text().toUtf8();
  return rust::String(utf8.constData(), utf8.size());
}

void qt_clipboard_set_text(rust::Str text) {
  auto *clipboard = QGuiApplication::clipboard();
  clipboard->setText(QString::fromUtf8(text.data(), static_cast<int>(text.size())));
}

bool qt_clipboard_has_text() {
  auto *clipboard = QGuiApplication::clipboard();
  const auto *mime = clipboard->mimeData();
  return mime != nullptr && mime->hasText();
}

rust::Vec<rust::String> qt_clipboard_formats() {
  auto *clipboard = QGuiApplication::clipboard();
  const auto *mime = clipboard->mimeData();
  rust::Vec<rust::String> result;
  if (mime == nullptr) {
    return result;
  }
  for (const auto &fmt : mime->formats()) {
    const QByteArray utf8 = fmt.toUtf8();
    result.push_back(rust::String(utf8.constData(), utf8.size()));
  }
  return result;
}

rust::Vec<std::uint8_t> qt_clipboard_get(rust::Str mime) {
  auto *clipboard = QGuiApplication::clipboard();
  const auto *data = clipboard->mimeData();
  rust::Vec<std::uint8_t> result;
  if (data == nullptr) {
    return result;
  }
  const QString qmime = QString::fromUtf8(mime.data(), static_cast<int>(mime.size()));
  const QByteArray bytes = data->data(qmime);
  result.reserve(bytes.size());
  for (int i = 0; i < bytes.size(); ++i) {
    result.push_back(static_cast<std::uint8_t>(bytes[i]));
  }
  return result;
}

void qt_clipboard_clear() {
  QGuiApplication::clipboard()->clear();
}

void qt_clipboard_set(rust::Vec<QtClipboardEntry> entries) {
  auto *mime_data = new QMimeData();
  for (const auto &entry : entries) {
    const QString qmime = QString::fromUtf8(entry.mime.data(), static_cast<int>(entry.mime.size()));
    const QByteArray bytes(reinterpret_cast<const char *>(entry.data.data()),
                           static_cast<int>(entry.data.size()));
    mime_data->setData(qmime, bytes);
  }
  QGuiApplication::clipboard()->setMimeData(mime_data);
}
