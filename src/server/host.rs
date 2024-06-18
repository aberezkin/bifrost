use std::net::IpAddr;
use std::str::FromStr;

use regex::Regex;
use serde::de::{Deserializer, Visitor};
use serde::{Deserialize, Serialize, Serializer};

use derive_more::Display;

#[derive(Debug, Display)]
#[display(fmt = "{} {:?}", wildcard, labels)]
pub(crate) struct HostSpec {
    /// This list is reversed as it's easier to start matching from the end of the list.
    labels: Vec<String>,
    wildcard: bool,
}

#[derive(Debug, PartialEq, Display)]
pub(crate) enum HostSpecParseError {
    EmptyStr,
    EmptyLabel,
    InvalidLabel,
    InvalidWildcard,
    UnexpectedIp,
}

impl FromStr for HostSpec {
    type Err = HostSpecParseError;

    /// Hostname is the fully qualified domain name of a network host.
    /// This matches the RFC 1123 definition of a hostname with 2 notable exceptions:
    ///
    /// 1. IPs are not allowed.
    ///
    /// 2. A hostname may be prefixed with a wildcard label (*.). The wildcard label must appear by itself as the first label.
    ///
    /// Hostname can be “precise” which is a domain name without the terminating dot of a network host (e.g. “foo.example.com”)
    /// or “wildcard”, which is a domain name prefixed with a single wildcard label (e.g. *.example.com).
    ///
    /// Note that as per RFC1035 and RFC1123, a label must consist of lower case alphanumeric characters or ‘-’,
    /// and must start and end with an alphanumeric character. No other punctuation is allowed.
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let is_ip = IpAddr::from_str(value).is_ok();

        if is_ip {
            return Err(HostSpecParseError::UnexpectedIp);
        }

        let host_label_regex: Regex = Regex::new(r"^[a-z0-9]([a-z0-9-]*[a-z0-9])?$").unwrap();

        if value.is_empty() {
            return Err(HostSpecParseError::EmptyStr);
        }

        let mut labels = vec![];
        let mut wildcard = false;

        for label in value.split('.').rev() {
            if label.is_empty() {
                return Err(HostSpecParseError::EmptyLabel);
            }

            // If we still iterate after we found a wildcard, it's an invalid hostname
            if wildcard {
                return Err(HostSpecParseError::InvalidWildcard);
            }

            if label == "*" {
                wildcard = true;
            } else {
                if !host_label_regex.is_match(label) {
                    return Err(HostSpecParseError::InvalidLabel);
                }

                labels.push(label.to_string());
            }
        }

        Ok(Self { labels, wildcard })
    }
}

impl HostSpec {
    pub(crate) fn matches(&self, hostname: &Hostname) -> bool {
        let wildcard_addition = if self.wildcard { 1 } else { 0 };

        if self.labels.len() + wildcard_addition != hostname.labels.len() {
            return false;
        }

        for (label, hostname_label) in self.labels.iter().zip(hostname.labels.iter()) {
            if label != hostname_label {
                return false;
            }
        }

        if self.wildcard {
            return hostname.labels.len() > self.labels.len();
        }

        true
    }

    fn stringify(&self) -> String {
        let mut string = String::new();

        if self.wildcard {
            string.push('*');
        }

        for label in self.labels.iter().rev() {
            string.push('.');
            string.push_str(label);
        }

        string
    }
}

struct HostVisitor;

impl<'de> Visitor<'de> for HostVisitor {
    type Value = HostSpec;

    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        formatter.write_str("a valid RFC 1123 hostname")
    }

    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        HostSpec::from_str(value).map_err(serde::de::Error::custom)
    }
}

impl<'de> Deserialize<'de> for HostSpec {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_string(HostVisitor)
    }
}

impl Serialize for HostSpec {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.stringify())
    }
}

#[derive(Debug, PartialEq)]
pub(crate) enum HostnameParseError {
    EmptyStr,
    EmptyLabel,
    InvalidLabel,
    UnexpectedWildcard,
    UnexpectedIp,
}

impl From<HostSpecParseError> for HostnameParseError {
    fn from(error: HostSpecParseError) -> Self {
        match error {
            HostSpecParseError::EmptyStr => HostnameParseError::EmptyStr,
            HostSpecParseError::EmptyLabel => HostnameParseError::EmptyLabel,
            HostSpecParseError::InvalidLabel => HostnameParseError::InvalidLabel,
            HostSpecParseError::InvalidWildcard => HostnameParseError::UnexpectedWildcard,
            HostSpecParseError::UnexpectedIp => HostnameParseError::UnexpectedIp,
        }
    }
}

#[derive(Debug)]
pub(crate) struct Hostname {
    labels: Vec<String>,
}

impl FromStr for Hostname {
    type Err = HostnameParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let spec = HostSpec::from_str(s)?;

