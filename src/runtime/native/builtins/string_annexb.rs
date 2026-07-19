#[cfg(not(feature = "std"))]
use crate::prelude::*;

use crate::{
    error::{Error, Result},
    runtime::{
        Context,
        call::RuntimeCallArgs,
        native::{NativeFunctionKind, function::StringAnnexBFunctionKind},
    },
    value::{ObjectId, Value},
};

const ATTRIBUTE_COLOR: &str = "color";
const ATTRIBUTE_HREF: &str = "href";
const ATTRIBUTE_NAME: &str = "name";
const ATTRIBUTE_SIZE: &str = "size";
const ESCAPED_QUOTE: &str = "&quot;";
const TAG_A: &str = "a";
const TAG_B: &str = "b";
const TAG_BIG: &str = "big";
const TAG_BLINK: &str = "blink";
const TAG_FONT: &str = "font";
const TAG_I: &str = "i";
const TAG_SMALL: &str = "small";
const TAG_STRIKE: &str = "strike";
const TAG_SUB: &str = "sub";
const TAG_SUP: &str = "sup";
const TAG_TT: &str = "tt";

const STRING_ANNEX_B_METHODS: &[StringAnnexBFunctionKind] = &[
    StringAnnexBFunctionKind::Anchor,
    StringAnnexBFunctionKind::Big,
    StringAnnexBFunctionKind::Blink,
    StringAnnexBFunctionKind::Bold,
    StringAnnexBFunctionKind::Fixed,
    StringAnnexBFunctionKind::FontColor,
    StringAnnexBFunctionKind::FontSize,
    StringAnnexBFunctionKind::Italics,
    StringAnnexBFunctionKind::Link,
    StringAnnexBFunctionKind::Small,
    StringAnnexBFunctionKind::Strike,
    StringAnnexBFunctionKind::Sub,
    StringAnnexBFunctionKind::Substr,
    StringAnnexBFunctionKind::Sup,
];

#[derive(Clone, Copy)]
struct HtmlWrapper {
    tag: &'static str,
    attribute: Option<&'static str>,
}

impl Context {
    pub(in crate::runtime::native) fn install_string_annex_b_prototype_methods(
        &mut self,
        prototype: ObjectId,
    ) -> Result<()> {
        for method in STRING_ANNEX_B_METHODS {
            let kind = NativeFunctionKind::StringPrototypeAnnexB(*method);
            let function = self.create_native_function(kind, Value::Undefined)?;
            self.define_non_enumerable_object_property(prototype, method.name(), function)?;
        }
        Ok(())
    }

