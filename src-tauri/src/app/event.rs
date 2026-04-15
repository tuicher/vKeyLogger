use std::{sync::Mutex, thread};
use std::sync::mpsc;

use rdev::{listen, Button, EventType};
use serde::Serialize;
use tauri::{menu::MenuItem, AppHandle, Emitter, Manager, Wry};

use crate::app::state::AppState;

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type")]
pub enum InputEvent {
    KeyEvent { pressed: bool, name: String },
    MouseButtonEvent { pressed: bool, button: MouseButton },
    MouseMoveEvent { x: f64, y: f64 },
    MouseWheelEvent { delta_x: i64, delta_y: i64 },
}

#[derive(Debug, Clone, Serialize)]
pub enum MouseButton {
    Left,
    Right,
    Middle,
    Other,
}

pub fn map_mouse_button(button: Button) -> MouseButton {
    match button {
        Button::Left => MouseButton::Left,
        Button::Right => MouseButton::Right,
        Button::Middle => MouseButton::Middle,
        _ => MouseButton::Other,
    }
}

pub fn start_listener(app_handle: AppHandle, toggle_menu_item: MenuItem<Wry>) {
    // Canal para enviar eventos desde el hook (debe ser rápido) al hilo procesador
    let (tx, rx) = mpsc::channel::<rdev::Event>();

    // Hilo procesador: aquí va toda la lógica pesada (Mutex, emit, etc.)
    let app_handle_processor = app_handle.clone();
    thread::spawn(move || {
        for event in rx {
            let state = app_handle_processor.state::<Mutex<AppState>>();
            let mut app_state = state.lock().unwrap();

            // track pressed keys
            if let EventType::KeyPress(key) = event.event_type {
                let key_name = format!("{:?}", key);
                if key_name.contains('(') {
                    continue;
                }
                if app_state.pressed_keys.contains(&key_name) {
                    continue;
                }
                app_state.pressed_keys.push(key_name);
                if app_state.toggle_shortcut == app_state.pressed_keys {
                    app_state.toggle_listener(&app_handle_processor, &toggle_menu_item);

                    if !app_state.listening {
                        for key_name in &app_state.pressed_keys {
                            app_handle_processor
                                .emit_to(
                                    "main",
                                    "input-event",
                                    InputEvent::KeyEvent {
                                        pressed: false,
                                        name: key_name.clone(),
                                    },
                                )
                                .unwrap()
                        }
                    }
                }
            } else if let EventType::KeyRelease(key) = event.event_type {
                let key_name = format!("{:?}", key);
                if key_name.contains('(') {
                    continue;
                }
                app_state.pressed_keys.retain(|k| k != &key_name);
            }

            if !app_state.listening {
                continue;
            }

            let input_event = match event.event_type {
                EventType::KeyPress(key) => Some(InputEvent::KeyEvent {
                    pressed: true,
                    name: format!("{:?}", key),
                }),
                EventType::KeyRelease(key) => Some(InputEvent::KeyEvent {
                    pressed: false,
                    name: format!("{:?}", key),
                }),
                EventType::ButtonPress(button) => Some(InputEvent::MouseButtonEvent {
                    pressed: true,
                    button: map_mouse_button(button),
                }),
                EventType::ButtonRelease(button) => Some(InputEvent::MouseButtonEvent {
                    button: map_mouse_button(button),
                    pressed: false,
                }),
                EventType::MouseMove { x, y } => {
                    #[cfg(target_os = "macos")]
                    let (logical_x, logical_y) = (
                        x - app_state.monitor_position.0 as f64,
                        y - app_state.monitor_position.1 as f64,
                    );

                    #[cfg(not(target_os = "macos"))]
                    let (logical_x, logical_y) = {
                        let (offset_x, offset_y) = app_state.monitor_position;
                        (x - offset_x as f64, y - offset_y as f64)
                    };

                    Some(InputEvent::MouseMoveEvent {
                        x: logical_x,
                        y: logical_y,
                    })
                }
                EventType::Wheel { delta_x, delta_y } => {
                    Some(InputEvent::MouseWheelEvent { delta_x, delta_y })
                }
            };

            app_handle_processor.emit("input-event", input_event).unwrap();
        }
    });

    // Hilo del hook: solo reenvía el evento por el canal y retorna inmediatamente
    thread::spawn(move || {
        println!("Starting global input listener...");
        if let Err(err) = listen(move |event| {
            let _ = tx.send(event);
        }) {
            eprintln!("rdev listen failed: {:?}", err);
        }
    });
}
