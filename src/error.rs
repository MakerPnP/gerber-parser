use gerber_types::{CoordinateFormat, GerberError};
use regex::Regex;
use std::fmt::Formatter;
use std::num::{ParseFloatError, ParseIntError};
use thiserror::Error;

#[non_exhaustive]
#[derive(Error, Debug)]
pub enum ParseError {
    #[error("IO Error parsing Gerber file: {0}")]
    IoError(String),
}

#[non_exhaustive]
#[derive(Error, Debug)]
pub enum ContentError {
    #[error("Document included a line that isn't valid.")]
    UnknownCommand {},
    #[error("Document included a line that isn't supported.")]
    UnsupportedCommand {},
    #[error("Missing M02 statement at end of file")]
    NoEndOfFile,
    #[error("Command was uniquely identified, but did not match regex: {regex}.")]
    NoRegexMatch { regex: Regex },
    #[error(
        "Command was uniquely identified, and matched expected regex, \
    but did not contain the expected capture.\nRegex: {regex}. capture index: {capture_index}"
    )]
    MissingRegexCapture { regex: Regex, capture_index: usize },
    #[error(
        "Command was uniquely identified, and matched expected regex, \
    but did not contain the expected named capture.\nRegex: {regex}. capture name: {capture_name}"
    )]
    MissingRegexNamedCapture { regex: Regex, capture_name: String },
    #[error("After gerber doc was already assigned a name, another name command was found.")]
    TriedToSetImageNameTwice {},
    #[error("After gerber doc was already assigned a unit, another unit command was found.")]
    TriedToSetUnitsTwice {},
    #[error(
        "After gerber doc was already assigned a format specification, \
    another format specification command was found."
    )]
    TriedToFormatTwice {},
    #[error("Set unit command included unrecognized units: {units_str}.")]
    InvalidUnitFormat { units_str: String },
    #[error(
        "Error parsing format spec line. Looking for 2 digits but found 1 or none.\n
    expected something like \'%FSLAX23Y23*%\'."
    )]
    ParseFormatErrorWrongNumDigits {},
    #[error("format spec integer value must be between 1 and 6. Found {digit_found}.")]
    ParseFormatErrorInvalidDigit { digit_found: u8 },
    #[error("Error parsing char as base 10 digit: '{char_found:?}'.")]
    ParseDigitError { char_found: char },
    #[error("Error parsing string as an integer, cause: '{cause}'.")]
    ParseIntegerError { cause: ParseIntError },
    #[error("Error parsing string as a decimal, cause: '{cause}'.")]
    ParseDecimalError { cause: ParseFloatError },
    #[error("Invalid layer parameter. expected format: `L<integer>`. found: '{0}'.")]
    InvalidLayerParameter(String),
    #[error("tried to parse '{aperture_code_str}' as an aperture code (integer) greater than 9 but failed.")]
    ApertureCodeParseFailed { aperture_code_str: String },
    #[error("tried to parse '{aperture_definition_str}' as an aperture definition but failed.")]
    ApertureDefinitionParseFailed { aperture_definition_str: String },
    #[error("tried to parse the definition of aperture '{aperture_code}' but failed.")]
    ParseApertureDefinitionBodyError { aperture_code: i32 },
    #[error("tried to parse the definition of aperture '{aperture_code}' but it already exists.")]
    ApertureDefinedTwice { aperture_code: i32 },
    #[error(
        "tried to parse the definition of aperture, but it uses an unknown type: '{type_str}'."
    )]
    UnknownApertureType { type_str: String },
    #[error(
        "tried to parse coordinate number out of '{coord_num_str}' but failed. \
    This means a coordinate was captured, but could not be parsed as an i64."
    )]
    FailedToParseCoordinate { coord_num_str: String },
    #[error("Operation statement called before format specification.")]
    OperationBeforeFormat {},
    #[error("Unable to parse file attribute (TF).")]
    FileAttributeParseError {},
    #[error("Unsupported Part type '{part_type}' in TF statement.")]
    UnsupportedPartType { part_type: String },
    #[error("Unsupported Polarity type '{polarity_type}' in TF statement.")]
    UnsupportedPolarityType { polarity_type: String },
    #[error(
        "The AttributeName '{attribute_name}' is currently not supported for File Attributes."
    )]
    UnsupportedFileAttribute { attribute_name: String },
    #[error("The File attribute '{file_attribute}' cannot be parsed.")]
    InvalidFileAttribute { file_attribute: String },
    #[error("Invalid parameter. '{parameter}' cannot be parsed.")]
    InvalidParameter { parameter: String },
    #[error("The Aperture attribute '{aperture_attribute}' cannot be parsed or is mis-formed.")]
    InvalidApertureAttribute { aperture_attribute: String },
    #[error("The Aperture attribute '{aperture_attribute}' is not supported.")]
    UnsupportedApertureAttribute { aperture_attribute: String },
    #[error("The Object attribute '{object_attribute}' is not supported.")]
    UnsupportedObjectAttribute { object_attribute: String },
    #[error("Failed to parse delete attribute '{delete_attribute}'.")]
    InvalidDeleteAttribute { delete_attribute: String },
    #[error(
        "tried to parse a number in the drill tolerance aperture attribute, \
    but found {number_str} which could not be parsed as an f64."
    )]
    DrillToleranceParseNumError { number_str: String },
    #[error("IO error occurred: {0}")]
    IoError(String),
    #[error("Macro name is invalid.")]
    InvalidMacroName,
    #[error("Unsupported macro definition.")]
    UnsupportedMacroDefinition,
    #[error("Invalid macro definition. cause: '{0}'")]
    InvalidMacroDefinition(String),
    #[error(
        "Incorrect aperture definition arguments count. code: '{aperture_code}', name: '{aperture_name}"
    )]
    IncorrectDefinitionArgCount {
        aperture_code: i32,
        aperture_name: String,
    },
    #[error("Insufficient arguments.")]
    InsufficientArguments,
    #[error("Invalid date time format: '{0}', expected RFC3339 format: 'YYYY-MM-DDTHH:MM:SSZ'")]
    InvalidDateTime(String),
    #[error("Invalid UUID: '{0}'")]
    InvalidUuid(String),
    #[error("Coordinate format mismatch. expected: '{format:?}', found: '{cause:?}'")]
    CoordinateFormatMismatch {
        format: CoordinateFormat,
        cause: GerberError,
    },
    #[error("No end of line found, expected line to end with '*%'. line: '{line}'")]
    NoEndOfLine { line: String },
    #[error("Coordinate data without an operation code (deprecated modal D01) is only valid after a D01.")]
    CoordinateDataWithoutOperationCode,
}

impl ContentError {
    /// line number is 1-based, for humans.
    pub fn to_with_context(self, line: Option<(usize, String)>) -> GerberParserErrorWithContext {
        GerberParserErrorWithContext { error: self, line }
    }
}

impl PartialEq for ContentError {
    /// Hack to simplify testing. Always returns false.
    fn eq(&self, _: &Self) -> bool {
        false
    }
}

#[derive(Error, Debug, PartialEq)]
pub struct GerberParserErrorWithContext {
    pub error: ContentError,
    /// line number is 1-based, for humans.
    pub line: Option<(usize, String)>,
}

impl std::fmt::Display for GerberParserErrorWithContext {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match &self.line {
            Some((number, content)) => {
                write!(f, "Error: {}\nLine {}: '{}'", self.error, number, content)
            }
            _ => {
                write!(f, "Error at unspecified line: {}", self.error)
            }
        }
    }
}
