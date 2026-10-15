use crate::identity::{format_adapter_id, AdapterIdStyle, GpuKey};

use super::tokens::{self, Token};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GpuChoice {
    WindowsDecides,
    PowerSaving,
    HighPerformance,
    Specific(GpuKey),
}

fn is_app_gpu_token(name: &str) -> bool {
    name.eq_ignore_ascii_case("GpuPreference") || name.eq_ignore_ascii_case("SpecificAdapter")
}

fn is_high_perf_token(name: &str) -> bool {
    name.eq_ignore_ascii_case("HighPerfAdapter")
}

fn gpu_token(name: &str, value: String) -> Token {
    Token {
        name: name.to_string(),
        value,
    }
}

fn other_tokens(tokens: &[Token]) -> Vec<Token> {
    tokens
        .iter()
        .filter(|token| !is_app_gpu_token(&token.name))
        .cloned()
        .collect()
}

pub fn apply_choice(tokens: &[Token], choice: &GpuChoice, style: AdapterIdStyle) -> Vec<Token> {
    let others = other_tokens(tokens);
    let mut gpu = Vec::new();
    match choice {
        GpuChoice::WindowsDecides => {}
        GpuChoice::PowerSaving => gpu.push(gpu_token("GpuPreference", "1".to_string())),
        GpuChoice::HighPerformance => gpu.push(gpu_token("GpuPreference", "2".to_string())),
        GpuChoice::Specific(key) => {
            gpu.push(gpu_token("SpecificAdapter", format_adapter_id(*key, style)));
            gpu.push(gpu_token("GpuPreference", "1073741824".to_string()));
        }
    }
    // DagorEngine reads only values that start with GpuPreference, so GPU tokens go first.
    gpu.extend(others);
    gpu
}

pub fn apply_high_perf(
    global: &[Token],
    adapter: Option<GpuKey>,
    style: AdapterIdStyle,
) -> Vec<Token> {
    match adapter {
        None => global
            .iter()
            .filter(|token| !is_high_perf_token(&token.name))
            .cloned()
            .collect(),
        Some(key) => {
            let id = format_adapter_id(key, style);
            let mut out = Vec::with_capacity(global.len() + 1);
            let mut replaced = false;
            for token in global {
                if is_high_perf_token(&token.name) {
                    if !replaced {
                        out.push(gpu_token(&token.name, id.clone()));
                        replaced = true;
                    }
                } else {
                    out.push(token.clone());
                }
            }
            if !replaced {
                out.push(gpu_token("HighPerfAdapter", id));
            }
            out
        }
    }
}

pub fn value_after(
    before: Option<&str>,
    choice: &GpuChoice,
    style: AdapterIdStyle,
) -> Option<String> {
    let parsed = before.map(tokens::parse).unwrap_or_default();
    let next = apply_choice(&parsed, choice, style);
    if next.is_empty() {
        return None;
    }
    if let Some(text) = before {
        if parsed == next {
            return Some(text.to_string());
        }
    }
    Some(tokens::format(&next))
}

pub fn value_after_high_perf(
    before: Option<&str>,
    adapter: Option<GpuKey>,
    style: AdapterIdStyle,
) -> Option<String> {
    let parsed = before.map(tokens::parse).unwrap_or_default();
    let next = apply_high_perf(&parsed, adapter, style);
    if next.is_empty() {
        return None;
    }
    if let Some(text) = before {
        if parsed == next {
            return Some(text.to_string());
        }
    }
    Some(tokens::format(&next))
}

#[cfg(test)]
mod tests {
    use super::*;

    const PEAK_BEFORE: &str = "AppStatus=0;AutoHDREnable=2097;";
    const GLOBAL_BEFORE: &str = "SwapEffectUpgradeEnable=1;AutoHDREnable=1;";

    fn dgpu_key() -> GpuKey {
        GpuKey::parse("1002&747E&2427148C").unwrap()
    }

    fn igpu_key() -> GpuKey {
        GpuKey::parse("1002&164E&88771043").unwrap()
    }

    #[test]
    fn windows_decides_removes_gpu_tokens_only() {
        let tokens = tokens::parse(
            "AppStatus=0;AutoHDREnable=2097;SpecificAdapter=1002&747E&2427148C;GpuPreference=1073741824;",
        );
        let next = apply_choice(&tokens, &GpuChoice::WindowsDecides, AdapterIdStyle::Padded);
        assert_eq!(tokens::format(&next), PEAK_BEFORE);
    }

    #[test]
    fn windows_decides_on_gpu_only_value_deletes_it() {
        assert_eq!(
            value_after(
                Some("GpuPreference=1; "),
                &GpuChoice::WindowsDecides,
                AdapterIdStyle::Padded
            ),
            None
        );
        assert_eq!(
            value_after(None, &GpuChoice::WindowsDecides, AdapterIdStyle::Padded),
            None
        );
    }

