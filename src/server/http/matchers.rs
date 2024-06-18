use std::{iter::zip, str::FromStr};

use itertools::Itertools;
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
        PathPrefix::from_str(value).map_err(serde::de::Error::custom)
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

use derive_more::Display;

#[derive(Debug, Display)]
pub(crate) enum PathPrefixParseError {
    Empty,
    NoSlashPrefix,
    ConsecutiveSlashes,
}

impl FromStr for PathPrefix {
    type Err = PathPrefixParseError;

    /// PathPrefix and Exact paths must be syntactically valid:
    ///
    /// - Must begin with the / character
    /// - Must not contain consecutive / characters (e.g. /foo///, //).
    fn from_str(string: &str) -> Result<Self, Self::Err> {
        let mut segments: Vec<&str> = string.split('/').collect();

        match segments.first() {
            None => return Err(PathPrefixParseError::Empty),
            Some(first_segment) if !first_segment.is_empty() => {
                return Err(PathPrefixParseError::NoSlashPrefix);
            }
            _ => {}
        }

        for (first, second) in segments.iter().tuples() {
            if (true, true) == (first.is_empty(), second.is_empty()) {
                return Err(PathPrefixParseError::ConsecutiveSlashes);
            }
        }

        let last = segments.last();

        Ok(match last {
            None => Self(vec![]),
            Some(segment) => {
                let is_traling_slash = segment.is_empty();

                if is_traling_slash {
                    segments.pop();
                }

                Self(segments.into_iter().map(|s| s.to_string()).collect())
            }
        })
    }
}

impl PathPrefix {
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
    fn parse_fails_without_first_slash() {
        let prefix = PathPrefix::from_str("abc");

        assert!(prefix.is_err())
    }

    #[test]
    fn parse_fails_on_consecutive_slashes() {
        let prefix = PathPrefix::from_str("/abc//");

        assert!(prefix.is_err());

        let prefix = PathPrefix::from_str("//");

        assert!(prefix.is_err());
    }

    #[test]
    fn prefix_matches() {
        let prefix = PathPrefix::from_str("/abc").unwrap();

        assert!(prefix.matches("/abc"));
        assert!(prefix.matches("/abc/def"));
        assert!(prefix.matches("/abc/def/"));
        assert!(prefix.matches("/abc/def/ghi"));
        assert!(!prefix.matches("/abcdef"));
    }

    #[test]
    fn trailing_slash_in_definition_is_ignored() {
        let prefix = PathPrefix::from_str("/abc/").unwrap();

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
        /// TODO: leading slash validation
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

// TODO: tests
impl PathMatch {
    pub(crate) fn matches(&self, value_to_match: &str) -> bool {
        match self {
            PathMatch::Exact { value } => value_to_match == value,
            PathMatch::Prefix { value } => value.matches(value_to_match),
            PathMatch::Regex { value } => value.is_match(value_to_match),
        }
    }
}

#[cfg(test)]
mod test_matches {
    use super::*;

    #[test]
    fn exact_matcher() {
        let matcher = PathMatch::Exact {
            value: "/exact".to_owned(),
        };

        assert!(matcher.matches("/exact"));
        assert!(!matcher.matches("/exactly"));
        assert!(!matcher.matches("/"));
    }

    #[test]
    fn prefix_matcher() {
        let matcher = PathMatch::Prefix {
            value: PathPrefix::from_str("/prefix").unwrap(),
        };

        assert!(matcher.matches("/prefix"));
        assert!(matcher.matches("/prefix/"));
        assert!(matcher.matches("/prefix/one"));
        assert!(matcher.matches("/prefix/one/two"));
        assert!(matcher.matches("/prefix/one/three"));
        assert!(!matcher.matches("/not-prefix/one/three"));
    }

    #[test]
    fn regex_matcher() {
        let matcher = PathMatch::Regex {
            value: Regex::from_str("/prefix/[0-9]+$").unwrap(),
        };

        assert!(!matcher.matches("/prefix"));
        assert!(!matcher.matches("/prefix"));

        assert!(matcher.matches("/prefix/1"));
        assert!(matcher.matches("/prefix/123"));
        assert!(matcher.matches("/prefix/123213809124091289490"));

        assert!(!matcher.matches("prefix/1"));
        assert!(!matcher.matches("/prefix/a"));
        assert!(!matcher.matches("/prefix/123a"));
        assert!(!matcher.matches("/prefix/123foo"));
        assert!(!matcher.matches("/prefix/123/foo"));

        let matcher = PathMatch::Regex {
            value: Regex::from_str("/prefix/[0-9A-Za-z-_]+/foo$").unwrap(),
        };

        assert!(matcher.matches("/prefix/123foobarbaz/foo"));
        assert!(matcher.matches("/prefix/123-foo-bar_baz/foo"));
        assert!(!matcher.matches("/prefix/123-foo-bar_baz/foobar"));

        // NOTE: I'm not sure if we should support this as it might be
        // not expected behavior from consumers and kind of makes them add
        // ^ prefix before each and every regex path that they use.
        // This is going to have to be consulted with consumers.
        assert!(matcher.matches("another/prefix/123-foo-bar_baz/foo"));
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
    // If a query param is repeated in an HTTP request, the behavior is purposely left undefined,
    // since different data planes have different capabilities. However, it is recommended that implementations
    // should match against the first value of the param if the data plane supports it, as this behavior is expected
    // in other load balancing contexts outside of the Gateway API.
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
