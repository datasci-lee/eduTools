use edutools_core::ink::{
    effective_width, InkCommand, InkGeometry, InkLayerState, InkPoint, InkStroke, InkStrokeContent,
    InkStrokeTool, PathCommand, Point, Size,
};
use edutools_core::laser::{LaserPoint, LaserTrail};

const EPSILON: f64 = 1e-9;

fn point(x: f64, y: f64) -> Point {
    Point { x, y }
}

fn ink_point(x: f64, y: f64, pressure: f64, timestamp: f64) -> InkPoint {
    InkPoint::new(point(x, y), pressure, timestamp)
}

fn stroke(
    identifier: impl Into<String>,
    tool: InkStrokeTool,
    width: f64,
    uses_pressure: bool,
    points: Vec<InkPoint>,
) -> InkStroke {
    InkStroke::new(
        identifier,
        tool,
        [0.2, 0.4, 0.8, 1.0],
        width,
        uses_pressure,
        points,
        InkStrokeContent::Freehand,
    )
}

fn approximately_equal(first: f64, second: f64) -> bool {
    (first - second).abs() <= EPSILON
}

#[test]
fn normalized_and_screen_coordinates_round_trip() {
    let screen_size = Size {
        width: 1_920.0,
        height: 1_080.0,
    };
    let screen_point = point(384.0, 810.0);
    let normalized = InkGeometry::normalized_point(screen_point, screen_size);
    let round_tripped = InkGeometry::screen_point(normalized, screen_size);

    assert!((round_tripped.x - screen_point.x).hypot(round_tripped.y - screen_point.y) < 0.000_001);
}

#[test]
fn catmull_rom_passes_through_source_points() {
    let source = vec![
        ink_point(0.1, 0.2, 1.0, 0.0),
        ink_point(0.35, 0.8, 1.0, 1.0),
        ink_point(0.7, 0.3, 1.0, 2.0),
        ink_point(0.9, 0.6, 1.0, 3.0),
    ];
    let smoothed = InkGeometry::smoothed_points(&source);

    assert!(source.iter().all(|source_point| {
        smoothed.iter().any(|candidate| {
            approximately_equal(candidate.location.x, source_point.location.x)
                && approximately_equal(candidate.location.y, source_point.location.y)
        })
    }));
}

#[test]
fn shift_snap_rules_match_pen_highlighter_and_square() {
    let start = point(100.0, 100.0);
    let proposals = [
        point(220.0, 112.0),
        point(205.0, 198.0),
        point(112.0, 220.0),
    ];
    let expected_angles = [0.0, std::f64::consts::PI / 4.0, std::f64::consts::PI / 2.0];

    for (proposal, expected_angle) in proposals.into_iter().zip(expected_angles) {
        let endpoint = InkGeometry::snapped_pen_endpoint(start, proposal);
        let actual_angle = (endpoint.y - start.y).atan2(endpoint.x - start.x);
        assert!(approximately_equal(actual_angle, expected_angle));
    }

    assert_eq!(
        InkGeometry::snapped_highlighter_endpoint(point(90.0, 140.0), point(260.0, 310.0)),
        point(260.0, 140.0)
    );
    assert_eq!(
        InkGeometry::snapped_square_endpoint(point(100.0, 100.0), point(160.0, 220.0)),
        point(220.0, 220.0)
    );
}

#[test]
fn pressure_minimum_width_ratio_adjusts_effective_width() {
    assert!(approximately_equal(
        effective_width(10.0, 0.0, 0.2, true),
        2.0
    ));
    assert!(approximately_equal(
        effective_width(10.0, 1.0, 0.2, true),
        10.0
    ));
    assert!(approximately_equal(
        effective_width(10.0, 0.5, 0.2, true),
        6.0
    ));
    assert!(approximately_equal(
        effective_width(10.0, 0.5, 1.0, true),
        10.0
    ));
    assert!(approximately_equal(
        effective_width(10.0, 0.0, 0.2, false),
        10.0
    ));
}

#[test]
fn laser_trail_path_uses_curves_and_handles_short_inputs() {
    let points = [
        point(10.0, 20.0),
        point(35.0, 80.0),
        point(90.0, 45.0),
        point(140.0, 110.0),
    ];
    let curved = InkGeometry::laser_trail_path(&points);
    assert_eq!(curved.first(), Some(&PathCommand::MoveTo(points[0])));
    assert_eq!(curved.last(), Some(&PathCommand::LineTo(points[3])));
    assert!(curved
        .iter()
        .any(|command| matches!(command, PathCommand::QuadTo { .. })));

    assert_eq!(
        InkGeometry::laser_trail_path(&points[..2]),
        vec![
            PathCommand::MoveTo(points[0]),
            PathCommand::LineTo(points[1])
        ]
    );
    assert!(InkGeometry::laser_trail_path(&[]).is_empty());
    assert!(InkGeometry::laser_trail_path(&points[..1]).is_empty());
}

