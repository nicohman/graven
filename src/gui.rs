extern crate azul;
extern crate ravenlib;
extern crate reqwest;
use azul::app_state::AppStateNoData;
use azul::widgets::text_input::*;
use azul::window_state::WindowSize;
use azul::{prelude::*, widgets::button::Button, widgets::label::Label};
use config::*;
use ravenlib::error::*;
use ravenlib::ravenserver::*;
use ravenlib::*;
use std::cell::RefCell;
use std::collections::BTreeSet;
use std::fs;
use std::io::Read;
use std::io::Seek;
use std::io::SeekFrom;
use std::io::Write;
use std::process::Command;
use std::sync::Arc;
use themes::*;
use ErrorKind::*;
use NodeType::*;
use RavenServerErrorKind::*;
enum Popup {
    New,
    Install,
    Installed,
    DidNotExist,
    NotSignedIn,
    PublishedOnline,
    UpdatedOnline,
    NotSelected,
}
use Popup::*;
impl Popup {
    fn label(&self) -> String {
        match self {
            New => "Create",
            Install => "Install",
            Installed => "Theme Installed",
            DidNotExist => "Theme Does Not Exist",
            NotSignedIn => "You are not logged into ThemeHub",
            PublishedOnline => "Published theme online",
            UpdatedOnline => "Updated theme online",
            NotSelected => "You don't have a theme selected",
        }
        .to_string()
    }
    fn callback(&self) -> Callback<DataModel> {
        Callback(match self {
            New => create_callback,
            Install => install_callback,
            _ => empty_callback,
        })
    }
    fn is_input(&self) -> bool {
        match self {
            Installed => false,
            DidNotExist => false,
            NotSignedIn => false,
            PublishedOnline => false,
            UpdatedOnline => false,
            NotSelected => false,
            _ => true,
        }
    }
}
struct DataModel {
    config: Config,
    themes: Vec<Theme>,
    selected_theme: Option<usize>,
    text: Vec<TextId>,
    screenshots: Vec<Option<String>>,
    font: FontId,
    popup_input: TextInputState,
    popup_shown: bool,
    popup_current: Popup,
}
impl Layout for DataModel {
    fn layout(&self, info: WindowInfo<Self>) -> Dom<Self> {
        let mut set = vec![On::MouseUp];
        let buts = self
            .themes
            .iter()
            .enumerate()
            .map(|(i, theme)| NodeData {
                node_type: NodeType::Label(theme.name.clone()),
                classes: if self.selected_theme == Some(i) {
                    vec!["theme-item".into(), "selected".into()]
                } else {
                    vec!["theme-item".into()]
                },
                force_enable_hit_test: set.clone(),
                ..Default::default()
            })
            .collect::<Dom<Self>>()
            .with_id("themes-list")
            .with_callback(On::MouseUp, Callback(select_theme));
        let delete_button = Button::with_label("Delete Theme")
            .dom()
            .with_class("bot-button")
            .with_callback(On::MouseUp, Callback(delete_callback));
        let load_button = Button::with_label("Load Theme")
            .dom()
            .with_class("bot-button")
            .with_callback(On::MouseUp, Callback(load_callback));
        let refresh_button = Button::with_label("Refresh Last Theme")
            .dom()
            .with_class("bot-button")
            .with_callback(On::MouseUp, Callback(refresh_callback));
        let mut cur_theme = Dom::new(Div).with_id("cur-theme");
        let online_button = Button::with_label("View on ThemeHub")
            .dom()
            .with_class("bot-button")
            .with_callback(On::MouseUp, Callback(online_callback));
        let new_button = Button::with_label("New Theme")
            .dom()
            .with_class("bot-button")
            .with_callback(On::MouseUp, Callback(show_new_box));
        let install_button = Button::with_label("Install Theme")
            .dom()
            .with_class("bot-button")
            .with_callback(On::MouseUp, Callback(show_install_box));
        let open_button = Button::with_label("View in File Manager")
            .dom()
            .with_class("bot-button")
            .with_callback(On::MouseUp, Callback(open_callback));
        let publish_button = Button::with_label("Publish on ThemeHub")
            .dom()
            .with_class("bot-button")
            .with_callback(On::MouseUp, Callback(publish_callback));
        if self.selected_theme.is_some() && self.selected_theme.unwrap() < self.themes.len() {
            let theme = &self.themes[self.selected_theme.unwrap()];

            let name = Dom::new(Label(theme.name.clone())).with_class("theme-name");
            let option_list =
                Dom::new(Text(self.text[self.selected_theme.unwrap()])).with_class("option-list");
            cur_theme = cur_theme.with_child(name);
            if theme.screenshot != default_screen() && theme.screenshot.len() > 0 {
                if info.resources.has_image(theme.name.clone()) {
                    let screenshot =
                        Dom::new(Image(info.resources.get_image(theme.name.clone()).unwrap()))
                            .with_class("theme-image");
                    cur_theme = cur_theme.with_child(screenshot);
                }
            }
            cur_theme = cur_theme.with_child(option_list);
        } else {
            cur_theme = cur_theme.with_child(Dom::new(Label(format!("No Theme selected."))));
        }
        let mut bottom_bar = Dom::new(Div)
            .with_id("bottom-bar")
            .with_child(online_button)
            .with_child(open_button)
            .with_child(refresh_button)
            .with_child(load_button)
            .with_child(delete_button)
            .with_child(new_button)
            .with_child(install_button)
            .with_child(publish_button);
        let right = Dom::new(Div)
            .with_id("right")
            .with_child(cur_theme)
            .with_child(bottom_bar);
        let mut dom = Dom::new(Div)
            .with_id("main")
            .with_child(buts)
            .with_child(right);
        if self.popup_shown {
            let close_popup = Button::with_label("x")
                .dom()
                .with_class("popup-close")
                .with_callback(On::MouseUp, Callback(hide_popup));
            let mut popup = Dom::new(Div).with_class("popup").with_child(close_popup);
            if self.popup_current.is_input() {
                let submit = Button::with_label(self.popup_current.label())
                    .dom()
                    .with_class("popup-submit")
                    .with_callback(On::MouseUp, self.popup_current.callback());
                let popup_input = TextInput::new()
                    .bind(info.window, &self.popup_input, &self)
                    .dom(&self.popup_input)
                    .with_class("popup-input");
                popup = popup.with_child(popup_input).with_child(submit);
            } else {
                let ok_button = Button::with_label("OK")
                    .dom()
                    .with_class("popup-submit")
                    .with_callback(On::MouseUp, Callback(hide_popup));
                let label = Label::new(self.popup_current.label())
                    .dom()
                    .with_class("popup-label");
                popup = popup.with_child(label).with_child(ok_button);
            }
            dom = dom.with_child(popup);
        }
        dom
    }
}
fn publish_callback(
    state: &mut AppState<DataModel>,
    event: WindowEvent<DataModel>,
) -> UpdateScreen {
    state.data.modify(|data| {
        if data.selected_theme.is_some() {
            let selected_theme = data.selected_theme.unwrap();
            let upload = upload_theme(data.themes[selected_theme].name.as_str());
            match upload {
                Ok(up) => {
                    if up {
                        println!("Uploaded new theme!");
                        data.popup_shown = true;
                        data.popup_current = PublishedOnline;
                    } else {
                        println!("Updated theme");
                        data.popup_shown = true;
                        data.popup_current = UpdatedOnline;
                    }
                }
                Err(Error(Server(rse), _)) => match rse {
                    NotLoggedIn => {
                        println!("Tried to publish online, but user is not logged in.");
                        data.popup_shown = true;
                        data.popup_current = NotSignedIn;
                    }
                    _ => println!("{:?}", rse),
                },
                _ => {
                    println!("{:?}", upload);
                }
            }
        } else {
            data.popup_shown = true;
            data.popup_current = NotSelected;
        }
    });
    UpdateScreen::Redraw
}
fn empty_callback(state: &mut AppState<DataModel>, event: WindowEvent<DataModel>) -> UpdateScreen {
    UpdateScreen::DontRedraw
}
fn install_callback(
    state: &mut AppState<DataModel>,
    event: WindowEvent<DataModel>,
) -> UpdateScreen {
    let mut option_string = String::new();
    state.data.modify(|data| {
        let name = data.popup_input.text.clone();
        println!("Installing a theme named {}", name);
        let res = get_metadata(name.as_str());
        if res.is_err() {
            let err = res.err().unwrap();
            match err {
                Error(Server(rse), _) => {
                    match rse {
                        DoesNotExist(s) => {
                            println!("This theme does not exist.");
                            data.popup_current = DidNotExist;
                        }
                        _ => println!("Error encountered.\n {:?}", rse),
                    };
                }
                _ => println!("Error encountered.\n {:?}", err),
            };
        } else {
            download_theme(name.as_str(), true).expect("Couldn't install theme");
            let theme = load_theme(name.as_str()).unwrap();
            option_string = theme_text(&theme);
            data.themes.push(theme);
        }
    });
    if option_string.len() > 0 {
        let font_id = FontId::BuiltinFont("sans-serif".into());
        let text_id = state.resources.add_text_cached(
            option_string,
            &font_id,
            StyleFontSize(PixelValue::px(10.0)),
            None,
        );
        state.data.modify(|data| {
            data.text.push(text_id);
            data.popup_current = Installed;
        });
    }
    UpdateScreen::Redraw
}
fn show_install_box(
    state: &mut AppState<DataModel>,
    event: WindowEvent<DataModel>,
) -> UpdateScreen {
    state.data.modify(|data| {
        data.popup_current = Popup::Install;
        data.popup_shown = true;
        data.popup_input = TextInputState::default();
    });
    UpdateScreen::Redraw
}
fn hide_popup(state: &mut AppState<DataModel>, event: WindowEvent<DataModel>) -> UpdateScreen {
    state.data.modify(|data| {
        data.popup_shown = false;
    });
    UpdateScreen::Redraw
}
fn create_callback(state: &mut AppState<DataModel>, event: WindowEvent<DataModel>) -> UpdateScreen {
    let mut option_string = String::new();
    state.data.modify(|data| {
        println!("Making a theme named {}", data.popup_input.text);
        new_theme(data.popup_input.text.as_str()).unwrap();
        let theme = load_theme(data.popup_input.text.as_str()).unwrap();
        data.popup_shown = false;
        option_string = theme_text(&theme);
        data.themes.push(theme);
    });
    let font_id = FontId::BuiltinFont("sans-serif".into());
    let text_id = state.resources.add_text_cached(
        option_string,
        &font_id,
        StyleFontSize(PixelValue::px(10.0)),
        None,
    );
    state.data.modify(|data| {
        data.text.push(text_id);
    });
    UpdateScreen::Redraw
}
fn show_new_box(state: &mut AppState<DataModel>, event: WindowEvent<DataModel>) -> UpdateScreen {
    state.data.modify(|data| {
        data.popup_current = Popup::New;
        data.popup_shown = true;
        data.popup_input = TextInputState::default();
    });
    UpdateScreen::Redraw
}

