use iced::{
    Element, Length,
    widget::{button, column, container, row, text, text_input, toggler},
};

// ── Generic probe message ──────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum ProbeMsg<Child = ()> {
    SetString { index: usize, value: String },
    SetBool { index: usize, value: bool },
    SelectVariant { index: usize },
    Child(Child),
}

// ── Field metadata ─────────────────────────────────────────────────

#[derive(Debug, Clone, Copy)]
pub enum FieldKind {
    String,
    Multiline,
    Bool,
    EnumVariant,
}

#[derive(Debug, Clone)]
pub struct FieldInfo {
    pub name: &'static str,
    pub kind: FieldKind,
}

#[derive(Debug, Clone)]
pub enum FieldValue {
    String(String),
    Bool(bool),
}

// ── The Probe trait ────────────────────────────────────────────────

pub trait Probe: Sized {
    type ChildMsg: Clone;

    fn describe(&self) -> Vec<FieldInfo>;
    fn field_value(&self, index: usize) -> FieldValue;
    fn apply(&mut self, msg: ProbeMsg<Self::ChildMsg>);

    fn variants(&self) -> Vec<&'static str> {
        vec![]
    }
    fn current_variant_index(&self) -> usize {
        0
    }
}

// ── Generic rendering ──────────────────────────────────────────────

pub fn probe_view<T: Probe>(value: &T) -> Element<'_, ProbeMsg<T::ChildMsg>> {
    let variants = value.variants();
    let has_variants = !variants.is_empty();

    if has_variants {
        let current = value.current_variant_index();
        let tabs: Vec<_> = variants
            .iter()
            .enumerate()
            .map(|(i, label)| {
                if i == current {
                    button(text(*label).size(14)).into()
                } else {
                    button(text(*label).size(14))
                        .on_press(ProbeMsg::SelectVariant { index: i })
                        .into()
                }
            })
            .collect();

        let fields = value.describe();
        let field_widgets: Vec<Element<'_, ProbeMsg<T::ChildMsg>>> = fields
            .iter()
            .enumerate()
            .map(|(i, field)| render_field(field, i, value))
            .collect();

        return column![row(tabs).spacing(4), column(field_widgets).spacing(4)].into();
    }

    let fields = value.describe();
    let widgets: Vec<Element<'_, ProbeMsg<T::ChildMsg>>> = fields
        .iter()
        .enumerate()
        .map(|(i, field)| render_field(field, i, value))
        .collect();
    column(widgets).spacing(4).into()
}

fn render_field<'a, T: Probe>(
    field: &FieldInfo,
    index: usize,
    value: &'a T,
) -> Element<'a, ProbeMsg<T::ChildMsg>> {
    let label = text(format!("{}:", field.name)).width(100);
    let control: Element<'_, ProbeMsg<T::ChildMsg>> = match field.kind {
        FieldKind::String => {
            let current = match value.field_value(index) {
                FieldValue::String(s) => s,
                _ => String::new(),
            };
            text_input("", &current)
                .on_input(move |s| ProbeMsg::SetString { index, value: s })
                .into()
        }
        FieldKind::Multiline => {
            let current = match value.field_value(index) {
                FieldValue::String(s) => s,
                _ => String::new(),
            };
            container(
                text_input("", &current)
                    .on_input(move |s| ProbeMsg::SetString { index, value: s })
                    .width(Length::Fill),
            )
            .padding(4)
            .style(|_theme: &iced::Theme| container::Style {
                border: iced::Border {
                    color: iced::Color::from_rgb(0.3, 0.3, 0.35),
                    width: 1.0,
                    radius: 4.0.into(),
                },
                ..Default::default()
            })
            .height(Length::Fixed(64.0))
            .into()
        }
        FieldKind::Bool => {
            let current = match value.field_value(index) {
                FieldValue::Bool(b) => b,
                _ => false,
            };
            toggler(current)
                .on_toggle(move |b| ProbeMsg::SetBool { index, value: b })
                .into()
        }
        FieldKind::EnumVariant => {
            text("").into()
        }
    };
    row![label, control].spacing(8).into()
}

pub fn probe_update<T: Probe>(value: &mut T, msg: ProbeMsg<T::ChildMsg>) {
    value.apply(msg);
}