    pub(in crate::runtime::native) fn eval_string_prototype_annex_b(
        &mut self,
        kind: StringAnnexBFunctionKind,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Result<Value> {
        if kind == StringAnnexBFunctionKind::Substr {
            return self.eval_string_prototype_substr(args.as_slice(), this_value);
        }
        let wrapper = html_wrapper(kind)?;
        self.eval_string_html_wrapper(wrapper, args.as_slice(), this_value)
    }

    fn eval_string_html_wrapper(
        &mut self,
        wrapper: HtmlWrapper,
        args: &[Value],
        this_value: &Value,
    ) -> Result<Value> {
        let text = self.string_receiver_utf16(this_value)?;
        let attribute = match wrapper.attribute {
            Some(_) => {
                let value = args.first().cloned().unwrap_or(Value::Undefined);
                Some(self.string_argument_utf16(&value)?)
            }
            None => None,
        };

        let mut output = Vec::new();
        append_ascii(&mut output, "<");
        append_ascii(&mut output, wrapper.tag);
        if let (Some(name), Some(value)) = (wrapper.attribute, attribute.as_deref()) {
            append_ascii(&mut output, " ");
            append_ascii(&mut output, name);
            append_ascii(&mut output, "=\"");
            append_escaped_attribute(&mut output, value);
            append_ascii(&mut output, "\"");
        }
        append_ascii(&mut output, ">");
        output.extend_from_slice(&text);
        append_ascii(&mut output, "</");
        append_ascii(&mut output, wrapper.tag);
        append_ascii(&mut output, ">");
        self.check_utf16_string_len(&output)?;
        self.heap_utf16_string_value(&output)
    }

    fn eval_string_prototype_substr(
        &mut self,
        args: &[Value],
        this_value: &Value,
    ) -> Result<Value> {
        let text = self.string_receiver_utf16(this_value)?;
        let text_len = text.len();
        let start_value = args.first().cloned().unwrap_or(Value::Undefined);
        let start_integer = self.to_integer_or_infinity(&start_value)?;
        let start = Self::substr_start(start_integer, text_len)?;
        let available = text_len
            .checked_sub(start)
            .ok_or_else(|| Error::runtime("substring start exceeds string length"))?;
        let result_len = match args.get(1) {
            None | Some(Value::Undefined) => available,
            Some(value) => {
                let length = self.to_integer_or_infinity(value)?;
                Self::substr_length(length, available)?
            }
        };
        let end = start
            .checked_add(result_len)
            .ok_or_else(|| Error::limit("substring end overflowed"))?;
        let output = text
            .get(start..end)
            .map(<[u16]>::to_vec)
            .ok_or_else(|| Error::runtime("substring range is outside the string"))?;
        self.heap_utf16_string_value(&output)
    }

    fn substr_start(integer: f64, text_len: usize) -> Result<usize> {
        if integer == f64::NEG_INFINITY {
            return Ok(0);
        }
        if integer == f64::INFINITY {
            return Ok(text_len);
        }
        let text_len_number = Self::usize_to_number(
            text_len,
            "String.prototype.substr length exceeded supported range",
        )?;
        let start = if integer < 0.0 {
            (text_len_number + integer).max(0.0)
        } else {
            integer.min(text_len_number)
        };
        Self::finite_nonnegative_integer_to_usize(
            start,
            "String.prototype.substr start exceeded supported range",
        )
    }

    fn substr_length(integer: f64, available: usize) -> Result<usize> {
        if integer <= 0.0 || integer == f64::NEG_INFINITY {
            return Ok(0);
        }
        if integer == f64::INFINITY {
            return Ok(available);
        }
        let available_number = Self::usize_to_number(
            available,
            "String.prototype.substr length exceeded supported range",
        )?;
        Self::finite_nonnegative_integer_to_usize(
            integer.min(available_number),
            "String.prototype.substr result length exceeded supported range",
        )
    }
}

fn html_wrapper(kind: StringAnnexBFunctionKind) -> Result<HtmlWrapper> {
    let wrapper = match kind {
        StringAnnexBFunctionKind::Anchor => HtmlWrapper {
            tag: TAG_A,
            attribute: Some(ATTRIBUTE_NAME),
        },
        StringAnnexBFunctionKind::Big => HtmlWrapper {
            tag: TAG_BIG,
            attribute: None,
        },
        StringAnnexBFunctionKind::Blink => HtmlWrapper {
            tag: TAG_BLINK,
            attribute: None,
        },
        StringAnnexBFunctionKind::Bold => HtmlWrapper {
            tag: TAG_B,
            attribute: None,
        },
        StringAnnexBFunctionKind::Fixed => HtmlWrapper {
            tag: TAG_TT,
            attribute: None,
        },
        StringAnnexBFunctionKind::FontColor => HtmlWrapper {
            tag: TAG_FONT,
            attribute: Some(ATTRIBUTE_COLOR),
        },
        StringAnnexBFunctionKind::FontSize => HtmlWrapper {
            tag: TAG_FONT,
            attribute: Some(ATTRIBUTE_SIZE),
        },
        StringAnnexBFunctionKind::Italics => HtmlWrapper {
            tag: TAG_I,
            attribute: None,
        },
        StringAnnexBFunctionKind::Link => HtmlWrapper {
            tag: TAG_A,
            attribute: Some(ATTRIBUTE_HREF),
        },
        StringAnnexBFunctionKind::Small => HtmlWrapper {
            tag: TAG_SMALL,
            attribute: None,
        },
        StringAnnexBFunctionKind::Strike => HtmlWrapper {
            tag: TAG_STRIKE,
            attribute: None,
        },
        StringAnnexBFunctionKind::Sub => HtmlWrapper {
            tag: TAG_SUB,
            attribute: None,
        },
        StringAnnexBFunctionKind::Sup => HtmlWrapper {
            tag: TAG_SUP,
            attribute: None,
        },
        StringAnnexBFunctionKind::Substr => {
            return Err(Error::runtime("substr is not an HTML wrapper"));
        }
    };
    Ok(wrapper)
}

fn append_ascii(output: &mut Vec<u16>, text: &str) {
    output.extend(text.bytes().map(u16::from));
}

fn append_escaped_attribute(output: &mut Vec<u16>, value: &[u16]) {
    for unit in value {
        if *unit == u16::from(b'"') {
            append_ascii(output, ESCAPED_QUOTE);
        } else {
            output.push(*unit);
        }
    }
}
