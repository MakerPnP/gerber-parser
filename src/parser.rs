use crate::document::GerberDoc;
use crate::error::ContentError;
use crate::gerber_types::{
    Aperture, ApertureAttribute, ApertureFunction, ApertureMacro, CenterLinePrimitive, Circle,
    CirclePrimitive, Command, CoordinateFormat, DCode, ExtendedCode, FiducialScope, FileAttribute,
    FileFunction, FilePolarity, FunctionCode, GCode, InterpolationMode, MCode, MacroBoolean,
    MacroContent, MacroDecimal, MacroInteger, Operation, OutlinePrimitive, Part, Polarity, Polygon,
    PolygonPrimitive, QuadrantMode, Rectangular, SmdPadType, StepAndRepeat, Unit,
    VectorLinePrimitive,
};
use crate::util::{partial_coordinates_from_gerber, partial_coordinates_offset_from_gerber};
use crate::ParseError;
use gerber_types::{ApertureBlock, AttributeDeletionCriterion, AxisSelect, CommentContent, ComponentCharacteristics, ComponentDrill, ComponentMounting, ComponentOutline, CoordinateMode, CopperType, DrillFunction, DrillRouteType, ExtendedPosition, GenerationSoftware, GerberDate, IPC4761ViaProtection, Ident, ImageMirroring, ImageName, ImageOffset, ImagePolarity, ImageRotation, ImageScaling, Mirroring, Net, NonPlatedDrill, ObjectAttribute, Pin, PlatedDrill, Position, Profile, Rotation, Scaling, StandardComment, ThermalPrimitive, Uuid, VariableDefinition, ZeroOmission};
use lazy_regex::*;
use regex::Regex;
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Lines, Read};
use std::iter::FromIterator;
use std::str::Chars;
use std::sync::LazyLock;

// naively define some regex terms
// FUTURE investigate which ones can be done without regex for better performance.
// FUTURE using named captures in all regular expressions would be nice, this would enable the use of string
//        constants for the capture names, this would improve the error messages too.
static RE_UNITS: Lazy<Regex> = lazy_regex!(r"%MO(.*)\*%");
static RE_COMMENT: Lazy<Regex> = lazy_regex!(r"G04 ((#@! (?P<standard>.*))|(?P<string>.*))\*");
static RE_AXIS_SELECT: Lazy<Regex> = lazy_regex!(r"%AS(?P<axisselect>AXBY|AYBX)\*%");
static RE_IMAGE_POLARITY: Lazy<Regex> = lazy_regex!(r"%IP(?P<polarity>POS|NEG)\*%");
static RE_IMAGE_ROTATION: Lazy<Regex> = lazy_regex!(r"%IR(?P<rotation>0|90|180|270)\*%");
static RE_IMAGE_MIRRORING: Lazy<Regex> = lazy_regex!(r"%MI(A(?P<a>0|1))?(B(?P<b>0|1))?\*%");
static RE_IMAGE_OFFSET: Lazy<Regex> =
    lazy_regex!(r"%OF(A(?P<a>[+-]?[0-9]+(?:\.[0-9]*)?))?(B(?P<b>[+-]?[0-9]+(?:\.[0-9]*)?))?\*%");
static RE_IMAGE_SCALING: Lazy<Regex> =
    lazy_regex!(r"%SF(A(?P<a>[+-]?[0-9]+(?:\.[0-9]*)?))?(B(?P<b>[+-]?[0-9]+(?:\.[0-9]*)?))?\*%");
static RE_LOAD_MIRRORING: Lazy<Regex> = lazy_regex!(r"%LM(?P<mirroring>N|X|Y|XY)\*%");

// Note: scaling cannot be negative, or 0.
static RE_LOAD_SCALING: Lazy<Regex> = lazy_regex!(r"%LS(?P<value>[0-9]+(?:\.[0-9]*)?)\*%");
static RE_LOAD_ROTATION: Lazy<Regex> = lazy_regex!(r"%LR(?P<value>[+-]?[0-9]+(?:\.[0-9]*)?)\*%");
static RE_FORMAT_SPEC: Lazy<Regex> = lazy_regex!(r"%FS[LT][AI]X(.*)Y(.*)\*%");

/// https://regex101.com/r/YNnrmK/1
static RE_APERTURE: Lazy<Regex> =
    lazy_regex!(r"%ADD([0-9]+)([._$a-zA-Z][._$a-zA-Z0-9]{0,126})(?:,\s?(.*))?\*%");
static RE_APERTURE_BLOCK: Lazy<Regex> = lazy_regex!(r"%AB(D(?P<code>[0-9]+))?\*%");
static RE_INTERPOLATION: Lazy<Regex> =
    lazy_regex!(r"X?(-?[0-9]+)?Y?(-?[0-9]+)?I?(-?[0-9]+)?J?(-?[0-9]+)?D(0)?1\*");
static RE_MOVE_OR_FLASH: Lazy<Regex> = lazy_regex!(r"X?(-?[0-9]+)?Y?(-?[0-9]+)?D(0)?[2-3]*");
static RE_IMAGE_NAME: Lazy<Regex> = lazy_regex!(r"%IN(.*)\*%");
static RE_STEP_REPEAT: Lazy<Regex> =
    lazy_regex!(r"%SRX([0-9]+)Y([0-9]+)I(\d+\.?\d*)J(\d+\.?\d*)\*%");
static RE_MACRO_UNSIGNED_INTEGER: Lazy<Regex> =
    lazy_regex!(r"^(?:(?P<value>[0-9]+)|(?P<variable>\$[0-9]+)|(?P<expression>.*))$");
static RE_MACRO_BOOLEAN: Lazy<Regex> =
    lazy_regex!(r"^(?:(?P<value>0|1$)|(?P<variable>\$[0-9]+)|(?P<expression>.*))$");
static RE_MACRO_DECIMAL: Lazy<Regex> = lazy_regex!(
    r"^(?:(?P<value>[+-]?[0-9]+(?:\.[0-9]*)?)|(?P<variable>\$[0-9]+)|(?P<expression>.+))$"
);
static RE_MACRO_VARIABLE: Lazy<Regex> =
    lazy_regex!(r"\$(?P<number>\d+)\s*=\s*(?P<expression>[^*]+)\s*");

struct ParserContext<T: Read> {
    line_number: usize,
    lines: Lines<BufReader<T>>,
    aperture_attributes: HashMap<String, ApertureAttribute>,
    object_attributes: HashMap<String, ObjectAttribute>,
}

impl<T: Read> ParserContext<T> {
    pub fn new(lines: Lines<BufReader<T>>) -> ParserContext<T> {
        ParserContext {
            line_number: 0,
            lines,
            aperture_attributes: HashMap::new(),
            object_attributes: HashMap::new(),
        }
    }

    pub fn next(&mut self) -> Option<Result<String, ContentError>> {
        let line = self.lines.next();
        if line.is_some() {
            self.line_number += 1;
        }
        line.map(|result| {
            result.map_err(|e| {
                ContentError::IoError(
                    format!("IO error on line: {}, error: {}", self.line_number, e).to_string(),
                )
            })
        })
    }
}

/// Parse a gerber string (in BufReader) to a GerberDoc
///
/// Take the contents of a Gerber (.gbr) file and parse it to a GerberDoc struct. The parsing does some semantic
/// checking, but is certainly not exhaustive - so don't rely on it to check if your Gerber file is valid according to
/// the spec. Some of the parsing steps are greedy - they may match something unexpected (rather than panicking) if
/// there is a typo/fault in your file.
///
/// If a fatal error occurs (like an IO error), then a partial GerberDoc will be returned along
/// with the error.
#[allow(clippy::result_large_err)]
pub fn parse<T: Read>(reader: BufReader<T>) -> Result<GerberDoc, (GerberDoc, ParseError)> {
    let mut gerber_doc = GerberDoc::default();

    let mut parser_context = ParserContext::new(reader.lines());

    let mut parse_error: Option<ParseError> = None;

    loop {
        let Some(line_result) = parser_context.next() else {
            break;
        };

        let line_number = parser_context.line_number;

        let raw_line = match line_result {
            Ok(line) => line,
            Err(ContentError::IoError(error)) => {
                parse_error = Some(ParseError::IoError(error));
                break;
            }
            _ => continue,
        };
        let line = raw_line.trim();

        // Show the line
        log::trace!("Line: {}. Content: {:?}", line_number + 1, &line);

        if !line.is_empty() {
            let line_results = parse_line(line, &mut gerber_doc, &mut parser_context);
            for result in line_results.into_iter().flatten() {
                let final_result = match result {
                    Ok(command) => {
                        log::trace!("Parsed command: {:?}", command);
                        Ok(command)
                    }
                    Err(ContentError::IoError(error)) => {
                        log::error!("IO error: {}", error);
                        parse_error = Some(ParseError::IoError(error));
                        break;
                    }
                    Err(error_without_context) => {
                        let contexted_error = error_without_context
                            .to_with_context(Some((line_number, line.to_string())));
                        log::error!("Content error: {}", contexted_error);
                        Err(contexted_error)
                    }
                };
                gerber_doc.commands.push(final_result);
            }
        }
    }

    match gerber_doc.commands.last() {
        None => gerber_doc
            .commands
            .push(Err(ContentError::NoEndOfFile.to_with_context(None))),
        Some(Ok(Command::FunctionCode(FunctionCode::MCode(MCode::EndOfFile)))) => {}
        _ => gerber_doc
            .commands
            .push(Err(ContentError::NoEndOfFile.to_with_context(None))),
    }

    match parse_error {
        None => Ok(gerber_doc),
        Some(error) => Err((gerber_doc, error)),
    }
}

