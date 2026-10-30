use edutools_core::board::{BoardAppearance, BoardKind};
use edutools_core::settings::SettingsColor;

// Swift: BoardColorTests.blackStrokeDisplaysWhiteOnBlackboard()
#[test]
fn black_stroke_displays_white_on_blackboard() {
    let stored_color = SettingsColor {
        red: 0.0,
        green: 0.0,
        blue: 0.0,
        alpha: 1.0,
    };
    let displayed_color = BoardAppearance::display_color(&stored_color, Some(BoardKind::Black));

    assert!(BoardAppearance::is_black(&stored_color));
    assert!(displayed_color.red >= 0.999);
    assert!(displayed_color.green >= 0.999);
    assert!(displayed_color.blue >= 0.999);
}
