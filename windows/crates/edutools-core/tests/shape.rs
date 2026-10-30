use edutools_core::shape::{
    LassoGeometry, Point, Rect, ScreenCoverEngine, SelectionContent, SelectionItem, ShapeKind,
    ShapeRecognizer, ShapeSelection,
};

const TOLERANCE: f64 = 1e-9;

// Swift: RecognitionTests.nearlyStraightStrokeIsRecognized()
#[test]
fn nearly_straight_stroke_is_recognized() {
    let recognized = ShapeRecognizer::recognize(&[
        Point::new(0.0, 0.0),
        Point::new(25.0, 1.0),
        Point::new(50.0, -1.0),
        Point::new(75.0, 1.0),
        Point::new(100.0, 0.0),
    ]);

    assert_eq!(recognized, Some(ShapeKind::Line));
}

// Swift: RecognitionTests.closedFourSidedStrokeIsRectangle()
#[test]
fn closed_four_sided_stroke_is_rectangle() {
    let recognized = ShapeRecognizer::recognize(&[
        Point::new(0.0, 0.0),
        Point::new(50.0, 1.0),
        Point::new(100.0, 0.0),
        Point::new(99.0, 40.0),
        Point::new(100.0, 80.0),
        Point::new(50.0, 79.0),
        Point::new(0.0, 80.0),
        Point::new(1.0, 40.0),
        Point::new(0.0, 0.0),
    ]);

    assert_eq!(recognized, Some(ShapeKind::Rectangle));
}

// Swift: RecognitionTests.closedRoundStrokeIsEllipse()
#[test]
fn closed_round_stroke_is_ellipse() {
    let points = (0..=24)
        .map(|index| {
            let angle = f64::from(index) / 24.0 * 2.0 * std::f64::consts::PI;
            Point::new(50.0 + angle.cos() * 40.0, 50.0 + angle.sin() * 40.0)
        })
        .collect::<Vec<_>>();

    assert_eq!(
        ShapeRecognizer::recognize(&points),
        Some(ShapeKind::Ellipse)
    );
}

// Swift: RecognitionTests.openZigzagIsRejected()
#[test]
fn open_zigzag_is_rejected() {
    let recognized = ShapeRecognizer::recognize(&[
        Point::new(0.0, 0.0),
        Point::new(80.0, 80.0),
        Point::new(10.0, 70.0),
        Point::new(90.0, 10.0),
        Point::new(40.0, 60.0),
    ]);

    assert_eq!(recognized, None);
}

// Swift: SelectionTests.concaveLassoRespectsBoundary()
#[test]
fn concave_lasso_respects_boundary() {
    let lasso = [
        Point::new(0.0, 0.0),
        Point::new(100.0, 0.0),
        Point::new(100.0, 100.0),
        Point::new(50.0, 45.0),
        Point::new(0.0, 100.0),
    ];

    assert!(LassoGeometry::contains(Point::new(20.0, 30.0), &lasso));
    assert!(!LassoGeometry::contains(Point::new(50.0, 70.0), &lasso));
}

// Swift: SelectionTests.movementOffsetsEveryNormalizedCoordinate()
#[test]
fn movement_offsets_every_normalized_coordinate() {
    let selection = ShapeSelection {
        items: vec![selection_item()],
    };
    let moved = selection.moved(Point::new(0.1, -0.05));
    let points = &moved.items[0].points;

    assert_eq!(points.len(), 2);
    assert_point(points[0], Point::new(0.3, 0.15));
    assert_point(points[1], Point::new(0.5, 0.35));
}

// Swift: SelectionTests.overlappingCoverClickRemovesOnlyTopmost()
#[test]
fn overlapping_cover_click_removes_only_topmost() {
    let selected = selection_item();
    let lower_cover = SelectionItem {
        id: 2,
        points: vec![Point::new(0.1, 0.1), Point::new(0.6, 0.6)],
        content: SelectionContent::Cover,
    };
    let upper_cover = SelectionItem {
        id: 3,
        points: vec![Point::new(0.3, 0.3), Point::new(0.8, 0.8)],
        content: SelectionContent::Cover,
    };

    let remaining = ScreenCoverEngine::removing_topmost_cover(
        Point::new(0.4, 0.4),
        &[selected, lower_cover, upper_cover],
    );

    assert_eq!(
        remaining.iter().map(|item| item.id).collect::<Vec<_>>(),
        vec![1, 2]
    );
}

// Swift: SelectionTests.resizingMapsCoordinatesIntoNewBounds()
#[test]
fn resizing_maps_coordinates_into_new_bounds() {
    let selection = ShapeSelection {
        items: vec![selection_item()],
    };
    let resized = selection.resized(Rect::new(0.2, 0.2, 0.2, 0.2), Rect::new(0.1, 0.1, 0.6, 0.4));
    let points = &resized.items[0].points;

    assert_eq!(points.len(), 2);
    assert_point(points[0], Point::new(0.1, 0.1));
    assert_point(points[1], Point::new(0.7, 0.5));
}

// Swift: SelectionTests.selectionStroke()
fn selection_item() -> SelectionItem {
    SelectionItem {
        id: 1,
        points: vec![Point::new(0.2, 0.2), Point::new(0.4, 0.4)],
        content: SelectionContent::Shape(ShapeKind::Rectangle),
    }
}

// Swift: TestSupport.pointsAreApproximatelyEqual(_:_:)
fn assert_point(actual: Point, expected: Point) {
    assert!((actual.x - expected.x).abs() <= TOLERANCE);
    assert!((actual.y - expected.y).abs() <= TOLERANCE);
}
