use std::collections::BTreeMap;
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct GpuKey {
    pub vendor: u16,
    pub device: u16,
    pub subsys: u32,
}

pub type AdapterId = GpuKey;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AdapterKind {
    Discrete,
    Integrated,
    Virtual,
    Software,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AdapterIdStyle {
    Padded,
    Compact,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AdapterIdError {
    Empty,
    BadShape,
    BadVendor,
    BadDevice,
    BadSubsys,
}

impl fmt::Display for AdapterIdError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => write!(f, "adapter id is empty"),
            Self::BadShape => write!(f, "adapter id must look like VVVV&DDDD&SSSSSSSS"),
            Self::BadVendor => write!(f, "bad vendor part in adapter id"),
            Self::BadDevice => write!(f, "bad device part in adapter id"),
            Self::BadSubsys => write!(f, "bad subsys part in adapter id"),
        }
    }
}

impl std::error::Error for AdapterIdError {}

fn parse_hex_u32(part: &str) -> Option<u32> {
    let part = part.trim();
    if part.is_empty() {
        return None;
    }
    if part.starts_with('+') || part.starts_with('-') {
        return None;
    }
    u32::from_str_radix(part, 16).ok()
}

pub fn parse_adapter_id(text: &str) -> Result<GpuKey, AdapterIdError> {
    let text = text.trim();
    if text.is_empty() {
        return Err(AdapterIdError::Empty);
    }
    let parts: Vec<&str> = text.split('&').collect();
    if parts.len() != 3 {
        return Err(AdapterIdError::BadShape);
    }
    let vendor = parse_hex_u32(parts[0]).ok_or(AdapterIdError::BadVendor)?;
    let device = parse_hex_u32(parts[1]).ok_or(AdapterIdError::BadDevice)?;
    let subsys = parse_hex_u32(parts[2]).ok_or(AdapterIdError::BadSubsys)?;
    if vendor > u32::from(u16::MAX) {
        return Err(AdapterIdError::BadVendor);
    }
    if device > u32::from(u16::MAX) {
        return Err(AdapterIdError::BadDevice);
    }
    Ok(GpuKey {
        vendor: vendor as u16,
        device: device as u16,
        subsys,
    })
}

pub fn format_adapter_id(key: GpuKey, style: AdapterIdStyle) -> String {
    match style {
        AdapterIdStyle::Padded => {
            format!("{:04X}&{:04X}&{:08X}", key.vendor, key.device, key.subsys)
        }
        AdapterIdStyle::Compact => {
            format!("{:X}&{:X}&{:X}", key.vendor, key.device, key.subsys)
        }
    }
}

impl GpuKey {
    pub fn parse(text: &str) -> Result<Self, AdapterIdError> {
        parse_adapter_id(text)
    }

    pub fn format(self, style: AdapterIdStyle) -> String {
        format_adapter_id(self, style)
    }
}

fn is_physical(kind: AdapterKind) -> bool {
    matches!(kind, AdapterKind::Discrete | AdapterKind::Integrated)
}

