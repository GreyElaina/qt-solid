use std::{
    env,
    path::{Path, PathBuf},
};

#[path = "build/qt_taffy_layout.rs"]
mod qt_taffy_layout;
#[path = "build/qt_wgpu_renderer.rs"]
mod qt_wgpu_renderer;

use qt_build_utils::QtBuild;

fn add_include_if_exists(build: &mut cc::Build, path: impl AsRef<Path>) {
    let path = path.as_ref();
    if path.exists() {
        build.include(path);
    }
}

fn find_qt_private_include_dirs(qt_build: &QtBuild) -> Vec<PathBuf> {
    let qt_version = qt_build.version().to_string();
    let mut include_dirs = Vec::new();

    for include_path in qt_build.include_paths() {
        let versioned_root = include_path.join(&qt_version);
        let qtcore_private = versioned_root.join("QtCore/private");
        let qtgui_private = versioned_root.join("QtGui/private");
        let qtwidgets_private = versioned_root.join("QtWidgets/private");
        if qtcore_private.exists() || qtgui_private.exists() || qtwidgets_private.exists() {
            include_dirs.push(versioned_root.clone());
            let qtcore_dir = versioned_root.join("QtCore");
            if qtcore_dir.exists() {
                include_dirs.push(qtcore_dir);
            }
            let qtgui_dir = versioned_root.join("QtGui");
            if qtgui_dir.exists() {
                include_dirs.push(qtgui_dir);
            }
            let qtwidgets_dir = versioned_root.join("QtWidgets");
            if qtwidgets_dir.exists() {
                include_dirs.push(qtwidgets_dir);
            }
        }
    }

    // Add mkspecs include dir for qplatformdefs.h (needed by QWidgetLineControl).
    if let Ok(output) =
        std::process::Command::new(env::var("QMAKE").unwrap_or_else(|_| "qmake".to_string()))
            .args(["-query", "QT_HOST_DATA"])
            .output()
    {
        if output.status.success() {
            let host_data = String::from_utf8_lossy(&output.stdout).trim().to_string();
            let mkspecs_dir = PathBuf::from(&host_data).join("mkspecs");
            let spec = match env::var("CARGO_CFG_TARGET_OS").as_deref() {
                Ok("macos") => "macx-clang",
                Ok("windows") => "win32-msvc",
                Ok("linux") => "linux-g++",
                _ => "macx-clang",
            };
            let spec_dir = mkspecs_dir.join(spec);
            if spec_dir.exists() {
                include_dirs.push(spec_dir);
            }
        }
    }

    include_dirs.sort();
    include_dirs.dedup();
    include_dirs
}

fn find_qt_gui_rhi_include_dirs(qt_build: &QtBuild) -> Vec<PathBuf> {
    let qt_version = qt_build.version().to_string();
    let mut include_dirs = Vec::new();

    for include_path in qt_build.include_paths() {
        let candidate = include_path.join(&qt_version).join("QtGui");
        if candidate.join("rhi/qrhi.h").exists() {
            include_dirs.push(candidate);
        }
    }

    include_dirs.sort();
    include_dirs.dedup();
    include_dirs
}

fn assert_supported_macos_qt_version(qt_build: &QtBuild) {
    let version = qt_build.version();
    let supported = version.major == 6 && version.minor == 10;
    assert!(
        supported,
        "macOS native build requires Qt 6.10.x for Cocoa dispatcher private shim; found {}",
        version
    );
}

