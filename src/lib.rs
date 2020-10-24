#[macro_use]
extern crate gtk_extras;

use gtk::prelude::*;
use gtk_extras::{
    settings::{GeditPreferencesEditor, GnomeDesktopInterface},
    ImageSelection, SelectionVariant, ToggleVariant, VariantToggler,
};

use std::cell::Cell;
use std::rc::Rc;
use std::ops::Deref;
use std::fs;
use std::string::String;
use std::path::PathBuf;
use gio::{Settings, SettingsExt};

#[derive(Clone, Copy, Debug)]
enum ThemeVariant {
    Dark,
    Slim,
}

pub const DARK: u8 = 0b01;
pub const SLIM: u8 = 0b10;

struct ThemeDescriptor {
    name: String,
    display_name: String,
    preview: String,
    gedit_theme: Option<String>,
    shell_theme: Option<String>
}

fn str_to_string(s: Option<&str>) -> Option<String> {
    match s {
        Some(v) => return Some(String::from(v)),
        None => return None
    }
}

fn read_theme_descriptor<'a>(theme: &str) -> Option<ThemeDescriptor> {
    let mut buf: PathBuf = [r"/", "usr", "share", "themes", theme].iter().collect();
    buf.set_extension("json");
    let res = fs::read_to_string(buf);
    if res.is_err() {
        return None;
    }
    let s = res.unwrap();
    let jres = json::parse(&s);
    if jres.is_err() {
        return None;
    }
    let j = jres.unwrap();
    return Some(ThemeDescriptor {
        name: String::from(theme),
        display_name: String::from(j["Name"].as_str().unwrap()),
        preview: String::from(j["Preview"].as_str().unwrap()),
        gedit_theme: str_to_string(j["GEditTheme"].as_str()),
        shell_theme: str_to_string(j["ShellTheme"].as_str())
    });
}

fn gen_theme_list() -> Vec<ThemeDescriptor> {
    let mut vec: Vec<ThemeDescriptor> = Vec::new();
    let paths = fs::read_dir("/usr/share/themes").unwrap();
    for path in paths {
        let mothefuckingbullshitlanguage = path.unwrap().path();
        let mut buf = PathBuf::from(mothefuckingbullshitlanguage.clone());
        buf.push("index.theme");
        if !buf.exists() {
            continue;
        }
        let so = mothefuckingbullshitlanguage.file_name();
        let file_name: <str as ToOwned>::Owned;
        let desc: ThemeDescriptor;
        match so {
            Some(s) => file_name = s.to_string_lossy().into_owned(),
            None => continue
        }
        match read_theme_descriptor(&file_name) {
            Some(d) => desc = d,
            None => desc = ThemeDescriptor {
                name: file_name.clone(),
                display_name: file_name.clone(),
                preview: String::from("/usr/share/icons/pop-os-branding/pop_icon.svg"),
                gedit_theme: None,
                shell_theme: Some(file_name.clone())
            }
        }
        vec.push(desc);
    }
    return vec;
}

pub struct PopThemeSwitcher(gtk::Container);

