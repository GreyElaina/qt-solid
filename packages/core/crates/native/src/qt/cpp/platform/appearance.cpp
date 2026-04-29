std::uint8_t qt_system_color_scheme() {
#if QT_VERSION >= QT_VERSION_CHECK(6, 5, 0)
  auto scheme = QGuiApplication::styleHints()->colorScheme();
  switch (scheme) {
  case Qt::ColorScheme::Light:
    return 1;
  case Qt::ColorScheme::Dark:
    return 2;
  default:
    return 0;
  }
#else
  const QColor bg = QGuiApplication::palette().color(QPalette::Window);
  return bg.lightness() < 128 ? 2 : 1;
#endif
}

QtScreenDpiInfo qt_screen_dpi_info(std::uint32_t id) {
  if (!g_host || !g_host->started()) {
    throw_error("call QtApp.start before reading screen DPI");
  }

  auto *widget = g_host->registry().widget_ptr(id);
  if (!widget) {
    throw_error("invalid widget id for screen DPI query");
  }

  auto *screen = widget->screen();
  if (!screen) {
    // Fallback to primary screen.
    screen = QGuiApplication::primaryScreen();
  }
  if (!screen) {
    return QtScreenDpiInfo{96.0, 96.0, 1.0, {0, 0, 0, 0}};
  }

  auto avail = screen->availableGeometry();
  return QtScreenDpiInfo{
      screen->logicalDotsPerInchX(),
      screen->logicalDotsPerInchY(),
      screen->devicePixelRatio(),
      QtScreenGeometry{avail.x(), avail.y(), avail.width(), avail.height()},
  };
}
