//! The task bar icon over a real D-Bus: a private session bus, with this test
//! playing the desktop shell (the StatusNotifierWatcher that icons register
//! with, and the host that reads their properties). It checks what Plasma
//! would see: the icon registers, shows the language as a letter, and follows
//! language changes.
//!
//! Needs `dbus-daemon`; without it the test says so and skips, like the font
//! tests do.

use ime_core::Language;
use std::io::{BufRead, BufReader};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use zbus::blocking::{Connection, Proxy, connection, proxy};
use zbus::proxy::CacheProperties;
use zbus::zvariant::OwnedValue;

const WATCHER_NAME: &str = "org.kde.StatusNotifierWatcher";
const WATCHER_PATH: &str = "/StatusNotifierWatcher";
const ITEM_INTERFACE: &str = "org.kde.StatusNotifierItem";
const ITEM_PATH: &str = "/StatusNotifierItem";
const TIMEOUT: Duration = Duration::from_secs(10);

/// The shell's side: remembers which items registered.
struct Watcher {
    items: Arc<Mutex<Vec<String>>>,
}

#[zbus::interface(name = "org.kde.StatusNotifierWatcher")]
impl Watcher {
    async fn register_status_notifier_item(&self, service: &str) -> zbus::fdo::Result<()> {
        self.items.lock().unwrap().push(service.to_owned());
        Ok(())
    }

    async fn register_status_notifier_host(&self, _service: &str) -> zbus::fdo::Result<()> {
        Ok(())
    }

    #[zbus(property)]
    fn registered_status_notifier_items(&self) -> Vec<String> {
        self.items.lock().unwrap().clone()
    }

    #[zbus(property)]
    fn is_status_notifier_host_registered(&self) -> bool {
        true
    }

    #[zbus(property)]
    fn protocol_version(&self) -> i32 {
        0
    }
}

/// A private session bus that is shut down with the test.
struct Bus {
    child: Child,
    address: String,
}

impl Bus {
    fn start() -> Option<Self> {
        let mut child = Command::new("dbus-daemon")
            .args(["--session", "--nofork", "--print-address=1"])
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .ok()?;
        let mut address = String::new();
        BufReader::new(child.stdout.take()?)
            .read_line(&mut address)
            .ok()?;
        let address = address.trim().to_owned();
        if address.is_empty() {
            let _ = child.kill();
            return None;
        }
        Some(Self { child, address })
    }
}