fn parse_line<T: Read>(
    line: &str,
    gerber_doc: &mut GerberDoc,
    parser_context: &mut ParserContext<T>,
) -> Result<Vec<Result<Command, ContentError>>, ContentError> {
    let mut linechars = line.chars();

    match linechars.next().unwrap() {
        // Safety: already explicitly checked that the line is not empty
        'G' => {
            match linechars.next().ok_or(ContentError::UnknownCommand {})? {
                '0' => {
                    let remaining_line = &line[3..];
                    let using_deprecated_syntax = remaining_line.len() > 1;
                    let mut commands = Vec::with_capacity(1);
                    match linechars.next().ok_or(ContentError::UnknownCommand {})? {
                        '1' => {
                            // G01
                            commands.push(Ok(FunctionCode::GCode(GCode::InterpolationMode(
                                InterpolationMode::Linear,
                            ))
                            .into()));
                            if using_deprecated_syntax {
                                commands.push(parse_interpolate_move_or_flash(
                                    remaining_line,
                                    gerber_doc,
                                    &mut linechars,
                                ));
                            }
                        }
                        '2' => {
                            // G02
                            commands.push(Ok(FunctionCode::GCode(GCode::InterpolationMode(
                                InterpolationMode::ClockwiseCircular,
                            ))
                            .into()));
                            if using_deprecated_syntax {
                                commands.push(parse_interpolate_move_or_flash(
                                    remaining_line,
                                    gerber_doc,
                                    &mut linechars,
                                ));
                            }
                        }
                        '3' => {
                            // G03
                            commands.push(Ok(FunctionCode::GCode(GCode::InterpolationMode(
                                InterpolationMode::CounterclockwiseCircular,
                            ))
                            .into()));
                            if using_deprecated_syntax {
                                commands.push(parse_interpolate_move_or_flash(
                                    remaining_line,
                                    gerber_doc,
                                    &mut linechars,
                                ));
                            }
                        }
                        '4' => {
                            // G04
                            commands.push(parse_comment(line, parser_context))
                        }
                        _ => commands.push(Err(ContentError::UnknownCommand {})),
                    }
                    Ok(commands)
                }
                '3' => Ok(vec![
                    match linechars.next().ok_or(ContentError::UnknownCommand {})? {
                        '6' => Ok(FunctionCode::GCode(GCode::RegionMode(true)).into()), // G36
                        '7' => Ok(FunctionCode::GCode(GCode::RegionMode(false)).into()), // G37
                        _ => Err(ContentError::UnknownCommand {}),
                    },
                ]),
                '7' => Ok(vec![
                    match linechars.next().ok_or(ContentError::UnknownCommand {})? {
                        // the G74 command is technically part of the Deprecated commands
                        '4' => Ok(
                            FunctionCode::GCode(GCode::QuadrantMode(QuadrantMode::Single)).into(),
                        ), // G74
                        '5' => Ok(
                            FunctionCode::GCode(GCode::QuadrantMode(QuadrantMode::Multi)).into(),
                        ), // G75
                        _ => Err(ContentError::UnknownCommand {}),
                    },
                ]),
                _ => Ok(vec![Err(ContentError::UnknownCommand {})]),
            }
        }
        '%' => {
            Ok(vec![
                match linechars.next().ok_or(ContentError::UnknownCommand {})? {
                    'M' => match linechars.next().ok_or(ContentError::UnknownCommand {})? {
                        // MO
                        'O' => match parse_units(line, gerber_doc) {
                            Ok(units) => {
                                gerber_doc.units = Some(units);
                                Ok(Command::ExtendedCode(ExtendedCode::Unit(units)))
                            }
                            Err(e) => Err(e),
                        },
                        // MI
                        'I' => parse_image_mirroring(line),
                        _ => Err(ContentError::UnknownCommand {}),
                    },
                    'F' => match parse_format_spec(line, gerber_doc) {
                        Ok(format_spec) => {
                            gerber_doc.format_specification = Some(format_spec);
                            Ok(ExtendedCode::CoordinateFormat(format_spec).into())
                        }
                        Err(e) => Err(e),
                    },
                    'A' => match linechars.next().ok_or(ContentError::UnknownCommand {})? {
                        // AB
                        'B' => match parse_aperture_block(line) {
                            Ok(maybe_code) => {
                                let aperture_block = match maybe_code {
                                    None => ApertureBlock::Close,
                                    Some(code) => ApertureBlock::Open { code },
                                };
                                Ok(ExtendedCode::ApertureBlock(aperture_block).into())
                            }
                            Err(err) => Err(err),
                        },
                        // AD
                        'D' => {
                            match parse_aperture_defs(line, gerber_doc) {
                                Ok((code, ap)) => {
                                    gerber_doc.apertures.insert(code, ap.clone());
                                    // Safety: While insert can 'fail' (misbehave) if the key
                                    // already exists,
                                    // `parse_aperture_defs` explicitly checks for this
                                    Ok(ExtendedCode::ApertureDefinition(
                                        gerber_types::ApertureDefinition::new(code, ap),
                                    )
                                    .into())
                                }
                                Err(err) => Err(err),
                            }
                        }
                        // AM
                        'M' => match parse_aperture_macro_definition(line, parser_context) {
                            Ok(macro_def) => Ok(Command::ExtendedCode(
                                ExtendedCode::ApertureMacro(macro_def),
                            )),
                            Err(err) => Err(err),
                        },
                        // AS
                        'S' => parse_axis_select(line),
                        _ => Err(ContentError::UnknownCommand {}),
                    },
                    'L' => match linechars.next().ok_or(ContentError::UnknownCommand {})? {
                        // LP
                        'P' => match linechars.next().ok_or(ContentError::UnknownCommand {})? {
                            // LPD
                            'D' => Ok(ExtendedCode::LoadPolarity(Polarity::Dark).into()),
                            // LPC
                            'C' => Ok(ExtendedCode::LoadPolarity(Polarity::Clear).into()),
                            _ => Err(ContentError::UnknownCommand {}),
                        },
                        // LM
                        'M' => parse_load_mirroring(line),
                        // LR
                        'R' => parse_load_rotation(line),
                        // LS
                        'S' => parse_load_scaling(line),
                        _ => Err(ContentError::UnknownCommand {}),
                    },
                    'T' => match linechars.next().ok_or(ContentError::UnknownCommand {})? {
                        'F' => {
                            linechars = trim_attr_line(linechars)?;

                            parse_file_attribute(linechars.clone())
                                .map(|attr| ExtendedCode::FileAttribute(attr).into())
                        }
                        'A' => {
                            linechars = trim_attr_line(linechars)?;

                            parse_aperture_attribute(linechars.clone())
                                .map(|(name, attr)| {
                                    parser_context.aperture_attributes.insert(name, attr.clone());
                                    ExtendedCode::ApertureAttribute(attr).into()
                                })
                        }
                        'O' => {
                            linechars = trim_attr_line(linechars)?;
                            parse_object_attribute(linechars.clone())
                                .map(|(name, attr)| {
                                    parser_context.object_attributes.insert(name, attr.clone());
                                    ExtendedCode::ObjectAttribute(attr).into()
                                })
                        }
                        'D' => parse_delete_attribute(linechars.clone(), parser_context)
                            .map(ExtendedCode::DeleteAttribute)
                            .map(Command::ExtendedCode),
                        _ => Err(ContentError::UnknownCommand {}),
                    },
                    'S' => match linechars.next().ok_or(ContentError::UnknownCommand {})? {
                        'R' => match linechars.next().ok_or(ContentError::UnknownCommand {})? {
                            'X' => parse_step_repeat_open(line),
                            // a statement %SR*% closes a step repeat command, which has no parameters
                            '*' => Ok(ExtendedCode::StepAndRepeat(StepAndRepeat::Close).into()),
                            _ => Err(ContentError::UnknownCommand {}),
                        },
                        // SF
                        'F' => parse_image_scaling(line),
                        _ => Err(ContentError::UnknownCommand {}),
                    },
                    'O' => match linechars.next().ok_or(ContentError::UnknownCommand {})? {
                        // OF
                        'F' => parse_image_offset(line),
                        _ => Err(ContentError::UnknownCommand {}),
                    },
                    'I' => match linechars.next().ok_or(ContentError::UnknownCommand {})? {
                        'N' => {
                            // Image Name, 8.1.3. Deprecated, but still used by fusion 360, etc.
                            match parse_image_name(line, gerber_doc) {
                                Ok(name) => {
                                    gerber_doc.image_name = Some(name.clone());
                                    Ok(ExtendedCode::ImageName(ImageName { name }).into())
                                }
                                Err(e) => Err(e),
                            }
                        }
                        // IP
                        'P' => parse_image_polarity(line),
                        // IR
                        'R' => parse_image_rotation(line),
                        _ => Err(ContentError::UnknownCommand {}),
                    },
                    _ => Err(ContentError::UnknownCommand {}),
                },
            ])
        }
        'X' | 'Y' => Ok(vec![parse_interpolate_move_or_flash(
            line,
            gerber_doc,
            &mut linechars,
        )]),
        'D' => {
            // select aperture D<num>* (where num >= 10) or command where num < 10
            linechars.next_back(); // remove the trailing '*'
            Ok(vec![parse_aperture_selection_or_command(
                line,
                linechars.clone(),
            )])
        }
        'M' => Ok(vec![Ok(FunctionCode::MCode(MCode::EndOfFile).into())]),
        _ => Ok(vec![Err(ContentError::UnknownCommand {})]),
    }
}

fn parse_image_mirroring(line: &str) -> Result<Command, ContentError> {
    match RE_IMAGE_MIRRORING.captures(line) {
        Some(captures) => {
            let a = captures
                .name("a")
                .map(|a| {
                    a.as_str()
                        .parse::<u8>()
                        .map_err(|e| ContentError::ParseIntegerError { cause: e })
                })
                .transpose()?
                .unwrap_or(0);
            let b = captures
                .name("b")
                .map(|b| {
                    b.as_str()
                        .parse::<u8>()
                        .map_err(|e| ContentError::ParseIntegerError { cause: e })
                })
                .transpose()?
                .unwrap_or(0);

            let image_mirroring = match (a, b) {
                (0, 0) => ImageMirroring::None,
                (1, 0) => ImageMirroring::A,
                (0, 1) => ImageMirroring::B,
                (1, 1) => ImageMirroring::AB,
                _ => {
                    // only unreachable due to regex
                    unreachable!()
                }
            };

            Ok(ExtendedCode::MirrorImage(image_mirroring).into())
        }
        None => Err(ContentError::NoRegexMatch {
            regex: RE_IMAGE_MIRRORING.clone(),
        }),
    }
}

fn parse_image_offset(line: &str) -> Result<Command, ContentError> {
    const DEFAULT_OFFSET: f64 = 0.0;

    match RE_IMAGE_OFFSET.captures(line) {
        Some(captures) => {
            let a = captures
                .name("a")
                .map(|a| {
                    a.as_str()
                        .parse::<f64>()
                        .map_err(|e| ContentError::ParseDecimalError { cause: e })
                })
                .transpose()?
                .unwrap_or(DEFAULT_OFFSET);

            let b = captures
                .name("b")
                .map(|a| {
                    a.as_str()
                        .parse::<f64>()
                        .map_err(|e| ContentError::ParseDecimalError { cause: e })
                })
                .transpose()?
                .unwrap_or(DEFAULT_OFFSET);

            Ok(ExtendedCode::OffsetImage(ImageOffset { a, b }).into())
        }
        None => Err(ContentError::NoRegexMatch {
            regex: RE_IMAGE_OFFSET.clone(),
        }),
    }
}

fn parse_image_scaling(line: &str) -> Result<Command, ContentError> {
    const DEFAULT_SCALING: f64 = 1.0;

    match RE_IMAGE_SCALING.captures(line) {
        Some(captures) => {
            let a = captures
                .name("a")
                .map(|a| {
                    a.as_str()
                        .parse::<f64>()
                        .map_err(|e| ContentError::ParseDecimalError { cause: e })
                })
                .transpose()?
                .unwrap_or(DEFAULT_SCALING);

            let b = captures
                .name("b")
                .map(|a| {
                    a.as_str()
                        .parse::<f64>()
                        .map_err(|e| ContentError::ParseDecimalError { cause: e })
                })
                .transpose()?
                .unwrap_or(DEFAULT_SCALING);

            Ok(ExtendedCode::ScaleImage(ImageScaling { a, b }).into())
        }
        None => Err(ContentError::NoRegexMatch {
            regex: RE_IMAGE_SCALING.clone(),
        }),
    }
}

fn parse_image_rotation(line: &str) -> Result<Command, ContentError> {
    build_enum_map!(IMAGE_ROTATION, ImageRotation);

    match RE_IMAGE_ROTATION.captures(line) {
        Some(captures) => {
            let value = captures
                .name("rotation")
                .ok_or(ContentError::MissingRegexNamedCapture {
                    regex: RE_IMAGE_ROTATION.clone(),
                    capture_name: "rotation".to_string(),
                })?
                .as_str();

            let image_rotation = IMAGE_ROTATION.get(&value.to_lowercase()).ok_or(
                ContentError::InvalidParameter {
                    parameter: value.to_string(),
                },
            )?;

            Ok(ExtendedCode::RotateImage(*image_rotation).into())
        }
        None => Err(ContentError::NoRegexMatch {
            regex: RE_IMAGE_ROTATION.clone(),
        }),
    }
}

