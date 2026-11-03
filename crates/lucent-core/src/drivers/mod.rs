//! Keyboard drivers. Each one is written and tested against real hardware.

pub mod gmmk_v1;
pub mod gmmk_v1_keys;

use crate::driver::KeyboardDriver;
use crate::keyboard::model::Model;
use hidapi::HidApi;
use serde::Serialize;

/// USB vendors whose keyboards may speak a protocol the app knows (Sonix chips like the GMMK
/// v1's, and Glorious). A keyboard from them without a model is reported, never talked to.
const FAMILY_VENDORS: &[(u16, &str)] = &[(0x0C45, "Sonix"), (0x320F, "Glorious")];

/// A keyboard on USB that the app has no model for.
#[derive(Debug, Clone, Serialize)]
pub struct UnknownKeyboard {
    /// Hex, e.g. "0C45".
    pub vid: String,
    pub pid: String,
    pub vendor: String,
    pub manufacturer: String,
    pub product: String,
}

pub enum Detection {
    /// A keyboard with a model, opened with its driver.
    Known { model: Model, driver: Box<dyn KeyboardDriver> },
    /// A model matched but its driver could not open the keyboard.
    Failed { model: Model, error: String },
    /// A keyboard from a known family, without a model: nothing is sent to it.
    Unconfigured(UnknownKeyboard),
    None,
}

/// Looks for a keyboard on USB. Models are tried in order, so a contributor's model placed
/// before the built-in ones wins for the same USB ids.
pub fn detect(api: &HidApi, models: &[Model]) -> Detection {
    let present = |vid: u16, pid: u16| api.device_list().any(|d| d.vendor_id() == vid && d.product_id() == pid);
    for model in models {
        let Some((vid, pid)) = model.usb.as_ref().and_then(|u| u.parse()) else { continue };
        if !present(vid, pid) {
            continue;
        }
        let opened = match model.driver.as_deref() {
            Some("gmmk_v1") => gmmk_v1::GmmkV1::open(api, model).map(|d| Box::new(d) as Box<dyn KeyboardDriver>),
            other => Err(format!("Unknown driver {other:?} in model {}", model.id)),
        };
        return match opened {
            Ok(driver) => Detection::Known { model: model.clone(), driver },
            Err(error) => Detection::Failed { model: model.clone(), error },
        };
    }
    let devices: Vec<HidInterface> = api
        .device_list()
        .map(|d| HidInterface {
            vid: d.vendor_id(),
            pid: d.product_id(),
            usage_page: d.usage_page(),
            usage: d.usage(),
            manufacturer: d.manufacturer_string().unwrap_or_default().to_string(),
            product: d.product_string().unwrap_or_default().to_string(),
        })
        .collect();
    match find_unknown(&devices) {
        Some(k) => Detection::Unconfigured(k),
        None => Detection::None,
    }
}

/// Opens the first keyboard with a built-in model (for the developer tools).
pub fn open_keyboard(api: &HidApi) -> Result<Box<dyn KeyboardDriver>, String> {
    match detect(api, &crate::keyboard::model::builtin_models()) {
        Detection::Known { driver, .. } => Ok(driver),
        Detection::Failed { error, .. } => Err(error),
        Detection::Unconfigured(k) => Err(format!("{} {} ({}:{}) has no model", k.manufacturer, k.product, k.vid, k.pid)),
        Detection::None => Err("No supported keyboard found on USB".into()),
    }
}

/// One HID interface, as enumeration reports it.
pub struct HidInterface {
    pub vid: u16,
    pub pid: u16,
    pub usage_page: u16,
    pub usage: u16,
    pub manufacturer: String,
    pub product: String,
}

/// The first keyboard (usage page 1, usage 6) from a family vendor: their USB ids are also used
/// by webcams and other gadgets, which are not keyboards.
pub fn find_unknown(devices: &[HidInterface]) -> Option<UnknownKeyboard> {
    devices.iter().find_map(|d| {
        let (_, vendor) = FAMILY_VENDORS.iter().find(|(v, _)| *v == d.vid)?;
        (d.usage_page == 0x01 && d.usage == 0x06).then(|| UnknownKeyboard {
            vid: format!("{:04X}", d.vid),
            pid: format!("{:04X}", d.pid),
            vendor: vendor.to_string(),
            manufacturer: d.manufacturer.clone(),
            product: d.product.clone(),
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hid(vid: u16, pid: u16, usage_page: u16, usage: u16) -> HidInterface {
        HidInterface { vid, pid, usage_page, usage, manufacturer: "M".into(), product: "P".into() }
    }

    #[test]
    fn reports_only_keyboards_from_known_vendors() {
        // A Sonix webcam's HID button and another brand's keyboard are not candidates.
        assert!(find_unknown(&[hid(0x0C45, 0x6366, 0x0C, 0x01), hid(0x046D, 0xC33C, 0x01, 0x06)]).is_none());
        let k = find_unknown(&[hid(0x320F, 0x5044, 0xFF1C, 0x92), hid(0x320F, 0x5044, 0x01, 0x06)]).unwrap();
        assert_eq!((k.vid.as_str(), k.pid.as_str(), k.vendor.as_str()), ("320F", "5044", "Glorious"));
    }
}
