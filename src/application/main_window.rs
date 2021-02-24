use cascade::cascade;
use glib::{clone, subclass};
use gtk::prelude::*;
use gtk::subclass::prelude::*;
use std::{
    cell::RefCell,
    rc::Rc,
    sync::atomic::{AtomicUsize, Ordering},
};

use super::{shortcuts_window, Keyboard, KeyboardLayer, Page, Picker};
use crate::{Daemon, DaemonBoard, DaemonClient, DaemonDummy, DaemonServer, DerefCell};

#[derive(Default)]
pub struct MainWindowInner {
    back_button: DerefCell<gtk::Button>,
    count: AtomicUsize,
    header_bar: DerefCell<gtk::HeaderBar>,
    keyboard_list_box: DerefCell<gtk::ListBox>,
    layer_switcher: DerefCell<gtk::StackSwitcher>,
    picker: DerefCell<Picker>,
    stack: DerefCell<gtk::Stack>,
    keyboards: RefCell<Vec<Keyboard>>,
}

impl ObjectSubclass for MainWindowInner {
    const NAME: &'static str = "S76ConfiguratorMainWindow";

    type ParentType = gtk::ApplicationWindow;
    type Type = MainWindow;
    type Interfaces = ();

    type Instance = subclass::simple::InstanceStruct<Self>;
    type Class = subclass::simple::ClassStruct<Self>;

    glib::object_subclass!();

    fn new() -> Self {
        Self::default()
    }
}

impl ObjectImpl for MainWindowInner {
    fn constructed(&self, window: &MainWindow) {
        self.parent_constructed(window);

        let back_button = cascade! {
            gtk::Button::new();
            ..add(&gtk::Image::from_icon_name(Some("go-previous-symbolic"), gtk::IconSize::Button));
            ..connect_clicked(clone!(@weak window => move |_| {
                window.show_keyboard_list();
            }));
        };

        let layer_switcher = gtk::StackSwitcher::new();

        let menu = cascade! {
            gio::Menu::new();
            ..append_section(None, &cascade! {
                gio::Menu::new();
                ..append(Some("Load Layout"), Some("kbd.load"));
                ..append(Some("Save Layout"), Some("kbd.save"));
                ..append(Some("Reset Layout"), Some("kbd.reset"));
            });
            ..append_section(None, &cascade! {
                gio::Menu::new();
                ..append(Some("Keyboard Shortcuts"), Some("win.show-help-overlay"));
                ..append(Some("About Keyboard Configurator"), Some("app.about"));
            });
        };

        let header_bar = cascade! {
            gtk::HeaderBar::new();
            ..set_show_close_button(true);
            ..pack_start(&back_button);
            ..set_custom_title(Some(&layer_switcher));
            ..pack_end(&cascade! {
                gtk::MenuButton::new();
                ..set_menu_model(Some(&menu));
                ..add(&cascade! {
                    gtk::Image::from_icon_name(Some("open-menu-symbolic"), gtk::IconSize::Button);
                });
            });
        };

        let no_boards_msg = concat! {
            "<span size='x-large' weight='bold'>No keyboard detected</span>\n",
            "Make sure your built-in keyboard has up to date\n",
            "System76 Open Firmware.\n",
            "If using an external keyboard, make sure it is\n",
            "plugged in properly.",
        };
        let no_boards = cascade! {
            gtk::Box::new(gtk::Orientation::Vertical, 24);
            ..add(&cascade! {
                gtk::Image::from_pixbuf(
                    cascade! {
                        gtk::IconTheme::default();
                        ..add_resource_path("/com/system76/keyboard-configurator/icons");
                    }
                    .load_icon(
                        "input-keyboard-symbolic",
                        256,
                        gtk::IconLookupFlags::empty(),
                    )
                    .unwrap_or(None)
                    .as_ref(),
                );
                ..set_halign(gtk::Align::Center);
            });
            ..add(&cascade! {
                gtk::Label::new(Some(no_boards_msg));
                ..set_justify(gtk::Justification::Center);
                ..set_use_markup(true);
            });
            ..show_all();
        };

        let keyboard_list_box = cascade! {
            gtk::ListBox::new();
            ..set_placeholder(Some(&no_boards));
        };

        let stack = cascade! {
            gtk::Stack::new();
            ..add(&keyboard_list_box);
        };

        let picker = Picker::new();

        cascade! {
            window;
            ..set_title("System76 Keyboard Configurator");
            ..set_position(gtk::WindowPosition::Center);
            ..set_default_size(1024, 768);
            ..set_titlebar(Some(&header_bar));
            ..add(&cascade! {
                gtk::ScrolledWindow::new::<gtk::Adjustment, gtk::Adjustment>(None, None);
                ..add(&stack);
            });
            ..set_help_overlay(Some(&shortcuts_window()));
            ..set_focus(None::<&gtk::Widget>);
            ..show_all();
        };
        back_button.set_visible(false);

        glib::timeout_add_seconds_local(
            5,
            clone!(@weak window => @default-return glib::Continue(false), move || {
                println!("Foo");
                // needs to refresh all daemons, if multiple
                // for keyboard
                //     remove if not connected
                // check for new keyboards
                //     foreach
                //         add if not already
                // Have BTreeMap of Keyboard
                // XXX
                for keyboard in window.inner().keyboards.borrow().iter() {
                    keyboard.board().0.refresh();
                }
                println!("Bar");
                glib::Continue(true)
            }),
        );

        self.back_button.set(back_button);
        self.header_bar.set(header_bar);
        self.keyboard_list_box.set(keyboard_list_box);
        self.layer_switcher.set(layer_switcher);
        self.picker.set(picker);
        self.stack.set(stack);
    }
}
impl WidgetImpl for MainWindowInner {
    fn destroy(&self, window: &MainWindow) {
        self.parent_destroy(window);
        info!("Window close");
    }
}
impl ContainerImpl for MainWindowInner {}
impl BinImpl for MainWindowInner {}
impl WindowImpl for MainWindowInner {}
impl ApplicationWindowImpl for MainWindowInner {}