fn parse_image_polarity(line: &str) -> Result<Command, ContentError> {
    build_enum_map!(IMAGE_POLARITY, ImagePolarity);

    match RE_IMAGE_POLARITY.captures(line) {
        Some(captures) => {
            let value = captures
                .name("polarity")
                .ok_or(ContentError::MissingRegexNamedCapture {
                    regex: RE_IMAGE_POLARITY.clone(),
                    capture_name: "polarity".to_string(),
                })?
                .as_str();

            let image_polarity = IMAGE_POLARITY.get(&value.to_lowercase()).ok_or(
                ContentError::InvalidParameter {
                    parameter: value.to_string(),
                },
            )?;

            Ok(ExtendedCode::ImagePolarity(*image_polarity).into())
        }
        None => Err(ContentError::NoRegexMatch {
            regex: RE_IMAGE_POLARITY.clone(),
        }),
    }
}

fn parse_axis_select(line: &str) -> Result<Command, ContentError> {
    build_enum_map!(AXIS_SELECT_MAP, AxisSelect);

    match RE_AXIS_SELECT.captures(line) {
        Some(captures) => {
            let value = captures
                .name("axisselect")
                .ok_or(ContentError::MissingRegexNamedCapture {
                    regex: RE_AXIS_SELECT.clone(),
                    capture_name: "axisselect".to_string(),
                })?
                .as_str();

            let axis_select = AXIS_SELECT_MAP.get(&value.to_lowercase()).ok_or(
                ContentError::InvalidParameter {
                    parameter: value.to_string(),
                },
            )?;

            Ok(ExtendedCode::AxisSelect(*axis_select).into())
        }
        None => Err(ContentError::NoRegexMatch {
            regex: RE_AXIS_SELECT.clone(),
        }),
    }
}

fn parse_interpolate_move_or_flash(
    line: &str,
    gerber_doc: &mut GerberDoc,
    linechars: &mut Chars,
) -> Result<Command, ContentError> {
    linechars.next_back();
    match linechars
        .next_back()
        .ok_or(ContentError::UnknownCommand {})?
    {
        '1' => parse_interpolation(line, gerber_doc), // D01
        '2' => parse_move_or_flash(line, gerber_doc, false), // D02
        '3' => parse_move_or_flash(line, gerber_doc, true), // D03
        _ => Err(ContentError::UnknownCommand {}),
    }
}

fn parse_load_mirroring(line: &str) -> Result<Command, ContentError> {
    build_enum_map!(LOAD_MIRRORING_MAP, Mirroring);

    match RE_LOAD_MIRRORING.captures(line) {
        Some(captures) => {
            let value = captures
                .name("mirroring")
                .ok_or(ContentError::MissingRegexNamedCapture {
                    regex: RE_LOAD_MIRRORING.clone(),
                    capture_name: "mirroring".to_string(),
                })?
                .as_str();

            let mirroring = LOAD_MIRRORING_MAP.get(&value.to_lowercase()).ok_or(
                ContentError::InvalidParameter {
                    parameter: value.to_string(),
                },
            )?;

            Ok(ExtendedCode::LoadMirroring(*mirroring).into())
        }
        None => Err(ContentError::NoRegexMatch {
            regex: RE_LOAD_MIRRORING.clone(),
        }),
    }
}

fn parse_load_scaling(line: &str) -> Result<Command, ContentError> {
    match RE_LOAD_SCALING.captures(line) {
        Some(captures) => {
            let scale = captures
                .name("value")
                .ok_or(ContentError::MissingRegexNamedCapture {
                    regex: RE_LOAD_SCALING.clone(),
                    capture_name: "value".to_string(),
                })?
                .as_str()
                .parse::<f64>()
                .map_err(|cause| ContentError::ParseDecimalError { cause })?;

            if scale <= 0.0 {
                // Gerber spec 2025.05 - 4.9.5 Load Scaling (LS) "<Scale> A decimal > 0."
                return Err(ContentError::InvalidParameter {
                    parameter: scale.to_string(),
                });
            }

            Ok(ExtendedCode::LoadScaling(Scaling { scale }).into())
        }
        None => Err(ContentError::NoRegexMatch {
            regex: RE_LOAD_MIRRORING.clone(),
        }),
    }
}

fn parse_load_rotation(line: &str) -> Result<Command, ContentError> {
    match RE_LOAD_ROTATION.captures(line) {
        Some(captures) => {
            let rotation = captures
                .name("value")
                .ok_or(ContentError::MissingRegexNamedCapture {
                    regex: RE_LOAD_ROTATION.clone(),
                    capture_name: "value".to_string(),
                })?
                .as_str()
                .parse::<f64>()
                .map_err(|cause| ContentError::ParseDecimalError { cause })?;

            Ok(ExtendedCode::LoadRotation(Rotation { rotation }).into())
        }
        None => Err(ContentError::NoRegexMatch {
            regex: RE_LOAD_ROTATION.clone(),
        }),
    }
}

/// Parse a Gerber Comment, there are two types, string and standard
///
/// String: `G04 (String)*`
/// Standard: `G04 #@! (Attribute)*`
fn parse_comment<T: Read>(line: &str, parser_context: &mut ParserContext<T>) -> Result<Command, ContentError> {
    match RE_COMMENT.captures(line) {
        Some(regmatch) => {
            let string_content = regmatch
                .name("string")
                .map(|string| string.as_str().to_string());

            let standard_content = regmatch
                .name("standard")
                .map(|standard| {
                    let mut comment_chars = standard.as_str().chars();
                    match comment_chars
                        .next()
                        .ok_or(ContentError::UnknownCommand {})?
                    {
                        'T' => match comment_chars
                            .next()
                            .ok_or(ContentError::UnknownCommand {})?
                        {
                            'F' => parse_file_attribute(comment_chars.clone())
                                .map(StandardComment::FileAttribute),
                            'A' => parse_aperture_attribute(comment_chars.clone())
                                .map(|(name, attr)|{
                                    parser_context.aperture_attributes.insert(name, attr.clone());
                                    StandardComment::ApertureAttribute(attr)
                                }),
                            'O' => parse_object_attribute(comment_chars.clone())
                                .map(|(name, attr)|{
                                    parser_context.object_attributes.insert(name, attr.clone());
                                    StandardComment::ObjectAttribute(attr)
                                }),
                            'D' => parse_comment_delete_attribute(comment_chars.clone(), parser_context)
                                .map(StandardComment::DeleteAttribute),
                            _ => Err(ContentError::UnknownCommand {}),
                        },
                        _ => Err(ContentError::UnknownCommand {}),
                    }
                })
                .transpose()?;

            let content = match (string_content, standard_content) {
                (Some(string), None) => CommentContent::String(string),
                (None, Some(standard)) => CommentContent::Standard(standard),

                _ => {
                    // only unreachable due to regex
                    unreachable!()
                }
            };
            Ok(FunctionCode::GCode(GCode::Comment(content)).into())
        }
        None => Err(ContentError::NoRegexMatch {
            regex: RE_COMMENT.clone(),
        }),
    }
}

/// parse an image name. This is optional and deprecated, but included in all exports from Fusion 360
fn parse_image_name(line: &str, gerber_doc: &GerberDoc) -> Result<String, ContentError> {
    if gerber_doc.image_name.is_some() {
        Err(ContentError::TriedToSetImageNameTwice {})
    } else {
        match RE_IMAGE_NAME.captures(line) {
            Some(regmatch) => {
                let image_name = regmatch
                    .get(1)
                    .ok_or(ContentError::MissingRegexCapture {
                        regex: RE_IMAGE_NAME.clone(),
                        capture_index: 1,
                    })?
                    .as_str();
                Ok(String::from(image_name))
            }
            None => Err(ContentError::NoRegexMatch {
                regex: RE_IMAGE_NAME.clone(),
            }),
        }
    }
}