    #[test]
    fn power_saving_and_high_performance_go_first() {
        let tokens = tokens::parse(PEAK_BEFORE);
        let saving = apply_choice(&tokens, &GpuChoice::PowerSaving, AdapterIdStyle::Padded);
        assert_eq!(
            tokens::format(&saving),
            "GpuPreference=1;AppStatus=0;AutoHDREnable=2097;"
        );
        let perf = apply_choice(&tokens, &GpuChoice::HighPerformance, AdapterIdStyle::Padded);
        assert_eq!(
            tokens::format(&perf),
            "GpuPreference=2;AppStatus=0;AutoHDREnable=2097;"
        );
    }

    #[test]
    fn specific_writes_both_tokens_first_in_order() {
        let tokens = tokens::parse(PEAK_BEFORE);
        let next = apply_choice(
            &tokens,
            &GpuChoice::Specific(dgpu_key()),
            AdapterIdStyle::Padded,
        );
        assert_eq!(
            tokens::format(&next),
            "SpecificAdapter=1002&747E&2427148C;GpuPreference=1073741824;AppStatus=0;AutoHDREnable=2097;"
        );
    }

    #[test]
    fn untouched_value_keeps_exact_bytes() {
        assert_eq!(
            value_after(
                Some("GpuPreference=1; "),
                &GpuChoice::PowerSaving,
                AdapterIdStyle::Padded
            ),
            Some("GpuPreference=1; ".to_string())
        );
    }

    #[test]
    fn applying_same_choice_twice_changes_nothing() {
        let base = tokens::parse(PEAK_BEFORE);
        for choice in [
            GpuChoice::WindowsDecides,
            GpuChoice::PowerSaving,
            GpuChoice::HighPerformance,
            GpuChoice::Specific(dgpu_key()),
        ] {
            let once = apply_choice(&base, &choice, AdapterIdStyle::Padded);
            let twice = apply_choice(&once, &choice, AdapterIdStyle::Padded);
            assert_eq!(once, twice, "not idempotent for {choice:?}");
            let first = value_after(Some(PEAK_BEFORE), &choice, AdapterIdStyle::Padded);
            let second = value_after(first.as_deref(), &choice, AdapterIdStyle::Padded);
            assert_eq!(first, second, "value not stable for {choice:?}");
        }
    }

    #[test]
    fn malformed_input_never_panics() {
        let tokens = tokens::parse("===;NoEquals;GpuPreference=2;");
        for choice in [
            GpuChoice::WindowsDecides,
            GpuChoice::PowerSaving,
            GpuChoice::HighPerformance,
            GpuChoice::Specific(igpu_key()),
        ] {
            let _ = apply_choice(&tokens, &choice, AdapterIdStyle::Compact);
            let _ = value_after(Some("===;NoEquals;"), &choice, AdapterIdStyle::Compact);
        }
    }

    #[test]
    fn high_perf_set_appends_and_keeps_rest() {
        let global = tokens::parse(GLOBAL_BEFORE);
        let next = apply_high_perf(&global, Some(dgpu_key()), AdapterIdStyle::Padded);
        assert_eq!(
            tokens::format(&next),
            "SwapEffectUpgradeEnable=1;AutoHDREnable=1;HighPerfAdapter=1002&747E&2427148C;"
        );
    }

    #[test]
    fn high_perf_replace_stays_in_place() {
        let global = tokens::parse(
            "SwapEffectUpgradeEnable=1;HighPerfAdapter=1002&164E&88771043;AutoHDREnable=1;",
        );
        let next = apply_high_perf(&global, Some(dgpu_key()), AdapterIdStyle::Padded);
        assert_eq!(
            tokens::format(&next),
            "SwapEffectUpgradeEnable=1;HighPerfAdapter=1002&747E&2427148C;AutoHDREnable=1;"
        );
    }

    #[test]
    fn high_perf_remove_keeps_rest() {
        let global = tokens::parse(
            "SwapEffectUpgradeEnable=1;AutoHDREnable=1;HighPerfAdapter=1002&747E&2427148C;",
        );
        let next = apply_high_perf(&global, None, AdapterIdStyle::Padded);
        assert_eq!(tokens::format(&next), GLOBAL_BEFORE);
    }

    #[test]
    fn high_perf_untouched_keeps_exact_bytes() {
        let before =
            "SwapEffectUpgradeEnable=1;AutoHDREnable=1;HighPerfAdapter=1002&747E&2427148C;";
        assert_eq!(
            value_after_high_perf(Some(before), Some(dgpu_key()), AdapterIdStyle::Padded),
            Some(before.to_string())
        );
    }
}
