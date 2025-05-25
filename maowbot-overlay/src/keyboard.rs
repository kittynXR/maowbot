use anyhow::Result;
use crate::ffi::{self, VROverlayHandle, HmdMatrix34, LaserHit, K_UNTRACKED_DEVICE_INDEX_HMD, vr_keyboard_init_rendering, vr_keyboard_render};
use std::ffi::{c_void, CString};


#[derive(Clone)]
struct KeyButton {
    label: String,
    key: String,  // What gets typed
    x: f32,
    y: f32,
    width: f32,
    height: f32,
}

pub struct VirtualKeyboard {
    handle: VROverlayHandle,
    width: f32,
    height: f32,
    keys: Vec<KeyButton>,
    selected_key: Option<usize>,
    input_buffer: String,
    visible: bool,
}

impl VirtualKeyboard {
    pub fn new() -> Result<Self> {
        let handle = ffi::create_overlay_raw(
            "maowbot.keyboard",
            "Virtual Keyboard",
            0.5, // 50cm wide
            false
        )?;

        // Define keyboard layout
        let keys = Self::create_qwerty_layout();

        Ok(Self {
            handle,
            width: 0.5,
            height: 0.3,
            keys,
            selected_key: None,
            input_buffer: String::new(),
            visible: false,
        })
    }

    fn create_qwerty_layout() -> Vec<KeyButton> {
        let mut keys = Vec::new();

        // Row 1 - numbers
        let row1 = ["1", "2", "3", "4", "5", "6", "7", "8", "9", "0", "-", "="];
        for (i, key) in row1.iter().enumerate() {
            keys.push(KeyButton {
                label: key.to_string(),
                key: key.to_string(),
                x: 0.05 + (i as f32) * 0.075,
                y: 0.05,
                width: 0.07,
                height: 0.07,
            });
        }

        // Row 2 - qwerty
        let row2 = ["q", "w", "e", "r", "t", "y", "u", "i", "o", "p"];
        for (i, key) in row2.iter().enumerate() {
            keys.push(KeyButton {
                label: key.to_uppercase(),
                key: key.to_string(),
                x: 0.08 + (i as f32) * 0.075,
                y: 0.13,
                width: 0.07,
                height: 0.07,
            });
        }

        // Row 3 - asdf
        let row3 = ["a", "s", "d", "f", "g", "h", "j", "k", "l"];
        for (i, key) in row3.iter().enumerate() {
            keys.push(KeyButton {
                label: key.to_uppercase(),
                key: key.to_string(),
                x: 0.11 + (i as f32) * 0.075,
                y: 0.21,
                width: 0.07,
                height: 0.07,
            });
        }

        // Row 4 - zxcv
        let row4 = ["z", "x", "c", "v", "b", "n", "m"];
        for (i, key) in row4.iter().enumerate() {
            keys.push(KeyButton {
                label: key.to_uppercase(),
                key: key.to_string(),
                x: 0.14 + (i as f32) * 0.075,
                y: 0.29,
                width: 0.07,
                height: 0.07,
            });
        }

        // Space bar
        keys.push(KeyButton {
            label: "Space".to_string(),
            key: " ".to_string(),
            x: 0.2,
            y: 0.37,
            width: 0.4,
            height: 0.07,
        });

        // Backspace
        keys.push(KeyButton {
            label: "←".to_string(),
            key: "\x08".to_string(), // Backspace character
            x: 0.85,
            y: 0.05,
            width: 0.1,
            height: 0.07,
        });

        // Enter
        keys.push(KeyButton {
            label: "Enter".to_string(),
            key: "\n".to_string(),
            x: 0.85,
            y: 0.21,
            width: 0.1,
            height: 0.15,
        });

        keys
    }

    pub fn position_at_hip(&mut self, hip_tracker_index: Option<u32>) {
        let transform = if let Some(idx) = hip_tracker_index {
            // Attach to hip tracker - positioned in front and tilted up
            HmdMatrix34 {
                m: [
                    [1.0, 0.0, 0.0, 0.0],     // Right
                    [0.0, 0.866, -0.5, -0.3], // Up (tilted back 30 degrees)
                    [0.0, 0.5, 0.866, 0.5],   // Forward
                ]
            }
        } else {
            // Position in front of HMD
            HmdMatrix34 {
                m: [
                    [1.0, 0.0, 0.0, 0.0],
                    [0.0, 1.0, 0.0, -0.3],
                    [0.0, 0.0, 1.0, -0.8],
                ]
            }
        };

        let device_index = hip_tracker_index.unwrap_or(K_UNTRACKED_DEVICE_INDEX_HMD);

        unsafe {
            ffi::vr_set_overlay_transform_tracked_device_relative(
                self.handle,
                device_index,
                &transform as *const _,
            );
        }
    }

    pub fn set_visible(&mut self, visible: bool) {
        self.visible = visible;
        unsafe {
            if visible {
                ffi::vr_show_overlay(self.handle);
            } else {
                ffi::vr_hide_overlay(self.handle);
            }
        }
    }