        if spec.wildcard {
            Err(HostnameParseError::UnexpectedWildcard)
        } else {
            Ok(Self {
                labels: spec.labels,
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn host_spec_empty_str() {
        let result = HostSpec::from_str("");

        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), HostSpecParseError::EmptyStr);
    }

    #[test]
    fn host_spec_empty_label() {
        let result = HostSpec::from_str(".com");

        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), HostSpecParseError::EmptyLabel);
    }

    #[test]
    fn host_spec_empty_label_in_the_middle() {
        let result = HostSpec::from_str("test..com");

        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), HostSpecParseError::EmptyLabel);
    }

    #[test]
    fn host_spec_invalid_label_unsopported_chars() {
        let result = HostSpec::from_str("invalid_domain.com");

        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), HostSpecParseError::InvalidLabel);
    }

    #[test]
    fn host_spec_invalid_label_hypens() {
        let result = HostSpec::from_str("-invalid.com");

        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), HostSpecParseError::InvalidLabel);

        let result = HostSpec::from_str("invalid-.com");

        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), HostSpecParseError::InvalidLabel);
    }

    #[test]
    fn host_spec_unexpected_ipv4() {
        let result = HostSpec::from_str("12.12.12.12");

        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), HostSpecParseError::UnexpectedIp);
    }

    #[test]
    fn host_spec_unexpected_ipv6() {
        let result = HostSpec::from_str("2001:db8::8a2e:370:7334");

        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), HostSpecParseError::UnexpectedIp);
    }

    #[test]
    fn valid_precise_hostname() {
        let result = HostSpec::from_str("test.com");

        assert!(result.is_ok());
        assert_eq!(result.unwrap().labels, vec!["com", "test"]);

        let result = HostSpec::from_str("subdomain.test.com");

        assert!(result.is_ok());
        assert_eq!(result.unwrap().labels, vec!["com", "test", "subdomain"]);

        let result = HostSpec::from_str("many.subdomains.test.com");

        assert!(result.is_ok());
        assert_eq!(
            result.unwrap().labels,
            vec!["com", "test", "subdomains", "many"]
        );
    }

    #[test]
    fn invalid_wildcard_hostname_in_middle() {
        let result = HostSpec::from_str("test.*.com");

        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), HostSpecParseError::InvalidWildcard);
    }

    #[test]
    fn invalid_wildcard_hostname_multiple_wildcards() {
        let result = HostSpec::from_str("*.*.com");

        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), HostSpecParseError::InvalidWildcard);
    }

    #[test]
    fn valid_wildcard_hostname() {
        let result = HostSpec::from_str("*.test.com");

        assert!(result.is_ok());
        assert_eq!(result.unwrap().labels, vec!["com", "test"]);

        let result = HostSpec::from_str("*.subdomain.test.com");

        assert!(result.is_ok());
        assert_eq!(result.unwrap().labels, vec!["com", "test", "subdomain"]);

        let result = HostSpec::from_str("*.many.subdomains.test.com");

        assert!(result.is_ok());
        assert_eq!(
            result.unwrap().labels,
            vec!["com", "test", "subdomains", "many"]
        );
    }

    #[test]
    fn unexpected_wildcard_hostname() {
        let result = Hostname::from_str("*.com");

        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), HostnameParseError::UnexpectedWildcard);

        let result = Hostname::from_str("*.*.com");

        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), HostnameParseError::UnexpectedWildcard);
    }

    #[test]
    fn host_spec_match_exact() {
        let host_spec = HostSpec::from_str("test.com").unwrap();
        let hostname = Hostname::from_str("test.com").unwrap();

        assert!(host_spec.matches(&hostname))
    }

    #[test]
    fn host_spec_match_subdomains() {
        let host_spec = HostSpec::from_str("sub.test.com").unwrap();
        let hostname = Hostname::from_str("sub.test.com").unwrap();

        assert!(host_spec.matches(&hostname))
    }

    #[test]
    fn host_spec_match_wildcard() {
        let host_spec = HostSpec::from_str("*.test.com").unwrap();
        let hostname = Hostname::from_str("other-sub.test.com").unwrap();

        assert!(host_spec.matches(&hostname))
    }

    #[test]
    fn host_spec_missmatch() {
        let host_spec = HostSpec::from_str("test.com").unwrap();
        let hostname = Hostname::from_str("not-test.com").unwrap();

        assert!(!host_spec.matches(&hostname))
    }

    #[test]
    fn host_spec_missmatch_subdomain() {
        let host_spec = HostSpec::from_str("sub.test.com").unwrap();
        let hostname = Hostname::from_str("not-sub.test.com").unwrap();

        assert!(!host_spec.matches(&hostname))
    }

    #[test]
    fn host_spec_missmatch_wildcard() {
        let host_spec = HostSpec::from_str("*.test.com").unwrap();
        let hostname = Hostname::from_str("sub2.sub1.test.com").unwrap();

        assert!(!host_spec.matches(&hostname))
    }
}
