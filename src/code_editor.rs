use egui::text::LayoutJob;
use egui::{vec2, Color32, FontId, Response, TextFormat};

pub fn code_editor(ui: &mut egui::Ui, code: &mut String, wrap_width: f32) -> Response {
    let mut layouter = |ui: &egui::Ui, string: &str, _wrap_width: f32| {
        let mut layout_job = highlight(ui.ctx(), string);
        layout_job.wrap.max_width = wrap_width; // no wrapping
        ui.fonts().layout_job(layout_job)
    };

    ui.add(
        egui::widgets::TextEdit::multiline(code)
            .lock_focus(true)
            .margin(vec2(0., 10.))
            .desired_rows(16)
            .desired_width(f32::INFINITY)
            .text_color(Color32::BLACK)
            .font(FontId::monospace(20.0))
            .layouter(&mut layouter)
            .frame(false),
    )
}

/// Memoized Code highlighting
pub fn highlight(ctx: &egui::Context, code: &str) -> LayoutJob {
    let theme = &CodeTheme::light();

    impl egui::util::cache::ComputerMut<(&CodeTheme, &str), LayoutJob> for Highlighter {
        fn compute(&mut self, (theme, code): (&CodeTheme, &str)) -> LayoutJob {
            self.highlight(theme, code)
        }
    }

    type HighlightCache = egui::util::cache::FrameCache<LayoutJob, Highlighter>;

    ctx.memory()
        .caches
        .cache::<HighlightCache>()
        .get((theme, code))
}

// ----------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, enum_map::Enum)]
enum TokenType {
    Comment,
    Keyword,
    Literal,
    StringLiteral,
    Punctuation,
    Whitespace,
}

#[derive(Clone, Hash, PartialEq)]
pub struct CodeTheme {
    formats: enum_map::EnumMap<TokenType, egui::TextFormat>,
}

impl Default for CodeTheme {
    fn default() -> Self {
        Self::light()
    }
}

impl CodeTheme {
    pub fn light() -> Self {
        let medium = FontId::monospace(20.0);
        let regular = FontId::new(
            20.0,
            egui::FontFamily::Name("Fira Code Regular".into()).into(),
        );
        let bold = FontId::new(20.0, egui::FontFamily::Name("Fira Code Bold".into()).into());

        Self {
            formats: enum_map::enum_map![
                TokenType::Comment => TextFormat::simple(medium.clone(), Color32::GRAY),
                TokenType::Keyword => TextFormat::simple(bold.clone(), Color32::from_rgb(0, 0, 0)),
                TokenType::Literal => TextFormat::simple(medium.clone(), Color32::from_rgb(40, 40, 40)),
                TokenType::StringLiteral => TextFormat::simple(regular.clone(), Color32::from_rgb(180, 180, 180)),
                TokenType::Punctuation => TextFormat::simple(medium.clone(), Color32::DARK_GRAY),
                TokenType::Whitespace => TextFormat::simple(medium.clone(), Color32::TRANSPARENT),
            ],
        }
    }
}

#[derive(Default)]
struct Highlighter {}

impl Highlighter {
    #[allow(clippy::unused_self, clippy::unnecessary_wraps)]
    fn highlight(&self, theme: &CodeTheme, mut text: &str) -> LayoutJob {
        // Extremely simple syntax highlighter for when we compile without syntect

        let mut job = LayoutJob::default();

        while !text.is_empty() {
            if text.starts_with("//") {
                let end = text.find('\n').unwrap_or(text.len());
                job.append(&text[..end], 0.0, theme.formats[TokenType::Comment].clone());
                text = &text[end..];
            } else if text.starts_with('"') {
                let end = text[1..]
                    .find('"')
                    .map(|i| i + 2)
                    .or_else(|| text.find('\n'))
                    .unwrap_or(text.len());
                job.append(
                    &text[..end],
                    0.0,
                    theme.formats[TokenType::StringLiteral].clone(),
                );
                text = &text[end..];
            } else if text.starts_with(|c: char| c.is_ascii_alphanumeric()) {
                let end = text[1..]
                    .find(|c: char| !c.is_ascii_alphanumeric())
                    .map_or_else(|| text.len(), |i| i + 1);
                let word = &text[..end];
                let tt = if is_keyword(word) {
                    TokenType::Keyword
                } else {
                    TokenType::Literal
                };
                job.append(word, 0.0, theme.formats[tt].clone());
                text = &text[end..];
            } else if text.starts_with(|c: char| c.is_ascii_whitespace()) {
                let end = text[1..]
                    .find(|c: char| !c.is_ascii_whitespace())
                    .map_or_else(|| text.len(), |i| i + 1);
                job.append(
                    &text[..end],
                    0.0,
                    theme.formats[TokenType::Whitespace].clone(),
                );
                text = &text[end..];
            } else {
                let mut it = text.char_indices();
                it.next();
                let end = it.next().map_or(text.len(), |(idx, _chr)| idx);
                job.append(
                    &text[..end],
                    0.0,
                    theme.formats[TokenType::Punctuation].clone(),
                );
                text = &text[end..];
            }
        }

        job
    }
}

fn is_keyword(word: &str) -> bool {
    matches!(
        word,
        "as" | "async"
            | "await"
            | "break"
            | "const"
            | "continue"
            | "crate"
            | "dyn"
            | "else"
            | "enum"
            | "extern"
            | "false"
            | "fn"
            | "for"
            | "if"
            | "impl"
            | "in"
            | "let"
            | "loop"
            | "match"
            | "mod"
            | "move"
            | "mut"
            | "pub"
            | "ref"
            | "return"
            | "self"
            | "Self"
            | "static"
            | "struct"
            | "super"
            | "trait"
            | "true"
            | "type"
            | "unsafe"
            | "use"
            | "where"
            | "while"
    )
}
