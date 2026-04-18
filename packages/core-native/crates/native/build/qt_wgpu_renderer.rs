use std::path::Path;

pub(crate) struct QtWgpuRendererBuildSpec {
    pub(crate) rerun_if_changed: &'static [&'static str],
    pub(crate) include_dirs: &'static [&'static str],
    pub(crate) cpp_sources: &'static [&'static str],
}

pub(crate) fn spec() -> QtWgpuRendererBuildSpec {
    QtWgpuRendererBuildSpec {
        rerun_if_changed: &[
            "../qt-wgpu-renderer/include/texture_paint_host_widget.h",
            "../qt-wgpu-renderer/include/qt_wgpu_rhi.h",
            "../qt-wgpu-renderer/src/cpp/qt_wgpu_rhi.cpp",
            "../qt-wgpu-renderer/src/cpp/texture_paint_host_widget.cpp",
        ],
        include_dirs: &["../qt-wgpu-renderer/include"],
        cpp_sources: &[
            "../qt-wgpu-renderer/src/cpp/qt_wgpu_rhi.cpp",
            "../qt-wgpu-renderer/src/cpp/texture_paint_host_widget.cpp",
        ],
    }
}

impl QtWgpuRendererBuildSpec {
    pub(crate) fn emit_rerun_if_changed(&self) {
        for path in self.rerun_if_changed {
            println!("cargo:rerun-if-changed={path}");
        }
    }

    pub(crate) fn add_include_dirs(&self, build: &mut cc::Build) {
        for path in self.include_dirs {
            let path = Path::new(path);
            if path.exists() {
                build.include(path);
            }
        }
    }

    pub(crate) fn add_cpp_sources(&self, build: &mut cc::Build) {
        for path in self.cpp_sources {
            build.file(path);
        }
    }
}
