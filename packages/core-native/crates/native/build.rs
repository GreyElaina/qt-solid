use std::{
    env, fs,
    path::{Path, PathBuf},
    process::Command,
};

#[path = "build/qt_wgpu_renderer.rs"]
mod qt_wgpu_renderer;

use qt_build_utils::QtBuild;
use qt_solid_native_build::{
    render_opaque_dispatch_cpp, render_qt_node_methods_rs, render_qt_widget_entities_rs,
    render_widget_create_cases_cpp, render_widget_event_mounts_cpp,
    render_widget_host_includes_cpp, render_widget_host_method_dispatch_cpp,
    render_widget_kind_enum_cpp, render_widget_kind_from_tag_cpp, render_widget_kind_values_cpp,
    render_widget_override_classes_cpp, render_widget_probe_cases_cpp,
    render_widget_prop_dispatch_cpp, render_widget_top_level_cases_cpp,
};

fn add_include_if_exists(build: &mut cc::Build, path: impl AsRef<Path>) {
    let path = path.as_ref();
    if path.exists() {
        build.include(path);
    }
}

fn resolve_node_include_dir() -> Option<PathBuf> {
    if let Ok(node_dir) = env::var("npm_config_nodedir") {
        let node_dir = PathBuf::from(node_dir);
        for include_dir in [node_dir.join("include/node"), node_dir.clone()] {
            if include_dir.exists() {
                return Some(include_dir);
            }
        }
    }

    if let Ok(include_dir) = env::var("NODE_INCLUDE_DIR") {
        let include_dir = PathBuf::from(include_dir);
        if include_dir.exists() {
            return Some(include_dir);
        }
    }

    let output = Command::new("node")
        .args(["-p", "process.execPath"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }

    let exec_path = PathBuf::from(String::from_utf8(output.stdout).ok()?.trim());
    let exec_dir = exec_path.parent()?;
    let candidates = [
        exec_dir.join("include/node"),
        exec_dir.parent()?.join("include/node"),
    ];
    candidates.into_iter().find(|include_dir| include_dir.exists())
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

fn qmake_query(key: &str) -> Option<String> {
    let output = Command::new("qmake").args(["-query", key]).output().ok()?;
    if !output.status.success() {
        return None;
    }

    let value = String::from_utf8(output.stdout).ok()?;
    let value = value.trim();
    if value.is_empty() {
        None
    } else {
        Some(value.to_owned())
    }
}

fn resolve_qsb_tool() -> PathBuf {
    if let Ok(path) = env::var("QSB") {
        let path = PathBuf::from(path);
        if path.exists() {
            return path;
        }
    }

    if let Some(host_bins) = qmake_query("QT_HOST_BINS") {
        let path = PathBuf::from(host_bins).join("qsb");
        if path.exists() {
            return path;
        }
    }

    PathBuf::from("qsb")
}

fn compile_qsb_shader(qsb: &Path, input: &Path, output: &Path) {
    let status = Command::new(qsb)
        .args(["--qt6", "-s", "-o"])
        .arg(output)
        .arg(input)
        .status()
        .unwrap_or_else(|error| panic!("run {} for {}: {error}", qsb.display(), input.display()));
    assert!(
        status.success(),
        "qsb failed for {} -> {}",
        input.display(),
        output.display()
    );
}

fn render_embedded_cpp_bytes(symbol: &str, bytes: &[u8]) -> String {
    let mut out = String::new();
    out.push_str(&format!("static const unsigned char {symbol}[] = {{\n"));
    for chunk in bytes.chunks(12) {
        out.push_str("    ");
        for (index, byte) in chunk.iter().enumerate() {
            if index > 0 {
                out.push_str(", ");
            }
            out.push_str(&format!("0x{byte:02x}"));
        }
        out.push_str(",\n");
    }
    out.push_str("};\n");
    out.push_str(&format!(
        "static constexpr std::size_t {symbol}Len = sizeof({symbol});\n"
    ));
    out
}

fn render_window_compositor_shader_header(vertex_qsb: &[u8], fragment_qsb: &[u8]) -> String {
    let mut out = String::new();
    out.push_str(&render_embedded_cpp_bytes(
        "kWindowCompositorVertQsb",
        vertex_qsb,
    ));
    out.push('\n');
    out.push_str(&render_embedded_cpp_bytes(
        "kWindowCompositorFragQsb",
        fragment_qsb,
    ));
    out
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

fn write_if_changed(path: &Path, content: &str) {
    match fs::read_to_string(path) {
        Ok(existing) if existing == content => {}
        _ => fs::write(path, content).unwrap_or_else(|error| {
            panic!("write {}: {error}", path.display());
        }),
    }
}

fn main() {
    let qt_wgpu_renderer = qt_wgpu_renderer::spec();

    println!("cargo:rerun-if-env-changed=npm_config_nodedir");
    println!("cargo:rerun-if-env-changed=NODE_INCLUDE_DIR");
    println!("cargo:rerun-if-env-changed=QSB");
    println!("cargo:rerun-if-changed=build/qt_wgpu_renderer.rs");
    println!("cargo:rerun-if-changed=include/custom_paint_host_widget.h");
    println!("cargo:rerun-if-changed=include/rust_widget_binding_host.h");
    println!("cargo:rerun-if-changed=include/qt/ffi.h");
    println!("cargo:rerun-if-changed=include/qt/macos_event_buffer_bridge.h");
    println!("cargo:rerun-if-changed=include/qt_cocoa_dispatcher_private_shim.h");
    println!("cargo:rerun-if-changed=shaders/window_compositor.vert");
    println!("cargo:rerun-if-changed=shaders/window_compositor.frag");
    println!("cargo:rerun-if-changed=src/qt/cpp/ffi.cpp");
    println!("cargo:rerun-if-changed=src/qt/cpp/event.cpp");
    println!("cargo:rerun-if-changed=src/qt/cpp/uv_pump.cpp");
    println!("cargo:rerun-if-changed=src/qt/cpp/debug.cpp");
    println!("cargo:rerun-if-changed=src/qt/cpp/registry/host.cpp");
    println!("cargo:rerun-if-changed=src/qt/cpp/registry/core.cpp");
    println!("cargo:rerun-if-changed=src/qt/cpp/macos_event_buffer_bridge.mm");
    println!("cargo:rerun-if-changed=src/qt/ffi.rs");
    println!("cargo:rerun-if-changed=src/qt/ffi_host.rs");
    println!("cargo:rerun-if-changed=src/qt/runtime.rs");
    println!("cargo:rerun-if-changed=src/qt/mod.rs");
    println!("cargo:rerun-if-changed=src/window_host.rs");
    println!("cargo:rerun-if-changed=../native-build/src/lib.rs");
    println!("cargo:rerun-if-changed=../native-build/src/schema.rs");
    println!("cargo:rerun-if-changed=../native-build/src/napi_codegen.rs");
    println!("cargo:rerun-if-changed=../native-build/src/qt_codegen.rs");
    qt_wgpu_renderer.emit_rerun_if_changed();
    napi_build::setup();

    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR"));
    let qt_node_methods = out_dir.join("qt_node_methods.rs");
    write_if_changed(
        &out_dir.join("qt_widget_entities.rs"),
        &render_qt_widget_entities_rs(),
    );
    write_if_changed(&qt_node_methods, &render_qt_node_methods_rs());
    write_if_changed(
        &out_dir.join("qt_widget_host_includes.inc"),
        &render_widget_host_includes_cpp(),
    );
    write_if_changed(
        &out_dir.join("qt_widget_kind_enum.inc"),
        &render_widget_kind_enum_cpp(),
    );
    write_if_changed(
        &out_dir.join("qt_widget_kind_from_tag.inc"),
        &render_widget_kind_from_tag_cpp(),
    );
    write_if_changed(
        &out_dir.join("qt_widget_kind_values.inc"),
        &render_widget_kind_values_cpp(),
    );
    write_if_changed(
        &out_dir.join("qt_widget_top_level_cases.inc"),
        &render_widget_top_level_cases_cpp(),
    );
    write_if_changed(
        &out_dir.join("qt_widget_probe_cases.inc"),
        &render_widget_probe_cases_cpp(),
    );
    write_if_changed(
        &out_dir.join("qt_widget_create_cases.inc"),
        &render_widget_create_cases_cpp(),
    );
    write_if_changed(
        &out_dir.join("qt_widget_overrides.inc"),
        &render_widget_override_classes_cpp(),
    );
    write_if_changed(
        &out_dir.join("qt_widget_event_mounts.inc"),
        &render_widget_event_mounts_cpp(),
    );
    write_if_changed(
        &out_dir.join("qt_widget_prop_dispatch.inc"),
        &render_widget_prop_dispatch_cpp(),
    );
    write_if_changed(
        &out_dir.join("qt_widget_host_methods.inc"),
        &render_widget_host_method_dispatch_cpp(),
    );
    write_if_changed(
        &out_dir.join("qt_opaque_dispatch.inc"),
        &render_opaque_dispatch_cpp(),
    );
    let qsb = resolve_qsb_tool();
    let window_compositor_vert_qsb = out_dir.join("window_compositor.vert.qsb");
    let window_compositor_frag_qsb = out_dir.join("window_compositor.frag.qsb");
    compile_qsb_shader(
        &qsb,
        Path::new("shaders/window_compositor.vert"),
        &window_compositor_vert_qsb,
    );
    compile_qsb_shader(
        &qsb,
        Path::new("shaders/window_compositor.frag"),
        &window_compositor_frag_qsb,
    );
    let window_compositor_shader_header = render_window_compositor_shader_header(
        &fs::read(&window_compositor_vert_qsb).unwrap_or_else(|error| {
            panic!("read {}: {error}", window_compositor_vert_qsb.display())
        }),
        &fs::read(&window_compositor_frag_qsb).unwrap_or_else(|error| {
            panic!("read {}: {error}", window_compositor_frag_qsb.display())
        }),
    );
    write_if_changed(
        &out_dir.join("qt_window_compositor_shaders.inc"),
        &window_compositor_shader_header,
    );

    let qt_build = QtBuild::new(vec![
        "Core".to_owned(),
        "Gui".to_owned(),
        "Widgets".to_owned(),
    ])
    .expect("failed to detect Qt installation");

    let node_include_dir = resolve_node_include_dir();
    let qt_include_dirs = qt_build.include_paths();
    let qt_private_include_dirs = find_qt_private_include_dirs(&qt_build);
    let qt_gui_rhi_include_dirs = find_qt_gui_rhi_include_dirs(&qt_build);

    if env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("macos") {
        assert_supported_macos_qt_version(&qt_build);
    }

    let mut build = cxx_build::bridges(["src/qt/ffi.rs"]);
    build.file("src/qt/cpp/ffi.cpp");
    qt_wgpu_renderer.add_cpp_sources(&mut build);
    if env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("macos") {
        build.file("src/qt/cpp/macos_event_buffer_bridge.mm");
        build.flag_if_supported("-fblocks");
    }
    build.std("c++17");
    if env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows")
        && env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc")
    {
        build.flag_if_supported("/Zc:__cplusplus");
        build.flag_if_supported("/EHsc");
        build.flag_if_supported("/permissive-");
    }

    add_include_if_exists(&mut build, "include");
    qt_wgpu_renderer.add_include_dirs(&mut build);
    add_include_if_exists(&mut build, &out_dir);

    if let Some(node_include_dir) = node_include_dir {
        add_include_if_exists(&mut build, node_include_dir);
    }

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