/// gerber spec (2021.02) 4.5.1.1 "Except for the comment all the parameters can be a decimal, integer, macro variables or an
/// arithmetic expression."
///
/// Safety: the method should only be called if the line starts with %AM
fn parse_aperture_macro_definition<T: Read>(
    first_line: &str,
    parser_context: &mut ParserContext<T>,
) -> Result<ApertureMacro, ContentError> {
    let mut macro_content: String = String::from(first_line);
    while !macro_content.ends_with("*%") {
        let Some(line_result) = parser_context.next() else {
            break;
        };
        let line = line_result?.trim().to_string();
        macro_content.push_str(&line);
    }

    // Extract the macro name from the AM command
    let re_macro = Regex::new(r"%AM(?P<name>[^*%]*)\*(?P<remainder>.*)\*%").unwrap();
    let captures = re_macro
        .captures(&macro_content)
        .ok_or(ContentError::InvalidMacroDefinition(macro_content.clone()))?;

    let macro_name = captures
        .name("name")
        .map(|m| m.as_str().trim())
        .unwrap()
        .to_string();

    let remainder = captures
        .name("remainder")
        .map(|m| m.as_str().trim())
        .unwrap();

    let mut content = Vec::new();

    let chunks = remainder.split('*');

    log::trace!("macro chunks: {:?}", chunks);

    for chunk in chunks {
        if let Some(stripped) = chunk.strip_prefix("0 ") {
            // Handle the special-case comment primitive

            // Gerber spec: 4.5.1.2 "The comment primitive starts with the ‘0’ code followed by a space and then a
            // single-line text string"
            content.push(MacroContent::Comment(stripped.trim().to_string()));
            continue;
        }

        if chunk.starts_with("$") {
            // Handle the special-case variable definition primitive

            if let Some(captures) = RE_MACRO_VARIABLE.captures(chunk) {
                let number = captures
                    .name("number")
                    .map(|number| number.as_str().parse::<u32>())
                    .ok_or(ContentError::InvalidMacroDefinition(
                        "Missing variable number".to_string(),
                    ))?
                    .map_err(|error| {
                        ContentError::InvalidMacroDefinition(
                            format!("Invalid variable number, cause: {}", error).to_string(),
                        )
                    })?;

                let expression = captures
                    .name("expression")
                    .map(|expression| expression.as_str().to_string())
                    .ok_or(ContentError::InvalidMacroDefinition(
                        "Missing expression for variable definition".to_string(),
                    ))?;

                content.push(MacroContent::VariableDefinition(VariableDefinition {
                    number,
                    expression,
                }));
            }

            // Gerber spec: 4.5.1.2 "The comment primitive starts with the ‘0’ code followed by a space and then a
            // single-line text string"
            continue;
        }

        let mut params: Vec<String> = line_to_params(chunk);

        if !params.is_empty() {
            // Parse outline primitive (type 4)
            // TODO would be nice to have constants for the values in `gerber-types`. e.g. const APERTURE_MACRO_TYPE_OUTLINE: u8 = 4;
            match params[0].parse::<u8>() {
                Ok(1) => {
                    // Handle circle primitive
                    let param_count_excluding_code = params.len() - 1;

                    if !(4..=5).contains(&param_count_excluding_code) {
                        // exposure + diameter + center x + center y [ + rotation]
                        return Err(ContentError::InvalidMacroDefinition(
                            "expected 4-5 parameters for circle".to_string(),
                        ));
                    }

                    // reverse the params, so we can pop them one at a time.
                    params.reverse();
                    let _primitive_code = params.pop();

                    let exposure_str = params.pop().unwrap().trim().to_string();
                    let exposure = parse_macro_boolean(&exposure_str)?;

                    let diameter_str = params.pop().unwrap().trim().to_string();
                    let diameter = parse_macro_decimal(&diameter_str)?;

                    let center_x_str = params.pop().unwrap().trim().to_string();
                    let center_x = parse_macro_decimal(&center_x_str)?;

                    let center_y_str = params.pop().unwrap().trim().to_string();
                    let center_y = parse_macro_decimal(&center_y_str)?;

                    // Parse rotation angle from the last line
                    let angle = params
                        .pop()
                        .map(|angle_str| parse_macro_decimal(&angle_str))
                        .transpose()?;

                    let circle = CirclePrimitive {
                        exposure,

                        diameter,
                        center: (center_x, center_y),
                        angle,
                    };
                    content.push(MacroContent::Circle(circle));
                }
                Ok(4) => {
                    // Handle outline primitive
                    let param_count_excluding_code = params.len() - 1;

                    if param_count_excluding_code < 6 {
                        // exposure + #vertices + start x + start y [+ (x,y)] + end x + end y [ + rotation]
                        return Err(ContentError::InvalidMacroDefinition(
                            "expected minimum of 6 parameters for outline".to_string(),
                        ));
                    }

                    // reverse the params, so we can pop them one at a time.
                    params.reverse();
                    let _primitive_code = params.pop();

                    let exposure_str = params.pop().unwrap().trim().to_string();
                    let exposure = parse_macro_boolean(&exposure_str)?;

                    let num_vertices =
                        params.pop().unwrap().trim().parse::<u32>().map_err(|_| {
                            ContentError::ApertureDefinitionParseFailed {
                                aperture_definition_str: chunk.to_string(),
                            }
                        })?;

                    // Collect points from remaining parameters
                    let mut points = Vec::new();

                    // `num_points` is actually one fewer than the number of points in the macro definition, so we use `<=` here
                    while points.len() <= num_vertices as usize {
                        if params.len() >= 2 {
                            let x_str = params.pop().unwrap().trim().to_string();
                            let x = parse_macro_decimal(&x_str)?;
                            let y_str = params.pop().unwrap().trim().to_string();
                            let y = parse_macro_decimal(&y_str)?;

                            points.push((x, y));
                        } else {
                            return Err(ContentError::InvalidMacroDefinition(
                                "Missing outline point line.".to_string(),
                            ));
                        }
                    }

                    // Parse rotation angle from the last line
                    let angle = params
                        .pop()
                        .map(|angle_str| parse_macro_decimal(&angle_str))
                        .transpose()?
                        .unwrap_or(MacroDecimal::Value(0.0));

                    let outline = OutlinePrimitive {
                        exposure,
                        points,
                        angle,
                    };

                    content.push(MacroContent::Outline(outline));
                }
                Ok(5) => {
                    // Handle polygon primitive
                    let param_count_excluding_code = params.len() - 1;

                    if !(5..=6).contains(&param_count_excluding_code) {
                        // exposure + #vertices + center x + center y + diameter [ + rotation]
                        return Err(ContentError::InvalidMacroDefinition(
                            "Expected 5-6 parameters for polygon".to_string(),
                        ));
                    }

                    // reverse the params, so we can pop them one at a time.
                    params.reverse();
                    let _primitive_code = params.pop();

                    let exposure_str = params.pop().unwrap().trim().to_string();
                    let exposure = parse_macro_boolean(&exposure_str)?;

                    let vertices_str = params.pop().unwrap().trim().to_string();
                    let vertices = parse_macro_integer(&vertices_str)?;

                    let center_x_str = params.pop().unwrap().trim().to_string();
                    let center_x = parse_macro_decimal(&center_x_str)?;

                    let center_y_str = params.pop().unwrap().trim().to_string();
                    let center_y = parse_macro_decimal(&center_y_str)?;

                    let diameter_str = params.pop().unwrap().trim().to_string();
                    let diameter = parse_macro_decimal(&diameter_str)?;

                    // Parse rotation angle from the last parameter
                    let angle = params
                        .pop()
                        .map(|angle_str| parse_macro_decimal(&angle_str))
                        .transpose()?
                        .unwrap_or(MacroDecimal::Value(0.0));

                    let polygon_primitive = PolygonPrimitive {
                        exposure,
                        vertices,
                        center: (center_x, center_y),
                        diameter,
                        angle,
                    };

                    content.push(MacroContent::Polygon(polygon_primitive));
                }
                Ok(7) => {
                    // Thermal primitive

                    let param_count_excluding_code = params.len() - 1;

                    if param_count_excluding_code != 6 {
                        // center x + center y + outer diameter + inner diameter + gap + rotation
                        return Err(ContentError::InvalidMacroDefinition(
                            "Expected 6 parameters for thermal".to_string(),
                        ));
                    }

                    // reverse the params, so we can pop them one at a time.
                    params.reverse();
                    let _primitive_code = params.pop();

                    let center_x_str = params.pop().unwrap().trim().to_string();
                    let center_x = parse_macro_decimal(&center_x_str)?;

                    let center_y_str = params.pop().unwrap().trim().to_string();
                    let center_y = parse_macro_decimal(&center_y_str)?;

                    let outer_diameter_str = params.pop().unwrap().trim().to_string();
                    let outer_diameter = parse_macro_decimal(&outer_diameter_str)?;

                    let inner_diameter_str = params.pop().unwrap().trim().to_string();
                    let inner_diameter = parse_macro_decimal(&inner_diameter_str)?;

                    let gap_str = params.pop().unwrap().trim().to_string();
                    let gap = parse_macro_decimal(&gap_str)?;

                    let angle_str = params.pop().unwrap().trim().to_string();
                    let angle = parse_macro_decimal(&angle_str)?;

                    let thermal_primitive = ThermalPrimitive {
                        center: (center_x, center_y),
                        outer_diameter,
                        inner_diameter,
                        gap,
                        angle,
                    };

                    content.push(MacroContent::Thermal(thermal_primitive));
                }

                Ok(20) => {
                    // Vector-line primitive
                    let param_count_excluding_code = params.len() - 1;

                    if !(6..=7).contains(&param_count_excluding_code) {
                        // exposure + width + start x + start y + end x + end y [ + rotation]
                        return Err(ContentError::InvalidMacroDefinition(
                            "Expected 6-7 parameters for vector-line".to_string(),
                        ));
                    }

                    // reverse the params, so we can pop them one at a time.
                    params.reverse();
                    let _primitive_code = params.pop();

                    let exposure_str = params.pop().unwrap().trim().to_string();
                    let exposure = parse_macro_boolean(&exposure_str)?;

                    let width_str = params.pop().unwrap().trim().to_string();
                    let width = parse_macro_decimal(&width_str)?;

                    let start_x_str = params.pop().unwrap().trim().to_string();
                    let start_x = parse_macro_decimal(&start_x_str)?;

                    let start_y_str = params.pop().unwrap().trim().to_string();
                    let start_y = parse_macro_decimal(&start_y_str)?;

                    let end_x_str = params.pop().unwrap().trim().to_string();
                    let end_x = parse_macro_decimal(&end_x_str)?;

                    let end_y_str = params.pop().unwrap().trim().to_string();
                    let end_y = parse_macro_decimal(&end_y_str)?;

                    // Parse rotation angle from the last parameter
                    let angle = params
                        .pop()
                        .map(|angle_str| parse_macro_decimal(&angle_str))
                        .transpose()?
                        .unwrap_or(MacroDecimal::Value(0.0));

                    let vector_line = VectorLinePrimitive {
                        exposure,
                        width,
                        start: (start_x, start_y),
                        end: (end_x, end_y),
                        angle,
                    };
                    content.push(MacroContent::VectorLine(vector_line));
                }
                Ok(21) => {
                    // Center-line primitive
                    let param_count_excluding_code = params.len() - 1;

                    if !(5..=6).contains(&param_count_excluding_code) {
                        // exposure + width + height + center x + center y [ + rotation]
                        return Err(ContentError::InvalidMacroDefinition(
                            "Expected 5-6 parameters for center-line".to_string(),
                        ));
                    }

                    // reverse the params, so we can pop them one at a time.
                    params.reverse();
                    let _primitive_code = params.pop();

                    let exposure_str = params.pop().unwrap().trim().to_string();
                    let exposure = parse_macro_boolean(&exposure_str)?;

                    let width_str = params.pop().unwrap().trim().to_string();
                    let width = parse_macro_decimal(&width_str)?;

                    let height_str = params.pop().unwrap().trim().to_string();
                    let height = parse_macro_decimal(&height_str)?;

                    let center_x_str = params.pop().unwrap().trim().to_string();
                    let center_x = parse_macro_decimal(&center_x_str)?;

                    let center_y_str = params.pop().unwrap().trim().to_string();
                    let center_y = parse_macro_decimal(&center_y_str)?;

                    // Parse rotation angle from the last parameter
                    let angle = params
                        .pop()
                        .map(|angle_str| parse_macro_decimal(&angle_str))
                        .transpose()?
                        .unwrap_or(MacroDecimal::Value(0.0));

                    let center_line = CenterLinePrimitive {
                        exposure,
                        dimensions: (width, height),
                        center: (center_x, center_y),
                        angle,
                    };
                    content.push(MacroContent::CenterLine(center_line));
                }
                _ => {
                    log::error!(
                        "Unsupported primitive type: {}, chunk: {}, params: {}",
                        params[0],
                        chunk,
                        params[1..].join(", ")
                    );

                    return Err(ContentError::UnsupportedMacroDefinition);
                }
            }
        }
    }

    Ok(ApertureMacro {
        name: macro_name,
        content,
    })
}

/// parse a Gerber unit statement (e.g. '%MOMM*%')
fn parse_units(line: &str, gerber_doc: &GerberDoc) -> Result<Unit, ContentError> {
    // Check that the units are not set yet (that would imply the set unit command is present twice)
    if gerber_doc.units.is_some() {
        Err(ContentError::TriedToSetUnitsTwice {})
    } else {
        match RE_UNITS.captures(line) {
            Some(regmatch) => {
                let units_str = regmatch
                    .get(1)
                    .ok_or(ContentError::MissingRegexCapture {
                        regex: RE_UNITS.clone(),
                        capture_index: 1,
                    })?
                    .as_str();
                match units_str {
                    "MM" => Ok(Unit::Millimeters),
                    "IN" => Ok(Unit::Inches),
                    _ => Err(ContentError::InvalidUnitFormat {
                        units_str: line.to_string(),
                    }),
                }
            }
            None => Err(ContentError::NoRegexMatch {
                regex: RE_UNITS.clone(),
            }),
        }
    }
}

