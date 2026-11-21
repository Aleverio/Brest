#![allow(clippy::expect_used, clippy::panic)]

use figcraft_import::{ImportContext, import, parse_color};
use figcraft_ir::{Align, Asset, Axis, Color, Kind, WarningCode, WidgetKind};
use serde_json::json;

const FIXTURE: &str = include_str!("../../../fixtures/figcraft-page.json");

#[test]
fn normalises_realistic_page() {
    let mut context = ImportContext {
        page_name: Some("Halaman Contoh".into()),
        ..ImportContext::default()
    };
    context.assets.push(Asset {
        id: 1,
        name: "avatar-hash".into(),
        mime: "image/png".into(),
        bytes: vec![0x89, b'P', b'N', b'G'],
    });

    let design = import(FIXTURE, &context).expect("fixture should import");

    assert_eq!(design.version, figcraft_ir::IR_VERSION);
    assert_eq!(design.name, "Halaman Contoh");
    assert_eq!(design.roots.len(), 2);

    let card = &design.roots[0];
    // Absolute (100,50) relative to a page root stays (100,50).
    assert_eq!((card.rect.origin.x, card.rect.origin.y), (100.0, 50.0));
    assert_eq!((card.rect.size.width, card.rect.size.height), (320.0, 180.0));
    match &card.kind {
        Kind::Frame(spec) => {
            assert_eq!(spec.layout, Axis::Vertical);
            assert_eq!(spec.gap, 12.0);
            assert_eq!(spec.padding.left, 20.0);
        }
        other => panic!("expected frame, got {other:?}"),
    }

    // Children are parent-relative: (120,70) - (100,50) = (20,20).
    let title = &card.children[0];
    assert_eq!((title.rect.origin.x, title.rect.origin.y), (20.0, 20.0));
    match &title.kind {
        Kind::Text(spec) => {
            assert_eq!(spec.text, "Halo {{name}}");
            assert_eq!(spec.bindings, vec!["name"]);
            assert_eq!(spec.align, Align::Start);
            assert!(spec.bold);
            assert_eq!(spec.color, Color::new(0x14, 0x18, 0x1E, 255));
        }
        other => panic!("expected text, got {other:?}"),
    }

    match &card.children[1].kind {
        Kind::Widget(spec) => assert_eq!(spec.kind, WidgetKind::Button),
        other => panic!("expected button widget, got {other:?}"),
    }

    // The gradient is not supported: first solid fallback is used, warning recorded.
    let accent = &card.children[2];
    assert!(design
        .warnings
        .iter()
        .any(|w| w.code == WarningCode::UnsupportedPaint && w.node == Some(accent.id)));

    // Image is resolved from host-supplied bytes, not a raw path.
    match &design.roots[1].kind {
        Kind::Image { asset } => assert_eq!(*asset, 1),
        other => panic!("expected image, got {other:?}"),
    }

    assert_eq!(design.bindings(), vec!["name".to_string()]);
    assert!(design.validate().is_empty());
}

#[test]
fn rejects_invalid_json() {
    let error = import("{", &ImportContext::default()).expect_err("invalid JSON must fail");
    assert!(error.to_string().contains("invalid FigCraft JSON"));
}

#[test]
fn limits_are_enforced() {
    let raw = r#"{"id":"1:1","type":"FRAME","absoluteBounds":{"x":0,"y":0,"width":10,"height":10}}"#;
    let context = ImportContext {
        limits: figcraft_import::ImportLimits {
            max_nodes: 1,
            max_depth: 0,
            max_asset_bytes: 0,
        },
        ..ImportContext::default()
    };
    // Root itself is node 1 and depth 0; this should still succeed.
    assert!(import(raw, &context).is_ok());
}

#[test]
fn parses_colour_aliases() {
    assert_eq!(parse_color(&json!("#fff")), Some(Color::WHITE));
    assert_eq!(parse_color(&json!("#11223380")), Some(Color::new(0x11, 0x22, 0x33, 0x80)));
    assert_eq!(
        parse_color(&json!({"r":1,"g":0,"b":0,"a":1})),
        Some(Color::new(255, 0, 0, 255))
    );
    assert_eq!(parse_color(&json!([255, 128, 0])), Some(Color::new(255, 128, 0, 255)));
}
