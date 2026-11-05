//! Real TeX math rendering shared by preview and export.
//!
//! MiTeX translates LaTeX math into Typst's math language, and Typst performs
//! the actual layout. This is intentionally separate from the terminal
//! concealment fallback: Unicode substitutions cannot lay out fractions,
//! matrices, stretchy delimiters, or multi-level scripts correctly.

use anyhow::{anyhow, Result};
use typst_as_lib::{typst_kit_options::TypstKitFontOptions, TypstEngine};

const TYPST_MATH_PRELUDE: &str = r#"
#let matrix = math.mat
#let pmatrix = math.mat.with(delim: "(")
#let bmatrix = math.mat.with(delim: "[")
#let Bmatrix = math.mat.with(delim: "{")
#let vmatrix = math.mat.with(delim: "|")
#let Vmatrix = math.mat.with(delim: "||")
"#;

/// Render one LaTeX math expression as a self-contained SVG fragment.
///
/// The SVG is suitable for embedding in HTML and for terminal image
/// protocols. The caller owns caching because preview and export have
/// different cache lifetimes and sizing requirements.
pub fn render_svg(latex: &str) -> Result<String> {
    let typst_math = mitex::convert_math(latex, None)
        .map_err(|error| anyhow!("LaTeX conversion failed: {error}"))?;
    let typst_math = typst_math.replace("diff", "partial");
    let source = format!("{TYPST_MATH_PRELUDE}\n#set page(width: auto, height: auto, margin: 0pt)\n$ {typst_math} $");
    let engine = TypstEngine::builder()
        .search_fonts_with(TypstKitFontOptions::new().include_system_fonts(false))
        .main_file(source)
        .build();
    let compiled = engine.compile::<typst_layout::PagedDocument>();
    let document = compiled
        .output
        .map_err(|errors| anyhow!("Typst math compilation failed: {errors:?}"))?;
    let page = document
        .pages()
        .first()
        .ok_or_else(|| anyhow!("Typst produced no math page"))?;
    Ok(typst_svg::svg(page, &typst_svg::SvgOptions::default()))
}

/// Render one expression to PNG for terminals supporting Kitty/iTerm images.
pub fn render_png(latex: &str, pixel_per_pt: f32, fg_color: Option<&str>) -> Result<Vec<u8>> {
    let clean = latex.replace(r"\navla", r"\nabla");
    let typst_math = mitex::convert_math(&clean, None)
        .map_err(|error| anyhow!("LaTeX conversion failed: {error}"))?;
    let typst_math = typst_math.replace("diff", "partial");
    let color = fg_color.unwrap_or("#e0af68");
    let source = format!(
        "{TYPST_MATH_PRELUDE}\n#set page(width: auto, height: auto, margin: (x: 4pt, y: 3pt), fill: none)\n#set text(fill: rgb(\"{color}\"), size: 16pt)\n$ {typst_math} $"
    );
    let engine = TypstEngine::builder()
        .search_fonts_with(TypstKitFontOptions::new().include_system_fonts(false))
        .main_file(source)
        .build();
    let compiled = engine.compile::<typst_layout::PagedDocument>();
    let document = compiled
        .output
        .map_err(|errors| anyhow!("Typst math compilation failed: {errors:?}"))?;
    let page = document
        .pages()
        .first()
        .ok_or_else(|| anyhow!("Typst produced no math page"))?;
    let options = typst_render::RenderOptions {
        pixel_per_pt: typst_utils::Scalar::new(pixel_per_pt as f64),
        ..Default::default()
    };
    typst_render::render(page, &options)
        .encode_png()
        .map_err(|error| anyhow!("PNG encoding failed: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_real_fraction_and_scripts() {
        let svg = render_svg(r"\frac{x^2}{y^2}").unwrap();
        assert!(svg.contains("<svg"));
        assert!(svg.contains("<path") || svg.contains("<text"));
    }

    #[test]
    fn reports_invalid_math() {
        assert!(render_svg(r"\frac{").is_err());
    }

    #[test]
    fn renders_curl_matrix_png() {
        let latex = r"\nabla \times \vec{F} = \begin{vmatrix} \hat{i} & \hat{j} & \hat{k} \\ \partial_x & \partial_y & \partial_z \\ F_x & F_y & F_z \end{vmatrix}";
        let typst_math = mitex::convert_math(latex, None).unwrap();
        eprintln!("MITEX OUTPUT: {}", typst_math);
        let res = render_png(latex, 2.0, None);
        if let Err(e) = &res {
            eprintln!("CURL ERROR: {:?}", e);
        }
        assert!(res.is_ok());
    }
}
