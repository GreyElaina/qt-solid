struct RasterizedGlyphInfo {
    double x;
    double y;
    std::uint32_t width;
    std::uint32_t height;
    double bearing_x;
    double bearing_y;
    double scale_factor;
    std::vector<std::uint8_t> pixels; // RGBA8 premultiplied
    int run_index;
};

struct GlyphCollectionResult {
    QPainterPath outline_path;
    std::vector<RasterizedGlyphInfo> rasterized_glyphs;
};

static RasterizedGlyphInfo rasterize_color_glyph(
    const QRawFont& raw_font,
    quint32 glyph_id,
    QPointF position,
    int run_index)
{
    QRectF bbox = raw_font.boundingRect(glyph_id);
    if (bbox.isEmpty()) {
        return {position.x(), position.y(), 0, 0, 0.0, 0.0, 1.0, {}, run_index};
    }

    // Scale up for HiDPI so the rasterized bitmap matches device pixels.
    double dpr = 1.0;
    if (auto* screen = QGuiApplication::primaryScreen()) {
        dpr = screen->devicePixelRatio();
    }
    QTransform scale_transform;
    scale_transform.scale(dpr, dpr);

    QImage img = raw_font.alphaMapForGlyph(glyph_id, QRawFont::PixelAntialiasing, scale_transform);
    if (img.isNull() || img.width() <= 0 || img.height() <= 0) {
        return {position.x(), position.y(), 0, 0, 0.0, 0.0, dpr, {}, run_index};
    }

    // Convert to RGBA8 premultiplied for vello/wgpu.
    img = img.convertToFormat(QImage::Format_RGBA8888_Premultiplied);

    std::uint32_t w = static_cast<std::uint32_t>(img.width());
    std::uint32_t h = static_cast<std::uint32_t>(img.height());
    std::vector<std::uint8_t> pixels(w * h * 4);
    for (std::uint32_t row = 0; row < h; ++row) {
        const uchar* src = img.constScanLine(row);
        std::memcpy(pixels.data() + row * w * 4, src, w * 4);
    }

    // Bearing is in font units (at pixelSize), bitmap is at dpr scale.
    // Report pixel dimensions at device scale; Rust side will place using
    // bearing + (w/dpr, h/dpr) as logical size.
    return {
        position.x(),
        position.y(),
        w, h,
        bbox.x(),
        bbox.y(),
        dpr,
        std::move(pixels),
        run_index,
    };
}

// Collect glyphs from a single-style layout.
// Outlines go to path; color glyphs (empty pathForGlyph) are rasterized.
static GlyphCollectionResult collect_glyphs_with_color_fallback(const QTextLayout& layout) {
    GlyphCollectionResult result;
    const auto glyph_runs = layout.glyphRuns();
    for (const QGlyphRun& run : glyph_runs) {
        QRawFont raw_font = run.rawFont();
        const auto indexes = run.glyphIndexes();
        const auto positions = run.positions();

        for (int i = 0; i < indexes.size(); ++i) {
            QPainterPath glyph_path = raw_font.pathForGlyph(indexes[i]);
            if (!glyph_path.isEmpty()) {
                glyph_path.translate(positions[i]);
                result.outline_path.addPath(glyph_path);
            } else {
                auto rasterized = rasterize_color_glyph(raw_font, indexes[i], positions[i], 0);
                if (rasterized.width > 0 && rasterized.height > 0) {
                    result.rasterized_glyphs.push_back(std::move(rasterized));
                }
            }
        }
    }
    return result;
}

// Convert rasterized glyphs into CXX wire types.
static void fill_rasterized_glyph_wire(
    const GlyphCollectionResult& gr,
    rust::Vec<QtRasterizedGlyph>& out_glyphs)
{
    for (const auto& rg : gr.rasterized_glyphs) {
        QtRasterizedGlyph wire;
        wire.x = rg.x;
        wire.y = rg.y;
        wire.width = rg.width;
        wire.height = rg.height;
        wire.bearing_x = rg.bearing_x;
        wire.bearing_y = rg.bearing_y;
        wire.scale_factor = rg.scale_factor;
        wire.run_index = static_cast<std::uint32_t>(rg.run_index);
        wire.pixels.reserve(rg.pixels.size());
        for (auto byte : rg.pixels) {
            wire.pixels.push_back(byte);
        }
        out_glyphs.push_back(std::move(wire));
    }
}

// Helper: construct QFont from common parameters.
QFont make_qfont(rust::Str font_family, double font_size, std::int32_t font_weight, bool font_italic) {
  QFont font;
  if (font_family.size() > 0) {
    font = QFont(QString::fromUtf8(font_family.data(), static_cast<int>(font_family.size())));
  } else {
    font = QGuiApplication::font();
  }
  font.setStyleStrategy(QFont::PreferOutline);
  font.setPointSizeF(font_size);
  if (font_weight > 0) {
    font.setWeight(static_cast<QFont::Weight>(font_weight));
  }
  if (font_italic) {
    font.setItalic(true);
  }
  return font;
}

// Helper: collect all glyphs from a QTextLayout into a single QPainterPath.
QPainterPath collect_glyph_path(const QTextLayout &layout) {
  QPainterPath combined_path;
  const auto glyph_runs = layout.glyphRuns();
  for (const QGlyphRun &run : glyph_runs) {
    QRawFont raw_font = run.rawFont();
    const auto indexes = run.glyphIndexes();
    const auto positions = run.positions();
    for (int i = 0; i < indexes.size(); ++i) {
      QPainterPath glyph_path = raw_font.pathForGlyph(indexes[i]);
      if (!glyph_path.isEmpty()) {
        glyph_path.translate(positions[i]);
        combined_path.addPath(glyph_path);
      }
    }
  }
  return combined_path;
}

// Helper: serialize a QPainterPath into Vec<QtShapedPathEl>.
rust::Vec<QtShapedPathEl> serialize_painter_path(const QPainterPath &path) {
  rust::Vec<QtShapedPathEl> elements;
  const int count = path.elementCount();
  for (int i = 0; i < count; /* advanced in loop */) {
    const QPainterPath::Element &el = path.elementAt(i);
    switch (el.type) {
    case QPainterPath::MoveToElement:
      elements.push_back(QtShapedPathEl{0, el.x, el.y, 0, 0, 0, 0});
      ++i;
      break;
    case QPainterPath::LineToElement:
      elements.push_back(QtShapedPathEl{1, el.x, el.y, 0, 0, 0, 0});
      ++i;
      break;
    case QPainterPath::CurveToElement: {
      double c1x = el.x, c1y = el.y;
      double c2x = 0, c2y = 0, ex = 0, ey = 0;
      if (i + 1 < count) {
        const auto &d1 = path.elementAt(i + 1);
        c2x = d1.x;
        c2y = d1.y;
      }
      if (i + 2 < count) {
        const auto &d2 = path.elementAt(i + 2);
        ex = d2.x;
        ey = d2.y;
      }
      elements.push_back(QtShapedPathEl{2, c1x, c1y, c2x, c2y, ex, ey});
      i += 3;
      break;
    }
    default:
      ++i;
      break;
    }
  }
  return elements;
}
