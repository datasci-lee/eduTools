use edutools_core::keystroke::{KeystrokeDisplayFilter, Modifier, ModifierSet};

// Swift: ShortcutPresentationTests.modifiedKeystrokeDisplayFiltersTyping()
#[test]
fn modified_keystroke_display_filters_typing() {
    let displayed_control_c =
        KeystrokeDisplayFilter::display_string(ModifierSet::empty().with(Modifier::Control), "C");
    let rejected_plain_c = KeystrokeDisplayFilter::display_string(ModifierSet::empty(), "C");
    let rejected_shift_c =
        KeystrokeDisplayFilter::display_string(ModifierSet::empty().with(Modifier::Shift), "C");

    assert_eq!(displayed_control_c.as_deref(), Some("Ctrl+C"));
    assert_eq!(rejected_plain_c, None);
    assert_eq!(rejected_shift_c, None);
}
