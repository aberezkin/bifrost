use regex::Regex;
use serde::{Deserialize, Serialize};

use hyper::{body::Incoming, Request};

#[derive(Deserialize, Serialize, Debug)]
#[serde(tag = "type")]
pub(crate) enum StringMatch {
    Exact {
        value: String,
    },
    Prefix {
        value: String,
    },
    // TODO: regex support
    Regex {
        #[serde(with = "serde_regex")]
        value: Regex,
    },
}

// TODO: tests and matchers module
impl StringMatch {
    pub(crate) fn matches(&self, value_to_match: &str) -> bool {
        match self {
            StringMatch::Exact { value } => value_to_match == value,
            // TODO: proper prefix matching Prefix:/abc should match /abc/def but not /abcdef
            StringMatch::Prefix { value } => value_to_match.starts_with(value),
            StringMatch::Regex { value } => value.is_match(value_to_match),
        }
    }
}

#[derive(Deserialize, Serialize, Debug)]
pub(crate) struct Matcher {
    // NOTE: All fields here should be matched using AND
    pub(crate) path: StringMatch,
    // TODO: method, headers, query
}

impl Matcher {
    pub(crate) fn matches(&self, req: &Request<Incoming>) -> bool {
        // TODO: method, headers, query
        self.path.matches(req.uri().path())
    }
}
