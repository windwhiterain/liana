use proc_macro::TokenStream;
use quote::{quote, format_ident};
use syn::{parse_macro_input, Data, DeriveInput, Fields, LitStr};

#[proc_macro_derive(Probe, attributes(probe))]
pub fn derive_probe(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = &input.ident;
    let msg_name = format_ident!("{}ProbeMsg", name);

    let probe_attrs = parse_probe_attrs(&input.attrs);
    let _tags_inlined = probe_attrs.tags_inlined;

    match &input.data {
        Data::Struct(data) => derive_struct(name, &msg_name, data),
        Data::Enum(data) => derive_enum(name, &msg_name, data),
        Data::Union(_) => panic!("#[derive(Probe)] does not support unions"),
    }
    .into()
}

fn derive_struct(
    name: &syn::Ident,
    msg_name: &syn::Ident,
    data: &syn::DataStruct,
) -> proc_macro2::TokenStream {
    let fields = match &data.fields {
        Fields::Named(fields) => &fields.named,
        _ => panic!("#[derive(Probe)] only supports named fields on structs"),
    };

    let field_names: Vec<_> = fields.iter().map(|f| f.ident.as_ref().unwrap()).collect();
    let field_kinds: Vec<_> = fields.iter().map(|f| infer_field_kind(f)).collect();
    let field_attrs: Vec<_> = fields.iter().map(|f| parse_field_attrs(&f.attrs)).collect();

    let describe_fields = field_names.iter().enumerate().map(|(i, name)| {
        let name_str = name.to_string();
        let kind = &field_kinds[i];
        let display = field_attrs[i].label.as_ref().unwrap_or(&name_str);
        quote! {
            crate::probe::FieldInfo { name: #display, kind: crate::probe::FieldKind::#kind }
        }
    });

    let field_value_arms = field_names.iter().enumerate().map(|(i, name)| {
        let kind_str = field_kinds[i].to_string();
        if kind_str.contains("Bool") {
            quote! { #i => crate::probe::FieldValue::Bool(self.#name), }
        } else if kind_str.contains("Multiline") {
            quote! { #i => crate::probe::FieldValue::TextContent(&self.#name), }
        } else {
            quote! { #i => crate::probe::FieldValue::String(self.#name.clone()), }
        }
    });

    let apply_arms = field_names.iter().enumerate().map(|(i, name)| {
        let kind_str = field_kinds[i].to_string();
        if kind_str.contains("Bool") {
            quote! {
                crate::probe::ProbeMsg::SetBool { index: #i, value } => { self.#name = value; }
            }
        } else if kind_str.contains("Multiline") {
            quote! {
                crate::probe::ProbeMsg::TextAction { index: #i, action } => {
                    self.#name.perform(action);
                }
            }
        } else {
            quote! {
                crate::probe::ProbeMsg::SetString { index: #i, value } => { self.#name = value; }
            }
        }
    });

    quote! {
        #[derive(Debug, Clone)]
        #[doc(hidden)]
        pub enum #msg_name {}

        impl crate::probe::Probe for #name {
            type ChildMsg = #msg_name;

            fn describe(&self) -> Vec<crate::probe::FieldInfo> {
                vec![#(#describe_fields),*]
            }

            fn field_value<'a>(&'a self, index: usize) -> crate::probe::FieldValue<'a> {
                match index {
                    #(#field_value_arms)*
                    _ => crate::probe::FieldValue::String(String::new()),
                }
            }

            fn apply(&mut self, msg: crate::probe::ProbeMsg<Self::ChildMsg>) {
                match msg {
                    #(#apply_arms)*
                    _ => {}
                }
            }
        }
    }
}

fn derive_enum(
    name: &syn::Ident,
    msg_name: &syn::Ident,
    data: &syn::DataEnum,
) -> proc_macro2::TokenStream {
    let variant_names: Vec<_> = data.variants.iter().map(|v| &v.ident).collect();
    let variant_labels: Vec<_> = variant_names.iter().map(|v| v.to_string()).collect();

    // describe: delegate to inner
    let describe_body = {
        let arms: Vec<_> = data.variants.iter().map(|v| {
            let vname = &v.ident;
            if has_data(v) {
                quote! { Self::#vname(inner) => inner.describe(), }
            } else {
                quote! { Self::#vname => vec![], }
            }
        }).collect();
        quote! { match self { #(#arms)* } }
    };

    // field_value: delegate to inner
    let field_value_body = {
        let arms: Vec<_> = data.variants.iter().map(|v| {
            let vname = &v.ident;
            if has_data(v) {
                quote! { Self::#vname(inner) => inner.field_value(index), }
            } else {
                quote! { Self::#vname => crate::probe::FieldValue::String(String::new()), }
            }
        }).collect();
        quote! { match self { #(#arms)* } }
    };

    // select_arms: change variant
    let select_arms: Vec<_> = variant_names.iter().enumerate().map(|(i, _)| {
        let vname = &variant_names[i];
        let setter = if has_data(&data.variants[i]) {
            quote! { *self = Self::#vname(Default::default()); }
        } else {
            quote! { *self = Self::#vname; }
        };
        quote! { #i => { #setter } }
    }).collect();

    // set_string_arms: delegate to inner probe for variants with data
    let set_string_delegate: Vec<_> = data.variants.iter().map(|v| {
        let vname = &v.ident;
        if has_data(v) {
            quote! {
                Self::#vname(inner) => {
                    inner.apply(crate::probe::ProbeMsg::SetString { index, value });
                }
            }
        } else {
            quote! {}
        }
    }).collect();

    // set_bool_delegate: delegate to inner probe for variants with data
    let set_bool_delegate: Vec<_> = data.variants.iter().map(|v| {
        let vname = &v.ident;
        if has_data(v) {
            quote! {
                Self::#vname(inner) => {
                    inner.apply(crate::probe::ProbeMsg::SetBool { index, value });
                }
            }
        } else {
            quote! {}
        }
    }).collect();

    // text_action_delegate: delegate to inner probe for variants with data
    let text_action_delegate: Vec<_> = data.variants.iter().map(|v| {
        let vname = &v.ident;
        if has_data(v) {
            quote! {
                Self::#vname(inner) => {
                    inner.apply(crate::probe::ProbeMsg::TextAction { index, action });
                }
            }
        } else {
            quote! {}
        }
    }).collect();

    // cv_arms: current_variant_index
    let cv_arms: Vec<_> = variant_names.iter().enumerate().map(|(i, vname)| {
        if has_data(&data.variants[i]) {
            quote! { Self::#vname(_) => #i, }
        } else {
            quote! { Self::#vname => #i, }
        }
    }).collect();

    quote! {
        #[derive(Debug, Clone)]
        #[doc(hidden)]
        pub enum #msg_name {}

        impl crate::probe::Probe for #name {
            type ChildMsg = #msg_name;

            fn describe(&self) -> Vec<crate::probe::FieldInfo> {
                #describe_body
            }

            fn field_value<'a>(&'a self, index: usize) -> crate::probe::FieldValue<'a> {
                #field_value_body
            }

            fn apply(&mut self, msg: crate::probe::ProbeMsg<Self::ChildMsg>) {
                match msg {
                    crate::probe::ProbeMsg::SelectVariant { index } => {
                        match index {
                            #(#select_arms)*
                            _ => {}
                        }
                    }
                    crate::probe::ProbeMsg::SetString { index, value } => {
                        match self {
                            #(#set_string_delegate)*
                            _ => {}
                        }
                    }
                    crate::probe::ProbeMsg::SetBool { index, value } => {
                        match self {
                            #(#set_bool_delegate)*
                            _ => {}
                        }
                    }
                    crate::probe::ProbeMsg::TextAction { index, action } => {
                        match self {
                            #(#text_action_delegate)*
                            _ => {}
                        }
                    }
                    _ => {}
                }
            }

            fn variants(&self) -> Vec<&'static str> {
                vec![#(#variant_labels),*]
            }

            fn current_variant_index(&self) -> usize {
                match self {
                    #(#cv_arms)*
                }
            }
        }
    }
}

fn has_data(variant: &syn::Variant) -> bool {
    match &variant.fields {
        Fields::Named(fields) => !fields.named.is_empty(),
        Fields::Unnamed(fields) => !fields.unnamed.is_empty(),
        Fields::Unit => false,
    }
}

fn parse_probe_attrs(attrs: &[syn::Attribute]) -> ProbeAttrs {
    let mut result = ProbeAttrs::default();
    for attr in attrs {
        if !attr.path().is_ident("probe") {
            continue;
        }
        if let Ok(list) = attr.meta.require_list() {
            list.parse_nested_meta(|meta| {
                if meta.path.is_ident("transparent") {
                    result.transparent = true;
                } else if meta.path.is_ident("tags") {
                    let value: LitStr = meta.value()?.parse()?;
                    result.tags_inlined = value.value() == "inlined";
                }
                Ok(())
            }).ok();
        }
    }
    result
}

#[derive(Default)]
struct ProbeAttrs {
    transparent: bool,
    tags_inlined: bool,
}

fn parse_field_attrs(attrs: &[syn::Attribute]) -> FieldAttrs {
    let mut result = FieldAttrs::default();
    for attr in attrs {
        if !attr.path().is_ident("probe") {
            continue;
        }
        if let Ok(list) = attr.meta.require_list() {
            list.parse_nested_meta(|meta| {
                if meta.path.is_ident("with") {
                    let value: LitStr = meta.value()?.parse()?;
                    result.with = Some(value.value());
                } else if meta.path.is_ident("label") {
                    let value: LitStr = meta.value()?.parse()?;
                    result.label = Some(value.value());
                } else if meta.path.is_ident("kind") {
                    let value: LitStr = meta.value()?.parse()?;
                    result.kind_override = Some(value.value());
                }
                Ok(())
            }).ok();
        }
    }
    result
}

#[derive(Default)]
struct FieldAttrs {
    with: Option<String>,
    label: Option<String>,
    kind_override: Option<String>,
}

fn infer_field_kind(field: &syn::Field) -> proc_macro2::TokenStream {
    let ty = &field.ty;
    let ty_str = quote! { #ty }.to_string();

    if let Some(override_kind) = parse_field_attrs(&field.attrs).kind_override {
        let ident = format_ident!("{}", override_kind);
        return quote! { #ident };
    }
    if ty_str.contains("TextEditor") {
        quote! { Multiline }
    } else if ty_str.contains("String") || ty_str.contains("str") {
        quote! { String }
    } else if ty_str.contains("bool") {
        quote! { Bool }
    } else {
        quote! { String }
    }
}
