use std::{borrow::Cow};

use consume_api::api::ParamValue;

pub(crate) struct JsonFilterKey<'a>(pub &'a str);

impl<'a> From<JsonFilterKey<'a>> for Cow<'a, str> {
    fn from(k: JsonFilterKey<'a>) -> Self {
        format!("filter[{}]", k.0).into()
    }
}

pub(crate) struct PageKey<'a>(pub &'a str);

impl<'a> From<PageKey<'a>> for Cow<'a, str> {
    fn from(k: PageKey<'a>) -> Self {
        format!("page[{}]", k.0).into()
    }
}

pub(crate) struct SortValue<'a>{
    pub key: &'a str,
    pub asc: bool,
}

impl<'a> ParamValue<'a> for SortValue<'a> {
    fn as_value(&self) -> Cow<'a, str> {
        match self.asc {
            true => self.key.into(),
            false => format!("-{}", self.key).into(),
        }
    }
}

pub(crate) struct SortValues<'a>(pub Vec<SortValue<'a>>);

impl<'a> ParamValue<'a> for SortValues<'a> {
    fn as_value(&self) -> Cow<'a, str> {
        self.0.iter().map(|f| f.as_value()).collect::<Vec<Cow<'a, str>>>().join(",").into()
    }
}
