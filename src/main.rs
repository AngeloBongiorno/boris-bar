use anyhow::{Context, Result};
use global_hotkey::{
    GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState,
    hotkey::{
        CMD_OR_CTRL as HK_CMD_OR_CTRL, Code as HotkeyCode, HotKey as GlobalShortcut,
        Modifiers as GlobalModifiers,
    },
};
use rodio::{Decoder, DeviceSinkBuilder, Float, MixerDeviceSink, Player};
use std::{collections::HashMap, io::Cursor, thread, time::Duration};
use tao::{
    event::{Event, StartCause},
    event_loop::{ControlFlow, EventLoopBuilder},
};
use tray_icon::{
    Icon, TrayIconBuilder, TrayIconEvent,
    menu::{
        IsMenuItem, Menu, MenuEvent, MenuItem, PredefinedMenuItem,
        accelerator::{
            Accelerator as MenuShortcut, CMD_OR_CTRL as M_CMD_OR_CTRL, Code as MenuCode,
            Modifiers as MenuModifiers,
        },
    },
};

const FADE: Duration = Duration::from_millis(40);
const FADE_STEPS: u32 = 8;

struct ClipData {
    id: &'static str,
    label: &'static str,
    digit: u8,
    bytes: &'static [u8],
}

const SFORZO: ClipData = ClipData {
    id: "fai uno sforzo",
    label: "Fai uno sforzo",
    digit: 1,
    bytes: include_bytes!(concat!(env!("OUT_DIR"), "/fai_uno_sforzo.mp3")),
};

const BASITI: ClipData = ClipData {
    id: "tutti basiti",
    label: "Tutti basiti",
    digit: 2,
    bytes: include_bytes!(concat!(env!("OUT_DIR"), "/tutti_basiti.mp3")),
};

const CANE: ClipData = ClipData {
    id: "a cazzo di cane",
    label: "A cazzo di cane",
    digit: 3,
    bytes: include_bytes!(concat!(env!("OUT_DIR"), "/a_cazzo_di_cane.mp3")),
};

struct HotkeyManager {
    manager: GlobalHotKeyManager,
    by_id: HashMap<u32, Clip>,
}

impl HotkeyManager {
    fn new() -> Result<Self> {
        Ok(Self {
            manager: GlobalHotKeyManager::new()
                .context("failed to create global hotkey manager")?,
            by_id: HashMap::new(),
        })
    }

    fn register(&mut self, clip: Clip) -> Result<()> {
        let hotkey = clip.global_shortcut();
        self.manager.register(hotkey)?;
        self.by_id.insert(hotkey.id(), clip);
        Ok(())
    }

    fn clip_for(&self, id: u32) -> Option<Clip> {
        self.by_id.get(&id).copied()
    }
}

struct Audio {
    sink: MixerDeviceSink,
    current: Option<(Clip, Player)>,
}

impl Audio {
    fn new() -> anyhow::Result<Self> {
        let sink = DeviceSinkBuilder::open_default_sink().context("failed to open device sink")?;
        Ok(Self {
            sink,
            current: None,
        })
    }

    fn trigger(&mut self, clip: Clip) -> anyhow::Result<()> {
        let toggle_off = matches!(&self.current, Some((s, p)) if *s == clip && !p.empty());

        self.fade_out_current();

        if toggle_off {
            return Ok(());
        }

        let player = Player::connect_new(self.sink.mixer());
        player.append(Decoder::try_from(Cursor::new(clip.bytes()))?);
        self.current = Some((clip, player));
        Ok(())
    }

    fn fade_out_current(&mut self) {
        let Some((_, player)) = self.current.take() else {
            return;
        };

        // Already finished on its own — nothing to fade, just drop it.
        if player.empty() {
            return;
        }

        thread::spawn(move || {
            for step in (0..FADE_STEPS).rev() {
                player.set_volume(step as Float / FADE_STEPS as Float);
                thread::sleep(FADE / FADE_STEPS);
            }
        });
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Clip {
    Sforzo,
    Basiti,
    Cane,
}

impl Clip {
    const ALL: &'static [Clip] = &[Clip::Sforzo, Clip::Basiti, Clip::Cane];

    const fn data(self) -> &'static ClipData {
        match self {
            Clip::Sforzo => &SFORZO,
            Clip::Basiti => &BASITI,
            Clip::Cane => &CANE,
        }
    }

    const fn id(self) -> &'static str {
        self.data().id
    }

