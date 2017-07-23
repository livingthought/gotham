//! Defines functionality for operating on `Request` query strings

use std;
use std::error::Error;
use std::collections::HashMap;
use std::str::FromStr;
use std::string::ParseError;
use std::str::ParseBoolError;
use std::num::{ParseIntError, ParseFloatError};

use state::State;
use http::{form_url_decode, FormUrlDecoded};

/// Provides a mapping of keys from `Request` query string to their supplied values
#[derive(Debug)]
pub struct QueryStringMapping {
    data: HashMap<String, Vec<FormUrlDecoded>>,
}

impl QueryStringMapping {
    /// Returns a reference for `Request` query string values mapped to the key.
    pub fn get(&self, key: &str) -> Option<&Vec<FormUrlDecoded>> {
        self.data.get(key)
    }

    /// Determines if `Request` query string values exist for the key.
    pub fn contains_key(&self, key: &str) -> bool {
        self.data.contains_key(key)
    }

    /// Adds an empty value for a key, useful for keys that are considered
    /// optional and haven't been explicitly provided as part of a `Request` query string.
    pub fn add_unmapped_segment(&mut self, key: &str) {
        match form_url_decode(key) {
            Ok(key) => {
                trace!(" unmapped segment {} was added to QueryStringMapping", key);
                self.data.insert(key, Vec::new());
            }
            Err(_) => {
                trace!(" unmapped segment {} was unable to be decoded and will not be added to QueryStringMapping",
                       key)
            }
        };
    }
}

/// Splits a query string into pairs and provides a mapping of keys to values.
///
/// For keys which are represented 1..n times in the query string the resultant Vec will be
/// populated with each value provided.
///
/// For keys that are provided but don't have a value associated an empty String will be stored.
///
/// #Examples
///
/// ```rust
/// # extern crate gotham;
/// #
/// # use gotham::http::query_string::split;
/// #
/// # pub fn main() {
///       let res = split(Some("key=val&key2=val"));
///       assert_eq!("val", res.get("key").unwrap().first().unwrap().val());
///       assert_eq!("val", res.get("key2").unwrap().first().unwrap().val());
///
///       let res = split(Some("k%65y=val&key=%76al+2"));
///       assert_eq!("val", res.get("key").unwrap().first().unwrap().val());
///       assert_eq!("val 2", res.get("key").unwrap().last().unwrap().val());
///
///       let res = split(Some("key=val&key2="));
///       assert_eq!("val", res.get("key").unwrap().first().unwrap().val());
///       assert_eq!("", res.get("key2").unwrap().first().unwrap().val());
/// # }
/// ```
pub fn split<'r>(query: Option<&'r str>) -> QueryStringMapping {
    match query {
        Some(query) => {
            let pairs = query.split("&").filter(|pair| pair.contains("="));
            let data = pairs.fold(HashMap::new(), |mut acc, p| {
                let mut sp = p.split("=");
                let (k, v) = (sp.next().unwrap(), sp.next().unwrap());
                match form_url_decode(k) {
                    Ok(k) => {
                        let vec = acc.entry(k).or_insert(Vec::new());
                        match FormUrlDecoded::new(v) {
                            Some(dv) => vec.push(dv),
                            None => (),
                        }
                    }
                    Err(_) => (),
                };
                acc
            });

            QueryStringMapping { data }
        }
        None => QueryStringMapping { data: HashMap::new() },
    }
}

/// Derived through the macro of the same name supplied by `gotham-derive` for application defined
/// structs that will pass `Request` query string data to custom `Middleware` and `Handler`
/// implementations.
pub trait QueryStringExtractor {
    /// Populates the struct with data from the `Request` query string and adds it to `State`
    fn extract(state: &mut State, query: Option<&str>) -> Result<(), String>;
}

/// A `QueryStringExtractor` that does not extract/store any data.
///
/// Useful in purely static routes and within documentation.
#[derive(Debug)]
pub struct NoopQueryStringExtractor;
impl QueryStringExtractor for NoopQueryStringExtractor {
    fn extract(_state: &mut State, _query: Option<&str>) -> Result<(), String> {
        Ok(())
    }
}

#[derive(Debug)]
/// Represents an error in coverting a key=value pair from a `Request` query string into a
/// type safe value.
///
/// Deliberately kept generic as implementations of FromQueryString cannot be known in advance.
pub struct FromQueryStringError {
    description: String,
}

impl std::fmt::Display for FromQueryStringError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "Error decoding query string: {}", self.description)
    }
}

impl Error for FromQueryStringError {
    fn description(&self) -> &str {
        &self.description
    }
}

/// Converts string data received as part of a `Request` query string to type safe values for
/// usage by `Middleware` and `Handlers`.
pub trait FromQueryString {
    /// Converts a key=value pair from `Request` query string into a type safe value.
    ///
    /// # Panic
    /// If the input data is not of the expected format or size a panic will occur.
    fn from_query_string(&str, &[FormUrlDecoded]) -> Result<Self, FromQueryStringError>
        where Self: Sized;
}

impl<T> FromQueryString for Option<T>
    where T: FromQueryString
{
    fn from_query_string(key: &str,
                         values: &[FormUrlDecoded])
                         -> Result<Self, FromQueryStringError> {
        if values.len() == 0 {
            Ok(None)
        } else {
            match T::from_query_string(key, values) {
                Ok(v) => Ok(Some(v)),
                Err(v) => Err(v),
            }
        }
    }
}

impl<T> FromQueryString for Vec<T>
    where T: FromQueryString
{
    fn from_query_string(key: &str,
                         values: &[FormUrlDecoded])
                         -> Result<Self, FromQueryStringError> {
        values
            .windows(1)
            .map(|value| T::from_query_string(key, value))
            .collect()
    }
}

impl From<ParseIntError> for FromQueryStringError {
    fn from(err: ParseIntError) -> FromQueryStringError {
        FromQueryStringError { description: err.description().to_string() }
    }
}

impl From<ParseFloatError> for FromQueryStringError {
    fn from(err: ParseFloatError) -> FromQueryStringError {
        FromQueryStringError { description: err.description().to_string() }
    }
}

impl From<ParseBoolError> for FromQueryStringError {
    fn from(err: ParseBoolError) -> FromQueryStringError {
        FromQueryStringError { description: err.description().to_string() }
    }
}

impl From<ParseError> for FromQueryStringError {
    fn from(err: ParseError) -> FromQueryStringError {
        FromQueryStringError { description: err.description().to_string() }
    }
}

macro_rules! fstr {
    ($($t:ident),*) => { $(
        impl FromQueryString for $t {
            fn from_query_string(_key: &str, values: &[FormUrlDecoded])
                -> Result<Self, FromQueryStringError> {
                if values.len() == 1 {
                    Ok($t::from_str(values[0].val())?)
                } else {
                    Err(FromQueryStringError {
                            description: String::from("Invalid number of values")
                    })
                }
            }
        }
    )+}
}

fstr!(String,
      bool,
      f32,
      f64,
      isize,
      i8,
      i16,
      i32,
      i64,
      usize,
      u8,
      u16,
      u32,
      u64);