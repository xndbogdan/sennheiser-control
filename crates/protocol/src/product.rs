//! Headsets that speak the same GAIA v3 command set as the MOMENTUM 4.
//!
//! Every product here has a `protocol: "GAIAV3"` device model in Smart Control 4.9.4 (`assets/app/<model>.json`) that
//! uses the same vendor `0x0495` command ids as `m4.json`, and the official app reaches it over the same Bluetooth
//! Classic RFCOMM service (`GAIA3_SPP_UUID`). They differ in which features exist; see [`crate::Capabilities::new`].
//!
//! Not supported: the GAIA v3 CX earbuds (CX True Wireless, Sport True Wireless, CX Plus True Wireless, CX 500BT),
//! which the official app only reaches over Bluetooth LE, and older products (HD 450BT, MOMENTUM 3, PXC 550,
//! MOMENTUM True Wireless 1/2, …) that use the legacy GAIA or "Nautilus" protocols.

use crate::eq::{self, BANDS, Gen1Params};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
pub enum Product {
    #[default]
    Momentum4,
    Accentum,
    AccentumPlus,
    AccentumTw,
    MomentumTw3,
    MomentumTw4,
    MomentumSport,
}

impl Product {
    pub const ALL: [Product; 7] = [
        Product::Momentum4,
        Product::Accentum,
        Product::AccentumPlus,
        Product::AccentumTw,
        Product::MomentumTw3,
        Product::MomentumTw4,
        Product::MomentumSport,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Product::Momentum4 => "MOMENTUM 4",
            Product::Accentum => "ACCENTUM Wireless",
            Product::AccentumPlus => "ACCENTUM Plus Wireless",
            Product::AccentumTw => "ACCENTUM True Wireless",
            Product::MomentumTw3 => "MOMENTUM True Wireless 3",
            Product::MomentumTw4 => "MOMENTUM True Wireless 4",
            Product::MomentumSport => "MOMENTUM Sport",
        }
    }

    /// Bluetooth names (`DeviceType.deviceTypeByBluetoothNameMap`, bundle.pretty.js:21714).
    fn bt_names(self) -> &'static [&'static str] {
        match self {
            Product::Momentum4 => &["MOMENTUM 4", "Mars"],
            Product::Accentum => &["ACCENTUM", "VENUS"],
            Product::AccentumPlus => &["ACCENTUM Plus", "APHRODITE"],
            Product::AccentumTw => &["ACCENTUM TW"],
            Product::MomentumTw3 => &["MOMENTUM TW 3"],
            Product::MomentumTw4 => &["MOMENTUM TW 4", "MOMENTUM TW4"],
            Product::MomentumSport => &[
                "MOMENTUM SPORT",
                "MOMENTUM SPORT1",
                "MOMENTUM Sport",
                "Magnum",
                "MSPORT1-HR",
                "MSPORT1-Sensor",
                "Sensor-Magnum",
                "Sensor-MOMENTUM SPORT1",
                "Sensor-MOMENTUM Sport",
            ],
        }
    }

    /// `Versions_ModelId` prefixes; the rest of the string is the colour variant
    /// (`DeviceType.getDeviceTypeByManufacturerName`, bundle.pretty.js:21286).
    fn model_ids(self) -> &'static [&'static str] {
        match self {
            Product::Momentum4 => &["M4AEBT", "Mars", "MARS"],
            Product::Accentum => &["ACAEBT", "AC1", "VENUS"],
            Product::AccentumPlus => &["ACPAEBT", "ACP1", "APHRODITE"],
            Product::AccentumTw => &["ATW1", "Accentum TW"],
            Product::MomentumTw3 => &["MTW3"],
            Product::MomentumTw4 => &["MTW4"],
            Product::MomentumSport => &["MSPORT1", "Magnum"],
        }
    }

    /// Product for a Bluetooth name. The official app matches exactly; a name that starts with a known one followed
    /// by a space (e.g. "ACCENTUM Plus Wireless") is accepted too, since the model id read after connecting decides.
    pub fn from_bt_name(name: &str) -> Option<Product> {
        let name = name.trim();
        longest_prefix(name, Self::bt_names)
    }

    /// Product and colour variant from `Versions_ModelId` (e.g. `ACPAEBT Black` → Accentum Plus, "Black").
    pub fn from_model_id(model_id: &str) -> Option<(Product, &str)> {
        let id = model_id.trim();
        let p = longest_prefix(id, Self::model_ids)?;
        let len = p
            .model_ids()
            .iter()
            .filter(|m| starts_with_word(id, m))
            .map(|m| m.len())
            .max()?;
        let v = id[len..].trim();
        Some((p, if v.is_empty() { "Standard" } else { v }))
    }

    /// Whether this project has checked the product on real hardware. Everything else is built from the official
    /// app's device model only.
    pub fn verified(self) -> bool {
        self == Product::Momentum4
    }

    pub fn is_earbuds(self) -> bool {
        !matches!(
            self,
            Product::Momentum4 | Product::Accentum | Product::AccentumPlus
        )
    }

    /// `get5BandParametersForDevice` (bundle.pretty.js:70422).
    pub fn gen1_params(self) -> &'static Gen1Params {
        match self {
            Product::Momentum4 => &eq::M4_GEN1,
            Product::Accentum | Product::AccentumPlus => &eq::ACCENTUM_GEN1,
            Product::AccentumTw => &eq::ATW1_GEN1,
            Product::MomentumSport => &eq::MSPORT1_GEN1,
            Product::MomentumTw3 => &eq::MTW3_GEN1,
            Product::MomentumTw4 => &eq::MTW4_GEN1,
        }
    }

    /// Built-in EQ presets (`getOriginalPresets`, bundle.pretty.js:22448).
    pub fn presets(self) -> &'static [(&'static str, [f32; BANDS])] {
        match self {
            Product::Momentum4 => eq::PRESETS,
            Product::Accentum | Product::AccentumPlus => eq::ACCENTUM_PRESETS,
            Product::MomentumSport => eq::MSPORT1_PRESETS,
            Product::AccentumTw | Product::MomentumTw3 | Product::MomentumTw4 => eq::MTW3_PRESETS,
        }
    }
}

