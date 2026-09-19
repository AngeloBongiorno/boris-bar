use std::string::ToString;
use tao::{
    event::{Event, StartCause},
    event_loop::{ControlFlow, EventLoopBuilder},
};
use tray_icon::{
    menu::{Menu, MenuItem, IsMenuItem, MenuEvent, PredefinedMenuItem, accelerator::Modifiers},
    Icon, TrayIconBuilder, TrayIconEvent
};
use tray_icon::menu::accelerator::{Accelerator, Code};

#[derive(Debug)]
enum AppEvent {
    Tray(TrayIconEvent),
    Menu(MenuEvent),
}


struct SoundInfo {
    id: String,
    text: String,
    enabled: bool,
    accelerator: Option<Accelerator>,
}

impl SoundInfo {
    fn new(id: String, text: String, enabled: bool, accelerator: Option<Accelerator>) -> Self {
        SoundInfo {
            id,
            text,
            enabled,
            accelerator
        }
    }
}


fn main() {

    let sound_info: Vec<SoundInfo> = vec! [
        SoundInfo::new(
            "fai uno sforzo".to_string(),
            "Fai uno sforzo".to_string(),
            true,
            Some(Accelerator::new(Modifiers::META, Code::Digit1))
        ),
        SoundInfo::new(
            "tutti basiti".to_string(),
            "Tutti basiti".to_string(),
            true,
            Some(Accelerator::new(Modifiers::META, Code::Digit2))
        ),
        SoundInfo::new(
            "a cazzo di cane".to_string(),
            "A cazzo di cane".to_string(),
            true,
            Some(Accelerator::new(Modifiers::META, Code::Digit3))
        ),
    ];

    let event_loop = EventLoopBuilder::<AppEvent>::with_user_event().build();
    let proxy = event_loop.create_proxy();
    TrayIconEvent::set_event_handler(Some(move |event| {
        let _ = proxy.send_event(AppEvent::Tray(event));
    }));

    let proxy = event_loop.create_proxy();
    MenuEvent::set_event_handler(Some(move |event| {
        let _ = proxy.send_event(AppEvent::Menu(event));
    }));

    let menu = Menu::new();

    let item_quit   = MenuItem::new("quit", true, None);

    let mut sound_items: Vec<MenuItem> = sound_info.iter().map(|info| {
       MenuItem::with_id(&info.id, &info.text, info.enabled, info.accelerator)
    })
        .collect();
    let separator = PredefinedMenuItem::separator();
    let mut refs: Vec<&dyn IsMenuItem> = sound_items
        .iter()
        .map(|i| i as &dyn IsMenuItem)
        .collect();
    refs.push(&separator);
    refs.push(&item_quit);

    menu.append_items(
        &refs
    ).expect("Failed to build menu");

    let mut tray = None;

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;

        match event {
            Event::NewEvents(StartCause::Init) => {
                tray = Some(
                TrayIconBuilder::new()
                .with_menu(Box::new(menu.clone()))
                .with_tooltip("boris-bar")
                .with_icon(load_icon())
                .build()
                .expect("failed to create tray icon"),
                );
            },
            Event::UserEvent(AppEvent::Tray(e)) => {
                println!("tray {:?}", e);
            },
            Event::UserEvent(AppEvent::Menu(e)) => {
                if e.id == item_quit.id() {
                    tray.take();
                    *control_flow = ControlFlow::Exit;
                }
            },
            _ => {}
        }

    });
}

fn load_icon() -> Icon {
    let bytes = include_bytes!("../assets/fish.jpg");
    let image = image::load_from_memory(bytes)
        .expect("failed to decode icon")
        .into_rgba8();
    let (width, height) = image.dimensions();
    Icon::from_rgba(image.into_raw(), width, height).expect("failed to build icon")
}