/// parse a Gerber format spec statement (e.g. '%FSLAX23Y23*%')
/// FS<L,T><A|I>X<int><dec>Y<int><dec>*%
/// <L,T>  = Leading or Trailing (omitted = Leading)
/// <A|I> = Absolute or Incremental (omitted = Absolute)
/// <int> = number of integer digits (1-6)
fn parse_format_spec(line: &str, gerber_doc: &GerberDoc) -> Result<CoordinateFormat, ContentError> {
    // Ensure that FS was not set before, which would imply two FS statements in the same doc
    if gerber_doc.format_specification.is_some() {
        Err(ContentError::TriedToFormatTwice {})
    } else {
        match RE_FORMAT_SPEC.captures(line) {
            Some(regmatch) => {
                let fs_str = regmatch.get(0)
                    .ok_or(ContentError::MissingRegexCapture {
                        regex: RE_FORMAT_SPEC.clone(),
                        capture_index: 0,
                    })?
                    .as_str();

                let zero_omission = match fs_str.chars().nth(3) {
                    Some('T') => ZeroOmission::Trailing,
                    _ => ZeroOmission::Leading,
                };

                let mode = match fs_str.chars().nth(4) {
                    Some('I') => CoordinateMode::Incremental,
                    _ => CoordinateMode::Absolute,
                };

                let mut fs_chars = regmatch
                    .get(1)
                    .ok_or(ContentError::MissingRegexCapture {
                        regex: RE_FORMAT_SPEC.clone(),
                        capture_index: 1,
                    })?
                    .as_str()
                    .chars();
                let integer: u8 = parse_char(
                    fs_chars
                        .next()
                        .ok_or(ContentError::ParseFormatErrorWrongNumDigits {})?,
                )?;
                let decimal: u8 = parse_char(
                    fs_chars
                        .next()
                        .ok_or(ContentError::ParseFormatErrorWrongNumDigits {})?,
                )?;

                // the gerber spec states that the integer value can be at most 6
                if !(1..=6).contains(&integer) {
                    return Err(ContentError::ParseFormatErrorInvalidDigit {
                        digit_found: integer,
                    });
                }

                Ok(CoordinateFormat::new(zero_omission, mode, integer, decimal))
            }

            None => Err(ContentError::NoRegexMatch {
                regex: RE_FORMAT_SPEC.clone(),
            }),
        }
    }
}

/// helper function to move some ugly repeated .ok_or().unwrap().Arc<Mutex<Future>>
fn parse_char(char_in: char) -> Result<u8, ContentError> {
    Ok(char_in.to_digit(10).ok_or(ContentError::ParseDigitError {
        char_found: char_in,
    })? as u8)
}

fn parse_aperture_block(line: &str) -> Result<Option<i32>, ContentError> {
    let Some(captures) = RE_APERTURE_BLOCK.captures(line) else {
        return Err(ContentError::NoRegexMatch {
            regex: RE_APERTURE_BLOCK.clone(),
        });
    };

    captures
        .name("code")
        .map(|code| parse_aperture_code(code.as_str()))
        .transpose()
}

// parse a Gerber aperture definition e.g. '%ADD44R, 2.0X3.0*%')
fn parse_aperture_defs(
    line: &str,
    gerber_doc: &GerberDoc,
) -> Result<(i32, Aperture), ContentError> {
    let Some(captures) = RE_APERTURE.captures(line) else {
        return Err(ContentError::NoRegexMatch {
            regex: RE_APERTURE.clone(),
        });
    };

    // Sync captures with [`RE_APERTURE`] definition.
    // `%ADD([0-9]+)([._$a-zA-Z][._$a-zA-Z0-9]{0,126})(?:,\s?(.*))?\*%`
    const CAPTURE_APERTURE_CODE: usize = 1;
    const CAPTURE_APERTURE_NAME: usize = 2;
    const CAPTURE_APERTURE_ARGS: usize = 3;

    // Parse aperture code
    let code_str = captures
        .get(CAPTURE_APERTURE_CODE)
        .ok_or(ContentError::MissingRegexCapture {
            regex: RE_APERTURE.clone(),
            capture_index: CAPTURE_APERTURE_CODE,
        })?
        .as_str();
    let code = parse_aperture_code(code_str)?;

    let aperture_name = captures
        .get(CAPTURE_APERTURE_NAME)
        .ok_or(ContentError::MissingRegexCapture {
            regex: RE_APERTURE.clone(),
            capture_index: CAPTURE_APERTURE_NAME,
        })?
        .as_str();

    if gerber_doc.apertures.contains_key(&code) {
        return Err(ContentError::ApertureDefinedTwice {
            aperture_code: code,
        });
    }

    let aperture_args_str: Option<&str> = captures.get(CAPTURE_APERTURE_ARGS).map(|m| m.as_str());

    let is_macro = aperture_name.len() > 1;
    if is_macro {
        let optional_params: Option<Vec<MacroDecimal>> = aperture_args_str
            .map(|line| {
                let parse_results = line_to_args(line)
                    .iter()
                    .map(|param| {
                        let arg_str = param.trim().to_string();
                        parse_macro_decimal(&arg_str)
                    })
                    .collect::<Vec<_>>();

                parse_results
                    .into_iter()
                    .try_fold(vec![], |mut args, result| {
                        let arg = result?;
                        args.push(arg);
                        Ok(args)
                    })
            })
            .transpose()?;

        return Ok((
            code,
            Aperture::Macro(aperture_name.to_string(), optional_params),
        ));
    }

    let aperture_args_split: Option<(Vec<&str>, usize)> = aperture_args_str.map(|m| {
        let args: Vec<_> = m.split('X').collect();
        let len = args.len();
        (args, len)
    });

    match (aperture_name, aperture_args_split) {
        ("C", Some((args, count))) if (1..=2).contains(&count) => Ok((
            code,
            Aperture::Circle(Circle {
                diameter: args[0].trim().parse::<f64>().map_err(|_| {
                    ContentError::ParseApertureDefinitionBodyError {
                        aperture_code: code,
                    }
                })?,
                hole_diameter: if count > 1 {
                    Some(args[1].trim().parse::<f64>().map_err(|_| {
                        ContentError::ParseApertureDefinitionBodyError {
                            aperture_code: code,
                        }
                    })?)
                } else {
                    None
                },
            }),
        )),
        ("R", Some((args, count))) if (2..=3).contains(&count) => Ok((
            code,
            Aperture::Rectangle(Rectangular {
                x: parse_coord::<f64>(args[0])?,
                y: parse_coord::<f64>(args[1])?,
                hole_diameter: if count > 2 {
                    Some(args[2].trim().parse::<f64>().map_err(|_| {
                        ContentError::ParseApertureDefinitionBodyError {
                            aperture_code: code,
                        }
                    })?)
                } else {
                    None
                },
            }),
        )),
        ("O", Some((args, count))) if (2..=3).contains(&count) => Ok((
            code,
            Aperture::Obround(Rectangular {
                x: parse_coord::<f64>(args[0])?,
                y: parse_coord::<f64>(args[1])?,
                hole_diameter: if count > 2 {
                    Some(args[2].trim().parse::<f64>().map_err(|_| {
                        ContentError::ParseApertureDefinitionBodyError {
                            aperture_code: code,
                        }
                    })?)
                } else {
                    None
                },
            }),
        )),
        // note that for polygon we HAVE TO specify rotation if we want to add a hole
        ("P", Some((args, count))) if (2..=4).contains(&count) => Ok((
            code,
            Aperture::Polygon(Polygon {
                diameter: args[0].trim().parse::<f64>().map_err(|_| {
                    ContentError::ParseApertureDefinitionBodyError {
                        aperture_code: code,
                    }
                })?,
                vertices: args[1].trim().parse::<u8>().map_err(|_| {
                    ContentError::ParseApertureDefinitionBodyError {
                        aperture_code: code,
                    }
                })?,
                rotation: if count > 2 {
                    Some(args[2].trim().parse::<f64>().map_err(|_| {
                        ContentError::ParseApertureDefinitionBodyError {
                            aperture_code: code,
                        }
                    })?)
                } else {
                    None
                },
                hole_diameter: if count > 3 {
                    Some(args[3].trim().parse::<f64>().map_err(|_| {
                        ContentError::ParseApertureDefinitionBodyError {
                            aperture_code: code,
                        }
                    })?)
                } else {
                    None
                },
            }),
        )),
        (aperture_name, _) if ["C", "R", "O", "P"].contains(&aperture_name) => {
            // FUTURE this could be improved by also reporting the supported argument count range
            Err(ContentError::IncorrectDefinitionArgCount {
                aperture_code: code,
                aperture_name: aperture_name.to_string(),
            })
        }
        (unknown_type, _args) => Err(ContentError::UnknownApertureType {
            type_str: unknown_type.to_string(),
        }),
    }
}

fn parse_coord<T: std::str::FromStr>(coord_str: &str) -> Result<T, ContentError> {
    coord_str
        .trim()
        .parse::<T>()
        .map_err(|_| ContentError::FailedToParseCoordinate {
            coord_num_str: coord_str.to_string(),
        })
}

fn parse_aperture_code(code_str: &str) -> Result<i32, ContentError> {
    match code_str.parse::<i32>() {
        Ok(v) if v >= 10 => Ok(v),
        Ok(_v) => Err(ContentError::ApertureCodeParseFailed {
            aperture_code_str: code_str.to_string(),
        }),
        Err(_) => Err(ContentError::ApertureCodeParseFailed {
            aperture_code_str: code_str.to_string(),
        }),
    }
}
fn parse_aperture_selection_or_command(
    line: &str,
    linechars: Chars,
) -> Result<Command, ContentError> {
    let aperture_str = linechars.as_str();
    if let Ok(aperture_code) = parse_aperture_code(aperture_str) {
        Ok(FunctionCode::DCode(DCode::SelectAperture(aperture_code)).into())
    } else {
        parse_command(line)
    }
}

fn parse_command(command_str: &str) -> Result<Command, ContentError> {
    static RE_STANDALONE_D: Lazy<Regex> = lazy_regex!(r"D(0)?([1-3])\*");
    if let Some(captures) = RE_STANDALONE_D.captures(command_str) {
        let command_code = captures.get(2).unwrap().as_str();
        {
            let operation = match command_code {
                "1" => Operation::Interpolate(None, None),
                "2" => Operation::Move(None),
                "3" => Operation::Flash(None),
                _ => unreachable!(),
            };
            Ok(Command::FunctionCode(FunctionCode::DCode(
                DCode::Operation(operation),
            )))
        }
    } else {
        Err(ContentError::UnknownCommand {})
    }
}


fn format_coordinate_number(coord: &str, fs: &CoordinateFormat) -> Result<i64, ContentError> {
    let len = fs.integer + fs.decimal;
    Ok(match fs.zero_omission {
        ZeroOmission::Leading => {
            parse_coord::<i64>(coord)?
        }
        ZeroOmission::Trailing => {
            let len = if coord.starts_with("-") { len + 1 } else { len };
            let formated = format!("{:0<width$}", coord, width = len as usize);
            parse_coord::<i64>(formated.as_str())?
        }
    })
}