    pub fn process_input(&mut self) -> Result<Option<String>> {
        if !self.visible {
            return Ok(None);
        }

        // Test laser intersection with keyboard overlay
        for controller_idx in 0..2 {
            if !unsafe { ffi::vr_get_controller_connected(controller_idx) } {
                continue;
            }

            let hit = unsafe { ffi::vr_test_laser_intersection(controller_idx, self.handle) };

            if hit.hit {
                // Convert to pixel coordinates (keyboard is 512x384)
                let pixel_x = hit.u * 512.0;
                let pixel_y = hit.v * 384.0;

                // Find which key was hit
                self.selected_key = None;

                // Check regular keys
                let rows = [
                    ("1234567890-=", 0),
                    ("qwertyuiop", 1),
                    ("asdfghjkl", 2),
                    ("zxcvbnm", 3),
                ];

                let button_size = 35.0;
                let spacing = 2.0;

                for (row_chars, row_idx) in rows.iter() {
                    let x_offset = 10.0 + if *row_idx == 3 { 30.0 } else { *row_idx as f32 * 15.0 };
                    let y_offset = 80.0 + *row_idx as f32 * (button_size + spacing);

                    for (i, ch) in row_chars.chars().enumerate() {
                        let btn_x = x_offset + i as f32 * (button_size + spacing);
                        let btn_y = y_offset;

                        if pixel_x >= btn_x && pixel_x <= btn_x + button_size &&
                            pixel_y >= btn_y && pixel_y <= btn_y + button_size {
                            // Find the corresponding key in our keys vector
                            for (key_idx, key) in self.keys.iter().enumerate() {
                                if key.key == ch.to_string() {
                                    self.selected_key = Some(key_idx);
                                    break;
                                }
                            }
                        }
                    }
                }

                // Check special keys
                let special_y = 80.0 + 4.0 * (button_size + spacing) + 10.0;

                // Space
                if pixel_x >= 100.0 && pixel_x <= 300.0 &&
                    pixel_y >= special_y && pixel_y <= special_y + button_size {
                    for (key_idx, key) in self.keys.iter().enumerate() {
                        if key.key == " " {
                            self.selected_key = Some(key_idx);
                            break;
                        }
                    }
                }

                // Backspace
                if pixel_x >= 302.0 && pixel_x <= 402.0 &&
                    pixel_y >= special_y && pixel_y <= special_y + button_size {
                    for (key_idx, key) in self.keys.iter().enumerate() {
                        if key.key == "\x08" {
                            self.selected_key = Some(key_idx);
                            break;
                        }
                    }
                }

                // Enter
                if pixel_x >= 404.0 && pixel_x <= 484.0 &&
                    pixel_y >= special_y && pixel_y <= special_y + button_size {
                    for (key_idx, key) in self.keys.iter().enumerate() {
                        if key.key == "\n" {
                            self.selected_key = Some(key_idx);
                            break;
                        }
                    }
                }

                // Handle trigger press
                if unsafe { ffi::vr_get_controller_trigger_pressed(controller_idx) } {
                    if let Some(key_idx) = self.selected_key {
                        let key = &self.keys[key_idx];

                        // Haptic feedback
                        unsafe { ffi::vr_trigger_haptic_pulse(controller_idx, 2000) };

                        if key.key == "\x08" {
                            // Backspace
                            self.input_buffer.pop();
                        } else if key.key == "\n" {
                            // Enter - return the buffer
                            let result = self.input_buffer.clone();
                            self.input_buffer.clear();
                            return Ok(Some(result));
                        } else {
                            // Normal key
                            self.input_buffer.push_str(&key.key);
                        }
                    }
                }
            }
        }

        Ok(None)
    }

    pub fn init_rendering(&mut self, device: *mut c_void, context: *mut c_void) -> Result<()> {
        unsafe {
            if vr_keyboard_init_rendering(device, context) {
                Ok(())
            } else {
                Err(anyhow::anyhow!("Failed to initialize keyboard rendering"))
            }
        }
    }

    pub fn render(&mut self) -> Result<()> {
        if !self.visible {
            return Ok(());
        }

        let current_text = CString::new(self.input_buffer.as_str())?;
        let (selected_x, selected_y) = if let Some(idx) = self.selected_key {
            let key = &self.keys[idx];
            ((key.x + key.width / 2.0) * 512.0, (key.y + key.height / 2.0) * 384.0)
        } else {
            (-1.0, -1.0)
        };

        unsafe {
            vr_keyboard_render(
                self.handle,
                selected_x,
                selected_y,
                current_text.as_ptr(),
            );
        }

        Ok(())
    }
}

impl Drop for VirtualKeyboard {
    fn drop(&mut self) {
        unsafe {
            ffi::vr_destroy_overlay(self.handle);
        }
    }
}