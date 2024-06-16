use std::{iter::zip, str::FromStr};

use regex::Regex;
use serde::{de::Visitor, Deserialize, Deserializer, Serialize, Serializer};

use hyper::{body::Incoming, Request};

struct PrefixVisitor;

/// Basically a type removing a trailing slash
#[derive(Debug)]
pub(crate) struct PathPrefix(Vec<String>);

impl<'de> Visitor<'de> for PrefixVisitor {
    type Value = PathPrefix;

    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        formatter.write_str("a valid path")
    }

    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        Ok(PathPrefix::parse(value))
    }
}

impl<'de> Deserialize<'de> for PathPrefix {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_string(PrefixVisitor)
    }
}

impl Serialize for PathPrefix {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.0.join("/"))
    }
}

impl PathPrefix {
    // TODO: stricter checking according to the gateway api spec
    //
    // PathPrefix and Exact paths must be syntactically valid:
    //
    // - Must begin with the / character
    // - Must not contain consecutive / characters (e.g. /foo///, //).
    //
    // TODO: impl as FromStr
    fn parse(string: &str) -> Self {
        let mut segments: Vec<&str> = string.split('/').collect();

        let last = segments.last();

        match last {
            None => Self(vec![]),
            Some(segment) => {
                let is_traling_slash = segment.is_empty();

                if is_traling_slash {
                    segments.pop();
                }

                Self(segments.into_iter().map(|s| s.to_string()).collect())
            }
        }
    }

    /// Match a string aganst a prefix
    fn matches(&self, value_to_match: &str) -> bool {
        let segments: Vec<&str> = value_to_match.split('/').collect();
        let prefix = &self.0;

        if segments.len() < prefix.len() {
            return false;
        }

        for (prefix_segment, value_segment) in zip(prefix, segments) {
            if value_segment != prefix_segment {
                return false;
            }
        }

        true
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn prefix_matches() {
        let prefix = PathPrefix::parse("/abc");

        assert!(prefix.matches("/abc"));
        assert!(prefix.matches("/abc/def"));
        assert!(prefix.matches("/abc/def/"));
        assert!(prefix.matches("/abc/def/ghi"));
        assert!(!prefix.matches("/abcdef"));
    }

    #[test]
    fn trailing_slash_in_definition_is_ignored() {
        let prefix = PathPrefix::parse("/abc/");

        assert!(prefix.matches("/abc"));
        assert!(prefix.matches("/abc/def"));
        assert!(prefix.matches("/abc/def/"));
        assert!(prefix.matches("/abc/def/ghi"));
        assert!(!prefix.matches("/abcdef"));
    }
}

#[derive(Deserialize, Serialize, Debug)]
#[serde(tag = "type")]
pub(crate) enum PathMatch {
    Exact {
        value: String,
    },
    Prefix {
        value: PathPrefix,
    },
    Regex {
        #[serde(with = "serde_regex")]
        value: Regex,
    },
}

// TODO: tests and matchers module
impl PathMatch {
    pub(crate) fn matches(&self, value_to_match: &str) -> bool {
        match self {
            PathMatch::Exact { value } => value_to_match == value,
            // TODO: proper prefix matching Prefix:/abc should match /abc/def but not /abcdef
            PathMatch::Prefix { value } => value.matches(value_to_match),
            PathMatch::Regex { value } => value.is_match(value_to_match),
        }
    }
}

use http::{HeaderMap, HeaderValue, Method};

#[derive(Debug)]
pub(crate) struct MethodMatch(Method);

impl MethodMatch {
    fn parse(s: &str) -> Result<Self, http::method::InvalidMethod> {
        Ok(Self(Method::from_str(s)?))
    }

    fn stringify(&self) -> String {
        self.0.to_string()
    }

    fn matches(&self, req_method: &Method) -> bool {
        self.0 == req_method
    }
}

struct MethodMatchVisitor;

impl<'de> Visitor<'de> for MethodMatchVisitor {
    type Value = MethodMatch;

    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        formatter.write_str("a valid HTTP method")
    }

    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        MethodMatch::parse(value).map_err(serde::de::Error::custom)
    }
}

impl<'de> Deserialize<'de> for MethodMatch {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_string(MethodMatchVisitor)
    }
}

impl Serialize for MethodMatch {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.stringify())
    }
}

#[derive(Deserialize, Serialize, Debug)]
#[serde(tag = "type")]
pub(crate) enum HeaderMatch {
    Exact {
        value: String,
        name: String,
    },
    Regex {
        #[serde(with = "serde_regex")]
        value: Regex,
        name: String,
    },
}

impl HeaderMatch {
    // TODO: fix unwraps
    fn matches(&self, header_map: &HeaderMap<HeaderValue>) -> bool {
        match &self {
            Self::Exact { name, value } => header_map.get(name).map_or(false, |header_value| {
                header_value.to_str().unwrap() == value
            }),
            Self::Regex { name, value } => header_map.get(name).map_or(false, |header_value| {
                value.is_match(header_value.to_str().unwrap())
            }),
        }
    }
}

#[derive(Deserialize, Serialize, Debug)]
pub(crate) struct Matcher {
    // NOTE: All fields here should be matched using AND
    pub(crate) path: Option<PathMatch>,
    pub(crate) method: Option<MethodMatch>,
    // TODO:
    // If multiple entries specify equivalent header names, only the first entry with an equivalent
    // name MUST be considered for a match. Subsequent entries with an equivalent header name MUST be ignored.
    // Due to the case-insensitivity of header names, “foo” and “Foo” are considered equivalent.
    // Might be better to use a hashmap
    pub(crate) headers: Option<Vec<HeaderMatch>>,
    // TODO: query
    // If multiple entries specify equivalent query param names, only the first entry with an equivalent name MUST be considered for a match.
    // Subsequent entries with an equivalent query param name MUST be ignored.
    // If a query param is repeated in an HTTP request, the behavior is purposely left undefined, since different data planes have different capabilities. However, it is recommended that implementations should match against the first value of the param if the data plane supports it, as this behavior is expected in other load balancing contexts outside of the Gateway API.
}

impl Matcher {
    pub(crate) fn matches(&self, req: &Request<Incoming>) -> bool {
        let path_match = self
            .path
            .as_ref()
            .map_or(true, |path| path.matches(req.uri().path()));

        let method_match = self
            .method
            .as_ref()
            .map_or(true, |method| method.matches(req.method()));

        let headers_match = self.headers.as_ref().map_or(true, |headers| {
            headers
                .iter()
                .all(|headers_match| headers_match.matches(req.headers()))
        });

        path_match && method_match && headers_match
    }
}