// parse a Gerber interpolation command (e.g. 'X2000Y40000I300J50000D01*')
fn parse_interpolation(line: &str, gerber_doc: &GerberDoc) -> Result<Command, ContentError> {

    match RE_INTERPOLATION.captures(line) {
        Some(regmatch) => {
            let fs = gerber_doc
                .format_specification
                .ok_or(ContentError::OperationBeforeFormat {})?;

            let x_coord = regmatch
                .get(1)
                .map(|x| format_coordinate_number(x.as_str(), &fs))
                .transpose()?;
            let y_coord = regmatch
                .get(2)
                .map(|x| format_coordinate_number(x.as_str(), &fs))
                .transpose()?;
            let i_coord = regmatch
                .get(3)
                .map(|x| format_coordinate_number(x.as_str(), &fs))
                .transpose()?;
            let j_coord = regmatch
                .get(4)
                .map(|x| format_coordinate_number(x.as_str(), &fs))
                .transpose()?;

            let coordinates =
                partial_coordinates_from_gerber(x_coord, y_coord, fs).map_err(|error| {
                    ContentError::CoordinateFormatMismatch {
                        format: fs,
                        cause: error,
                    }
                })?;

            let offset =
                partial_coordinates_offset_from_gerber(i_coord, j_coord, fs).map_err(|error| {
                    ContentError::CoordinateFormatMismatch {
                        format: fs,
                        cause: error,
                    }
                })?;

            Ok(FunctionCode::DCode(DCode::Operation(Operation::Interpolate(
                coordinates,
                offset,
            )))
            .into())
        }
        None => Err(ContentError::NoRegexMatch {
            regex: RE_INTERPOLATION.clone(),
        }),
    }
}

// parse a Gerber move or flash command (e.g. 'X2000Y40000D02*')
fn parse_move_or_flash(
    line: &str,
    gerber_doc: &GerberDoc,
    flash: bool,
) -> Result<Command, ContentError> {
    match RE_MOVE_OR_FLASH.captures(line) {
        Some(regmatch) => {
            let fs = gerber_doc
                .format_specification
                .ok_or(ContentError::OperationBeforeFormat {})?;

            let x_coord = regmatch
                .get(1)
                .map(|x| format_coordinate_number(x.as_str(), &fs))
                .transpose()?;
            let y_coord = regmatch
                .get(2)
                .map(|x| format_coordinate_number(x.as_str(), &fs))
                .transpose()?;

            let coords =
                partial_coordinates_from_gerber(x_coord, y_coord, fs).map_err(|error| {
                    ContentError::CoordinateFormatMismatch {
                        format: fs,
                        cause: error,
                    }
                })?;

            if flash {
                Ok(FunctionCode::DCode(DCode::Operation(Operation::Flash(coords))).into())
            } else {
                Ok(FunctionCode::DCode(DCode::Operation(Operation::Move(coords))).into())
            }
        }
        None => Err(ContentError::NoRegexMatch {
            regex: RE_MOVE_OR_FLASH.clone(),
        }),
    }
}

// a step and repeat open statement has four (required) parameters that we need to extract
// X (pos int) Y (pos int), I (decimal), J (decimal)
fn parse_step_repeat_open(line: &str) -> Result<Command, ContentError> {
    match RE_STEP_REPEAT.captures(line) {
        Some(regmatch) => Ok(ExtendedCode::StepAndRepeat(StepAndRepeat::Open {
            repeat_x: parse_coord::<u32>(
                regmatch
                    .get(1)
                    .ok_or(ContentError::MissingRegexCapture {
                        regex: RE_STEP_REPEAT.clone(),
                        capture_index: 1,
                    })?
                    .as_str(),
            )?,
            repeat_y: parse_coord::<u32>(
                regmatch
                    .get(2)
                    .ok_or(ContentError::MissingRegexCapture {
                        regex: RE_STEP_REPEAT.clone(),
                        capture_index: 2,
                    })?
                    .as_str(),
            )?,
            distance_x: parse_coord::<f64>(
                regmatch
                    .get(3)
                    .ok_or(ContentError::MissingRegexCapture {
                        regex: RE_STEP_REPEAT.clone(),
                        capture_index: 3,
                    })?
                    .as_str(),
            )?,
            distance_y: parse_coord::<f64>(
                regmatch
                    .get(4)
                    .ok_or(ContentError::MissingRegexCapture {
                        regex: RE_STEP_REPEAT.clone(),
                        capture_index: 4,
                    })?
                    .as_str(),
            )?,
        })
        .into()),
        None => Err(ContentError::NoRegexMatch {
            regex: RE_STEP_REPEAT.clone(),
        }),
    }
}

/// Parse an Aperture Attribute (%TF.<AttributeName>[,<AttributeValue>]*%) into Command
///
/// ⚠️ This parsing statement needs a lot of tests and validation!
fn parse_file_attribute(line: Chars) -> Result<FileAttribute, ContentError> {
    macro_rules! with_side_and_optional_index {
        ($name:ident, $args:ident) => {
            Ok(FileAttribute::FileFunction(FileFunction::$name {
                pos: parse_position($args[0])?,
                index: $args.get(1).map(|value| parse_integer(value)).transpose()?,
            }))
        };
    }
    macro_rules! with_side {
        ($name:ident, $args:ident) => {
            Ok(FileAttribute::FileFunction(FileFunction::$name(
                parse_position($args[0])?,
            )))
        };
    }

    macro_rules! with_string {
        ($name:ident, $args:ident) => {
            Ok(FileAttribute::FileFunction(FileFunction::$name(
                $args[0].to_string(),
            )))
        };
    }

    macro_rules! with_layer_and_side {
        ($name:ident, $args:ident) => {
            Ok(FileAttribute::FileFunction(FileFunction::$name {
                layer: parse_layer($args[0])?,
                pos: parse_position($args[1])?,
            }))
        };
    }

    macro_rules! with_optional_side {
        ($name:ident, $args:ident) => {
            Ok(FileAttribute::FileFunction(FileFunction::$name(
                $args
                    .get(0)
                    .map(|value| parse_position(value))
                    .transpose()?,
            )))
        };
    }

    let attr_args = attr_args(line);

    log::trace!("TF args: {:?}, len: {}", attr_args, attr_args.len());

    let (first, remaining_args, remaining_len) = split_first_str(&attr_args);
    match (first, remaining_args, remaining_len) {
        (".Part", args, len) if len >= 1 => {
            let (first, remaining_args, remaining_len) = split_first_str(remaining_args);
            match (first, remaining_args, remaining_len) {
                ("Single", _, 0) => Ok(FileAttribute::Part(Part::Single)),
                ("Array", _, 0) => Ok(FileAttribute::Part(Part::Array)),
                ("FabricationPanel", _, 0) => Ok(FileAttribute::Part(Part::FabricationPanel)),
                ("Coupon", _, 0) => Ok(FileAttribute::Part(Part::Coupon)),
                ("Other", _, len) if len <= 1 => Ok(FileAttribute::Part(Part::Other(
                    args.get(1)
                        .ok_or(ContentError::InsufficientArguments)?
                        .to_string(),
                ))),
                (arg, _, _) => Err(ContentError::UnsupportedPartType {
                    part_type: arg.to_string(),
                }),
            }
        }
        (".FileFunction", remaining_args, len) if len >= 1 => {
            let (first, remaining_args, remaining_len) = split_first_str(remaining_args);
            match (first, remaining_args, remaining_len) {
                //
                // Data Layers
                //
                ("Copper", args, len) if (2..=3).contains(&len) => {
                    Ok(FileAttribute::FileFunction(FileFunction::Copper {
                        layer: parse_layer(args[0])?,
                        pos: parse_extended_position(args[1])?,
                        copper_type: args
                            .get(2)
                            .map(|value| parse_copper_type(value))
                            .transpose()?,
                    }))
                }
                ("Plated", args, len) if (3..=4).contains(&len) => {
                    Ok(FileAttribute::FileFunction(FileFunction::Plated {
                        from_layer: parse_integer(args[0])?,
                        to_layer: parse_integer(args[1])?,
                        drill: parse_plated_drill(args[2])?,
                        label: args
                            .get(3)
                            .map(|value| parse_drill_route_type(value))
                            .transpose()?,
                    }))
                }
                ("NonPlated", args, len) if (3..=4).contains(&len) => {
                    Ok(FileAttribute::FileFunction(FileFunction::NonPlated {
                        from_layer: parse_integer(args[0])?,
                        to_layer: parse_integer(args[1])?,
                        drill: parse_non_plated_drill(args[2])?,
                        label: args
                            .get(3)
                            .map(|value| parse_drill_route_type(value))
                            .transpose()?,
                    }))
                }
                ("Profile", args, len) if len <= 1 => {
                    Ok(FileAttribute::FileFunction(FileFunction::Profile(
                        args.first().map(|value| parse_profile(value)).transpose()?,
                    )))
                }
                ("Soldermask", args, len) if len <= 2 => {
                    with_side_and_optional_index!(SolderMask, args)
                }
                ("Legend", args, len) if len <= 2 => with_side_and_optional_index!(Legend, args),
                ("Component", args, 2) => with_layer_and_side!(Component, args),
                ("Paste", args, 1) => with_side!(Paste, args),
                ("Glue", args, 1) => with_side!(Glue, args),
                ("Carbonmask", args, len) if len <= 2 => {
                    with_side_and_optional_index!(CarbonMask, args)
                }
                ("Goldmask", args, len) if len <= 2 => {
                    with_side_and_optional_index!(GoldMask, args)
                }
                ("Heatsinkmask", args, len) if len <= 2 => {
                    with_side_and_optional_index!(HeatsinkMask, args)
                }
                ("Peelablemask", args, len) if len <= 2 => {
                    with_side_and_optional_index!(PeelableMask, args)
                }
                ("Silvermask", args, len) if len <= 2 => {
                    with_side_and_optional_index!(SilverMask, args)
                }
                ("Tinmask", args, len) if len <= 2 => with_side_and_optional_index!(TinMask, args),
                ("Depthrout", args, 1) => with_side!(DepthRoute, args),
                ("Vcut", args, len) if len <= 1 => with_optional_side!(VCut, args),
                ("Viafill", _, 0) => Ok(FileAttribute::FileFunction(FileFunction::ViaFill)),
                ("Pads", args, 1) => with_side!(Pads, args),
                ("Other", args, 1) => with_string!(Other, args),
                //
                // Drawing layers
                //
                ("Drillmap", _, 0) => Ok(FileAttribute::FileFunction(FileFunction::DrillMap)),
                ("FabricationDrawing", _, 0) => Ok(FileAttribute::FileFunction(
                    FileFunction::FabricationDrawing,
                )),
                ("Vcutmap", _, 0) => Ok(FileAttribute::FileFunction(FileFunction::VCutMap)),
                ("AssemblyDrawing", args, 1) => with_side!(AssemblyDrawing, args),
                ("ArrayDrawing", _, 0) => {
                    Ok(FileAttribute::FileFunction(FileFunction::ArrayDrawing))
                }
                ("OtherDrawing", args, 1) => with_string!(OtherDrawing, args),
                (arg, _, _) => Err(ContentError::UnsupportedFileAttribute {
                    attribute_name: arg.to_string(),
                }),
            }
        }
        (".FilePolarity", remaining_args, 1) => {
            let (first, remaining_args, remaining_len) = split_first_str(remaining_args);
            match (first, remaining_args, remaining_len) {
                ("Positive", _, 0) => Ok(FileAttribute::FilePolarity(FilePolarity::Positive)),
                ("Negative", _, 0) => Ok(FileAttribute::FilePolarity(FilePolarity::Negative)),
                (arg, _, _) => Err(ContentError::UnsupportedPolarityType {
                    polarity_type: arg.to_string(),
                }),
            }
        }
        (".SameCoordinates", args, len) if len <= 1 => Ok(FileAttribute::SameCoordinates(
            args.first().map(|value| parse_ident(value)).transpose()?,
        )),
        (".CreationDate", args, 1) => Ok(FileAttribute::CreationDate(parse_date_time(args[0])?)),
        (".GenerationSoftware", args, len) if len <= 3 => {
            Ok(FileAttribute::GenerationSoftware(GenerationSoftware {
                vendor: args[0].to_string(),
                application: args[1].to_string(),
                version: args.get(2).map(ToString::to_string),
            }))
        }
        (".ProjectId", args, 3) => Ok(FileAttribute::ProjectId {
            id: args[0].to_string(),
            uuid: parse_uuid(args[1])?,
            revision: args[2].to_string(),
        }),
        (".MD5", args, 1) => Ok(FileAttribute::Md5(args[0].to_string())),
        (arg, args, _) => Ok(FileAttribute::UserDefined {
            name: arg.to_string(),
            values: args.iter().map(|v| v.to_string()).collect(),
        }),
    }
}

