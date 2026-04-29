use std::path::Path;

pub(crate) struct QtTaffyLayoutBuildSpec {
    pub(crate) rerun_if_changed: &'static [&'static str],
    pub(crate) include_dirs: &'static [&'static str],
}

pub(crate) fn spec() -> QtTaffyLayoutBuildSpec {
    QtTaffyLayoutBuildSpec {
        rerun_if_changed: &[
            "include/qt_taffy_layout.h",
        ],
        include_dirs: &[],
    }
}

impl QtTaffyLayoutBuildSpec {
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
}