fn load_callback(state: &mut AppState<DataModel>, event: WindowEvent<DataModel>) -> UpdateScreen {
    let data = state.data.lock().unwrap();
    if data.selected_theme.is_some() {
        println!(
            "Loading theme {}",
            data.themes[data.selected_theme.unwrap()].name
        );
        run_theme(&data.themes[data.selected_theme.unwrap()]).unwrap();
        UpdateScreen::Redraw
    } else {
        drop(data);
        state.data.modify(|data| {
            data.popup_shown = true;
            data.popup_current = NotSelected;
        });
        UpdateScreen::DontRedraw
    }
}
fn open_callback(state: &mut AppState<DataModel>, event: WindowEvent<DataModel>) -> UpdateScreen {
    let data = state.data.lock().unwrap();
    if data.selected_theme.is_some() {
        let path =
            get_home() + "/.config/raven/themes/" + &data.themes[data.selected_theme.unwrap()].name;
        println!("{}", path);
        let output = Command::new("xdg-open")
            .arg(path)
            .spawn()
            .expect("Couldn't use xdg-open to open folder");
    } else {
        drop(data);
        state.data.modify(|data| {
            data.popup_shown = true;
            data.popup_current = NotSelected;
        });
    }
    UpdateScreen::DontRedraw
}
fn delete_callback(state: &mut AppState<DataModel>, event: WindowEvent<DataModel>) -> UpdateScreen {
    let mut up = UpdateScreen::DontRedraw;
    state.data.modify(|data| {
        if data.selected_theme.is_some() {
            println!(
                "Deleting theme {}",
                data.themes[data.selected_theme.unwrap()].name
            );
            del_theme(data.themes[data.selected_theme.unwrap()].name.as_str()).unwrap();
            data.themes.remove(data.selected_theme.unwrap());
            data.selected_theme = Some(0);
            up = UpdateScreen::Redraw
        } else {
            data.popup_shown = true;
            data.popup_current = NotSelected;
        }
    });
    up
}
fn refresh_callback(
    state: &mut AppState<DataModel>,
    event: WindowEvent<DataModel>,
) -> UpdateScreen {
    refresh_theme(state.data.lock().unwrap().config.last.clone()).unwrap();
    UpdateScreen::DontRedraw
}
fn online_callback(state: &mut AppState<DataModel>, event: WindowEvent<DataModel>) -> UpdateScreen {
    let data = state.data.lock().unwrap();
    if data.selected_theme.is_some() {
        let host = data.config.host.clone();
        let uri = host + "/themes/view/" + &data.themes[data.selected_theme.unwrap()].name;
        let output = Command::new("xdg-open")
            .arg(uri)
            .output()
            .expect("Couldn't use xdg-open to launch website");
    } else {
        drop(data);
        state.data.modify(|data| {
            data.popup_shown = true;
            data.popup_current = NotSelected;
        });
    }
    UpdateScreen::DontRedraw
}
fn select_theme(
    app_state: &mut AppState<DataModel>,
    event: WindowEvent<DataModel>,
) -> UpdateScreen {
    println!("{}", event.hit_dom_node);
    let selected = event
        .get_first_hit_child(event.hit_dom_node, On::MouseUp)
        .and_then(|x| Some(x.0));
    let mut should_redraw = UpdateScreen::DontRedraw;
    app_state.data.modify(|state| {
        if selected.is_some() && selected != state.selected_theme {
            state.selected_theme = selected;
            should_redraw = UpdateScreen::Redraw;
        }
        println!("Selected theme: {:?}", state.selected_theme);
    });
    should_redraw
}
fn theme_text(theme: &Theme) -> String {
    let mut text = String::new();
    if theme.description != default_desc() && theme.description.len() > 0 {
        text = text + "Description:\n\n" + theme.description.as_str() + "\n\n";
    }
    text += "Options Added: \n\n";
    let mut option_string = theme
        .options
        .iter()
        .fold(text, |acc, opt| acc + &format!("- {}\n", opt.to_string()));
    option_string += "Key-Value Options: \n\n";
    option_string = theme.kv.iter().fold(option_string, |acc, (k, v)| {
        acc + &format!("- {} : {}\n", k.as_str(), v)
    });
    return option_string;
}
fn main() {
    use Popup::*;
    if fs::metadata(get_home() + "/.config/raven/screenshots").is_err() {
        let cres = fs::create_dir(get_home() + "/.config/raven/screenshots");
        if cres.is_err() {
            println!(
                "Failed to init screenshot directory. Error Message: {:?}\n",
                cres
            );
        }
    }
    macro_rules! CSS_PATH {
        () => {
            concat!(env!("CARGO_MANIFEST_DIR"), "/src/gui.css")
        };
    }
    println!("Starting GUI");
    let mut themes = load_themes().unwrap();
    let font_id = FontId::BuiltinFont("sans-serif".into());

    let mut app = App::new(
        DataModel {
            config: get_config().unwrap(),
            selected_theme: None,
            themes: themes.clone(),
            text: vec![],
            screenshots: vec![],
            font: font_id.clone(),
            popup_input: TextInputState::default(),
            popup_shown: false,
            popup_current: New,
        },
        AppConfig::default(),
    );

    for (i, theme) in themes.iter().enumerate() {
        if theme.screenshot != default_screen() {
            let mut buf: Vec<u8> = vec![];
            let spath = get_home()
                + "/.config/raven/screenshots/"
                + &theme.screenshot.clone().replace("/", "").replace(":", "");
            if fs::metadata(&spath).is_err() {
                print!(
                    "Downloading {}'s screenshot from {}",
                    theme.name, theme.screenshot
                );
                let mut fd = fs::File::create(&spath).unwrap();
                let res = reqwest::get(&theme.screenshot.clone());
                if res.is_ok() {
                    let r = res.unwrap().read_to_end(&mut buf);
                    if r.is_err() {
                        println!("Failed reading. Error Message: \n{:?}", r);
                        continue;
                    } else {
                        fd.seek(SeekFrom::Start(0)).unwrap();
                        fd.write_all(&mut buf).expect("Couldn't write to file");
                    }
                } else {
                    println!("Failed downloading. Error Message: \n{:?}", res);
                    continue;
                }
            } else {
                let mut fd = fs::File::open(&spath).unwrap();
                fd.read_to_end(&mut buf).expect("Couldn't read file");
            }
            let ires = app.add_image(
                theme.name.clone(),
                &mut buf.as_slice(),
                ImageType::GuessImageFormat,
            );
            app.app_state.data.modify(|state| {
                state.screenshots.resize(i + 1, Some(String::new()));
                state.screenshots[i] = Some(theme.name.clone());
            });
        }
        let option_string = theme_text(&theme);
        let text_id = app.add_text_cached(option_string, &font_id, PixelValue::px(10.0), None);
        app.app_state.data.modify(|state| {
            state.text.push(text_id);
        });
    }
    let mut create_options = WindowCreateOptions::default();
    let mut size = WindowSize::default();
    size.dimensions = LogicalSize::new(980.0, 600.0);
    create_options.state.title = String::from("graven");
    create_options.state.size = size;
    #[cfg(debug_assertions)]
    let css = Css::hot_reload_override_native(CSS_PATH!()).unwrap();
    #[cfg(not(debug_assertions))]
    let css = Css::override_native(include_str!(CSS_PATH!())).unwrap();
    let window = Window::new(create_options, css).unwrap();
    app.run(window).unwrap();
}
