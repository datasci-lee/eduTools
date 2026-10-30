//! 화면 키별 창 위치 비율의 저장·복원 순수 로직.

use serde::Serialize;
use serde_json::Value;
use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub struct PositionRatio {
    pub x: f64,
    pub y: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScreenFrame {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WindowSize {
    pub width: f64,
    pub height: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WindowOrigin {
    pub x: f64,
    pub y: f64,
}

/// Swift `ScreenPositionStore`의 화면별 값과 마지막 화면 키를 JSON 값으로 옮긴 표현.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScreenPositionDocument {
    positions: BTreeMap<String, PositionRatio>,
    #[serde(skip_serializing_if = "Option::is_none")]
    last_display_key: Option<String>,
}

impl ScreenPositionDocument {
    pub fn ratio_for(&self, display_key: Option<&str>) -> Option<PositionRatio> {
        display_key.and_then(|key| self.positions.get(key).copied())
    }

    pub fn last_display_key(&self) -> Option<&str> {
        self.last_display_key.as_deref()
    }

    pub fn save(&mut self, ratio: PositionRatio, display_key: impl Into<String>) {
        let display_key = display_key.into();
        self.positions.insert(display_key.clone(), ratio);
        self.last_display_key = Some(display_key);
    }

    /// 같은 문서 안에서 창 종류별 저장 키를 쓰되 마지막 실제 화면 키는 공유한다.
    pub fn save_with_display_key(
        &mut self,
        ratio: PositionRatio,
        storage_key: impl Into<String>,
        display_key: impl Into<String>,
    ) {
        self.positions.insert(storage_key.into(), ratio);
        self.last_display_key = Some(display_key.into());
    }

    /// 손상된 개별 위치 항목은 Swift 저장소처럼 그 화면에서 값이 없는 것으로 취급한다.
    pub fn from_json(json: &str) -> Result<Self, ScreenPositionError> {
        let value: Value = serde_json::from_str(json)
            .map_err(|error| ScreenPositionError::InvalidJson(error.to_string()))?;
        let object = value
            .as_object()
            .ok_or(ScreenPositionError::RootNotObject)?;
        let mut positions = BTreeMap::new();
        if let Some(entries) = object.get("positions").and_then(Value::as_object) {
            for (key, entry) in entries {
                let Some(entry) = entry.as_object() else {
                    continue;
                };
                let (Some(x), Some(y)) = (
                    entry.get("x").and_then(Value::as_f64),
                    entry.get("y").and_then(Value::as_f64),
                ) else {
                    continue;
                };
                if x.is_finite() && y.is_finite() {
                    positions.insert(key.clone(), PositionRatio { x, y });
                }
            }
        }
        let last_display_key = object
            .get("lastDisplayKey")
            .and_then(Value::as_str)
            .map(str::to_owned);
        Ok(Self {
            positions,
            last_display_key,
        })
    }

    pub fn to_json(&self) -> Result<String, ScreenPositionError> {
        serde_json::to_string_pretty(self)
            .map_err(|error| ScreenPositionError::Encoding(error.to_string()))
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ScreenPositionError {
    InvalidJson(String),
    RootNotObject,
    Encoding(String),
}

impl fmt::Display for ScreenPositionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidJson(detail) => write!(formatter, "invalid position JSON: {detail}"),
            Self::RootNotObject => formatter.write_str("position JSON root is not an object"),
            Self::Encoding(detail) => write!(formatter, "position JSON encoding failed: {detail}"),
        }
    }
}

impl Error for ScreenPositionError {}

/// 저장값이 없으면 기본 비율을 쓰고, 창이 작업 영역을 벗어나지 않도록 원점을 클램프한다.
pub fn restored_origin(
    ratio: Option<PositionRatio>,
    default_ratio: PositionRatio,
    visible_frame: ScreenFrame,
    size: WindowSize,
) -> WindowOrigin {
    let available_width = (visible_frame.width - size.width).max(0.0);
    let available_height = (visible_frame.height - size.height).max(0.0);
    let ratio = ratio.unwrap_or(default_ratio);
    WindowOrigin {
        x: visible_frame.x + available_width * bounded_ratio(ratio.x, default_ratio.x),
        y: visible_frame.y + available_height * bounded_ratio(ratio.y, default_ratio.y),
    }
}

/// 창 원점을 화면별 0...1 비율로 저장한다. 창이 작업 영역 이상이면 분모 하한은 1이다.
pub fn saved_ratio(
    origin: WindowOrigin,
    visible_frame: ScreenFrame,
    size: WindowSize,
) -> PositionRatio {
    let available_width = (visible_frame.width - size.width).max(1.0);
    let available_height = (visible_frame.height - size.height).max(1.0);
    PositionRatio {
        x: ((origin.x - visible_frame.x) / available_width).clamp(0.0, 1.0),
        y: ((origin.y - visible_frame.y) / available_height).clamp(0.0, 1.0),
    }
}

fn bounded_ratio(value: f64, fallback: f64) -> f64 {
    let value = if value.is_finite() { value } else { fallback };
    if value.is_finite() {
        value.clamp(0.0, 1.0)
    } else {
        0.0
    }
}

#[cfg(test)]
mod tests {
    use super::{
        restored_origin, saved_ratio, PositionRatio, ScreenFrame, ScreenPositionDocument,
        WindowOrigin, WindowSize,
    };

    #[test]
    fn swift_restore_table_clamps_to_each_screen_and_uses_default() {
        let cases = [
            (
                "normal ratio",
                Some(ratio(0.5, 0.85)),
                ratio(0.1, 0.2),
                frame(0.0, 0.0, 1_000.0, 800.0),
                size(100.0, 50.0),
                origin(450.0, 637.5),
            ),
            (
                "lower boundary",
                Some(ratio(0.0, 0.0)),
                ratio(0.5, 0.5),
                frame(100.0, 200.0, 1_000.0, 800.0),
                size(100.0, 50.0),
                origin(100.0, 200.0),
            ),
            (
                "upper boundary",
                Some(ratio(1.0, 1.0)),
                ratio(0.5, 0.5),
                frame(100.0, 200.0, 1_000.0, 800.0),
                size(100.0, 50.0),
                origin(1_000.0, 950.0),
            ),
            (
                "out of range",
                Some(ratio(-0.2, 1.7)),
                ratio(0.5, 0.5),
                frame(0.0, 0.0, 1_000.0, 800.0),
                size(100.0, 50.0),
                origin(0.0, 750.0),
            ),
            (
                "window larger than work area",
                Some(ratio(0.25, 0.75)),
                ratio(0.5, 0.5),
                frame(100.0, 200.0, 1_000.0, 800.0),
                size(1_200.0, 900.0),
                origin(100.0, 200.0),
            ),
            (
                "missing saved value",
                None,
                ratio(0.95, 0.88),
                frame(0.0, 0.0, 1_000.0, 800.0),
                size(100.0, 50.0),
                origin(855.0, 660.0),
            ),
        ];

        for (name, stored, default_ratio, visible, size, expected) in cases {
            assert_origin_close(
                restored_origin(stored, default_ratio, visible, size),
                expected,
                name,
            );
        }
    }

    #[test]
    fn saved_ratio_round_trips_and_uses_one_point_denominator_floor() {
        let visible = frame(-320.0, 40.0, 1_280.0, 720.0);
        let window = size(180.0, 90.0);
        let expected = ratio(0.37, 0.62);
        let restored = restored_origin(Some(expected), ratio(0.5, 0.85), visible, window);
        assert_ratio_close(
            saved_ratio(restored, visible, window),
            expected,
            "round trip",
        );

        let visible = frame(100.0, 200.0, 1_000.0, 800.0);
        let equal = saved_ratio(origin(100.5, 200.5), visible, size(1_000.0, 800.0));
        let larger = saved_ratio(origin(102.0, 198.0), visible, size(1_200.0, 900.0));
        assert_eq!(equal, ratio(0.5, 0.5));
        assert_eq!(larger, ratio(1.0, 0.0));
        assert!(equal.x.is_finite() && equal.y.is_finite());
    }

    #[test]
    fn document_round_trips_screen_keys_and_rejects_only_broken_entries() {
        let mut document = ScreenPositionDocument::default();
        assert_eq!(document.ratio_for(Some("missing")), None);
        document.save(ratio(0.25, 0.75), "display-a");
        let encoded = document.to_json().expect("position document encodes");
        let decoded =
            ScreenPositionDocument::from_json(&encoded).expect("position document decodes");
        assert_eq!(
            decoded.ratio_for(Some("display-a")),
            Some(ratio(0.25, 0.75))
        );
        assert_eq!(decoded.last_display_key(), Some("display-a"));

        let damaged = ScreenPositionDocument::from_json(
            r#"{"positions":{"good":{"x":0.2,"y":0.4},"broken":{"x":"no","y":0.4}}}"#,
        )
        .expect("a broken entry does not reject the document");
        assert_eq!(damaged.ratio_for(Some("good")), Some(ratio(0.2, 0.4)));
        assert_eq!(damaged.ratio_for(Some("broken")), None);

        let mut namespaced = ScreenPositionDocument::default();
        namespaced.save_with_display_key(ratio(0.4, 0.6), "radial::display-a", "display-a");
        assert_eq!(
            namespaced.ratio_for(Some("radial::display-a")),
            Some(ratio(0.4, 0.6))
        );
        assert_eq!(namespaced.last_display_key(), Some("display-a"));
    }

    fn ratio(x: f64, y: f64) -> PositionRatio {
        PositionRatio { x, y }
    }

    fn frame(x: f64, y: f64, width: f64, height: f64) -> ScreenFrame {
        ScreenFrame {
            x,
            y,
            width,
            height,
        }
    }

    fn size(width: f64, height: f64) -> WindowSize {
        WindowSize { width, height }
    }

    fn origin(x: f64, y: f64) -> WindowOrigin {
        WindowOrigin { x, y }
    }

    fn assert_origin_close(actual: WindowOrigin, expected: WindowOrigin, name: &str) {
        assert!((actual.x - expected.x).abs() < 0.000_001, "{name}: x");
        assert!((actual.y - expected.y).abs() < 0.000_001, "{name}: y");
    }

    fn assert_ratio_close(actual: PositionRatio, expected: PositionRatio, name: &str) {
        assert!((actual.x - expected.x).abs() < 0.000_001, "{name}: x");
        assert!((actual.y - expected.y).abs() < 0.000_001, "{name}: y");
    }
}
