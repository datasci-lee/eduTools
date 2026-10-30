use edutools_core::eraser::{
    EraserAreaShape, EraserEngine, EraserGesture, EraserHitRule, EraserMode, EraserSettings,
    EraserTargetFilter,
};
use edutools_core::ink::{InkPoint, InkStroke, InkStrokeContent, InkStrokeTool, Point, Rect, Size};

fn point(x: f64, y: f64) -> Point {
    Point { x, y }
}

fn stroke(
    identifier: &str,
    tool: InkStrokeTool,
    width: f64,
    locations: &[(f64, f64)],
) -> InkStroke {
    InkStroke::new(
        identifier,
        tool,
        [0.2, 0.4, 0.8, 1.0],
        width,
        false,
        locations
            .iter()
            .enumerate()
            .map(|(index, &(x, y))| InkPoint::new(point(x, y), 1.0, index as f64))
            .collect(),
        InkStrokeContent::Freehand,
    )
}

fn partial_eraser(diameter: f64) -> EraserSettings {
    EraserSettings {
        diameter,
        mode: EraserMode::Partial,
        hit_rule: EraserHitRule::Touching,
        target_filter: EraserTargetFilter::All,
        area_shape: EraserAreaShape::Rectangle,
    }
}

fn area_eraser(hit_rule: EraserHitRule) -> EraserSettings {
    EraserSettings {
        diameter: 24.0,
        mode: EraserMode::Area,
        hit_rule,
        target_filter: EraserTargetFilter::All,
        area_shape: EraserAreaShape::Rectangle,
    }
}

#[test]
fn partial_erase_through_center_leaves_two_fragments() {
    let canvas_size = Size {
        width: 1_000.0,
        height: 100.0,
    };
    let original = stroke("center", InkStrokeTool::Pen, 2.0, &[(0.1, 0.5), (0.9, 0.5)]);

    let result = EraserEngine::erase(
        &[original],
        &EraserGesture::Sweep(vec![point(500.0, 50.0)]),
        &partial_eraser(40.0),
        canvas_size,
        0.2,
    );

    assert_eq!(result.len(), 2);
    assert!(result[0].points.iter().all(|point| point.location.x < 0.5));
    assert!(result[1].points.iter().all(|point| point.location.x > 0.5));
}

#[test]
fn highlighter_only_erase_preserves_pen() {
    let pen = stroke("pen", InkStrokeTool::Pen, 3.0, &[(0.5, 0.5)]);
    let highlighter = stroke(
        "highlighter",
        InkStrokeTool::Highlighter,
        18.0,
        &[(0.5, 0.5)],
    );
    let settings = EraserSettings {
        diameter: 24.0,
        mode: EraserMode::Stroke,
        hit_rule: EraserHitRule::Touching,
        target_filter: EraserTargetFilter::HighlighterOnly,
        area_shape: EraserAreaShape::Rectangle,
    };

    let result = EraserEngine::erase(
        &[pen.clone(), highlighter],
        &EraserGesture::Sweep(vec![point(500.0, 50.0)]),
        &settings,
        Size {
            width: 1_000.0,
            height: 100.0,
        },
        0.2,
    );

    assert_eq!(result, vec![pen]);
}

#[test]
fn area_erase_distinguishes_touching_from_contained() {
    let inside = stroke("inside", InkStrokeTool::Pen, 2.0, &[(0.3, 0.5), (0.4, 0.5)]);
    let crossing = stroke(
        "crossing",
        InkStrokeTool::Pen,
        2.0,
        &[(0.1, 0.5), (0.5, 0.5)],
    );
    let gesture = EraserGesture::Area(Rect {
        x: 20.0,
        y: 20.0,
        width: 30.0,
        height: 60.0,
    });
    let canvas_size = Size {
        width: 100.0,
        height: 100.0,
    };

    let touching = EraserEngine::erase(
        &[inside.clone(), crossing.clone()],
        &gesture,
        &area_eraser(EraserHitRule::Touching),
        canvas_size,
        0.2,
    );
    let contained = EraserEngine::erase(
        &[inside, crossing.clone()],
        &gesture,
        &area_eraser(EraserHitRule::Contained),
        canvas_size,
        0.2,
    );

    assert!(touching.is_empty());
    assert_eq!(contained, vec![crossing]);
}