fn main() {
    let qt_wgpu_renderer = qt_wgpu_renderer::spec();
    let qt_taffy_layout = qt_taffy_layout::spec();

    println!("cargo:rerun-if-changed=build/qt_wgpu_renderer.rs");
    println!("cargo:rerun-if-changed=include/qt/widget_host.h");
    println!("cargo:rerun-if-changed=include/qt/ffi.h");

    println!("cargo:rerun-if-changed=src/qt/cpp/ffi.cpp");
    println!("cargo:rerun-if-changed=src/qt/cpp/util.cpp");
    println!("cargo:rerun-if-changed=src/qt/cpp/inspector.cpp");
    println!("cargo:rerun-if-changed=src/qt/cpp/text.cpp");
    println!("cargo:rerun-if-changed=src/qt/cpp/window/text_edit.cpp");
    println!("cargo:rerun-if-changed=src/qt/cpp/window/widget.cpp");
    println!("cargo:rerun-if-changed=src/qt/cpp/window/input.cpp");
    println!("cargo:rerun-if-changed=src/qt/cpp/window/compositor.cpp");
    println!("cargo:rerun-if-changed=src/qt/cpp/widget_tree/layout.cpp");
    println!("cargo:rerun-if-changed=src/qt/cpp/widget_tree/mouse.cpp");
    println!("cargo:rerun-if-changed=src/qt/cpp/widget_tree/tree.cpp");
    println!("cargo:rerun-if-changed=src/qt/cpp/platform/clipboard.cpp");
    println!("cargo:rerun-if-changed=src/qt/cpp/platform/file_dialogs.cpp");
    println!("cargo:rerun-if-changed=src/qt/cpp/platform/appearance.cpp");
    println!("cargo:rerun-if-changed=src/qt/cpp/platform/popup_monitor.mm");
    // accessibility_bridge.mm replaced by Rust accessibility_bridge.rs
    println!("cargo:rerun-if-changed=src/qt/ffi.rs");

    println!("cargo:rerun-if-changed=src/qt/runtime.rs");
    println!("cargo:rerun-if-changed=src/qt/mod.rs");
    println!("cargo:rerun-if-changed=src/layout/ffi.rs");
    println!("cargo:rerun-if-changed=src/layout/registry_ffi.rs");

    qt_wgpu_renderer.emit_rerun_if_changed();
    qt_taffy_layout.emit_rerun_if_changed();
    napi_build::setup();

    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR"));

    let qt_build = QtBuild::new(vec![
        "Core".to_owned(),
        "Gui".to_owned(),
        "Widgets".to_owned(),
    ])
    .expect("failed to detect Qt installation");

    let qt_include_dirs = qt_build.include_paths();
    let qt_private_include_dirs = find_qt_private_include_dirs(&qt_build);
    let qt_gui_rhi_include_dirs = find_qt_gui_rhi_include_dirs(&qt_build);

    if env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("macos") {
        assert_supported_macos_qt_version(&qt_build);
    }

    let mut build = cxx_build::bridges([
        "src/qt/ffi.rs",
        "src/layout/ffi.rs",
        "src/layout/registry_ffi.rs",
    ]);
    build.file("src/qt/cpp/ffi.cpp");
    qt_wgpu_renderer.add_cpp_sources(&mut build);
    if env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("macos") {
        qt_wgpu_renderer.add_objc_sources(&mut build);
        // accessibility_bridge now in pure Rust
        build.flag_if_supported("-fblocks");
        build.file("src/qt/cpp/platform/popup_monitor.mm");
    }
    let windows_msvc = env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows")
        && env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc");
    if windows_msvc {
        build.std("c++20");
        build.define("NOMINMAX", None);
        build.flag_if_supported("/Zc:__cplusplus");
        build.flag_if_supported("/EHsc");
        build.flag_if_supported("/permissive-");
    } else {
        build.std("c++17");
    }

    add_include_if_exists(&mut build, "include");
    // qt-host crate headers (qt_host/host.h)
    add_include_if_exists(&mut build, "../qt-host/include");
    qt_wgpu_renderer.add_include_dirs(&mut build);
    qt_taffy_layout.add_include_dirs(&mut build);
    add_include_if_exists(&mut build, &out_dir);

    // Vendored libuv headers (compile-time only; symbols resolved dynamically at runtime).
    // add_include_if_exists(&mut build, "../../../../third_party/libuv-include");
    add_include_if_exists(
        &mut build,
        concat!(env!("CARGO_WORKSPACE_DIR"), "/third_party/libuv-include"),
    );

    for include_dir in qt_include_dirs {
        add_include_if_exists(&mut build, include_dir);
    }

    for include_dir in qt_private_include_dirs {
        add_include_if_exists(&mut build, include_dir);
    }

    for include_dir in qt_gui_rhi_include_dirs {
        add_include_if_exists(&mut build, include_dir);
    }

    if env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("linux") {
        println!("cargo:rustc-link-lib=dl");
    }
    if env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("macos") {
        println!("cargo:rustc-link-lib=framework=ApplicationServices");
    }

    qt_build.cargo_link_libraries(&mut build);
    build.compile("qt-solid-native");
}