fn parse_uuid(arg: &str) -> Result<Uuid, ContentError> {
    Uuid::parse_str(arg).map_err(|_error| ContentError::InvalidUuid(arg.to_string()))
}

fn parse_date_time(arg: &str) -> Result<GerberDate, ContentError> {
    GerberDate::parse_from_rfc3339(arg)
        .map_err(|_error| ContentError::InvalidDateTime(arg.to_string()))
}

fn parse_ident(arg: &str) -> Result<Ident, ContentError> {
    match Uuid::parse_str(arg) {
        Ok(uuid) => Ok(Ident::Uuid(uuid)),
        Err(_) => Ok(Ident::Name(arg.to_string())),
    }
}

fn parse_profile(arg: &str) -> Result<Profile, ContentError> {
    match arg.to_lowercase().as_str() {
        "p" => Ok(Profile::Plated),
        "np" => Ok(Profile::NonPlated),
        _ => Err(ContentError::InvalidParameter {
            parameter: arg.to_string(),
        }),
    }
}

fn parse_copper_type(arg: &str) -> Result<CopperType, ContentError> {
    match arg.to_lowercase().as_str() {
        "plane" => Ok(CopperType::Plane),
        "signal" => Ok(CopperType::Signal),
        "mixed" => Ok(CopperType::Mixed),
        "hatched" => Ok(CopperType::Hatched),
        _ => Err(ContentError::InvalidParameter {
            parameter: arg.to_string(),
        }),
    }
}

fn parse_extended_position(arg: &str) -> Result<ExtendedPosition, ContentError> {
    match arg.to_lowercase().as_str() {
        "top" => Ok(ExtendedPosition::Top),
        "inr" => Ok(ExtendedPosition::Inner),
        "bot" => Ok(ExtendedPosition::Bottom),
        _ => Err(ContentError::InvalidParameter {
            parameter: arg.to_string(),
        }),
    }
}

fn parse_position(arg: &str) -> Result<Position, ContentError> {
    match arg.to_lowercase().as_str() {
        "top" => Ok(Position::Top),
        "bot" => Ok(Position::Bottom),
        _ => Err(ContentError::InvalidParameter {
            parameter: arg.to_string(),
        }),
    }
}

fn parse_non_plated_drill(arg: &str) -> Result<NonPlatedDrill, ContentError> {
    match arg.to_lowercase().as_str() {
        "npth" => Ok(NonPlatedDrill::NonPlatedThroughHole),
        "buried" => Ok(NonPlatedDrill::Buried),
        "blind" => Ok(NonPlatedDrill::Blind),
        _ => Err(ContentError::InvalidParameter {
            parameter: arg.to_string(),
        }),
    }
}

fn parse_plated_drill(arg: &str) -> Result<PlatedDrill, ContentError> {
    match arg.to_lowercase().as_str() {
        "pth" => Ok(PlatedDrill::PlatedThroughHole),
        "buried" => Ok(PlatedDrill::Buried),
        "blind" => Ok(PlatedDrill::Blind),
        _ => Err(ContentError::InvalidParameter {
            parameter: arg.to_string(),
        }),
    }
}

fn parse_integer(arg: &str) -> Result<i32, ContentError> {
    arg.parse::<i32>()
        .map_err(|e| ContentError::ParseIntegerError { cause: e })
}

fn parse_layer(arg: &str) -> Result<i32, ContentError> {
    if !arg.starts_with("L") {
        return Err(ContentError::InvalidLayerParameter(arg.to_string()));
    }

    arg[1..]
        .parse::<i32>()
        .map_err(|e| ContentError::ParseIntegerError { cause: e })
}

fn parse_drill_route_type(arg: &str) -> Result<DrillRouteType, ContentError> {
    match arg.to_lowercase().as_str() {
        "drill" => Ok(DrillRouteType::Drill),
        "rout" => Ok(DrillRouteType::Route),
        "mixed" => Ok(DrillRouteType::Mixed),
        _ => Err(ContentError::InvalidParameter {
            parameter: arg.to_string(),
        }),
    }
}

fn split_first_str<'a>(slice: &'a [&'a str]) -> (&'a str, &'a [&'a str], usize) {
    slice
        .split_first()
        .map(|(head, tail)| (*head, tail, tail.len()))
        .unwrap()
}

/// Parse an Aperture Attribute (%TA.<AttributeName>[,<AttributeValue>]*%) into Command
///
/// ⚠️ This parsing statement needs a lot of tests and validation!
fn parse_aperture_attribute(partial_line: Chars) -> Result<(String, ApertureAttribute), ContentError> {
    use ContentError::UnsupportedApertureAttribute;

    build_enum_map!(IPC_MAP, IPC4761ViaProtection);
    build_enum_map!(COMPONENT_DRILL_MAP, ComponentDrill);
    build_enum_map!(DRILL_FUNCTION_MAP, DrillFunction);
    build_enum_map!(SMD_PAD_MAP, SmdPadType);
    build_enum_map!(COMPONENT_OUTLINE_MAP, ComponentOutline);

    fn lookup_in_map_optional<'a, T>(
        arg: Option<&&str>,
        map: &'a HashMap<String, T>,
    ) -> Result<Option<&'a T>, ContentError> {
        arg.map(|arg| {
            map.get(&arg.to_lowercase())
                .ok_or(ContentError::InvalidParameter {
                    parameter: arg.to_string(),
                })
        })
        .transpose()
    }

    fn lookup_in_map_required<'a, T>(
        arg: &str,
        map: &'a HashMap<String, T>,
    ) -> Result<&'a T, ContentError> {
        map.get(&arg.to_lowercase())
            .ok_or(ContentError::InvalidParameter {
                parameter: arg.to_string(),
            })
    }

    let raw_line = partial_line.as_str().to_string();
    let attr_args = attr_args(partial_line);

    log::trace!("TA ARGS: {:?}", attr_args);

    let (first, remaining_args, remaining_len) = split_first_str(&attr_args);
    let attr = match (first, remaining_args, remaining_len) {
        (".AperFunction", remaining_args, len) if len >= 1 => {
            let (first, remaining_args, remaining_len) = split_first_str(remaining_args);
            Ok(ApertureAttribute::ApertureFunction(
                match (first, remaining_args, remaining_len) {
                    // "Drill and rout layers"
                    ("ViaDrill", args, len) if len <= 1 => {
                        let function = lookup_in_map_optional(args.first(), &IPC_MAP)?.cloned();

                        ApertureFunction::ViaDrill(function)
                    }
                    ("BackDrill", _, 0) => ApertureFunction::BackDrill,
                    ("ComponentDrill", args, len) if len <= 1 => {
                        let function =
                            lookup_in_map_optional(args.first(), &COMPONENT_DRILL_MAP)?.cloned();
                        ApertureFunction::ComponentDrill { function }
                    }
                    ("MechanicalDrill", args, len) if len <= 1 => {
                        let function =
                            lookup_in_map_optional(args.first(), &DRILL_FUNCTION_MAP)?.cloned();
                        ApertureFunction::MechanicalDrill { function }
                    }
                    ("CastellatedDrill", _, 0) => ApertureFunction::CastellatedDrill,
                    ("OtherDrill", args, 1) => ApertureFunction::OtherDrill(args[0].to_string()),

                    // "Copper layers"
                    ("ComponentPad", _, 0) => ApertureFunction::ComponentPad,
                    ("SMDPad", args, 1) => {
                        let value = lookup_in_map_required(args[0], &SMD_PAD_MAP)?;
                        ApertureFunction::SmdPad(*value)
                    }
                    ("BGAPad", args, 1) => {
                        let value = lookup_in_map_required(args[0], &SMD_PAD_MAP)?;
                        ApertureFunction::BgaPad(*value)
                    }
                    ("ConnectorPad", _, 0) => ApertureFunction::ConnectorPad,
                    ("HeatsinkPad", _, 0) => ApertureFunction::HeatsinkPad,
                    ("ViaPad", _, 0) => ApertureFunction::ViaPad,
                    ("TestPad", _, 0) => ApertureFunction::TestPad,
                    ("CastellatedPad", _, 0) => ApertureFunction::CastellatedPad,
                    ("FiducialPad", args, 1) => match args[0] {
                        "Local" => ApertureFunction::FiducialPad(FiducialScope::Local),
                        "Global" => ApertureFunction::FiducialPad(FiducialScope::Global),
                        "Panel" => ApertureFunction::FiducialPad(FiducialScope::Panel),
                        _ => {
                            return Err(UnsupportedApertureAttribute {
                                aperture_attribute: raw_line,
                            })
                        }
                    },
                    ("ThermalReliefPad", _, 0) => ApertureFunction::ThermalReliefPad,
                    ("WasherPad", _, 0) => ApertureFunction::WasherPad,
                    ("AntiPad", _, 0) => ApertureFunction::AntiPad,
                    ("OtherPad", args, 1) => ApertureFunction::OtherPad(args[0].to_string()),
                    ("Conductor", _, 0) => ApertureFunction::Conductor,
                    ("EtchedComponent", _, 0) => ApertureFunction::EtchedComponent,
                    ("NonConductor", _, 0) => ApertureFunction::NonConductor,
                    ("CopperBalancing", _, 0) => ApertureFunction::CopperBalancing,
                    ("Border", _, 0) => ApertureFunction::Border,
                    ("OtherCopper", args, 1) => ApertureFunction::OtherCopper(args[0].to_string()),

                    // "Component layers"
                    ("ComponentMain", _, 0) => ApertureFunction::ComponentMain,
                    ("ComponentOutline", args, 1) => {
                        let value = lookup_in_map_required(args[0], &COMPONENT_OUTLINE_MAP)?;
                        ApertureFunction::ComponentOutline(*value)
                    }
                    ("ComponentPin", _, 0) => ApertureFunction::ComponentPin,

                    // "All data layers"
                    ("Profile", _, 0) => ApertureFunction::Profile,
                    ("NonMaterial", _, 0) => ApertureFunction::NonMaterial,
                    ("Material", _, 0) => ApertureFunction::Material,
                    ("Other", args, 1) => ApertureFunction::Other(args[0].to_string()),

                    // "Deprecated" (not in 2024.05 - 5.6.10 ".AperFunction")
                    ("Slot", _, 0) => ApertureFunction::Slot,
                    ("CutOut", _, 0) => ApertureFunction::CutOut,
                    ("Cavity", _, 0) => ApertureFunction::Cavity,
                    ("Drawing", _, 0) => ApertureFunction::Drawing,
                    _ => {
                        return Err(UnsupportedApertureAttribute {
                            aperture_attribute: raw_line,
                        })
                    }
                },
            ))
        }
        (".DrillTolerance", args, 2) => Ok(ApertureAttribute::DrillTolerance {
            plus: args[0].parse::<f64>().map_err(|_| {
                ContentError::DrillToleranceParseNumError {
                    number_str: args[0].to_string(),
                }
            })?,
            minus: args[1].parse::<f64>().map_err(|_| {
                ContentError::DrillToleranceParseNumError {
                    number_str: args[1].to_string(),
                }
            })?,
        }),
        (arg, args, _) => Ok(ApertureAttribute::UserDefined {
            name: arg.to_string(),
            values: args.iter().map(|v| v.to_string()).collect(),
        }),
    };

    attr.map(|it|(first.to_string(), it))
}