#[test]
fn one_hundred_one_strokes_permit_one_hundred_undos() {
    let mut layer = InkLayerState::new();
    for index in 0..101 {
        layer.execute(InkCommand::AddStroke(stroke(
            index.to_string(),
            InkStrokeTool::Pen,
            3.0,
            false,
            vec![ink_point(index as f64 / 101.0, 0.5, 1.0, index as f64)],
        )));
    }

    assert!((0..100).all(|_| layer.undo()));
    assert_eq!(layer.strokes().len(), 1);
    assert!(!layer.undo());
}

#[test]
fn new_stroke_clears_redo_history() {
    let mut layer = InkLayerState::new();
    layer.execute(InkCommand::AddStroke(stroke(
        "red",
        InkStrokeTool::Pen,
        3.0,
        false,
        vec![ink_point(0.5, 0.5, 1.0, 0.0)],
    )));
    assert!(layer.undo());
    layer.execute(InkCommand::AddStroke(stroke(
        "blue",
        InkStrokeTool::Pen,
        3.0,
        false,
        vec![ink_point(0.6, 0.5, 1.0, 1.0)],
    )));

    assert!(!layer.can_redo());
    assert!(!layer.redo());
}

#[test]
fn display_ink_and_history_survive_json_round_trip() {
    let display_id = "archive-display";
    let first = stroke(
        "first",
        InkStrokeTool::Pen,
        3.0,
        true,
        vec![
            ink_point(0.25, 0.75, 0.6, 1.0),
            ink_point(0.5, 0.5, 0.9, 2.0),
        ],
    );
    let mut second = stroke(
        "second",
        InkStrokeTool::Highlighter,
        18.0,
        false,
        vec![ink_point(0.8, 0.2, 1.0, 3.0)],
    );
    second.color = [0.9, 0.7, 0.1, 1.0];
    let mut layer = InkLayerState::new();
    layer.execute(InkCommand::AddDisplayStroke {
        display_id: display_id.to_owned(),
        stroke: first.clone(),
    });
    layer.execute(InkCommand::AddDisplayStroke {
        display_id: display_id.to_owned(),
        stroke: second.clone(),
    });
    assert!(layer.undo());

    let data = serde_json::to_vec(&layer.state_for(display_id)).expect("encode ink state");
    let mut decoded: InkLayerState = serde_json::from_slice(&data).expect("decode ink state");

    assert_eq!(decoded.strokes_for(display_id), &[first.clone()]);
    assert!(decoded.redo());
    assert_eq!(decoded.strokes_for(display_id), &[first.clone(), second]);
    assert!(decoded.undo());
    assert_eq!(decoded.strokes_for(display_id), &[first]);
}

#[test]
fn different_resolution_preserves_normalized_coordinates() {
    let display_id = "resolution-display";
    let mut layer = InkLayerState::new();
    layer.execute(InkCommand::AddDisplayStroke {
        display_id: display_id.to_owned(),
        stroke: stroke(
            "resolution",
            InkStrokeTool::Pen,
            3.0,
            false,
            vec![ink_point(0.25, 0.75, 1.0, 0.0)],
        ),
    });
    let data = serde_json::to_vec(&layer.state_for(display_id)).expect("encode ink state");
    let decoded: InkLayerState = serde_json::from_slice(&data).expect("decode ink state");
    let normalized = decoded.strokes_for(display_id)[0].points[0].location;

    let original = InkGeometry::screen_point(
        normalized,
        Size {
            width: 1_000.0,
            height: 500.0,
        },
    );
    let restored = InkGeometry::screen_point(
        normalized,
        Size {
            width: 2_000.0,
            height: 1_000.0,
        },
    );
    assert_eq!(original, point(250.0, 375.0));
    assert_eq!(restored, point(500.0, 750.0));
    assert_eq!(original.x / 1_000.0, restored.x / 2_000.0);
    assert_eq!(original.y / 500.0, restored.y / 1_000.0);
}

#[test]
fn laser_retract_keeps_one_expired_boundary_point() {
    let mut trail = LaserTrail::default();
    for time in [0.0, 1.0, 2.0, 3.0] {
        trail.push(LaserPoint {
            location: point(time, 0.0),
            time,
        });
    }

    trail.retract(3.5, 2.0);
    assert_eq!(
        trail
            .trail
            .iter()
            .map(|point| point.time)
            .collect::<Vec<_>>(),
        vec![1.0, 2.0, 3.0]
    );

    trail.retract(10.0, 2.0);
    assert!(trail.trail.is_empty());
}