pub fn ambiguous_keys(adapters: &[(GpuKey, AdapterKind)]) -> Vec<GpuKey> {
    let mut counts: BTreeMap<GpuKey, usize> = BTreeMap::new();
    for (key, kind) in adapters {
        if is_physical(*kind) {
            *counts.entry(*key).or_insert(0) += 1;
        }
    }
    counts
        .into_iter()
        .filter(|(_, count)| *count >= 2)
        .map(|(key, _)| key)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const D_GPU: &str = "1002&747E&2427148C";
    const I_GPU: &str = "1002&164E&88771043";

    fn dgpu_key() -> GpuKey {
        GpuKey {
            vendor: 0x1002,
            device: 0x747E,
            subsys: 0x2427_148C,
        }
    }

    fn igpu_key() -> GpuKey {
        GpuKey {
            vendor: 0x1002,
            device: 0x164E,
            subsys: 0x8877_1043,
        }
    }

    #[test]
    fn parses_reference_keys() {
        assert_eq!(parse_adapter_id(D_GPU), Ok(dgpu_key()));
        assert_eq!(parse_adapter_id(I_GPU), Ok(igpu_key()));
    }

    #[test]
    fn parse_is_case_insensitive_and_trimmed() {
        assert_eq!(parse_adapter_id("  1002&747e&2427148c  "), Ok(dgpu_key()));
        assert_eq!(
            parse_adapter_id("1002&747E&2427148C"),
            parse_adapter_id("1002&747e&2427148c")
        );
    }

    #[test]
    fn parse_accepts_unpadded_parts() {
        assert_eq!(
            parse_adapter_id("1&2e&3"),
            Ok(GpuKey {
                vendor: 1,
                device: 0x2E,
                subsys: 3,
            })
        );
    }

    #[test]
    fn formats_both_styles() {
        let key = GpuKey {
            vendor: 1,
            device: 0x2E,
            subsys: 3,
        };
        assert_eq!(
            format_adapter_id(key, AdapterIdStyle::Padded),
            "0001&002E&00000003"
        );
        assert_eq!(format_adapter_id(key, AdapterIdStyle::Compact), "1&2E&3");
    }

    #[test]
    fn reference_keys_round_trip_in_both_styles() {
        for key in [dgpu_key(), igpu_key()] {
            for style in [AdapterIdStyle::Padded, AdapterIdStyle::Compact] {
                let text = format_adapter_id(key, style);
                assert_eq!(parse_adapter_id(&text), Ok(key));
            }
        }
        assert_eq!(dgpu_key().format(AdapterIdStyle::Padded), D_GPU);
        assert_eq!(dgpu_key().format(AdapterIdStyle::Compact), D_GPU);
    }

    #[test]
    fn rejects_malformed_ids() {
        for bad in [
            "",
            "   ",
            "1002&747E",
            "1002&747E&2427148C&1",
            "1002",
            "ZZZZ&747E&2427148C",
            "1002&ZZZZ&2427148C",
            "1002&747E&ZZZZZZZZ",
            "1002&&2427148C",
            "&747E&2427148C",
            "1002&747E&",
            "1002&1747E&0",
            "1747E&1&0",
            "1002&747E&123456789",
            "0x1002&747E&2427148C",
            "-1&2&3",
        ] {
            assert!(parse_adapter_id(bad).is_err(), "should reject {bad:?}");
        }
    }

    #[test]
    fn twin_cards_are_ambiguous() {
        let adapters = [
            (dgpu_key(), AdapterKind::Discrete),
            (dgpu_key(), AdapterKind::Discrete),
        ];
        assert_eq!(ambiguous_keys(&adapters), vec![dgpu_key()]);
    }

    #[test]
    fn discrete_plus_integrated_sharing_a_key_is_ambiguous() {
        let adapters = [
            (dgpu_key(), AdapterKind::Discrete),
            (dgpu_key(), AdapterKind::Integrated),
        ];
        assert_eq!(ambiguous_keys(&adapters), vec![dgpu_key()]);
    }

    #[test]
    fn idd_clone_is_not_ambiguous() {
        let adapters = [
            (dgpu_key(), AdapterKind::Discrete),
            (dgpu_key(), AdapterKind::Virtual),
        ];
        assert!(ambiguous_keys(&adapters).is_empty());
    }

    #[test]
    fn virtual_and_software_only_matches_are_not_ambiguous() {
        let adapters = [
            (dgpu_key(), AdapterKind::Virtual),
            (dgpu_key(), AdapterKind::Software),
        ];
        assert!(ambiguous_keys(&adapters).is_empty());
    }

    #[test]
    fn distinct_keys_are_not_ambiguous() {
        let adapters = [
            (dgpu_key(), AdapterKind::Discrete),
            (igpu_key(), AdapterKind::Integrated),
        ];
        assert!(ambiguous_keys(&adapters).is_empty());
    }
}
