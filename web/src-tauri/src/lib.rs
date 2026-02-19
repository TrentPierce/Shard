use tauri::{
    menu::{Menu, MenuItem},
    tray::TrayIconBuilder,
    Manager,
};
use tauri_plugin_shell::ShellExt;
use tauri_plugin_shell::process::CommandEvent;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::fs::File;
use tokio::io::AsyncWriteExt;
use futures_util::StreamExt;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let quit_i = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
            let show_i = MenuItem::with_id(app, "show", "Show Shard", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&show_i, &quit_i])?;

            let _tray = TrayIconBuilder::new()
                .icon(app.default_window_icon().unwrap().clone())
                .menu(&menu)
                .menu_on_left_click(false)
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "quit" => std::process::exit(0),
                    "show" => {
                        if let Some(window) = app.get_webview_window("main") {
                            window.show().unwrap();
                            window.set_focus().unwrap();
                        }
                    }
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    if let tauri::tray::TrayIconEvent::Click {
                        button: tauri::tray::MouseButton::Left,
                        button_state: tauri::tray::MouseButtonState::Up,
                        ..
                    } = event
                    {
                        if let Some(window) = tray.app_handle().get_webview_window("main") {
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                    }
                })
                .build(app)?;

            if cfg!(debug_assertions) {
                app.handle().plugin(
                    tauri_plugin_log::Builder::default()
                        .level(log::LevelFilter::Info)
                        .build(),
                )?;
            }

            let app_handle = app.handle().clone();
            
            tauri::async_runtime::spawn(async move {
                let resource_dir = app_handle.path().resource_dir().unwrap_or_else(|_| PathBuf::from(""));
                
                #[cfg(target_os = "windows")]
                let lib_name = "resources\\shard_engine.dll";
                #[cfg(target_os = "macos")]
                let lib_name = "resources/libshard_engine.dylib";
                #[cfg(target_os = "linux")]
                let lib_name = "resources/libshard_engine.so";

                let bitnet_lib = resource_dir.join(lib_name);
                
                let app_data_dir = app_handle.path().app_data_dir().unwrap_or_else(|_| PathBuf::from("."));
                let _ = tokio::fs::create_dir_all(&app_data_dir).await;
                let bitnet_model = app_data_dir.join("model.bin");

                if !bitnet_model.exists() {
                    println!("Model not found at {:?}, downloading...", bitnet_model);
                    let model_url = "https://huggingface.co/radames/bitnet-llama-2/resolve/main/ggml-model-i2_s.gguf"; 
                    if let Ok(response) = reqwest::get(model_url).await {
                        let total_size = response.content_length().unwrap_or(1);
                        let mut downloaded = 0;
                        if let Ok(mut file) = File::create(&bitnet_model).await {
                            let mut stream = response.bytes_stream();
                            while let Some(chunk) = stream.next().await {
                                if let Ok(bytes) = chunk {
                                    downloaded += bytes.len() as u64;
                                    let progress = (downloaded as f64 / total_size as f64) * 100.0;
                                    let _ = app_handle.emit("download-progress", progress);
                                    let _ = file.write_all(&bytes).await;
                                }
                            }
                            let _ = app_handle.emit("download-complete", true);
                        }
                    }
                } else {
                    let _ = app_handle.emit("download-complete", true);
                }

                // Launch the daemon with explicit ports matching the dev environment
                let sidecar_command = app_handle
                    .shell()
                    .sidecar("shard-daemon")
                    .expect("failed to create sidecar command")
                    .args(["--control-port", "9091", "--tcp-port", "4001"])
                    .env("BITNET_LIB", bitnet_lib.to_string_lossy().to_string())
                    .env("BITNET_MODEL", bitnet_model.to_string_lossy().to_string())
                    .env("RUST_LOG", "info");

                let (mut rx, mut _child) = sidecar_command
                    .spawn()
                    .expect("Failed to spawn sidecar");

                tauri::async_runtime::spawn(async move {
                    while let Some(event) = rx.recv().await {
                        if let CommandEvent::Stdout(line) = event {
                            let line_str = String::from_utf8_lossy(&line);
                            println!("Sidecar: {}", line_str.trim_end());
                        } else if let CommandEvent::Stderr(line) = event {
                            let line_str = String::from_utf8_lossy(&line);
                            eprintln!("Sidecar: {}", line_str.trim_end());
                        }
                    }
                });
            });

            Ok(())
        })
        .on_window_event(|window, event| match event {
            tauri::WindowEvent::CloseRequested { api, .. } => {
                window.hide().unwrap();
                api.prevent_close();
            }
            _ => {}
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
