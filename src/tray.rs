use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tray_icon::{
    menu::{CheckMenuItem, Menu, MenuEvent, MenuId, MenuItem},
    TrayIcon, TrayIconBuilder,
};

pub struct TrayManager {
    _tray: TrayIcon,
    quit_id: MenuId,
    glm_id: MenuId,
    glm_enabled: Arc<AtomicBool>,
}

impl TrayManager {
    pub fn new(glm_enabled: Arc<AtomicBool>) -> anyhow::Result<Self> {
        let menu = Menu::new();

        let glm_item = CheckMenuItem::new("GLM обработка", true, false, None);
        let glm_id = glm_item.id().clone();
        menu.append(&glm_item)?;

        let quit_item = MenuItem::new("Выход", true, None);
        let quit_id = quit_item.id().clone();
        menu.append(&quit_item)?;

        let icon = create_icon()?;

        let tray = TrayIconBuilder::new()
            .with_menu(Box::new(menu))
            .with_tooltip("Flov - Voice Input (Ctrl+Win)")
            .with_icon(icon)
            .build()?;

        Ok(Self {
            _tray: tray,
            quit_id,
            glm_id,
            glm_enabled,
        })
    }

    pub fn check_events(&self) -> bool {
        if let Ok(event) = MenuEvent::receiver().try_recv() {
            if event.id == self.quit_id {
                return true;
            }
            if event.id == self.glm_id {
                let current = self.glm_enabled.load(Ordering::SeqCst);
                self.glm_enabled.store(!current, Ordering::SeqCst);
                tracing::info!("GLM processing: {}", !current);
            }
        }
        false
    }
}

fn create_icon() -> anyhow::Result<tray_icon::Icon> {
    let size = 32u32;
    let mut rgba = vec![0u8; (size * size * 4) as usize];

    let center = size as f32 / 2.0;
    let radius = center - 2.0;

    for y in 0..size {
        for x in 0..size {
            let dx = x as f32 - center;
            let dy = y as f32 - center;
            let dist = (dx * dx + dy * dy).sqrt();

            let idx = ((y * size + x) * 4) as usize;

            if dist <= radius {
                rgba[idx] = 220;     // R
                rgba[idx + 1] = 50;  // G
                rgba[idx + 2] = 50;  // B
                rgba[idx + 3] = 255; // A
            }
        }
    }

    let icon = tray_icon::Icon::from_rgba(rgba, size, size)?;
    Ok(icon)
}
