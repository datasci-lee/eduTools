use edutools_core::zoom::{Point, Rect, Size, ZoomFollowMode, ZoomGeometry};

const TOLERANCE: f64 = 1e-9;

// Swift: ZoomTests.magnificationStepsAndClamps()
#[test]
fn magnification_steps_and_clamps() {
    assert_eq!(ZoomGeometry::adjusted_magnification(2.0, 1), 2.25);
    assert_eq!(ZoomGeometry::adjusted_magnification(1.25, -1), 1.25);
    assert_eq!(ZoomGeometry::adjusted_magnification(8.0, 1), 8.0);
}

// Swift: ZoomTests.edgeFollowingMovesOnlyBeyondInnerEdge()
#[test]
fn edge_following_moves_only_beyond_inner_edge() {
    let screen_size = Size::new(1_000.0, 800.0);
    let center = Point::new(500.0, 400.0);
    let center_pointer_result = ZoomGeometry::followed_center(
        center,
        Point::new(500.0, 400.0),
        screen_size,
        2.0,
        ZoomFollowMode::Edge,
        1.0,
    );
    let edge_pointer_result = ZoomGeometry::followed_center(
        center,
        Point::new(740.0, 400.0),
        screen_size,
        2.0,
        ZoomFollowMode::Edge,
        1.0,
    );

    assert_eq!(center_pointer_result, center);
    assert_eq!(edge_pointer_result, Point::new(540.0, 400.0));
}

// Swift: ZoomTests.centerClampKeepsViewportOnScreen()
#[test]
fn center_clamp_keeps_viewport_on_screen() {
    assert_eq!(
        ZoomGeometry::clamped_center(Point::new(1.0, 799.0), Size::new(1_000.0, 800.0), 2.0,),
        Point::new(250.0, 600.0)
    );
}

// Swift: ZoomTests.frozenInputMapsBackToOriginalCoordinate()
#[test]
fn frozen_input_maps_back_to_original_coordinate() {
    let screen_size = Size::new(1_000.0, 800.0);
    let original_point = ZoomGeometry::original_point(
        Point::new(250.0, 600.0),
        screen_size,
        Rect::new(250.0, 200.0, 500.0, 400.0),
    );
    let normalized_point = Point::new(
        original_point.x / screen_size.width,
        original_point.y / screen_size.height,
    );

    assert!((normalized_point.x - 0.375).abs() <= TOLERANCE);
    assert!((normalized_point.y - 0.625).abs() <= TOLERANCE);
}
