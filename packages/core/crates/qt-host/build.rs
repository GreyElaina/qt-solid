use std::{
    env,
    path::{Path, PathBuf},
};

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

    // mkspecs include dir for qplatformdefs.h
    if let Ok(output) = std::process::Command::new(
        env::var("QMAKE").unwrap_or_else(|_| "qmake".to_string()),
    )
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

fn main() {
    println!("cargo:rerun-if-changed=src/ffi.rs");
    println!("cargo:rerun-if-changed=src/cpp/host/host.cpp");
    println!("cargo:rerun-if-changed=src/cpp/host/uv.cpp");
    println!("cargo:rerun-if-changed=src/cpp/host/wait_bridge.cpp");
    println!("cargo:rerun-if-changed=src/cpp/host/state.cpp");
    println!("cargo:rerun-if-changed=src/cpp/macos/event_buffer.mm");
    println!("cargo:rerun-if-changed=include/qt_host/host.h");
    println!("cargo:rerun-if-changed=include/qt_host/macos/event_buffer.h");
    println!("cargo:rerun-if-changed=include/qt_host/macos/cocoa_dispatcher_shim.h");

    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR"));

    let qt_build = QtBuild::new(vec![
        "Core".to_owned(),
        "Gui".to_owned(),
        "Widgets".to_owned(),
    ])
    .expect("failed to detect Qt installation");

    let qt_include_dirs = qt_build.include_paths();
    let qt_private_include_dirs = find_qt_private_include_dirs(&qt_build);

    let mut build = cxx_build::bridge("src/ffi.rs");
    // host.cpp is an amalgam translation unit that #includes uv.cpp, wait_bridge.cpp,
    // and state.cpp. Those files are NOT separate compilation units.
    build.file("src/cpp/host/host.cpp");

    if env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("macos") {
        build.file("src/cpp/macos/event_buffer.mm");
        build.flag_if_supported("-fblocks");
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

    // Include dirs: qt-host's own include dir + vendored libuv headers
    add_include_if_exists(&mut build, "include");
    add_include_if_exists(&mut build, &out_dir);
    // Vendored libuv headers (compile-time only; symbols resolved at runtime)
    add_include_if_exists(&mut build, "../../../../third_party/libuv-include");

    for include_dir in qt_include_dirs {
        add_include_if_exists(&mut build, include_dir);
    }
    for include_dir in qt_private_include_dirs {
        add_include_if_exists(&mut build, include_dir);
    }

    if env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("linux") {
        println!("cargo:rustc-link-lib=dl");
    }
    if env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("macos") {
        println!("cargo:rustc-link-lib=framework=ApplicationServices");
        println!("cargo:rustc-link-lib=framework=QuartzCore");
    }

    qt_build.cargo_link_libraries(&mut build);
    build.compile("qt-host-cpp");
}