fn starts_with_word(s: &str, prefix: &str) -> bool {
    s.len() >= prefix.len()
        && s.is_char_boundary(prefix.len())
        && s[..prefix.len()].eq_ignore_ascii_case(prefix)
        && s[prefix.len()..].chars().next().is_none_or(|c| c == ' ')
}

/// The product whose longest name is `s` itself or a whole-word prefix of `s` (case-insensitive).
fn longest_prefix(s: &str, names: fn(Product) -> &'static [&'static str]) -> Option<Product> {
    Product::ALL
        .iter()
        .flat_map(|&p| names(p).iter().map(move |n| (p, n)))
        .filter(|(_, n)| starts_with_word(s, n))
        .max_by_key(|(_, n)| n.len())
        .map(|(p, _)| p)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bt_names() {
        assert_eq!(
            Product::from_bt_name("MOMENTUM 4"),
            Some(Product::Momentum4)
        );
        assert_eq!(
            Product::from_bt_name("momentum 4 "),
            Some(Product::Momentum4)
        );
        assert_eq!(Product::from_bt_name("Mars"), Some(Product::Momentum4));
        assert_eq!(Product::from_bt_name("ACCENTUM"), Some(Product::Accentum));
        assert_eq!(
            Product::from_bt_name("ACCENTUM Plus"),
            Some(Product::AccentumPlus)
        );
        assert_eq!(
            Product::from_bt_name("ACCENTUM Plus Wireless"),
            Some(Product::AccentumPlus)
        );
        assert_eq!(
            Product::from_bt_name("ACCENTUM TW"),
            Some(Product::AccentumTw)
        );
        assert_eq!(
            Product::from_bt_name("MOMENTUM TW 4"),
            Some(Product::MomentumTw4)
        );
        assert_eq!(
            Product::from_bt_name("MOMENTUM Sport"),
            Some(Product::MomentumSport)
        );
        // legacy-protocol and BLE-only products stay unsupported
        for n in [
            "MOMENTUM TW",
            "MOMENTUM TW 2",
            "MOMENTUM 3",
            "HD 450BT",
            "CX 400BT TW",
            "CX Plus True Wireless",
            "CX True Wireless",
        ] {
            assert_eq!(Product::from_bt_name(n), None, "{n}");
        }
        assert_eq!(Product::from_bt_name("MOMENTUM 40"), None);
        assert_eq!(Product::from_bt_name("ACCENTUMX"), None);
    }

    #[test]
    fn model_ids() {
        assert_eq!(
            Product::from_model_id("M4AEBT Denim"),
            Some((Product::Momentum4, "Denim"))
        );
        assert_eq!(
            Product::from_model_id("M4AEBT"),
            Some((Product::Momentum4, "Standard"))
        );
        assert_eq!(
            Product::from_model_id("ACPAEBT White"),
            Some((Product::AccentumPlus, "White"))
        );
        assert_eq!(
            Product::from_model_id("ACAEBT SE"),
            Some((Product::Accentum, "SE"))
        );
        assert_eq!(
            Product::from_model_id("MTW4 Black Copper"),
            Some((Product::MomentumTw4, "Black Copper"))
        );
        assert_eq!(Product::from_model_id("HD450 B"), None);
    }
}
