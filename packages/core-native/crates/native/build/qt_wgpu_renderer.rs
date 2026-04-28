use std::path::Path;

pub(crate) struct QtWgpuRendererBuildSpec {
    pub(crate) rerun_if_changed: &'static [&'static str],
    pub(crate) include_dirs: &'static [&'static str],
    pub(crate) cpp_sources: &'static [&'static str],
}

pub(crate) fn spec() -> QtWgpuRendererBuildSpec {
    QtWgpuRendererBuildSpec {
        rerun_if_changed: &[
            "../qt-compositor/include/qt_wgpu_platform.h",
            "../qt-compositor/src/cpp/qt_wgpu_platform.mm",
        ],
        include_dirs: &["../qt-compositor/include"],
        cpp_sources: &[
            "../qt-compositor/src/cpp/qt_wgpu_platform.mm",
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
