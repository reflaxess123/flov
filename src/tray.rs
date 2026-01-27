use tray_icon::{
    menu::{Menu, MenuEvent, MenuId, MenuItem},
    Icon, TrayIcon, TrayIconBuilder,
};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum TrayState {
    Idle,        // Red - waiting
    Recording,   // Green - recording audio
    Transcribing,// Yellow - transcription in progress
    LlmProcessing,// Blue - LLM processing
}

pub struct TrayManager {
    tray: TrayIcon,
    quit_id: MenuId,
    red_icon: Icon,
    green_icon: Icon,
    yellow_icon: Icon,
    blue_icon: Icon,
}

impl TrayManager {
    pub fn new_simple() -> anyhow::Result<Self> {
        let menu = Menu::new();

        let quit_item = MenuItem::new("Выход", true, None);
        let quit_id = quit_item.id().clone();
        menu.append(&quit_item)?;

        let red_icon = create_icon(220, 50, 50)?;      // Idle
        let green_icon = create_icon(50, 200, 50)?;    // Recording
        let yellow_icon = create_icon(230, 200, 50)?;  // Transcribing
        let blue_icon = create_icon(50, 120, 220)?;    // LLM Processing

        let tray = TrayIconBuilder::new()
            .with_menu(Box::new(menu))
            .with_tooltip("Flov - Voice Input (Ctrl+Win)")
            .with_icon(red_icon.clone())
            .build()?;

        Ok(Self {
            tray,
            quit_id,
            red_icon,
            green_icon,
            yellow_icon,
            blue_icon,
        })
    }

    pub fn set_state(&self, state: TrayState) {
        let icon = match state {
            TrayState::Idle => self.red_icon.clone(),
            TrayState::Recording => self.green_icon.clone(),
            TrayState::Transcribing => self.yellow_icon.clone(),
            TrayState::LlmProcessing => self.blue_icon.clone(),
        };
        let _ = self.tray.set_icon(Some(icon));
    }

    pub fn check_events(&self) -> bool {
        if let Ok(event) = MenuEvent::receiver().try_recv() {
            if event.id == self.quit_id {
                return true;
            }
        }
        false
    }
}

fn create_icon(r: u8, g: u8, b: u8) -> anyhow::Result<Icon> {
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
                rgba[idx] = r;
                rgba[idx + 1] = g;
                rgba[idx + 2] = b;
                rgba[idx + 3] = 255;
            }
        }
    }

    let icon = Icon::from_rgba(rgba, size, size)?;
    Ok(icon)
}
