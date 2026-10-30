const ALLOWED_SVG_ELEMENTS: &[&str] = &[
    "svg",
    "g",
    "path",
    "rect",
    "circle",
    "ellipse",
    "line",
    "polyline",
    "polygon",
    "use",
    "defs",
    "clipPath",
    "gradient",
    "linearGradient",
    "radialGradient",
    "image",
];

pub fn validate_svg(label: &str, bytes: &[u8]) -> Result<(), String> {
    let source = std::str::from_utf8(bytes)
        .map_err(|error| format!("{label}: SVG가 UTF-8이 아닙니다: {error}"))?;
    let mut cursor = 0;
    while let Some(relative) = source[cursor..].find('<') {
        let start = cursor + relative;
        let rest = &source[start..];
        if rest.starts_with("<!--") {
            let Some(end) = rest.find("-->") else {
                return Err(format!("{label}: 닫히지 않은 SVG 주석"));
            };
            cursor = start + end + 3;
            continue;
        }
        if rest.starts_with("<?") {
            let Some(end) = rest.find("?>") else {
                return Err(format!("{label}: 닫히지 않은 XML 선언"));
            };
            cursor = start + end + 2;
            continue;
        }
        if rest.starts_with("<!") {
            let Some(end) = rest.find('>') else {
                return Err(format!("{label}: 닫히지 않은 XML 선언부"));
            };
            cursor = start + end + 1;
            continue;
        }

        let mut index = start + 1;
        if source.as_bytes().get(index) == Some(&b'/') {
            index += 1;
        }
        while source
            .as_bytes()
            .get(index)
            .is_some_and(u8::is_ascii_whitespace)
        {
            index += 1;
        }
        let name_start = index;
        while source
            .as_bytes()
            .get(index)
            .is_some_and(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b':' | b'-' | b'_'))
        {
            index += 1;
        }
        if name_start == index {
            return Err(format!("{label}: SVG 태그 이름을 읽을 수 없습니다"));
        }
        let qualified = &source[name_start..index];
        let name = qualified.rsplit(':').next().unwrap_or(qualified);
        if !ALLOWED_SVG_ELEMENTS.contains(&name) {
            return Err(format!("{label}: 지원하지 않는 SVG 요소 <{qualified}>"));
        }
        cursor = index;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::validate_svg;

    #[test]
    fn svg_allowed_element_table_accepts_supported_and_rejects_text_and_style() {
        let cases: [(&str, &[u8], bool); 3] = [
            (
                "allowed",
                br#"<svg><defs><linearGradient/></defs><g><path/><rect/><circle/></g></svg>"#,
                true,
            ),
            ("text", br#"<svg><text>not supported</text></svg>"#, false),
            (
                "style",
                br#"<svg><style>.a { fill: red; }</style><path/></svg>"#,
                false,
            ),
        ];
        for (name, svg, allowed) in cases {
            assert_eq!(validate_svg(name, svg).is_ok(), allowed, "{name}");
        }
    }
}