glib::wrapper! {
    pub struct MainWindow(ObjectSubclass<MainWindowInner>)
        @extends gtk::ApplicationWindow, gtk::Window, gtk::Bin, gtk::Container, gtk::Widget,
        @implements gio::ActionGroup, gio::ActionMap;
}

impl MainWindow {
    pub fn new(phony_board_names: Vec<String>) -> Self {
        let window: Self = glib::Object::new(&[]).unwrap();

        let daemon = daemon();

        for i in daemon.boards().expect("Failed to load boards") {
            let board = DaemonBoard(daemon.clone(), i);
            window.add_keyboard(board);
        }

        if !phony_board_names.is_empty() {
            let daemon = Rc::new(DaemonDummy::new(phony_board_names));

            for i in daemon.boards().unwrap() {
                let board = DaemonBoard(daemon.clone(), i);
                window.add_keyboard(board);
            }
        }

        window
    }

    fn inner(&self) -> &MainWindowInner {
        MainWindowInner::from_instance(self)
    }

    fn show_keyboard_list(&self) {
        let inner = self.inner();
        inner
            .stack
            .set_transition_type(gtk::StackTransitionType::SlideRight);
        inner.stack.set_visible_child(&*inner.keyboard_list_box);
        inner.header_bar.set_custom_title::<gtk::Widget>(None);
        inner.back_button.set_visible(false);
        if let Some(widget) = inner.picker.get_parent() {
            widget
                .downcast::<gtk::Container>()
                .unwrap()
                .remove(&*inner.picker);
        }
    }

    fn show_keyboard(&self, keyboard: &Keyboard) {
        let inner = self.inner();

        let keyboard_box = keyboard
            .get_parent()
            .unwrap()
            .downcast::<gtk::Box>()
            .unwrap();
        inner
            .stack
            .set_transition_type(gtk::StackTransitionType::SlideLeft);
        inner.stack.set_visible_child(&keyboard_box);
        inner
            .header_bar
            .set_custom_title(Some(&*inner.layer_switcher));
        inner.layer_switcher.set_stack(Some(keyboard.stack()));
        self.insert_action_group("kbd", Some(keyboard.action_group()));
        inner.back_button.set_visible(true);

        keyboard_box.add(&*inner.picker);
        inner.picker.set_keyboard(Some(keyboard.clone()));
        inner.picker.show_all();
    }

    fn add_keyboard(&self, board: DaemonBoard) {
        let model = match board.model() {
            Ok(model) => model,
            Err(err) => {
                error!("Failed to get board model: {}", err);
                return;
            }
        };

        if let Some(keyboard) = Keyboard::new_board(&model, board) {
            self.inner().keyboards.borrow_mut().push(keyboard.clone());

            keyboard.set_halign(gtk::Align::Center);
            keyboard.show_all();

            let attr_list = cascade! {
                pango::AttrList::new();
                ..insert(pango::Attribute::new_weight(pango::Weight::Bold));
            };
            let label = cascade! {
                gtk::Label::new(Some(&keyboard.display_name()));
                ..set_attributes(Some(&attr_list));
            };
            let window = self;
            let button = cascade! {
                gtk::Button::with_label("Configure Layout");
                ..set_halign(gtk::Align::Center);
                ..connect_clicked(clone!(@weak window, @weak keyboard => move |_| {
                    window.show_keyboard(&keyboard);
                }));
            };
            let keyboard_layer = cascade! {
                KeyboardLayer::new(Page::Keycaps, keyboard.keys().clone());
                ..set_selectable(false);
                ..set_halign(gtk::Align::Center);
            };
            let keyboard_box = cascade! {
                gtk::Box::new(gtk::Orientation::Vertical, 12);
                ..add(&label);
                ..add(&keyboard_layer);
                ..add(&button);
            };
            let row = cascade! {
                gtk::ListBoxRow::new();
                ..set_activatable(false);
                ..set_selectable(false);
                ..add(&keyboard_box);
                ..set_margin_top(12);
                ..set_margin_bottom(12);
                ..show_all();
            };
            self.inner().keyboard_list_box.add(&row);

            let keyboard_box = cascade! {
                gtk::Box::new(gtk::Orientation::Vertical, 12);
                ..set_visible(true);
                ..add(&keyboard);
            };
            self.inner().stack.add(&keyboard_box);

            // XXX if only one keyboard, show that with no back button
            self.inner().count.fetch_add(1, Ordering::Relaxed);
        } else {
            error!("Failed to locate layout for '{}'", model);
        }
    }
}

#[cfg(target_os = "linux")]
fn daemon() -> Rc<dyn Daemon> {
    if unsafe { libc::geteuid() == 0 } {
        info!("Already running as root");
        Rc::new(DaemonServer::new_stdio().expect("Failed to create server"))
    } else {
        info!("Not running as root, spawning daemon with pkexec");
        Rc::new(DaemonClient::new_pkexec())
    }
}

#[cfg(not(target_os = "linux"))]
fn daemon() -> Rc<dyn Daemon> {
    let server = DaemonServer::new_stdio().expect("Failed to create server");
    Rc::new(server)
}