impl Drop for Bus {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn wait_until(what: &str, condition: impl Fn() -> bool) {
    let deadline = Instant::now() + TIMEOUT;
    while Instant::now() < deadline {
        if condition() {
            return;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(condition(), "timed out waiting for {what}");
}

fn property<T: TryFrom<OwnedValue>>(proxy: &Proxy, name: &str) -> T
where
    T::Error: Into<zbus::Error>,
{
    proxy
        .get_property::<T>(name)
        .unwrap_or_else(|e| panic!("reading {name}: {e}"))
}

/// The tooltip as the specification defines it: (icon name, icon pixmaps,
/// title, description).
type ToolTip = (String, Vec<(i32, i32, Vec<u8>)>, String, String);

#[test]
fn the_icon_registers_shows_the_language_and_follows_it() {
    let Some(bus) = Bus::start() else {
        eprintln!(
            "SKIPPED: dbus-daemon is not installed; the task bar icon was not tried on a bus"
        );
        return;
    };
    // SAFETY: this is the only test in this binary, so no other thread reads
    // the environment while it is changed.
    unsafe { std::env::set_var("DBUS_SESSION_BUS_ADDRESS", &bus.address) };

    // The shell.
    let items = Arc::new(Mutex::new(Vec::new()));
    let _shell: Connection = connection::Builder::session()
        .unwrap()
        .serve_at(
            WATCHER_PATH,
            Watcher {
                items: items.clone(),
            },
        )
        .unwrap()
        .name(WATCHER_NAME)
        .unwrap()
        .build()
        .unwrap();

    // The daemon's side.
    let (asked_sender, asked) = std::sync::mpsc::channel();
    let (select, next) = (asked_sender.clone(), asked_sender);
    let news = easime::tray::start_with(easime::tray::Actions {
        open_settings: Box::new(|| {}),
        select_language: Box::new(move |language| {
            let _ = select.send(format!("select {language:?}"));
        }),
        next_language: Box::new(move || {
            let _ = next.send("next".to_owned());
        }),
    });
    news.send((Language::Korean, vec![Language::English, Language::Korean]))
        .unwrap();
    wait_until("the icon to register", || !items.lock().unwrap().is_empty());
    let service = items.lock().unwrap()[0].clone();
    // The item registers its bus name, or `name/path`; the path is the standard one.
    let destination = service.split('/').next().unwrap().to_owned();

    // The host: reads what Plasma reads.
    let host = connection::Builder::session().unwrap().build().unwrap();
    // Not cached: the icon announces changes with its own signals (NewToolTip,
    // NewIcon), which Plasma answers by asking again.
    let item: Proxy = proxy::Builder::new(&host)
        .destination(destination.as_str())
        .unwrap()
        .path(ITEM_PATH)
        .unwrap()
        .interface(ITEM_INTERFACE)
        .unwrap()
        .cache_properties(CacheProperties::No)
        .build()
        .unwrap();
    assert_eq!(property::<String>(&item, "Id"), "easime");
    assert_eq!(property::<String>(&item, "Category"), "ApplicationStatus");
    assert_eq!(property::<String>(&item, "Status"), "Active");

    // The language is the tooltip, once the news has arrived.
    wait_until("the tooltip to name Korean", || {
        item.get_property::<ToolTip>("ToolTip")
            .map(|tip| tip.3 == "한국어")
            .unwrap_or(false)
    });
    let korean: Vec<(i32, i32, Vec<u8>)> = property(&item, "IconPixmap");
    let sizes: Vec<i32> = korean.iter().map(|icon| icon.0).collect();
    assert_eq!(sizes, [16, 22, 24, 32, 48, 64]);
    for (width, height, data) in &korean {
        assert_eq!(width, height);
        assert_eq!(data.len() as i32, width * height * 4, "ARGB32 pixels");
    }
    // Opaque and translucent pixels both occur: a box with rounded corners.
    let biggest = &korean.last().unwrap().2;
    assert!(biggest.chunks(4).any(|px| px[0] == 0), "clear corners");
    assert!(biggest.chunks(4).any(|px| px[0] > 0), "drawn box");

    // A click on the icon, as the shell sends it, asks for the next language.
    item.call_method("Activate", &(10i32, 20i32)).unwrap();
    assert_eq!(asked.recv_timeout(TIMEOUT).unwrap(), "next");

    // A language change changes what the shell sees.
    news.send((
        Language::Japanese,
        vec![Language::English, Language::Japanese],
    ))
    .unwrap();
    wait_until("the tooltip to name Japanese", || {
        item.get_property::<ToolTip>("ToolTip")
            .map(|tip| tip.3 == "日本語")
            .unwrap_or(false)
    });
    let japanese: Vec<(i32, i32, Vec<u8>)> = property(&item, "IconPixmap");
    // The letters differ only if this machine has fonts for both (a CI runner
    // may have no CJK font; the box is then drawn empty for every language).
    let fonts = frontend_wayland::font::Fonts::load_system();
    if fonts.font_for('한').is_some() && fonts.font_for('あ').is_some() {
        assert_ne!(
            japanese.last().unwrap().2,
            korean.last().unwrap().2,
            "the letter changed"
        );
    } else {
        eprintln!("note: no installed font has both 한 and あ; the letter change is not compared");
    }
    news.send((Language::English, vec![Language::English]))
        .unwrap();
    wait_until("the tooltip to name English", || {
        item.get_property::<ToolTip>("ToolTip")
            .map(|tip| tip.3 == "English")
            .unwrap_or(false)
    });
}
