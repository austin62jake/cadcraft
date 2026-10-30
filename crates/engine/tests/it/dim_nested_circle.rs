//! DIMRADIUS, DIMDIAMETER and DIMJOGGED take a circle or arc inside a block reference, in nested
//! references too, as AutoCAD does (#1967). A pick on a block's line, or on a circle the reference
//! scales unevenly (drawn as an ellipse), is not a circle or arc.

use cadcraft_engine::Session;
use cadcraft_engine::doc::{DimKind, Dimension, EntityKind};
use cadcraft_engine::geom::Vec2;
use serde_json::json;

/// A drawing with block "P": a circle of radius 5 and an arc of radius 3 from 0° to 90° at its
/// origin, and a line from (10, 0) to (20, 0).
fn session() -> Session {
    let mut s = Session::new();
    s.execute("circle", &json!({"center": [0, 0], "radius": 5})).unwrap();
    s.execute("arc", &json!({"center": [0, 0], "radius": 3, "start": 0, "end": 90})).unwrap();
    s.execute("line", &json!({"points": [[10, 0], [20, 0]]})).unwrap();
    s.execute("selectall", &json!({})).unwrap();
    s.execute("block", &json!({"name": "P", "base": [0, 0], "keep": "delete"})).unwrap();
    s
}

fn dims(s: &Session) -> Vec<Dimension> {
    s.doc().unwrap().model.iter().filter_map(|e| if let EntityKind::Dimension(d) = &e.kind { Some(d.clone()) } else { None }).collect()
}

/// `p` written for a typed point.
fn pt(p: Vec2) -> String {
    format!("{},{}", p.x, p.y)
}

#[test]
fn radius_and_diameter_of_a_circle_or_arc_in_a_rotated_scaled_reference() {
    let mut s = session();
    s.execute("insert", &json!({"name": "P", "at": [40, 10], "rotation": 30, "scale": 2})).unwrap();
    let c = Vec2::new(40.0, 10.0);

    // The circle: radius 10 in the drawing, picked away from the arc and the line.
    let on_circle = c + Vec2::from_angle(200f64.to_radians()) * 10.0;
    s.script(&format!("dimradius {} {}\n", pt(on_circle), pt(c + Vec2::new(-15.0, -8.0)))).unwrap();
    assert!(s.running.is_none());
    let d = dims(&s);
    assert_eq!(d.len(), 1, "a radius dimension of the nested circle");
    assert!(matches!(d[0].kind, DimKind::Radius));
    assert!(d[0].defpt.xy().dist(c) < 1e-9, "centred on the circle: {:?}", d[0].defpt);
    assert!((d[0].p15.xy().dist(c) - 10.0).abs() < 1e-9, "radius {}", d[0].p15.xy().dist(c));

    // The arc: the block's 0°–90° become 30°–120°, radius 6; its diameter is 12.
    let on_arc = c + Vec2::from_angle(75f64.to_radians()) * 6.0;
    s.script(&format!("dimdiameter {} {}\n", pt(on_arc), pt(c + Vec2::new(2.0, 20.0)))).unwrap();
    let d = dims(&s);
    assert_eq!(d.len(), 2, "a diameter dimension of the nested arc");
    assert!(matches!(d[1].kind, DimKind::Diameter));
    assert!((d[1].p15.xy().dist(d[1].defpt.xy()) - 12.0).abs() < 1e-9, "diameter {}", d[1].p15.xy().dist(d[1].defpt.xy()));

    // DIMJOGGED takes it too: a jogged radius dimension of the nested circle.
    s.script(&format!(
        "dimjogged {} {} {} {}\n",
        pt(on_circle),
        pt(c + Vec2::new(-30.0, -20.0)),
        pt(c + Vec2::new(-12.0, -9.0)),
        pt(c + Vec2::new(-20.0, -12.0))
    ))
    .unwrap();
    let d = dims(&s);
    assert_eq!(d.len(), 3, "a jogged dimension of the nested circle");
    assert!(d[2].marks.jogged.is_some());
    assert!((d[2].p15.xy().dist(c) - 10.0).abs() < 1e-9);
}

#[test]
fn a_circle_two_references_deep_is_taken() {
    let mut s = session();
    // Block Q holds P at (100, 0), half size; Q is inserted at (0, -50).
    let p = s.execute("insert", &json!({"name": "P", "at": [100, 0], "scale": 0.5})).unwrap()["handle"].as_str().unwrap().to_string();
    s.execute("block", &json!({"name": "Q", "base": [0, 0], "handles": [p], "keep": "delete"})).unwrap();
    s.execute("insert", &json!({"name": "Q", "at": [0, -50]})).unwrap();
    s.script("dimradius 100,-52.5 110,-60\n").unwrap();
    let d = dims(&s);
    assert_eq!(d.len(), 1);
    assert!(d[0].defpt.xy().dist(Vec2::new(100.0, -50.0)) < 1e-9, "{:?}", d[0].defpt);
    assert!((d[0].p15.xy().dist(Vec2::new(100.0, -50.0)) - 2.5).abs() < 1e-9);
}

#[test]
fn a_blocks_line_or_an_unevenly_scaled_circle_is_not_a_circle() {
    let mut s = session();
    // Twice as wide as tall: the circle is drawn as an ellipse.
    s.execute("insert", &json!({"name": "P", "at": [0, 100], "scale": [2, 1]})).unwrap();
    s.script("dimradius 10,100\n").unwrap();
    assert!(s.log.iter().any(|l| l == "Object selected is not a circle or arc."), "{:?}", s.log);
    s.cancel();
    // The block's line, in an evenly scaled reference.
    s.execute("insert", &json!({"name": "P", "at": [0, 200]})).unwrap();
    s.script("dimdiameter 15,200\n").unwrap();
    assert!(s.log.iter().filter(|l| *l == "Object selected is not a circle or arc.").count() == 2, "{:?}", s.log);
    s.cancel();
    assert!(dims(&s).is_empty(), "no dimension made");
}