/// Parse an Object Attribute (%TO.<AttributeName>[,<AttributeValue>]*%) into Command
///
/// ⚠️ This parsing statement needs a lot of tests and validation at the current stage!
fn parse_object_attribute(partial_line: Chars) -> Result<(String, ObjectAttribute), ContentError> {
    macro_rules! parse_cc_decimal {
        ($cc:ident, $value:expr) => {{
            let decimal = $value
                .parse::<f64>()
                .map_err(|cause| ContentError::ParseDecimalError { cause })?;
            Ok(ObjectAttribute::ComponentCharacteristics(
                ComponentCharacteristics::$cc(decimal),
            ))
        }};
    }
    macro_rules! parse_cc_string {
        ($cc:ident, $value:expr) => {{
            Ok(ObjectAttribute::ComponentCharacteristics(
                ComponentCharacteristics::$cc($value.to_string()),
            ))
        }};
    }

    let attr_args = attr_args(partial_line);

    log::trace!("TO ARGS: {:?}", attr_args);

    let (first, remaining_args, remaining_len) = split_first_str(&attr_args);

    let attr = match (first, remaining_args, remaining_len) {
        (".N", args, len) if len >= 1 => {
            // See 2024.05 - 5.6.13 ".N (Net)" "
            let first = args.first().unwrap();
            if first.is_empty() {
                // ```
                // The empty string, defined by %TO.N,*% identifies objects not connected to a net, such as
                // tooling holes, text, logos, pads for component leads not connected to the component
                // circuitry
                // ```
                Ok(ObjectAttribute::Net(Net::None))
            } else if first.eq(&"N/C") {
                // ```
                // The name N/C, defined by %TO.N,N/C*%, identifies a single pad net, as an alternative to
                // giving each such net a unique name. (N/C stands for not-connected.)
                // ```
                Ok(ObjectAttribute::Net(Net::NotConnected))
            } else {
                let names = args
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<String>>();
                Ok(ObjectAttribute::Net(Net::Connected(names)))
            }
        }
        (".P", args, len) if len <= 3 => Ok(ObjectAttribute::Pin(Pin {
            refdes: args[0].to_string(),
            name: args[1].to_string(),
            function: args.get(2).map(ToString::to_string),
        })),
        (".C", args, 1) => Ok(ObjectAttribute::Component(args[0].to_string())),
        (".CRot", args, 1) => parse_cc_decimal!(Rotation, args[0]),
        (".CMfr", args, 1) => parse_cc_string!(Manufacturer, args[0]),
        (".CMPN", args, 1) => parse_cc_string!(MPN, args[0]),
        (".CVal", args, 1) => parse_cc_string!(Value, args[0]),
        (".CMnt", args, 1) => {
            let component_mounting = match args[0].to_lowercase().as_str() {
                "th" => Ok(ComponentMounting::ThroughHole),
                "smd" => Ok(ComponentMounting::SMD),
                "pressfit" => Ok(ComponentMounting::PressFit),
                "other" => Ok(ComponentMounting::Other),
                _ => Err(ContentError::InvalidParameter {
                    parameter: args[0].to_string(),
                }),
            };
            component_mounting.map(|component_mounting| {
                ObjectAttribute::ComponentCharacteristics(ComponentCharacteristics::Mount(
                    component_mounting,
                ))
            })
        }
        (".CFtp", args, 1) => parse_cc_string!(Footprint, args[0]),
        (".CPgN", args, 1) => parse_cc_string!(PackageName, args[0]),
        (".CPgD", args, 1) => parse_cc_string!(PackageDescription, args[0]),
        (".CHgt", args, 1) => parse_cc_decimal!(Height, args[0]),
        (".CLbN", args, 1) => parse_cc_string!(LibraryName, args[0]),
        (".CLbD", args, 1) => parse_cc_string!(LibraryDescription, args[0]),
        (arg, args, _) => Ok(ObjectAttribute::UserDefined {
            name: arg.to_string(),
            values: args.iter().map(|v| v.to_string()).collect(),
        }),
    };

    attr.map(|it|(first.to_string(), it))
}

fn parse_delete_attribute<T: Read>(line: Chars, parser_context: &mut ParserContext<T>) -> Result<AttributeDeletionCriterion, ContentError> {
    let line = trim_attr_line(line)?;
    let value = line.as_str().to_string();

    parse_delete_attribute_inner(parser_context, value)
}

fn parse_comment_delete_attribute<T: Read>(comment: Chars, parser_context: &mut ParserContext<T>) -> Result<AttributeDeletionCriterion, ContentError> {
    let value = comment.as_str().trim().to_string();
    parse_delete_attribute_inner(parser_context, value)
}

fn parse_delete_attribute_inner<T: Read>(parser_context: &mut ParserContext<T>, value: String) -> Result<AttributeDeletionCriterion, ContentError> {
    if value.is_empty() {
        return Ok(AttributeDeletionCriterion::AllApertureAndObjectAttributes)
    }
    if parser_context.aperture_attributes.contains_key(&value) {
        parser_context.aperture_attributes.remove(&value);
        return Ok(AttributeDeletionCriterion::SingleApertureAttribute(value))
    }
    if parser_context.object_attributes.contains_key(&value) {
        parser_context.object_attributes.remove(&value);
        return Ok(AttributeDeletionCriterion::SingleObjectAttribute(value))
    }

    Err(ContentError::InvalidDeleteAttribute { delete_attribute: value })
}

/// Split the line by commas and convert to a vector of strings
/// trimming whitespace from each parameter
fn line_to_params(line: &str) -> Vec<String> {
    line.split(',')
        .map(|param| param.trim().into())
        .filter(|param: &String| !param.is_empty())
        .collect()
}

/// Split the line by X and convert to a vector of strings
/// trimming whitespace from each parameter
fn line_to_args(line: &str) -> Vec<String> {
    line.split('X')
        .map(|param| param.trim().into())
        .filter(|param: &String| !param.is_empty())
        .collect()
}

/// value should be a pre-trimmed string.
fn parse_macro_decimal(value: &str) -> Result<MacroDecimal, ContentError> {
    let captures = RE_MACRO_DECIMAL
        .captures(value)
        .ok_or(ContentError::InvalidMacroDefinition(
            "Invalid parameter".to_string(),
        ))?;
    if let Some(value_match) = captures.name("value") {
        let value = value_match
            .as_str()
            .parse::<f64>()
            .map_err(|_| ContentError::InvalidMacroDefinition("Invalid decimal".to_string()))?;
        Ok(MacroDecimal::Value(value))
    } else if let Some(value_match) = captures.name("variable") {
        let variable = value_match
            .as_str()
            .trim_start_matches('$')
            .parse::<u32>()
            .map_err(|_| ContentError::InvalidMacroDefinition("Invalid variable".to_string()))?;
        Ok(MacroDecimal::Variable(variable))
    } else if let Some(value_match) = captures.name("expression") {
        let variable = value_match.as_str().to_string();
        Ok(MacroDecimal::Expression(variable))
    } else {
        // it has to match one of the named captures.
        unreachable!()
    }
}

/// value should be a pre-trimmed string.
fn parse_macro_boolean(value: &str) -> Result<MacroBoolean, ContentError> {
    let captures = RE_MACRO_BOOLEAN
        .captures(value)
        .ok_or(ContentError::InvalidMacroDefinition(
            "Invalid parameter".to_string(),
        ))?;
    if let Some(value_match) = captures.name("value") {
        let value = value_match
            .as_str()
            .parse::<u8>()
            .map_err(|_| ContentError::InvalidMacroDefinition("Invalid boolean".to_string()))?;
        Ok(MacroBoolean::Value(value == 1))
    } else if let Some(value_match) = captures.name("variable") {
        let variable = value_match
            .as_str()
            .trim_start_matches('$')
            .parse::<u32>()
            .map_err(|_| ContentError::InvalidMacroDefinition("Invalid variable".to_string()))?;
        Ok(MacroBoolean::Variable(variable))
    } else if let Some(value_match) = captures.name("expression") {
        let variable = value_match.as_str().to_string();
        Ok(MacroBoolean::Expression(variable))
    } else {
        // it has to match one of the named captures.
        unreachable!()
    }
}

/// value should be a pre-trimmed string.
fn parse_macro_integer(value: &str) -> Result<MacroInteger, ContentError> {
    let captures =
        RE_MACRO_UNSIGNED_INTEGER
            .captures(value)
            .ok_or(ContentError::InvalidMacroDefinition(
                "Invalid parameter".to_string(),
            ))?;
    if let Some(value_match) = captures.name("value") {
        let value = value_match
            .as_str()
            .parse::<u32>()
            .map_err(|_| ContentError::InvalidMacroDefinition("Invalid integer".to_string()))?;
        Ok(MacroInteger::Value(value))
    } else if let Some(value_match) = captures.name("variable") {
        let variable = value_match
            .as_str()
            .trim_start_matches('$')
            .parse::<u32>()
            .map_err(|_| ContentError::InvalidMacroDefinition("Invalid variable".to_string()))?;
        Ok(MacroInteger::Variable(variable))
    } else if let Some(value_match) = captures.name("expression") {
        let variable = value_match.as_str().to_string();
        Ok(MacroInteger::Expression(variable))
    } else {
        // it has to match one of the named captures.
        unreachable!()
    }
}

fn attr_args(partial_line: Chars<'_>) -> Vec<&str> {
    partial_line
        .as_str()
        .split(',')
        .map(|el| el.trim())
        .collect()
}

fn trim_attr_line(mut partial_line: Chars) -> Result<Chars, ContentError> {
    let last = partial_line.next_back();
    let second_last = partial_line.next_back();

    match (second_last, last) {
        (Some('*'), Some('%')) => Ok(partial_line),
        _ => Err(ContentError::NoEndOfLine {
            line: partial_line.as_str().to_string(),
        }),
    }
}
#[cfg(test)]
mod attr_args_tests {
    use super::*;

    #[test]
    pub fn test_attr_args() {
        let attribute_chars = "  .DrillTolerance  , 0.02  , 0.01   ".chars();
        let arguments = attr_args(attribute_chars);
        println!("arguments: {:?}", arguments);
        assert_eq!(arguments, vec![".DrillTolerance", "0.02", "0.01"])
    }

    #[test]
    pub fn test_ensure_line_end() {
        let line_chars = "line content*%".chars();
        let result = trim_attr_line(line_chars);
        assert!(result.is_ok());
    }

    #[test]
    pub fn test_ensure_line_end_missing() {
        let line_chars = "line content".chars();
        let result = trim_attr_line(line_chars);
        assert!(result.is_err());
    }
}
