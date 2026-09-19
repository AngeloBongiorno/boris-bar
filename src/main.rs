use anyhow::{Context, Result};
use rodio::{Decoder, DeviceSinkBuilder, MixerDeviceSink, Player, Float};
use std::io::Cursor;
use std::thread;
use std::time::Duration;
use tao::{
    event::{Event, StartCause},
    event_loop::{ControlFlow, EventLoopBuilder},
};
use tray_icon::menu::accelerator::{Accelerator, Code};
use tray_icon::{
    Icon, TrayIconBuilder, TrayIconEvent,
    menu::{IsMenuItem, Menu, MenuEvent, MenuItem, PredefinedMenuItem, accelerator::Modifiers},
};

const FADE: Duration = Duration::from_millis(40);
const FADE_STEPS: u32 = 8;

struct Audio {
    sink: MixerDeviceSink,
    current: Option<(Sound, Player)>,
}

impl Audio {
    fn new() -> anyhow::Result<Self> {
        let sink =
            DeviceSinkBuilder::open_default_sink().context("failed to open device sink")?;
        Ok(Self {
            sink,
            current: None
        })
    }

    fn trigger(&mut self, sound: Sound) -> anyhow::Result<()> {
        let toggle_off = matches!(&self.current, Some((s, p)) if *s == sound && !p.empty());

        self.fade_out_current();

        if toggle_off {
            return Ok(());
        }

        let player = Player::connect_new(self.sink.mixer());
        player.append(Decoder::try_from(Cursor::new(sound.bytes()))?);
        self.current = Some((sound, player));
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

#[derive(Clone, Copy, PartialEq, Debug)]
enum Sound {
    Sforzo,
    Basiti,
    Cane,
}

impl Sound {
    const ALL: &'static [Sound] = &[Sound::Sforzo, Sound::Basiti, Sound::Cane];

    const fn id(self) -> &'static str {
        match self {
            Sound::Sforzo => "fai uno sforzo",
            Sound::Basiti => "tutti basiti",
            Sound::Cane => "a cazzo di cane",
        }
    }

    const fn label(self) -> &'static str {
        match self {
            Sound::Sforzo => "Fai uno sforzo",
            Sound::Basiti => "Tutti basiti",
            Sound::Cane => "A cazzo di cane",
        }
    }

    fn accelerator(self) -> Accelerator {
        let code = match self {
            Sound::Sforzo => Code::Digit1,
            Sound::Basiti => Code::Digit2,
            Sound::Cane => Code::Digit3,
        };
        Accelerator::new(Modifiers::META, code)
    }

    fn from_id(id: &str) -> Option<Self> {
        Self::ALL.iter().copied().find(|s| s.id() == id)
    }

    fn bytes(self) -> &'static [u8] {
        match self {
            Sound::Sforzo => include_bytes!("../assets/clips/fai_uno_sforzo.mp3"),
            _ => todo!(),
        }
    }
}

#[derive(Debug)]
enum AppEvent {
    Tray(TrayIconEvent),
    Menu(MenuEvent),
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    thread::sleep(Duration::from_secs(1));

    let event_loop = EventLoopBuilder::<AppEvent>::with_user_event().build();
    let proxy = event_loop.create_proxy();
    TrayIconEvent::set_event_handler(Some(move |event| {
        let _ = proxy.send_event(AppEvent::Tray(event));
    }));
    let proxy = event_loop.create_proxy();
    MenuEvent::set_event_handler(Some(move |event| {
        let _ = proxy.send_event(AppEvent::Menu(event));
    }));

    let mut tray = None;
    let mut audio = Audio::new()?;

    let menu = Menu::new();

    let item_quit = MenuItem::new("quit", true, None);

    let sound_items: Vec<MenuItem> = Sound::ALL
        .iter()
        .map(|s| MenuItem::with_id(s.id(), s.label(), true, Some(s.accelerator())))
        .collect();

    let separator = PredefinedMenuItem::separator();
    let mut refs: Vec<&dyn IsMenuItem> = sound_items.iter().map(|i| i as &dyn IsMenuItem).collect();
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
                } else if let Some(sound) = Sound::from_id(e.id.0.as_str()) {
                    if let Err(err) = audio.trigger(sound) {
                        eprintln!("error playing sound: {err}");
                    }
                }
            }
            _ => {}
        }
    });
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