impl PopThemeSwitcher {
    pub fn new() -> Self {
        let shell = Settings::new("org.gnome.shell.extensions.user-theme");
        let gpe = GeditPreferencesEditor::new_checked();
        let gdi = GnomeDesktopInterface::new();
        let mut vec: Vec<SelectionVariant<usize>> = Vec::new();
        let themes = gen_theme_list();

        let variants = {
            let current_theme = gdi.gtk_theme();
            let current_theme = current_theme.as_ref().map_or("", |s| s.as_str());

            for i in 0..themes.len() {
                vec.push(SelectionVariant {
                    name:         &themes[i].display_name,
                    image:        Some(&themes[i].preview),
                    size_request: None,
                    active:       current_theme == &themes[i].name,
                    event:        i
                });
            }

            &vec[..]
        };

        let (tx, rx) = glib::MainContext::channel(glib::PRIORITY_DEFAULT);

        //TODO: fix extra events from ImageSelection::new
        let selection_ready = Rc::new(Cell::new(false));
        let handler = {
            let selection_ready = selection_ready.clone();
            move |event| {
                if selection_ready.get() {
                    let _ = tx.send(event);
                }
            }
        };

        let selection = cascade! {
            ImageSelection::new(&variants, "", handler);
            ..set_max_children_per_line(3);
            ..set_min_children_per_line(2);
            ..set_column_spacing(24);
            ..set_row_spacing(24);
            ..set_halign(gtk::Align::Center);
        };

        selection_ready.set(true);
        rx.attach(None, move |event| {
            let theme = &themes[event];
            if let Some(gpe) = gpe.as_ref() {
                if let Some(gedit_theme) = theme.gedit_theme.as_ref() {
                    gpe.set_scheme(gedit_theme);
                }
            }
            if let Some(shell_theme) = theme.shell_theme.as_ref() { //This will only work if user shell theme extension is installed
                match shell.set_string("name", shell_theme) {
                    Ok(_) => Settings::sync(),
                    Err(v) => print!("Error setting shell theme {}\n", v)
                }
            }
            gdi.set_gtk_theme(&theme.name);
            glib::Continue(true)
        });

        Self((*selection).clone().upcast::<gtk::Container>())
    }

    pub fn dark_and_slim() -> Self {
        let gpe = GeditPreferencesEditor::new_checked();
        let gdi = GnomeDesktopInterface::new();

        let mut flags = 0;

        let variants = {
            let current_theme = gdi.gtk_theme();
            let current_theme = current_theme.as_ref().map_or("", |s| s.as_str());

            let dark_mode = current_theme.contains("dark");
            let slim_mode = current_theme.contains("slim");

            if dark_mode {
                flags |= DARK;
            }

            if slim_mode {
                flags |= SLIM;
            }

            [
                ToggleVariant {
                    name:        "Dark Mode",
                    description: "Changes your applications to a dark theme for easier viewing at \
                                  night.",
                    event:       ThemeVariant::Dark,
                    active:      dark_mode,
                },
                ToggleVariant {
                    name:        "Slim Mode",
                    description: "Reduces the height of application headers.",
                    event:       ThemeVariant::Slim,
                    active:      slim_mode,
                },
            ]
        };

        let (tx, rx) = glib::MainContext::channel(glib::PRIORITY_DEFAULT);

        let theme_switcher = VariantToggler::new(&variants, move |event, active| {
            let _ = tx.send((event, active));
        });

        rx.attach(None, move |(event, active)| {
            match (event, active) {
                (ThemeVariant::Dark, true) => flags |= DARK,
                (ThemeVariant::Dark, false) => flags &= 255 ^ DARK,
                (ThemeVariant::Slim, true) => flags |= SLIM,
                (ThemeVariant::Slim, false) => flags &= 255 ^ SLIM,
            }

            let (gtk_theme, gedit_scheme) = if flags & (DARK | SLIM) == DARK | SLIM {
                ("Pop-slim-dark", "pop-dark")
            } else if flags & DARK != 0 {
                ("Pop-dark", "pop-dark")
            } else if flags & SLIM != 0 {
                ("Pop-slim", "pop-light")
            } else {
                ("Pop", "pop-light")
            };

            if let Some(gpe) = gpe.as_ref() {
                gpe.set_scheme(gedit_scheme);
            }
            gdi.set_gtk_theme(gtk_theme);

            glib::Continue(true)
        });

        Self(theme_switcher.into())
    }

    pub fn grab_focus(&self) {
        use gtk_extras::widgets::iter_from;

        for child in iter_from::<gtk::FlowBoxChild, gtk::Container>(&*self) {
            if let Some(inner) = child.get_child() {
                let inner = inner.downcast::<gtk::Container>().unwrap();
                if let Some(radio) = iter_from::<gtk::RadioButton, _>(&inner).next() {
                    if radio.get_active() {
                        child.grab_focus();
                    }
                }
            }
        }
    }
}

impl Deref for PopThemeSwitcher {
    type Target = gtk::Container;

    fn deref(&self) -> &Self::Target { &self.0 }
}