    const fn label(self) -> &'static str {
        self.data().label
    }

    fn menu_shortcut(self) -> MenuShortcut {
        let code = match self.data().digit {
            1 => MenuCode::Digit1,
            2 => MenuCode::Digit2,
            3 => MenuCode::Digit3,
            _ => unreachable!("unsupported digit"),
        };
        MenuShortcut::new(M_CMD_OR_CTRL | MenuModifiers::ALT, code)
    }

    fn global_shortcut(self) -> GlobalShortcut {
        let code = match self.data().digit {
            1 => HotkeyCode::Digit1,
            2 => HotkeyCode::Digit2,
            3 => HotkeyCode::Digit3,
            _ => unreachable!("unsupported digit"),
        };
        GlobalShortcut::new(Some(HK_CMD_OR_CTRL | GlobalModifiers::ALT), code)
    }

    fn from_id(id: &str) -> Option<Self> {
        Self::ALL.iter().copied().find(|s| s.id() == id)
    }

    fn bytes(self) -> &'static [u8] {
        self.data().bytes
    }
}

#[derive(Debug)]
enum AppEvent {
    Tray(TrayIconEvent),
    Menu(MenuEvent),
    Hotkey(GlobalHotKeyEvent),
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut hotkey_manager = HotkeyManager::new()?;
    let event_loop = EventLoopBuilder::<AppEvent>::with_user_event().build();
    let proxy = event_loop.create_proxy();
    TrayIconEvent::set_event_handler(Some(move |event| {
        let _ = proxy.send_event(AppEvent::Tray(event));
    }));
    let proxy = event_loop.create_proxy();
    MenuEvent::set_event_handler(Some(move |event| {
        let _ = proxy.send_event(AppEvent::Menu(event));
    }));
    let proxy = event_loop.create_proxy();
    GlobalHotKeyEvent::set_event_handler(Some(move |event| {
        let _ = proxy.send_event(AppEvent::Hotkey(event));
    }));

    let mut tray = None;
    let mut audio = Audio::new()?;

    let menu = Menu::new();

    let item_quit = MenuItem::new("Esci", true, None);

    let clip_items: Vec<MenuItem> = Clip::ALL
        .iter()
        .map(|s| MenuItem::with_id(s.id(), s.label(), true, Some(s.menu_shortcut())))
        .collect();

    for &clip in Clip::ALL {
        hotkey_manager.register(clip)?;
    }

    let separator = PredefinedMenuItem::separator();
    let mut refs: Vec<&dyn IsMenuItem> = clip_items.iter().map(|i| i as &dyn IsMenuItem).collect();
    refs.push(&separator);
    refs.push(&item_quit);

    menu.append_items(&refs)?;

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;

        match event {
            Event::NewEvents(StartCause::Init) => {
                let icon = load_icon().unwrap_or_else(|e| {
                    eprintln!("icon load failed: {e:#}");
                    fallback_icon()
                });
                tray = Some(
                    TrayIconBuilder::new()
                        .with_menu(Box::new(menu.clone()))
                        .with_tooltip("boris-bar")
                        .with_icon(icon)
                        .build()
                        .expect("failed to create tray icon"),
                );
            }
            Event::UserEvent(AppEvent::Tray(e)) => {
                println!("tray {:?}", e);
            }
            Event::UserEvent(AppEvent::Menu(e)) => {
                if e.id == item_quit.id() {
                    tray.take();
                    *control_flow = ControlFlow::Exit;
                } else if let Some(clip) = Clip::from_id(e.id.0.as_str()) {
                    play(&mut audio, clip)
                }
            }
            Event::UserEvent(AppEvent::Hotkey(e)) => {
                if e.state == HotKeyState::Pressed
                    && let Some(clip) = hotkey_manager.clip_for(e.id)
                {
                    play(&mut audio, clip);
                }
            }
            _ => {}
        }
    });
}

fn play(audio: &mut Audio, clip: Clip) {
    if let Err(err) = audio.trigger(clip) {
        eprintln!("failed to play {clip:?}: {err:#}");
    }
}

fn load_icon() -> Result<Icon> {
    let bytes = include_bytes!("../assets/fish.jpg");
    let image = image::load_from_memory(bytes)
        .context("Decoding embedded icon.")?
        .into_rgba8();
    let (width, height) = image.dimensions();
    let icon =
        Icon::from_rgba(image.into_raw(), width, height).context("Converting icon to RGBA")?;
    Ok(icon)
}

fn fallback_icon() -> Icon {
    const SIZE: u32 = 32;
    let mut rgba = Vec::with_capacity((SIZE * SIZE * 4) as usize);
    let c = (SIZE as f32 - 1.0) / 2.0;
    for y in 0..SIZE {
        for x in 0..SIZE {
            let d = (((x as f32 - c).powi(2)) + ((y as f32 - c).powi(2))).sqrt();
            let alpha = if d < c - 1.0 { 255u8 } else { 0 };
            rgba.extend_from_slice(&[240, 240, 240, alpha]);
        }
    }
    Icon::from_rgba(rgba, SIZE, SIZE).expect("fallback icon is always valid")
}
