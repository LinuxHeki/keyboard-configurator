use cascade::cascade;
use futures::{prelude::*, stream::FuturesUnordered};
use glib::clone;
use gtk::prelude::*;
use gtk::subclass::prelude::*;
use once_cell::sync::Lazy;
use std::{cell::RefCell, collections::HashMap};

use crate::Keyboard;
use backend::DerefCell;

mod picker_group;
mod picker_group_box;
mod picker_json;
mod picker_key;

use picker_group_box::PickerGroupBox;
use picker_json::picker_json;
use picker_key::PickerKey;

pub static SCANCODE_LABELS: Lazy<HashMap<String, String>> = Lazy::new(|| {
    let mut labels = HashMap::new();
    for group in picker_json() {
        for key in group.keys {
            labels.insert(key.keysym, key.label);
        }
    }
    labels
});

#[derive(Default)]
pub struct PickerInner {
    group_box: DerefCell<PickerGroupBox>,
    keyboard: RefCell<Option<Keyboard>>,
    mod_tap_box: DerefCell<gtk::Box>,
    mod_tap_check: DerefCell<gtk::CheckButton>,
    mod_tap_mods: DerefCell<gtk::ComboBoxText>,
}

#[glib::object_subclass]
impl ObjectSubclass for PickerInner {
    const NAME: &'static str = "S76KeyboardPicker";
    type ParentType = gtk::Box;
    type Type = Picker;
}

impl ObjectImpl for PickerInner {
    fn constructed(&self, picker: &Picker) {
        self.parent_constructed(picker);

        let group_box = cascade! {
            PickerGroupBox::new();
            ..connect_key_pressed(clone!(@weak picker => move |name| {
                picker.key_pressed(name)
            }));
        };

        // TODO: set initial values, bind change

        let mod_tap_check = cascade! {
            gtk::CheckButton::with_label("Mod-Tap");
            ..connect_toggled(clone!(@weak picker => move |_| {
                picker.mod_tap_updated();
            }));
        };

        let mod_tap_mods = cascade! {
            gtk::ComboBoxText::new();
            ..append(Some("LEFT_CTRL"), "Left Ctrl");
            ..append(Some("LEFT_SHIFT"), "Left Shift");
            ..append(Some("LEFT_ALT"), "Left Alt");
            ..append(Some("LEFT_SUPER"), "Left Super");
            ..append(Some("RIGHT_CTRL"), "Right Ctrl");
            ..append(Some("RIGHT_SHIFT"), "Right Shift");
            ..append(Some("RIGHT_ALT"), "Right Alt");
            ..append(Some("RIGHT_SUPER"), "Right Super");
            ..connect_property_active_id_notify(clone!(@weak picker => move |_| {
                picker.mod_tap_updated();
            }));
        };

        let mod_tap_box = cascade! {
            gtk::Box::new(gtk::Orientation::Horizontal, 8);
            ..add(&mod_tap_check);
            ..add(&mod_tap_mods);
        };

        cascade! {
            picker;
            ..set_spacing(18);
            ..set_orientation(gtk::Orientation::Vertical);
            ..add(&group_box);
            ..add(&mod_tap_box);
            ..show_all();
        };

        self.group_box.set(group_box);
        self.mod_tap_box.set(mod_tap_box);
        self.mod_tap_check.set(mod_tap_check);
        self.mod_tap_mods.set(mod_tap_mods);
    }
}

impl BoxImpl for PickerInner {}

impl WidgetImpl for PickerInner {}

impl ContainerImpl for PickerInner {}

glib::wrapper! {
    pub struct Picker(ObjectSubclass<PickerInner>)
        @extends gtk::Box, gtk::Container, gtk::Widget, @implements gtk::Orientable;
}

impl Picker {
    pub fn new() -> Self {
        glib::Object::new(&[]).unwrap()
    }

    fn inner(&self) -> &PickerInner {
        PickerInner::from_instance(self)
    }

    pub(crate) fn set_keyboard(&self, keyboard: Option<Keyboard>) {
        if let Some(old_kb) = &*self.inner().keyboard.borrow() {
            old_kb.set_picker(None);
        }

        if let Some(kb) = &keyboard {
            // Check that scancode is available for the keyboard
            self.inner().group_box.set_key_visibility(|name| {
                let visible = kb.has_scancode(name);
                // XXX only with mod-tap
                let sensitive = kb.layout().scancode_from_name(name).unwrap_or(0) < 256;
                (visible, sensitive)
            });
            kb.set_picker(Some(&self));

            self.inner()
                .mod_tap_box
                .set_visible(kb.layout().meta.has_mod_tap);
        }

        *self.inner().keyboard.borrow_mut() = keyboard;
    }

    pub(crate) fn set_selected(&self, scancode_names: Vec<String>) {
        // TODO selected needs to support mod tap
        self.inner().group_box.set_selected(scancode_names);
    }

    fn mod_(&self) -> Option<String> {
        if self.inner().mod_tap_check.get_active() {
            Some(self.inner().mod_tap_mods.get_active_id()?.into())
        } else {
            None
        }
    }

    fn key_pressed(&self, mut name: String) {
        let kb = match self.inner().keyboard.borrow().clone() {
            Some(kb) => kb,
            None => {
                return;
            }
        };
        let layer = kb.layer();

        if let Some(mod_) = self.mod_() {
            name = format!("MT({}, {})", mod_, name);
        }

        info!("Clicked {} layer {:?}", name, layer);
        if let Some(layer) = layer {
            let futures = FuturesUnordered::new();
            for i in kb.selected().iter().copied() {
                futures.push(clone!(@strong kb, @strong name => async move {
                    kb.keymap_set(i, layer, &name).await;
                }));
            }
            glib::MainContext::default().spawn_local(async { futures.collect::<()>().await });
        }
    }

    fn mod_tap_updated(&self) {}
}

#[cfg(test)]
mod tests {
    use crate::*;
    use backend::{layouts, Layout};
    use std::collections::HashSet;

    #[test]
    fn picker_has_keys() {
        let mut missing = HashSet::new();
        for i in layouts() {
            let layout = Layout::from_board(i).unwrap();
            for j in layout.default.map.values().flatten() {
                if SCANCODE_LABELS.keys().find(|x| x == &j).is_none() {
                    missing.insert(j.to_owned());
                }
            }
        }
        assert_eq!(missing, HashSet::new());
    }
}
