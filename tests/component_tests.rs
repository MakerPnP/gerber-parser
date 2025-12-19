use gerber_parser::{parse, ContentError, GerberParserErrorWithContext};
use gerber_types::{
    Aperture, ApertureAttribute, ApertureBlock, ApertureDefinition, ApertureFunction,
    ApertureMacro, AxisSelect, Circle, CirclePrimitive, Command, CommentContent,
    ComponentCharacteristics, ComponentDrill, ComponentMounting, ComponentOutline,
    CoordinateFormat, CoordinateMode, CoordinateOffset, Coordinates, CopperType, DCode,
    DrillFunction, DrillRouteType, ExtendedCode, ExtendedPosition, FiducialScope, FileAttribute,
    FileFunction, FilePolarity, FunctionCode, GCode, GenerationSoftware, GerberDate, GerberError,
    IPC4761ViaProtection, Ident, ImageMirroring, ImageOffset, ImagePolarity, ImageRotation,
    ImageScaling, InterpolationMode, MCode, MacroBoolean, MacroContent, MacroDecimal, MacroInteger,
    Mirroring, Net, NonPlatedDrill, ObjectAttribute, Operation, OutlinePrimitive, Part, Pin,
    PlatedDrill, Polarity, Polygon, PolygonPrimitive, Position, Profile, QuadrantMode, Rectangular,
    Rotation, Scaling, SmdPadType, StandardComment, StepAndRepeat, ThermalPrimitive, Unit, Uuid,
    VariableDefinition, VectorLinePrimitive, ZeroOmission,
};
use std::collections::HashMap;
use strum::VariantArray;
mod util;
use gerber_parser::util::{
    coordinates_from_gerber, gerber_to_reader, partial_coordinates_from_gerber,
    partial_coordinates_offset_from_gerber,
};
use util::testing::logging_init;

/// This macro is used extensively by the tests to parse, then filter commands based on the closure $c which takes
/// a single `Command` as an argument, the closure should return 'true' to keep the command, false otherwise.
/// The closure is often implemented using `matches!(command, ...)`
macro_rules! parse_and_filter {
    ($reader:ident, $commands:ident, $filtered_commands:ident, $c:expr) => {
        let $commands = parse($reader).unwrap().commands;
        println!("parsed commands:");
        dump_commands(&$commands);

        // then
        let filter_commands = |cmds:Vec<Result<Command, GerberParserErrorWithContext>>| -> Vec<Result<Command, GerberParserErrorWithContext>> {
            cmds.into_iter().filter($c).collect()};

        let $filtered_commands = filter_commands($commands);
        println!("filtered commands:");
        dump_commands(&$filtered_commands);

    };
}

fn dump_commands(commands: &[Result<Command, GerberParserErrorWithContext>]) {
    println!("{}", commands_to_string(commands));
    println!();
}

fn commands_to_string(commands: &[Result<Command, GerberParserErrorWithContext>]) -> String {
    commands
        .iter()
        .map(|it| format!("{:?}", it))
        .collect::<Vec<_>>()
        .join("\n")
}

/// This macro can be used like assert_eq!(), except that is converts the commands to a string, with newlines between
/// each command and compares those first.
///
/// Doing this makes it easier to see the differences in IDEs like RustRover which provide a 'diff' view
/// for analyzing failures.  Different or missing commands are *much* easier to spot.
///
/// It does two asserts, one for the string (debug) representation of the commands, and one for the commands' values.
macro_rules! assert_eq_commands {
    ($left:expr, $right:expr) => {{
        let left = $left;
        let right = $right;
        // ensures the debug representations are the same
        assert_eq!(commands_to_string(&left), commands_to_string(&right));
        // ensures the values are the same
        assert_eq!(left, right);
    }};
}

// #[test]
// fn test_full_gerber() {
//     let gerber_reader = gerber_to_reader(&SAMPLE_GERBER_1);
//     let gbr = parse_gerber(gerber_reader);
//     println!("{}",&gbr);
//     assert_eq!(gbr, GerberDoc::new());
// }

#[test]
fn format_specification() {
    // given
    logging_init();

    let reader_fs_1 = gerber_to_reader(
        "
    %FSLAX15Y15*%
    %MOMM*%
    M02*        
    ",
    );

    let reader_fs_2 = gerber_to_reader(
        "
    %FSLAX36Y36*%
    %MOIN*%
    G04 Actual apertures and draw commands go here*
    M02*        
    ",
    );

    let reader_fs_3 = gerber_to_reader(
        "
    %FSTAX36Y36*%
    %MOIN*%
    G04 Actual apertures and draw commands go here*
    M02*
    ",
    );

    assert_eq!(
        parse(reader_fs_1).unwrap().format_specification,
        Some(CoordinateFormat::new(
            ZeroOmission::Leading,
            CoordinateMode::Absolute,
            1,
            5
        ))
    );

    assert_eq!(
        parse(reader_fs_2).unwrap().format_specification,
        Some(CoordinateFormat::new(
            ZeroOmission::Leading,
            CoordinateMode::Absolute,
            3,
            6
        ))
    );

    assert_eq!(
        parse(reader_fs_3).unwrap().format_specification,
        Some(CoordinateFormat::new(
            ZeroOmission::Trailing,
            CoordinateMode::Absolute,
            3,
            6
        ))
    );
}

#[test]
fn units() {
    // given
    logging_init();

    let reader_mm = gerber_to_reader(
        "
    G04 The next line specifies the precision of the units*
    %FSLAX23Y23*%
    G04 The next line specifies the units (inches or mm)*
    %MOMM*%

    G04 Actual apertures and draw commands go here*
    M02*        
    ",
    );

    let reader_in = gerber_to_reader(
        "
    G04 The next line specifies the precision of the units*
    %FSLAX23Y23*%
    G04 The next line specifies the units (inches or mm)*
    %MOIN*%

    G04 Actual apertures and draw commands go here*
    M02*        
    ",
    );

    assert_eq!(parse(reader_mm).unwrap().units, Some(Unit::Millimeters));
    assert_eq!(parse(reader_in).unwrap().units, Some(Unit::Inches));
}

#[test]
#[allow(non_snake_case)]
fn G01_G03_standalone() {
    // given
    logging_init();

    let reader = gerber_to_reader(
        r#"
        G01*
        G02*
        G03*
        M02*
    "#,
    );

    // when
    parse_and_filter!(reader, commands, filtered_commands, |cmd| matches!(
        cmd,
        Ok(Command::FunctionCode(FunctionCode::GCode(_)))
            | Ok(Command::FunctionCode(FunctionCode::MCode(MCode::EndOfFile)))
    ));

    // then
    assert_eq!(
        filtered_commands,
        vec![
            Ok(Command::FunctionCode(FunctionCode::GCode(
                GCode::InterpolationMode(InterpolationMode::Linear)
            ))),
            Ok(Command::FunctionCode(FunctionCode::GCode(
                GCode::InterpolationMode(InterpolationMode::ClockwiseCircular)
            ))),
            Ok(Command::FunctionCode(FunctionCode::GCode(
                GCode::InterpolationMode(InterpolationMode::CounterclockwiseCircular)
            ))),
            Ok(Command::FunctionCode(FunctionCode::MCode(MCode::EndOfFile))),
        ]
    );
}

#[test]
#[allow(non_snake_case)]
fn G04_comments() {
    // given
    logging_init();

    let reader = gerber_to_reader(
        "
    G04 Comment before typical configuration lines*
    %FSLAX23Y23*%
    %MOMM*%
    G04 And now a comment after them*
    M02*        
    ",
    );

    // when
    parse_and_filter!(reader, commands, filtered_commands, |cmd| matches!(
        cmd,
        Ok(Command::FunctionCode(FunctionCode::GCode(GCode::Comment(
            _
        ))))
    ));

    // then
    assert_eq!(
        filtered_commands,
        vec![
            Ok(Command::FunctionCode(FunctionCode::GCode(GCode::Comment(
                CommentContent::String("Comment before typical configuration lines".to_string(),)
            )))),
            Ok(Command::FunctionCode(FunctionCode::GCode(GCode::Comment(
                CommentContent::String("And now a comment after them".to_string(),)
            )))),
        ]
    );
}

#[test]
fn aperture_selection() {
    // given
    logging_init();

    let reader = gerber_to_reader(
        "
    %FSLAX23Y23*%
    %MOMM*%

    %ADD999C, 0.01*%
    %ADD22R, 0.01X0.15*%

    G04 Select some valid apertures*
    D22*
    D999*
    D22*

    G04 Select an unknown aperture*
    D100*

    G04 Invalid aperture definition*
    %ADD101Z, 0*%
    G04 Select an incorrectly defined aperture*
    D101*

    M02*
    ",
    );

    // Note, it's important that aperture selections for unknown or invalid apertures appear in the commands so that
    // renderers can handle them appropriately, e.g. by displaying a error or rendering a placeholder where the aperture
    // should be.

    // when
    parse_and_filter!(reader, commands, filtered_commands, |cmd| matches!(
        cmd,
        Ok(Command::FunctionCode(FunctionCode::DCode(
            DCode::SelectAperture(_)
        )))
    ));

    // then
    assert_eq!(
        filtered_commands,
        vec![
            Ok(Command::FunctionCode(FunctionCode::DCode(
                DCode::SelectAperture(22)
            ))),
            Ok(Command::FunctionCode(FunctionCode::DCode(
                DCode::SelectAperture(999)
            ))),
            Ok(Command::FunctionCode(FunctionCode::DCode(
                DCode::SelectAperture(22)
            ))),
            Ok(Command::FunctionCode(FunctionCode::DCode(
                DCode::SelectAperture(100)
            ))),
            Ok(Command::FunctionCode(FunctionCode::DCode(
                DCode::SelectAperture(101)
            ))),
        ]
    )
}

/// Test the D01* statements (linear)
#[test]
#[allow(non_snake_case)]
fn D01_interpolation_linear() {
    // given
    logging_init();

    let reader = gerber_to_reader(
        "
    %FSLAX23Y23*%
    %MOMM*%

    %ADD999C, 0.01*%
    D999*

    X4000Y5000D01*
    X0Y0D01*
    X-1000Y-30000D01*

    M02*        
    ",
    );

    let fs = CoordinateFormat::new(ZeroOmission::Leading, CoordinateMode::Absolute, 2, 3);

    // when
    parse_and_filter!(reader, commands, filtered_commands, |cmd| matches!(
        cmd,
        Ok(Command::FunctionCode(FunctionCode::DCode(
            DCode::Operation(Operation::Interpolate(_, _))
        )))
    ));

    // then
    assert_eq!(
        filtered_commands,
        vec![
            Ok(Command::FunctionCode(FunctionCode::DCode(
                DCode::Operation(Operation::Interpolate(
                    coordinates_from_gerber(4000, 5000, fs).unwrap(),
                    None
                ))
            ))),
            Ok(Command::FunctionCode(FunctionCode::DCode(
                DCode::Operation(Operation::Interpolate(
                    coordinates_from_gerber(0, 0, fs).unwrap(),
                    None
                ))
            ))),
            Ok(Command::FunctionCode(FunctionCode::DCode(
                DCode::Operation(Operation::Interpolate(
                    coordinates_from_gerber(-1000, -30000, fs).unwrap(),
                    None
                ))
            )))
        ]
    )
}

/// Test the D01* statements (circular)
#[test]
#[allow(non_snake_case)]
fn D01_interpolation_circular() {
    // given
    logging_init();

    let reader = gerber_to_reader(
        "
    %FSLAX23Y23*%
    %MOMM*%

    %ADD999C, 0.01*%
    D999*

    X0Y0D01*
    X-1000Y-30000I200J-5000D01*

    M02*        
    ",
    );

    let fs = CoordinateFormat::new(ZeroOmission::Leading, CoordinateMode::Absolute, 2, 3);

    // when
    parse_and_filter!(reader, commands, filtered_commands, |cmd| matches!(
        cmd,
        Ok(Command::FunctionCode(FunctionCode::DCode(
            DCode::Operation(Operation::Interpolate(_, _))
        )))
    ));

    // then
    assert_eq!(
        filtered_commands,
        vec![
            Ok(Command::FunctionCode(FunctionCode::DCode(
                DCode::Operation(Operation::Interpolate(
                    coordinates_from_gerber(0, 0, fs).unwrap(),
                    None
                ))
            ))),
            Ok(Command::FunctionCode(FunctionCode::DCode(
                DCode::Operation(Operation::Interpolate(
                    coordinates_from_gerber(-1000, -30000, fs).unwrap(),
                    partial_coordinates_offset_from_gerber(Some(200), Some(-5000), fs).unwrap()
                ))
            )))
        ]
    )
}

/// Test the D02* statements
#[test]
#[allow(non_snake_case)]
fn DO2_move_to_command() {
    // given
    logging_init();

    let reader = gerber_to_reader(
        "
    %FSLAX23Y23*%
    %MOMM*%

    %ADD999C, 0.01*%
    D999*

    X0Y-333D02*
    X300Y300D01*

    X5555Y-12D02*
    X-300Y-300D01*

    M02*        
    ",
    );

    let fs = CoordinateFormat::new(ZeroOmission::Leading, CoordinateMode::Absolute, 2, 3);
    // when
    parse_and_filter!(reader, commands, filtered_commands, |cmd| matches!(
        cmd,
        Ok(Command::FunctionCode(FunctionCode::DCode(
            DCode::Operation(Operation::Move(_))
        )))
    ));

    // then
    assert_eq!(
        filtered_commands,
        vec![
            Ok(Command::FunctionCode(FunctionCode::DCode(
                DCode::Operation(Operation::Move(
                    coordinates_from_gerber(0, -333, fs).unwrap()
                ))
            ))),
            Ok(Command::FunctionCode(FunctionCode::DCode(
                DCode::Operation(Operation::Move(
                    coordinates_from_gerber(5555, -12, fs).unwrap()
                ))
            )))
        ]
    )
}

/// Test the D03* statements
#[test]
#[allow(non_snake_case)]
fn DO3_flash_command() {
    // given
    logging_init();

    let reader = gerber_to_reader(
        "
    %FSLAX23Y23*%
    %MOMM*%

    %ADD999C, 0.01*%
    D999*

    X4000Y-5000D03*
    X0Y0D03*

    M02*        
    ",
    );

    let fs = CoordinateFormat::new(ZeroOmission::Leading, CoordinateMode::Absolute, 2, 3);
    // when
    parse_and_filter!(reader, commands, filtered_commands, |cmd| matches!(
        cmd,
        Ok(Command::FunctionCode(FunctionCode::DCode(
            DCode::Operation(Operation::Flash(_))
        )))
    ));

    // then
    assert_eq!(
        filtered_commands,
        vec![
            Ok(Command::FunctionCode(FunctionCode::DCode(
                DCode::Operation(Operation::Flash(
                    coordinates_from_gerber(4000, -5000, fs).unwrap()
                ))
            ))),
            Ok(Command::FunctionCode(FunctionCode::DCode(
                DCode::Operation(Operation::Flash(coordinates_from_gerber(0, 0, fs).unwrap()))
            )))
        ]
    )
}

/// Gerber spec allows for omitted coordinates. This means that 'X100D03*' and 'Y100D03*' are
/// valid statements.
#[test]
fn omitted_coordinate() {
    // given
    logging_init();

    let reader_leading = gerber_to_reader(
        "
    %FSLAX23Y23*%
    %MOMM*%
    %ADD999C, 0.01*%    
    D999*

    G04 here the last coordinate is (0,0) - by construction*
    Y-3000D03*
    G04 Now we set X=1234, but the client needs to maintain the last Y coordinate, namely -3000*
    X1234D03*

    M02*        
    ",
    );

    let reader_trailing = gerber_to_reader(
        "
    %FSTAX23Y23*%
    %MOMM*%
    %ADD999C, 0.01*%
    D999*

    G04 here the last coordinate is (0,0) - by construction*
    Y-03D03*
    G04 Now we set X=1234, but the client needs to maintain the last Y coordinate, namely -3000*
    X001234D03*
    M02*
    ",
    );

    let fs = CoordinateFormat::new(ZeroOmission::Leading, CoordinateMode::Absolute, 2, 3);

    // when
    parse_and_filter!(reader_leading, commands, filtered_commands, |cmd| matches!(
        cmd,
        Ok(Command::FunctionCode(FunctionCode::DCode(
            DCode::Operation(Operation::Flash(_))
        )))
    ));

    // then
    assert_eq!(
        filtered_commands,
        vec![
            Ok(Command::FunctionCode(FunctionCode::DCode(
                DCode::Operation(Operation::Flash(
                    partial_coordinates_from_gerber(None, Some(-3000), fs).unwrap()
                ))
            ))),
            Ok(Command::FunctionCode(FunctionCode::DCode(
                DCode::Operation(Operation::Flash(
                    partial_coordinates_from_gerber(Some(1234), None, fs).unwrap()
                ))
            )))
        ]
    );
    parse_and_filter!(
        reader_trailing,
        commands,
        filtered_commands2,
        |cmd| matches!(
            cmd,
            Ok(Command::FunctionCode(FunctionCode::DCode(
                DCode::Operation(Operation::Flash(_))
            )))
        )
    );

    // Compare the two results nanos
    for (i, cmd) in filtered_commands.iter().enumerate() {
        let cmd2 = &filtered_commands2[i];
        match { (cmd, cmd2) } {
            (
                Ok(Command::FunctionCode(FunctionCode::DCode(DCode::Operation(Operation::Flash(
                    Some(c1),
                ))))),
                Ok(Command::FunctionCode(FunctionCode::DCode(DCode::Operation(Operation::Flash(
                    Some(c2),
                ))))),
            ) => {
                assert_eq!(c1.x, c2.x);
                assert_eq!(c1.y, c2.y);
            }
            _ => panic!("Unexpected command types"),
        }
    }
}

// See gerber spec 2021-02, section 4.5
#[test]
fn test_load_polarity_scaling_mirroring_rotation() {
    // given
    let reader = gerber_to_reader(
        r#"
    %LPD*%
    %LPC*%
    %LMN*%
    %LMX*%
    %LMY*%
    %LMXY*%
    %LR0*%
    %LR359.99*%
    %LR-359.99*%
    %LS0.5*%
    %LS99.99*%
    "#,
    );

    // when
    parse_and_filter!(reader, commands, filtered_commands, |cmd| matches!(
        cmd,
        Ok(Command::ExtendedCode(ExtendedCode::LoadPolarity(_)))
            | Ok(Command::ExtendedCode(ExtendedCode::LoadMirroring(_)))
            | Ok(Command::ExtendedCode(ExtendedCode::LoadScaling(_)))
            | Ok(Command::ExtendedCode(ExtendedCode::LoadRotation(_)))
    ));

    // then
    assert_eq_commands!(
        filtered_commands,
        vec![
            Ok(Command::ExtendedCode(ExtendedCode::LoadPolarity(
                Polarity::Dark
            ))),
            Ok(Command::ExtendedCode(ExtendedCode::LoadPolarity(
                Polarity::Clear
            ))),
            Ok(Command::ExtendedCode(ExtendedCode::LoadMirroring(
                Mirroring::None
            ))),
            Ok(Command::ExtendedCode(ExtendedCode::LoadMirroring(
                Mirroring::X
            ))),
            Ok(Command::ExtendedCode(ExtendedCode::LoadMirroring(
                Mirroring::Y
            ))),
            Ok(Command::ExtendedCode(ExtendedCode::LoadMirroring(
                Mirroring::XY
            ))),
            Ok(Command::ExtendedCode(ExtendedCode::LoadRotation(
                Rotation { rotation: 0.0 }
            ))),
            Ok(Command::ExtendedCode(ExtendedCode::LoadRotation(
                Rotation { rotation: 359.99 }
            ))),
            Ok(Command::ExtendedCode(ExtendedCode::LoadRotation(
                Rotation { rotation: -359.99 }
            ))),
            Ok(Command::ExtendedCode(ExtendedCode::LoadScaling(Scaling {
                scale: 0.5
            }))),
            Ok(Command::ExtendedCode(ExtendedCode::LoadScaling(Scaling {
                scale: 99.99
            }))),
        ]
    );
}

// See gerber spec 2021-02, section 4.5
#[test]
fn test_load_scaling_zero() {
    // given
    let reader = gerber_to_reader(
        r#"
    %LS0*%
    "#,
    );
    // when
    let doc = parse(reader).unwrap();
    dump_commands(&doc.commands);

    // then
    let errors = doc.into_errors();

    assert!(matches!(errors.first().unwrap(),
        GerberParserErrorWithContext {
            error: ContentError::InvalidParameter {
                parameter,
            },
            line: Some((number, content)),
        } if parameter.eq("0") && *number == 2 && content.eq("%LS0*%")
    ));
}

/// Test Step and Repeat command (%SR*%)
#[test]
fn step_and_repeat() {
    // given
    logging_init();

    let reader = gerber_to_reader(
        "
    %FSLAX23Y23*%
    %MOMM*%

    %ADD999C, 0.01*%
    D999*

    %SRX12Y6I3.33J8.120*%
    X4000Y5000D01*
    X0Y0D01*
    X-1000Y-30000D01*
    %SR*%

    M02*        
    ",
    );

    // when
    parse_and_filter!(reader, commands, filtered_commands, |cmd| matches!(
        cmd,
        Ok(Command::ExtendedCode(ExtendedCode::StepAndRepeat(_)))
    ));

    // then
    assert_eq!(
        filtered_commands,
        vec![
            Ok(Command::ExtendedCode(ExtendedCode::StepAndRepeat(
                StepAndRepeat::Open {
                    repeat_x: 12,
                    repeat_y: 6,
                    distance_x: 3.33,
                    distance_y: 8.12,
                }
            ))),
            Ok(Command::ExtendedCode(ExtendedCode::StepAndRepeat(
                StepAndRepeat::Close
            ))),
        ]
    )
}

#[test]
fn aperture_definitions() {
    // given
    logging_init();

    let reader = gerber_to_reader(
        "
    %FSLAX26Y26*%
    %MOMM*%

    G04 Aperture Definitions*
    %ADD999C, 0.01*%
    %ADD22R, 0.01X0.15*%
    %ADD23O, 0.01X0.15*%
    %ADD21P, 0.7X10*%
    %ADD24P, 0.7X10X16.5*%

    G04 Apertures with holes*
    %ADD123C, 0.01X0.003*%
    %ADD124R, 0.1X0.15X0.00001*%
    %ADD125O, 0.1X0.15X0.019*%
    %ADD126P, 1X7X5.5X0.7*%

    M02*        
    ",
    );

    // when
    let result = parse(reader);
    let doc = result.unwrap();
    println!("{:?}", doc.commands);
    let aperture_definitions = doc.apertures;

    // then
    assert_eq!(
        aperture_definitions,
        HashMap::from([
            (
                999,
                Aperture::Circle(Circle {
                    diameter: 0.01,
                    hole_diameter: None
                })
            ),
            (
                22,
                Aperture::Rectangle(Rectangular {
                    x: 0.01,
                    y: 0.15,
                    hole_diameter: None
                })
            ),
            (
                23,
                Aperture::Obround(Rectangular {
                    x: 0.01,
                    y: 0.15,
                    hole_diameter: None
                })
            ),
            (
                21,
                Aperture::Polygon(Polygon {
                    diameter: 0.7,
                    vertices: 10,
                    rotation: None,
                    hole_diameter: None
                })
            ),
            (
                24,
                Aperture::Polygon(Polygon {
                    diameter: 0.7,
                    vertices: 10,
                    rotation: Some(16.5),
                    hole_diameter: None
                })
            ),
            (
                123,
                Aperture::Circle(Circle {
                    diameter: 0.01,
                    hole_diameter: Some(0.003)
                })
            ),
            (
                124,
                Aperture::Rectangle(Rectangular {
                    x: 0.1,
                    y: 0.15,
                    hole_diameter: Some(0.00001)
                })
            ),
            (
                125,
                Aperture::Obround(Rectangular {
                    x: 0.1,
                    y: 0.15,
                    hole_diameter: Some(0.019)
                })
            ),
            (
                126,
                Aperture::Polygon(Polygon {
                    diameter: 1.0,
                    vertices: 7,
                    rotation: Some(5.5),
                    hole_diameter: Some(0.7)
                })
            ),
        ])
    )
}

// 2024.05 - 5.6.10 ".AperFunction"
#[test]
#[allow(non_snake_case)]
fn TA_aperture_attributes() {
    // given
    logging_init();

    let reader = gerber_to_reader(
        r#"
    %FSLAX23Y23*%
    %MOMM*%

    G04 "Drill and rout layers"*

    %TA.AperFunction,ViaDrill*%
    %TA.AperFunction,ViaDrill,Ia*%
    %TA.AperFunction,ViaDrill,Ib*%
    %TA.AperFunction,ViaDrill,IIa*%
    %TA.AperFunction,ViaDrill,IIb*%
    %TA.AperFunction,ViaDrill,IIIa*%
    %TA.AperFunction,ViaDrill,IIIb*%
    %TA.AperFunction,ViaDrill,IVa*%
    %TA.AperFunction,ViaDrill,IVb*%
    %TA.AperFunction,ViaDrill,V*%
    %TA.AperFunction,ViaDrill,VI*%
    %TA.AperFunction,ViaDrill,VII*%
    %TA.AperFunction,ViaDrill,None*%

    %TA.AperFunction,BackDrill*%

    %TA.AperFunction,ComponentDrill*%
    %TA.AperFunction,ComponentDrill,PressFit*%

    %TA.AperFunction,MechanicalDrill*%
    %TA.AperFunction,MechanicalDrill,Tooling*%
    %TA.AperFunction,MechanicalDrill,Breakout*%
    %TA.AperFunction,MechanicalDrill,Other*%

    %TA.AperFunction,CastellatedDrill*%

    %TA.AperFunction,OtherDrill*%
    %TA.AperFunction,OtherDrill,Value 1*%

    G04 "Copper layers"*

    %TA.AperFunction,ComponentPad*%

    %TA.AperFunction,SMDPad,CuDef*%
    %TA.AperFunction,SMDPad,SMDef*%

    %TA.AperFunction,BGAPad,CuDef*%
    %TA.AperFunction,BGAPad,SMDef*%

    %TA.AperFunction,ConnectorPad*%

    %TA.AperFunction,HeatsinkPad*%

    %TA.AperFunction,ViaPad*%

    %TA.AperFunction,TestPad*%

    %TA.AperFunction,CastellatedPad*%

    %TA.AperFunction,FiducialPad,Local*%
    %TA.AperFunction,FiducialPad,Global*%
    %TA.AperFunction,FiducialPad,Panel*%

    %TA.AperFunction,ThermalReliefPad*%

    %TA.AperFunction,WasherPad*%

    %TA.AperFunction,AntiPad*%

    %TA.AperFunction,OtherPad,Value 1*%

    %TA.AperFunction,Conductor*%

    %TA.AperFunction,EtchedComponent*%

    %TA.AperFunction,NonConductor*%

    %TA.AperFunction,CopperBalancing*%

    %TA.AperFunction,Border*%

    %TA.AperFunction,OtherCopper,Value 1*%

    G04 "Component layers"*

    %TA.AperFunction,ComponentMain*%

    %TA.AperFunction,ComponentOutline,Body*%
    %TA.AperFunction,ComponentOutline,Lead2Lead*%
    %TA.AperFunction,ComponentOutline,Footprint*%
    %TA.AperFunction,ComponentOutline,Courtyard*%

    %TA.AperFunction,ComponentPin*%

    G04 "All data layers"*

    %TA.AperFunction,Profile*%

    %TA.AperFunction,NonMaterial*%

    %TA.AperFunction,Material*%

    %TA.AperFunction,Other,Value 1*%

    G04 "Deprecated"*
    %TA.AperFunction,Slot*%
    %TA.AperFunction,CutOut*%
    %TA.AperFunction,Cavity*%
    %TA.AperFunction,Drawing*%

    M02*        
    "#,
    );

    macro_rules! af_without_args {
        ($name:ident) => {
            vec![Ok(Command::ExtendedCode(ExtendedCode::ApertureAttribute(
                ApertureAttribute::ApertureFunction(ApertureFunction::$name),
            )))]
        };
    }

    macro_rules! af_with_ipc4761 {
        ($name:ident) => {{
            let mut result = vec![ApertureFunction::$name(None)];
            result.extend(
                <IPC4761ViaProtection as VariantArray>::VARIANTS
                    .iter()
                    .cloned()
                    .map(|value| ApertureFunction::$name(Some(value)))
                    .collect::<Vec<_>>(),
            );

            result
                .into_iter()
                .map(|it| {
                    Ok(Command::ExtendedCode(ExtendedCode::ApertureAttribute(
                        ApertureAttribute::ApertureFunction(it),
                    )))
                })
                .collect::<Vec<_>>()
        }};
    }

    // when
    parse_and_filter!(reader, commands, filtered_commands, |cmd| matches!(
        cmd,
        Ok(Command::ExtendedCode(ExtendedCode::ApertureAttribute(_)))
    ));

    fn to_af_commands(
        afs: Vec<ApertureFunction>,
    ) -> Vec<Result<Command, GerberParserErrorWithContext>> {
        afs.into_iter()
            .map(|it| {
                Ok(Command::ExtendedCode(ExtendedCode::ApertureAttribute(
                    ApertureAttribute::ApertureFunction(it),
                )))
            })
            .collect()
    }

    let mut expected_commands: Vec<_> = vec![];

    //
    // "Drill and rout layers"
    //
    expected_commands.extend(af_with_ipc4761!(ViaDrill));
    expected_commands.extend(af_without_args!(BackDrill));
    expected_commands.extend(to_af_commands(vec![
        ApertureFunction::ComponentDrill { function: None },
        ApertureFunction::ComponentDrill {
            function: Some(ComponentDrill::PressFit),
        },
    ]));
    expected_commands.extend(to_af_commands(vec![
        ApertureFunction::MechanicalDrill { function: None },
        ApertureFunction::MechanicalDrill {
            function: Some(DrillFunction::Tooling),
        },
        ApertureFunction::MechanicalDrill {
            function: Some(DrillFunction::BreakOut),
        },
        ApertureFunction::MechanicalDrill {
            function: Some(DrillFunction::Other),
        },
    ]));
    expected_commands.extend(af_without_args!(CastellatedDrill));
    expected_commands.extend(to_af_commands(vec![ApertureFunction::OtherDrill(
        "Value 1".to_string(),
    )]));

    //
    // "Copper layers"
    //
    expected_commands.extend(af_without_args!(ComponentPad));
    expected_commands.extend(to_af_commands(vec![
        ApertureFunction::SmdPad(SmdPadType::CopperDefined),
        ApertureFunction::SmdPad(SmdPadType::SoldermaskDefined),
    ]));
    expected_commands.extend(to_af_commands(vec![
        ApertureFunction::BgaPad(SmdPadType::CopperDefined),
        ApertureFunction::BgaPad(SmdPadType::SoldermaskDefined),
    ]));
    expected_commands.extend(af_without_args!(ConnectorPad));
    expected_commands.extend(af_without_args!(HeatsinkPad));
    expected_commands.extend(af_without_args!(ViaPad));
    expected_commands.extend(af_without_args!(TestPad));
    expected_commands.extend(af_without_args!(CastellatedPad));
    expected_commands.extend(to_af_commands(vec![
        ApertureFunction::FiducialPad(FiducialScope::Local),
        ApertureFunction::FiducialPad(FiducialScope::Global),
        ApertureFunction::FiducialPad(FiducialScope::Panel),
    ]));
    expected_commands.extend(af_without_args!(ThermalReliefPad));
    expected_commands.extend(af_without_args!(WasherPad));
    expected_commands.extend(af_without_args!(AntiPad));
    expected_commands.extend(to_af_commands(vec![ApertureFunction::OtherPad(
        "Value 1".to_string(),
    )]));
    expected_commands.extend(af_without_args!(Conductor));
    expected_commands.extend(af_without_args!(EtchedComponent));
    expected_commands.extend(af_without_args!(NonConductor));
    expected_commands.extend(af_without_args!(CopperBalancing));
    expected_commands.extend(af_without_args!(Border));
    expected_commands.extend(to_af_commands(vec![ApertureFunction::OtherCopper(
        "Value 1".to_string(),
    )]));

    //
    // "Component layers"
    //
    expected_commands.extend(af_without_args!(ComponentMain));
    expected_commands.extend(to_af_commands(vec![
        ApertureFunction::ComponentOutline(ComponentOutline::Body),
        ApertureFunction::ComponentOutline(ComponentOutline::Lead2Lead),
        ApertureFunction::ComponentOutline(ComponentOutline::Footprint),
        ApertureFunction::ComponentOutline(ComponentOutline::Courtyard),
    ]));
    expected_commands.extend(af_without_args!(ComponentPin));

    //
    // "All data layers"
    //
    expected_commands.extend(af_without_args!(Profile));
    expected_commands.extend(af_without_args!(NonMaterial));
    expected_commands.extend(af_without_args!(Material));
    expected_commands.extend(to_af_commands(vec![ApertureFunction::Other(
        "Value 1".to_string(),
    )]));

    //
    // "Deprecated"
    //
    expected_commands.extend(af_without_args!(Slot));
    expected_commands.extend(af_without_args!(CutOut));
    expected_commands.extend(af_without_args!(Cavity));
    expected_commands.extend(af_without_args!(Drawing));

    // then
    assert_eq!(filtered_commands, expected_commands)
}

#[test]
#[allow(non_snake_case)]
fn TA_custom_attributes() {
    // given
    logging_init();

    let reader = gerber_to_reader(
        "
    %FSLAX23Y23*%
    %MOMM*%

    %TANonStandardAttribute  ,  Value 1  ,  Value 2  *%
    %TA.UnsupportedStandardAttribute,Value A,Value B*%

    M02*
    ",
    );

    // Note: leading and trailing spaces are trimmed

    // when
    parse_and_filter!(reader, commands, filtered_commands, |cmd| matches!(
        cmd,
        Ok(Command::ExtendedCode(ExtendedCode::ApertureAttribute(_)))
    ));

    // then
    assert_eq!(
        filtered_commands,
        vec![
            Ok(Command::ExtendedCode(ExtendedCode::ApertureAttribute(
                ApertureAttribute::UserDefined {
                    name: "NonStandardAttribute".to_string(),
                    values: vec!["Value 1".to_string(), "Value 2".to_string(),],
                }
            ))),
            Ok(Command::ExtendedCode(ExtendedCode::ApertureAttribute(
                ApertureAttribute::UserDefined {
                    name: ".UnsupportedStandardAttribute".to_string(),
                    values: vec!["Value A".to_string(), "Value B".to_string(),],
                }
            ))),
        ]
    )
}

#[test]
#[allow(non_snake_case)]
fn TF_custom_attributes() {
    // given
    logging_init();

    let reader = gerber_to_reader(
        "
    %FSLAX23Y23*%
    %MOMM*%

    %TFNonStandardAttribute  ,  Value 1  ,  Value 2  *%
    %TF.UnsupportedStandardAttribute,Value A,Value B*%

    M02*
    ",
    );

    // Note: leading and trailing spaces are trimmed

    // when
    parse_and_filter!(reader, commands, filtered_commands, |cmd| matches!(
        cmd,
        Ok(Command::ExtendedCode(ExtendedCode::FileAttribute(_)))
    ));

    // then
    assert_eq!(
        filtered_commands,
        vec![
            Ok(Command::ExtendedCode(ExtendedCode::FileAttribute(
                FileAttribute::UserDefined {
                    name: "NonStandardAttribute".to_string(),
                    values: vec!["Value 1".to_string(), "Value 2".to_string(),],
                }
            ))),
            Ok(Command::ExtendedCode(ExtendedCode::FileAttribute(
                FileAttribute::UserDefined {
                    name: ".UnsupportedStandardAttribute".to_string(),
                    values: vec!["Value A".to_string(), "Value B".to_string(),],
                }
            ))),
        ]
    )
}

#[test]
#[allow(non_snake_case)]
fn TO_custom_attributes() {
    // given
    logging_init();

    let reader = gerber_to_reader(
        "
    %FSLAX23Y23*%
    %MOMM*%

    %TONonStandardAttribute  ,  Value 1  ,  Value 2  *%
    %TO.UnsupportedStandardAttribute,Value A,Value B*%

    M02*
    ",
    );

    // Note: leading and trailing spaces are trimmed

    // when
    parse_and_filter!(reader, commands, filtered_commands, |cmd| matches!(
        cmd,
        Ok(Command::ExtendedCode(ExtendedCode::ObjectAttribute(_)))
    ));

    // then
    assert_eq!(
        filtered_commands,
        vec![
            Ok(Command::ExtendedCode(ExtendedCode::ObjectAttribute(
                ObjectAttribute::UserDefined {
                    name: "NonStandardAttribute".to_string(),
                    values: vec!["Value 1".to_string(), "Value 2".to_string(),],
                }
            ))),
            Ok(Command::ExtendedCode(ExtendedCode::ObjectAttribute(
                ObjectAttribute::UserDefined {
                    name: ".UnsupportedStandardAttribute".to_string(),
                    values: vec!["Value A".to_string(), "Value B".to_string(),],
                }
            ))),
        ]
    )
}

#[test]
#[allow(non_snake_case)]
fn TO_attributes() {
    // given
    logging_init();

    let reader = gerber_to_reader(
        "
    %FSLAX23Y23*%
    %MOMM*%

    %TO.N,*%
    %TO.N,N/C*%
    %TO.N,Net1*%
    %TO.N,Net1,Net2,Net3*%

    %TO.P,U1,1*%
    %TO.P,U1,EP,Thermal pad*%

    %TO.C,R1*%

    %TO.CRot,359.99*%
    %TO.CMfr,AVX*%
    %TO.CMPN,42AA69*%
    %TO.CVal,10uF 6.3V*%
    %TO.CMnt,TH*%
    %TO.CMnt,SMD*%
    %TO.CMnt,Pressfit*%
    %TO.CMnt,Other*%
    %TO.CPgN,AVX_F98_CASE_M*%
    %TO.CPgD,10uF 6.3V 0603 TANTALUM*%
    %TO.CHgt,6.9*%
    %TO.CLbN,Library Name*%
    %TO.CLbD,Library Description*%

    M02*
    ",
    );

    // Note: leading and trailing spaces are trimmed

    // when
    parse_and_filter!(reader, commands, filtered_commands, |cmd| matches!(
        cmd,
        Ok(Command::ExtendedCode(ExtendedCode::ObjectAttribute(_)))
    ));

    // then
    assert_eq!(
        filtered_commands,
        vec![
            Ok(Command::ExtendedCode(ExtendedCode::ObjectAttribute(
                ObjectAttribute::Net(Net::None)
            ))),
            Ok(Command::ExtendedCode(ExtendedCode::ObjectAttribute(
                ObjectAttribute::Net(Net::NotConnected)
            ))),
            Ok(Command::ExtendedCode(ExtendedCode::ObjectAttribute(
                ObjectAttribute::Net(Net::Connected(vec!["Net1".to_string()]))
            ))),
            Ok(Command::ExtendedCode(ExtendedCode::ObjectAttribute(
                ObjectAttribute::Net(Net::Connected(vec![
                    "Net1".to_string(),
                    "Net2".to_string(),
                    "Net3".to_string()
                ]))
            ))),
            Ok(Command::ExtendedCode(ExtendedCode::ObjectAttribute(
                ObjectAttribute::Pin(Pin {
                    refdes: "U1".to_string(),
                    name: "1".to_string(),
                    function: None,
                })
            ))),
            Ok(Command::ExtendedCode(ExtendedCode::ObjectAttribute(
                ObjectAttribute::Pin(Pin {
                    refdes: "U1".to_string(),
                    name: "EP".to_string(),
                    function: Some("Thermal pad".to_string()),
                })
            ))),
            Ok(Command::ExtendedCode(ExtendedCode::ObjectAttribute(
                ObjectAttribute::Component("R1".to_string())
            ))),
            Ok(Command::ExtendedCode(ExtendedCode::ObjectAttribute(
                ObjectAttribute::ComponentCharacteristics(ComponentCharacteristics::Rotation(
                    359.99
                ))
            ))),
            Ok(Command::ExtendedCode(ExtendedCode::ObjectAttribute(
                ObjectAttribute::ComponentCharacteristics(ComponentCharacteristics::Manufacturer(
                    "AVX".to_string()
                ))
            ))),
            Ok(Command::ExtendedCode(ExtendedCode::ObjectAttribute(
                ObjectAttribute::ComponentCharacteristics(ComponentCharacteristics::MPN(
                    "42AA69".to_string()
                ))
            ))),
            Ok(Command::ExtendedCode(ExtendedCode::ObjectAttribute(
                ObjectAttribute::ComponentCharacteristics(ComponentCharacteristics::Value(
                    "10uF 6.3V".to_string()
                ))
            ))),
            Ok(Command::ExtendedCode(ExtendedCode::ObjectAttribute(
                ObjectAttribute::ComponentCharacteristics(ComponentCharacteristics::Mount(
                    ComponentMounting::ThroughHole
                ))
            ))),
            Ok(Command::ExtendedCode(ExtendedCode::ObjectAttribute(
                ObjectAttribute::ComponentCharacteristics(ComponentCharacteristics::Mount(
                    ComponentMounting::SMD
                ))
            ))),
            Ok(Command::ExtendedCode(ExtendedCode::ObjectAttribute(
                ObjectAttribute::ComponentCharacteristics(ComponentCharacteristics::Mount(
                    ComponentMounting::PressFit
                ))
            ))),
            Ok(Command::ExtendedCode(ExtendedCode::ObjectAttribute(
                ObjectAttribute::ComponentCharacteristics(ComponentCharacteristics::Mount(
                    ComponentMounting::Other
                ))
            ))),
            Ok(Command::ExtendedCode(ExtendedCode::ObjectAttribute(
                ObjectAttribute::ComponentCharacteristics(ComponentCharacteristics::PackageName(
                    "AVX_F98_CASE_M".to_string()
                ))
            ))),
            Ok(Command::ExtendedCode(ExtendedCode::ObjectAttribute(
                ObjectAttribute::ComponentCharacteristics(
                    ComponentCharacteristics::PackageDescription(
                        "10uF 6.3V 0603 TANTALUM".to_string()
                    )
                )
            ))),
            Ok(Command::ExtendedCode(ExtendedCode::ObjectAttribute(
                ObjectAttribute::ComponentCharacteristics(ComponentCharacteristics::Height(6.9))
            ))),
            Ok(Command::ExtendedCode(ExtendedCode::ObjectAttribute(
                ObjectAttribute::ComponentCharacteristics(ComponentCharacteristics::LibraryName(
                    "Library Name".to_string()
                ))
            ))),
            Ok(Command::ExtendedCode(ExtendedCode::ObjectAttribute(
                ObjectAttribute::ComponentCharacteristics(
                    ComponentCharacteristics::LibraryDescription("Library Description".to_string())
                )
            ))),
        ]
    )
}

#[test]
#[allow(non_snake_case)]
fn TF_file_attributes() {
    logging_init();

    let reader = gerber_to_reader(
        "
    %FSLAX23Y23*%
    %MOMM*%

    %ADD999C, 0.01*%

    G04 Part*

    %TF.Part,Single*%
    %TF.Part,Array*%
    %TF.Part,FabricationPanel*%
    %TF.Part,Coupon*%
    %TF.Part,Other,Value 1*%

    G04 Data Layers*

    %TF.FileFunction,Copper,L1,Top*%
    %TF.FileFunction,Copper,L2,Inr*%
    %TF.FileFunction,Copper,L3,Inr*%
    %TF.FileFunction,Copper,L4,Bot*%
    %TF.FileFunction,Copper,L1,Top,Signal*%
    %TF.FileFunction,Copper,L2,Inr,Plane*%
    %TF.FileFunction,Copper,L3,Inr,Mixed*%
    %TF.FileFunction,Copper,L4,Bot,Hatched*%

    %TF.FileFunction,Plated,1,2,PTH*%
    %TF.FileFunction,Plated,1,6,PTH,Drill*%
    %TF.FileFunction,Plated,1,6,PTH,Rout*%
    %TF.FileFunction,Plated,1,6,PTH,Mixed*%
    %TF.FileFunction,Plated,1,2,Blind,Drill*%
    %TF.FileFunction,Plated,3,4,Buried,Drill*%

    %TF.FileFunction,NonPlated,1,2,NPTH*%
    %TF.FileFunction,NonPlated,1,6,NPTH,Drill*%
    %TF.FileFunction,NonPlated,1,6,NPTH,Rout*%
    %TF.FileFunction,NonPlated,1,6,NPTH,Mixed*%
    %TF.FileFunction,NonPlated,1,2,Blind,Drill*%
    %TF.FileFunction,NonPlated,3,4,Buried,Drill*%

    G04 Profile without a `P` or `NP` is off-spec, but this what DipTrace 4.3.0.6 generates*
    %TF.FileFunction,Profile*%
    %TF.FileFunction,Profile,P*%
    %TF.FileFunction,Profile,NP*%

    %TF.FileFunction,Soldermask,Top*%
    %TF.FileFunction,Soldermask,Bot*%
    %TF.FileFunction,Soldermask,Top,1*%
    %TF.FileFunction,Soldermask,Bot,2*%

    %TF.FileFunction,Legend,Top*%
    %TF.FileFunction,Legend,Bot*%
    %TF.FileFunction,Legend,Top,1*%
    %TF.FileFunction,Legend,Bot,2*%

    %TF.FileFunction,Component,L1,Top*%
    %TF.FileFunction,Component,L2,Bot*%

    %TF.FileFunction,Paste,Top*%
    %TF.FileFunction,Paste,Bot*%

    %TF.FileFunction,Glue,Top*%
    %TF.FileFunction,Glue,Bot*%

    %TF.FileFunction,Carbonmask,Top*%
    %TF.FileFunction,Carbonmask,Bot*%
    %TF.FileFunction,Carbonmask,Top,1*%
    %TF.FileFunction,Carbonmask,Bot,2*%

    %TF.FileFunction,Goldmask,Top*%
    %TF.FileFunction,Goldmask,Bot*%
    %TF.FileFunction,Goldmask,Top,1*%
    %TF.FileFunction,Goldmask,Bot,2*%

    %TF.FileFunction,Heatsinkmask,Top*%
    %TF.FileFunction,Heatsinkmask,Bot*%
    %TF.FileFunction,Heatsinkmask,Top,1*%
    %TF.FileFunction,Heatsinkmask,Bot,2*%

    %TF.FileFunction,Peelablemask,Top*%
    %TF.FileFunction,Peelablemask,Bot*%
    %TF.FileFunction,Peelablemask,Top,1*%
    %TF.FileFunction,Peelablemask,Bot,2*%

    %TF.FileFunction,Silvermask,Top*%
    %TF.FileFunction,Silvermask,Bot*%
    %TF.FileFunction,Silvermask,Top,1*%
    %TF.FileFunction,Silvermask,Bot,2*%

    %TF.FileFunction,Tinmask,Top*%
    %TF.FileFunction,Tinmask,Bot*%
    %TF.FileFunction,Tinmask,Top,1*%
    %TF.FileFunction,Tinmask,Bot,2*%

    %TF.FileFunction,Depthrout,Top*%
    %TF.FileFunction,Depthrout,Bot*%

    %TF.FileFunction,Vcut*%
    %TF.FileFunction,Vcut,Top*%
    %TF.FileFunction,Vcut,Bot*%

    %TF.FileFunction,Viafill*%

    %TF.FileFunction,Pads,Top*%
    %TF.FileFunction,Pads,Bot*%

    %TF.FileFunction,Other,Value 1*%

    G04 Drawing layers*

    %TF.FileFunction,Drillmap*%
    %TF.FileFunction,FabricationDrawing*%
    %TF.FileFunction,Vcutmap*%
    %TF.FileFunction,AssemblyDrawing,Top*%
    %TF.FileFunction,AssemblyDrawing,Bot*%
    %TF.FileFunction,ArrayDrawing*%
    %TF.FileFunction,OtherDrawing,Value 1*%

    G04 File Polarity*

    %TF.FilePolarity,Positive*%
    %TF.FilePolarity,Negative*%

    G04 Same Coordinates*

    %TF.SameCoordinates*%
    %TF.SameCoordinates,ident*%
    %TF.SameCoordinates,ffffffff-ffff-ffff-ffff-ffffffffffff*%

    G04 Creation Date*

    %TF.CreationDate,2015-02-23T15:59:51+01:00*%

    G04 Generation software*

    %TF.GenerationSoftware,MakerPnP,gerber-types*%
    %TF.GenerationSoftware,MakerPnP,gerber-types,0.4.0*%

    G04 Project Id.*

    %TF.ProjectId,My PCB,ffffffff-ffff-ffff-ffff-ffffffffffff,2.0*%

    G04 MD5*

    %TF.MD5,6ab9e892830469cdff7e3e346331d404*%

    M02*
    ",
    );

    macro_rules! ff_with_side_and_optional_index {
        ($name:ident) => {
            vec![
                Ok(Command::ExtendedCode(ExtendedCode::FileAttribute(
                    FileAttribute::FileFunction(FileFunction::$name {
                        pos: Position::Top,
                        index: None,
                    }),
                ))),
                Ok(Command::ExtendedCode(ExtendedCode::FileAttribute(
                    FileAttribute::FileFunction(FileFunction::$name {
                        pos: Position::Bottom,
                        index: None,
                    }),
                ))),
                Ok(Command::ExtendedCode(ExtendedCode::FileAttribute(
                    FileAttribute::FileFunction(FileFunction::$name {
                        pos: Position::Top,
                        index: Some(1),
                    }),
                ))),
                Ok(Command::ExtendedCode(ExtendedCode::FileAttribute(
                    FileAttribute::FileFunction(FileFunction::$name {
                        pos: Position::Bottom,
                        index: Some(2),
                    }),
                ))),
            ]
        };
    }

    macro_rules! ff_with_side_and_layer {
        ($name:ident) => {
            vec![
                Ok(Command::ExtendedCode(ExtendedCode::FileAttribute(
                    FileAttribute::FileFunction(FileFunction::$name {
                        pos: Position::Top,
                        layer: 1,
                    }),
                ))),
                Ok(Command::ExtendedCode(ExtendedCode::FileAttribute(
                    FileAttribute::FileFunction(FileFunction::$name {
                        pos: Position::Bottom,
                        layer: 2,
                    }),
                ))),
            ]
        };
    }

    macro_rules! ff_with_side {
        ($name:ident) => {
            vec![
                Ok(Command::ExtendedCode(ExtendedCode::FileAttribute(
                    FileAttribute::FileFunction(FileFunction::$name(Position::Top)),
                ))),
                Ok(Command::ExtendedCode(ExtendedCode::FileAttribute(
                    FileAttribute::FileFunction(FileFunction::$name(Position::Bottom)),
                ))),
            ]
        };
    }

    macro_rules! ff_with_optional_side {
        ($name:ident) => {
            vec![
                Ok(Command::ExtendedCode(ExtendedCode::FileAttribute(
                    FileAttribute::FileFunction(FileFunction::$name(None)),
                ))),
                Ok(Command::ExtendedCode(ExtendedCode::FileAttribute(
                    FileAttribute::FileFunction(FileFunction::$name(Some(Position::Top))),
                ))),
                Ok(Command::ExtendedCode(ExtendedCode::FileAttribute(
                    FileAttribute::FileFunction(FileFunction::$name(Some(Position::Bottom))),
                ))),
            ]
        };
    }

    macro_rules! ff_without_args {
        ($name:ident) => {
            vec![Ok(Command::ExtendedCode(ExtendedCode::FileAttribute(
                FileAttribute::FileFunction(FileFunction::$name),
            )))]
        };
    }

    macro_rules! ff_with_string {
        ($name:ident, $str:expr) => {
            vec![Ok(Command::ExtendedCode(ExtendedCode::FileAttribute(
                FileAttribute::FileFunction(FileFunction::$name($str.to_string())),
            )))]
        };
    }

    // and
    let mut expected_commands = vec![
        // Part
        Ok(Command::ExtendedCode(ExtendedCode::FileAttribute(
            FileAttribute::Part(Part::Single),
        ))),
        Ok(Command::ExtendedCode(ExtendedCode::FileAttribute(
            FileAttribute::Part(Part::Array),
        ))),
        Ok(Command::ExtendedCode(ExtendedCode::FileAttribute(
            FileAttribute::Part(Part::FabricationPanel),
        ))),
        Ok(Command::ExtendedCode(ExtendedCode::FileAttribute(
            FileAttribute::Part(Part::Coupon),
        ))),
        Ok(Command::ExtendedCode(ExtendedCode::FileAttribute(
            FileAttribute::Part(Part::Other("Value 1".to_string())),
        ))),
        // File Function
        Ok(Command::ExtendedCode(ExtendedCode::FileAttribute(
            FileAttribute::FileFunction(FileFunction::Copper {
                layer: 1,
                pos: ExtendedPosition::Top,
                copper_type: None,
            }),
        ))),
        Ok(Command::ExtendedCode(ExtendedCode::FileAttribute(
            FileAttribute::FileFunction(FileFunction::Copper {
                layer: 2,
                pos: ExtendedPosition::Inner,
                copper_type: None,
            }),
        ))),
        Ok(Command::ExtendedCode(ExtendedCode::FileAttribute(
            FileAttribute::FileFunction(FileFunction::Copper {
                layer: 3,
                pos: ExtendedPosition::Inner,
                copper_type: None,
            }),
        ))),
        Ok(Command::ExtendedCode(ExtendedCode::FileAttribute(
            FileAttribute::FileFunction(FileFunction::Copper {
                layer: 4,
                pos: ExtendedPosition::Bottom,
                copper_type: None,
            }),
        ))),
        Ok(Command::ExtendedCode(ExtendedCode::FileAttribute(
            FileAttribute::FileFunction(FileFunction::Copper {
                layer: 1,
                pos: ExtendedPosition::Top,
                copper_type: Some(CopperType::Signal),
            }),
        ))),
        Ok(Command::ExtendedCode(ExtendedCode::FileAttribute(
            FileAttribute::FileFunction(FileFunction::Copper {
                layer: 2,
                pos: ExtendedPosition::Inner,
                copper_type: Some(CopperType::Plane),
            }),
        ))),
        Ok(Command::ExtendedCode(ExtendedCode::FileAttribute(
            FileAttribute::FileFunction(FileFunction::Copper {
                layer: 3,
                pos: ExtendedPosition::Inner,
                copper_type: Some(CopperType::Mixed),
            }),
        ))),
        Ok(Command::ExtendedCode(ExtendedCode::FileAttribute(
            FileAttribute::FileFunction(FileFunction::Copper {
                layer: 4,
                pos: ExtendedPosition::Bottom,
                copper_type: Some(CopperType::Hatched),
            }),
        ))),
        // Plated
        Ok(Command::ExtendedCode(ExtendedCode::FileAttribute(
            FileAttribute::FileFunction(FileFunction::Plated {
                from_layer: 1,
                to_layer: 2,
                drill: PlatedDrill::PlatedThroughHole,
                label: None,
            }),
        ))),
        Ok(Command::ExtendedCode(ExtendedCode::FileAttribute(
            FileAttribute::FileFunction(FileFunction::Plated {
                from_layer: 1,
                to_layer: 6,
                drill: PlatedDrill::PlatedThroughHole,
                label: Some(DrillRouteType::Drill),
            }),
        ))),
        Ok(Command::ExtendedCode(ExtendedCode::FileAttribute(
            FileAttribute::FileFunction(FileFunction::Plated {
                from_layer: 1,
                to_layer: 6,
                drill: PlatedDrill::PlatedThroughHole,
                label: Some(DrillRouteType::Route),
            }),
        ))),
        Ok(Command::ExtendedCode(ExtendedCode::FileAttribute(
            FileAttribute::FileFunction(FileFunction::Plated {
                from_layer: 1,
                to_layer: 6,
                drill: PlatedDrill::PlatedThroughHole,
                label: Some(DrillRouteType::Mixed),
            }),
        ))),
        Ok(Command::ExtendedCode(ExtendedCode::FileAttribute(
            FileAttribute::FileFunction(FileFunction::Plated {
                from_layer: 1,
                to_layer: 2,
                drill: PlatedDrill::Blind,
                label: Some(DrillRouteType::Drill),
            }),
        ))),
        Ok(Command::ExtendedCode(ExtendedCode::FileAttribute(
            FileAttribute::FileFunction(FileFunction::Plated {
                from_layer: 3,
                to_layer: 4,
                drill: PlatedDrill::Buried,
                label: Some(DrillRouteType::Drill),
            }),
        ))),
        // Non-Plated
        Ok(Command::ExtendedCode(ExtendedCode::FileAttribute(
            FileAttribute::FileFunction(FileFunction::NonPlated {
                from_layer: 1,
                to_layer: 2,
                drill: NonPlatedDrill::NonPlatedThroughHole,
                label: None,
            }),
        ))),
        Ok(Command::ExtendedCode(ExtendedCode::FileAttribute(
            FileAttribute::FileFunction(FileFunction::NonPlated {
                from_layer: 1,
                to_layer: 6,
                drill: NonPlatedDrill::NonPlatedThroughHole,
                label: Some(DrillRouteType::Drill),
            }),
        ))),
        Ok(Command::ExtendedCode(ExtendedCode::FileAttribute(
            FileAttribute::FileFunction(FileFunction::NonPlated {
                from_layer: 1,
                to_layer: 6,
                drill: NonPlatedDrill::NonPlatedThroughHole,
                label: Some(DrillRouteType::Route),
            }),
        ))),
        Ok(Command::ExtendedCode(ExtendedCode::FileAttribute(
            FileAttribute::FileFunction(FileFunction::NonPlated {
                from_layer: 1,
                to_layer: 6,
                drill: NonPlatedDrill::NonPlatedThroughHole,
                label: Some(DrillRouteType::Mixed),
            }),
        ))),
        Ok(Command::ExtendedCode(ExtendedCode::FileAttribute(
            FileAttribute::FileFunction(FileFunction::NonPlated {
                from_layer: 1,
                to_layer: 2,
                drill: NonPlatedDrill::Blind,
                label: Some(DrillRouteType::Drill),
            }),
        ))),
        Ok(Command::ExtendedCode(ExtendedCode::FileAttribute(
            FileAttribute::FileFunction(FileFunction::NonPlated {
                from_layer: 3,
                to_layer: 4,
                drill: NonPlatedDrill::Buried,
                label: Some(DrillRouteType::Drill),
            }),
        ))),
        // Profile
        Ok(Command::ExtendedCode(ExtendedCode::FileAttribute(
            FileAttribute::FileFunction(FileFunction::Profile(None)),
        ))),
        Ok(Command::ExtendedCode(ExtendedCode::FileAttribute(
            FileAttribute::FileFunction(FileFunction::Profile(Some(Profile::Plated))),
        ))),
        Ok(Command::ExtendedCode(ExtendedCode::FileAttribute(
            FileAttribute::FileFunction(FileFunction::Profile(Some(Profile::NonPlated))),
        ))),
    ];

    expected_commands.extend(ff_with_side_and_optional_index!(SolderMask));
    expected_commands.extend(ff_with_side_and_optional_index!(Legend));
    expected_commands.extend(ff_with_side_and_layer!(Component));
    expected_commands.extend(ff_with_side!(Paste));
    expected_commands.extend(ff_with_side!(Glue));
    expected_commands.extend(ff_with_side_and_optional_index!(CarbonMask));
    expected_commands.extend(ff_with_side_and_optional_index!(GoldMask));
    expected_commands.extend(ff_with_side_and_optional_index!(HeatsinkMask));
    expected_commands.extend(ff_with_side_and_optional_index!(PeelableMask));
    expected_commands.extend(ff_with_side_and_optional_index!(SilverMask));
    expected_commands.extend(ff_with_side_and_optional_index!(TinMask));
    expected_commands.extend(ff_with_side!(DepthRoute));
    expected_commands.extend(ff_with_optional_side!(VCut));
    expected_commands.extend(ff_without_args!(ViaFill));
    expected_commands.extend(ff_with_side!(Pads));
    expected_commands.extend(ff_with_string!(Other, "Value 1"));

    expected_commands.extend(ff_without_args!(DrillMap));
    expected_commands.extend(ff_without_args!(FabricationDrawing));
    expected_commands.extend(ff_without_args!(VCutMap));
    expected_commands.extend(ff_with_side!(AssemblyDrawing));
    expected_commands.extend(ff_without_args!(ArrayDrawing));
    expected_commands.extend(ff_with_string!(OtherDrawing, "Value 1"));

    expected_commands.extend(vec![
        // Polarity
        Ok(Command::ExtendedCode(ExtendedCode::FileAttribute(
            FileAttribute::FilePolarity(FilePolarity::Positive),
        ))),
        Ok(Command::ExtendedCode(ExtendedCode::FileAttribute(
            FileAttribute::FilePolarity(FilePolarity::Negative),
        ))),
        // Same coordinates
        Ok(Command::ExtendedCode(ExtendedCode::FileAttribute(
            FileAttribute::SameCoordinates(None),
        ))),
        Ok(Command::ExtendedCode(ExtendedCode::FileAttribute(
            FileAttribute::SameCoordinates(Some(Ident::Name("ident".to_string()))),
        ))),
        Ok(Command::ExtendedCode(ExtendedCode::FileAttribute(
            FileAttribute::SameCoordinates(Some(Ident::Uuid(Uuid::max()))),
        ))),
        // Creation date
        Ok(Command::ExtendedCode(ExtendedCode::FileAttribute(
            FileAttribute::CreationDate(
                GerberDate::parse_from_rfc3339("2015-02-23T15:59:51+01:00").unwrap(),
            ),
        ))),
        // Generation software
        Ok(Command::ExtendedCode(ExtendedCode::FileAttribute(
            FileAttribute::GenerationSoftware(GenerationSoftware {
                vendor: "MakerPnP".to_string(),
                application: "gerber-types".to_string(),
                version: None,
            }),
        ))),
        Ok(Command::ExtendedCode(ExtendedCode::FileAttribute(
            FileAttribute::GenerationSoftware(GenerationSoftware {
                vendor: "MakerPnP".to_string(),
                application: "gerber-types".to_string(),
                version: Some("0.4.0".to_string()),
            }),
        ))),
        // Project Id
        Ok(Command::ExtendedCode(ExtendedCode::FileAttribute(
            FileAttribute::ProjectId {
                id: "My PCB".to_string(),
                uuid: Uuid::max(),
                revision: "2.0".to_string(),
            },
        ))),
        // Project Id
        Ok(Command::ExtendedCode(ExtendedCode::FileAttribute(
            FileAttribute::Md5("6ab9e892830469cdff7e3e346331d404".to_string()),
        ))),
    ]);
    println!("expected_commands:");
    dump_commands(&expected_commands);

    // when
    parse_and_filter!(reader, commands, filtered_commands, |cmd| matches!(
        cmd,
        Ok(Command::ExtendedCode(ExtendedCode::FileAttribute(_)))
    ));

    // then
    assert_eq!(filtered_commands, expected_commands,)
}

#[test]
#[should_panic]
fn conflicting_aperture_codes() {
    // given
    logging_init();

    let reader = gerber_to_reader(
        "
    %FSLAX23Y23*%
    %MOMM*%        
    %ADD24P, 0.7X10X16.5*%
    %ADD39P, 0.7X10X16.5*%
    G04 We cannot use the same code (24) again in the same document*
    %ADD24P, 1X10X20.0*%

    M02*      
    ",
    );
    let doc = parse(reader).unwrap();
    assert!(doc.errors().is_empty());
}

#[test]
#[should_panic]
fn missing_eof() {
    // given
    logging_init();

    let reader = gerber_to_reader(
        "
    %FSLAX23Y23*%
    %MOMM*%-

    G04 We should have a MO2 at the end, but what if we forget it?*      
    ",
    );
    let doc = parse(reader).unwrap();
    assert!(doc.errors().is_empty());
}

#[test]
#[should_panic]
fn multiple_unit_statements() {
    // given
    logging_init();

    let reader = gerber_to_reader(
        "
    %FSLAX23Y23*%
    %MOMM*%
    G04 We can only declare the unit type once in a document* 
    %MOIN*%
        
    M02*  
    ",
    );
    let doc = parse(reader).unwrap();
    assert!(doc.errors().is_empty());
}

#[test]
#[should_panic]
fn multiple_fs_statements() {
    // given
    logging_init();

    let reader = gerber_to_reader(
        "
    %FSLAX23Y23*%
    G04 We can only declare the format specification once in a document* 
    %FSLAX46Y46*%
    %MOMM*%
        
    M02*  
    ",
    );
    let doc = parse(reader).unwrap();
    assert!(doc.errors().is_empty());
}

#[test]
#[should_panic]
fn nonexistent_aperture_selection() {
    // given
    logging_init();

    let reader = gerber_to_reader(
        "
    %FSLAX23Y23*%        
    %MOMM*%

    %ADD100P, 0.7X10X16.5*%

    G04 We should not be able to select apertures that are not defined* 
    D
        
    M02*  
    ",
    );
    let doc = parse(reader).unwrap();
    assert!(doc.errors().is_empty());
}

/// This statement should fail as this is not within the format specification (2 integer, 3 decimal)
#[test]
fn coordinates_not_within_format() {
    // given
    logging_init();

    let reader = gerber_to_reader(
        "
    %FSLAX23Y23*%
    %MOMM*%

    %ADD999C, 0.01*%
    D999*

    X100000Y0D01*

    M02*        
    ",
    );
    // when
    let doc = parse(reader).unwrap();
    dump_commands(&doc.commands);

    // then
    let errors = doc.into_errors();

    assert!(matches!(errors.first().unwrap(),
        GerberParserErrorWithContext {
            error: ContentError::CoordinateFormatMismatch {
                format,
                cause: GerberError::CoordinateFormatError(_)
            },
            line: Some((number, content)),
        } if format.integer == 2 && format.decimal == 3 && *number == 8 && content.eq("X100000Y0D01*")
    ));
}

/// Test the D* statements, diptrace exports gerber files without the leading `0` on the `D0*` commands.
#[test]
#[allow(non_snake_case)]
fn diptrace_Dxx_statements() {
    // given
    logging_init();

    let reader = gerber_to_reader(
        r#"
    %TF.GenerationSoftware,Novarm,DipTrace,4.3.0.6*%
    %TF.CreationDate,2025-04-22T21:52:19+00:00*%
    %FSLAX35Y35*%
    %MOMM*%
    %TF.FileFunction,Copper,L1,Top*%
    %TF.Part,Single*%
    
    G37*
    G36*
    X2928500Y1670000D2*
    X2998500D1*
    Y1530000D1*
    G37*
    M02*

    "#,
    );

    // when
    parse_and_filter!(reader, commands, filtered_commands, |cmd| matches!(
        cmd,
        Ok(Command::FunctionCode(FunctionCode::DCode(
            DCode::Operation(Operation::Move(_))
        ))) | Ok(Command::FunctionCode(FunctionCode::DCode(
            DCode::Operation(Operation::Interpolate(_, _))
        )))
    ));

    // then
    let fs = CoordinateFormat::new(ZeroOmission::Leading, CoordinateMode::Absolute, 3, 5);

    assert_eq_commands!(
        filtered_commands,
        vec![
            Ok(Command::FunctionCode(FunctionCode::DCode(
                DCode::Operation(Operation::Move(
                    coordinates_from_gerber(2928500, 1670000, fs).unwrap()
                ))
            ))),
            Ok(Command::FunctionCode(FunctionCode::DCode(
                DCode::Operation(Operation::Interpolate(
                    partial_coordinates_from_gerber(Some(2998500), None, fs).unwrap(),
                    None
                ))
            ))),
            Ok(Command::FunctionCode(FunctionCode::DCode(
                DCode::Operation(Operation::Interpolate(
                    partial_coordinates_from_gerber(None, Some(1530000), fs).unwrap(),
                    None
                ))
            ))),
        ]
    );
}

// See gerber spec 2021-02, section 4.5
#[test]
fn test_outline_macro_and_aperture_definition() {
    // given
    let reader = gerber_to_reader(
        r#"
    %TF.GenerationSoftware,Novarm,DipTrace,4.3.0.6*%
    %TF.CreationDate,2025-04-24T12:32:15+00:00*%
    %FSLAX35Y35*%
    %MOMM*%
    %TF.FileFunction,Copper,L1,Top*%
    %TF.Part,Single*%
%AMOUTLINE0*
4,1,5,
-0.40659,0.19445,
-0.26517,0.33588,
-0.12374,0.33588,
0.40659,-0.19445,
0.19445,-0.40659,
-0.40659,0.19445,
0*%
    %ADD17OUTLINE0*%
    "#,
    );

    // Note, it's important that the macro definition isn't the only command in the file because it's important that
    // the macro parser finds the end of the macro definition correctly and that it doesn't consume additional lines.

    let expected_macro: ApertureMacro = ApertureMacro {
        name: "OUTLINE0".to_string(),
        content: vec![MacroContent::Outline(OutlinePrimitive {
            exposure: MacroBoolean::Value(true), // 1 indicates exposure on
            points: vec![
                (MacroDecimal::Value(-0.40659), MacroDecimal::Value(0.19445)),
                (MacroDecimal::Value(-0.26517), MacroDecimal::Value(0.33588)),
                (MacroDecimal::Value(-0.12374), MacroDecimal::Value(0.33588)),
                (MacroDecimal::Value(0.40659), MacroDecimal::Value(-0.19445)),
                (MacroDecimal::Value(0.19445), MacroDecimal::Value(-0.40659)),
                (MacroDecimal::Value(-0.40659), MacroDecimal::Value(0.19445)),
            ],
            angle: MacroDecimal::Value(0.0),
        })],
    };

    // when
    parse_and_filter!(reader, commands, filtered_commands, |cmd| matches!(
        cmd,
        Ok(Command::ExtendedCode(ExtendedCode::ApertureMacro(_)))
            | Ok(Command::ExtendedCode(ExtendedCode::ApertureDefinition(_)))
    ));

    // then
    assert_eq_commands!(
        filtered_commands,
        vec![
            Ok(Command::ExtendedCode(ExtendedCode::ApertureMacro(
                expected_macro
            ))),
            Ok(Command::ExtendedCode(ExtendedCode::ApertureDefinition(
                ApertureDefinition::new(17, Aperture::Macro("OUTLINE0".to_string(), None))
            ))),
        ]
    );
}

// See gerber spec 2021-02, section 4.5
#[test]
fn test_polygon_macro_and_aperture_definition() {
    // given
    let reader = gerber_to_reader(
        r#"
    %FSLAX36Y36*%
    %MOMM*%
    %AMDIAMOND*
    5,1,4,0,0,1,45*%
    %ADD10DIAMOND*%
    %LPD*%
    G75*
    G54D10*
    X0Y0D03*
    X5000000Y0D03*
    X2500000Y2500000D03*
    M02*
    "#,
    );

    // Note, it's important that the macro definition isn't the only command in the file because it's important that
    // the macro parser finds the end of the macro definition correctly and that it doesn't consume additional lines.

    let expected_macro: ApertureMacro = ApertureMacro {
        name: "DIAMOND".to_string(),
        content: vec![MacroContent::Polygon(PolygonPrimitive {
            exposure: MacroBoolean::Value(true), // 1 indicates exposure on
            vertices: MacroInteger::Value(4),    // diamond has 4 vertices
            center: (MacroDecimal::Value(0.0), MacroDecimal::Value(0.0)), // centered at origin
            diameter: MacroDecimal::Value(1.0),  // diameter of circumscribed circle
            angle: MacroDecimal::Value(45.0),    // 45 degree rotation
        })],
    };

    // when
    parse_and_filter!(reader, commands, filtered_commands, |cmd| matches!(
        cmd,
        Ok(Command::ExtendedCode(ExtendedCode::ApertureMacro(_)))
            | Ok(Command::ExtendedCode(ExtendedCode::ApertureDefinition(_)))
    ));

    // then
    assert_eq_commands!(
        filtered_commands,
        vec![
            Ok(Command::ExtendedCode(ExtendedCode::ApertureMacro(
                expected_macro
            ))),
            Ok(Command::ExtendedCode(ExtendedCode::ApertureDefinition(
                ApertureDefinition::new(10, Aperture::Macro("DIAMOND".to_string(), None))
            ))),
        ]
    );
}

// See gerber spec 2021-02, section 4.5
#[test]
fn test_thermal_macro_and_aperture_definition() {
    // given
    let reader = gerber_to_reader(
        r#"
    %FSLAX36Y36*%
    %MOMM*%
    %AMTHERMAL80*
    7,0,0.5,0.800,0.550,0.125,45*%
    %ADD19THERMAL80*%
    M02*
    "#,
    );

    // Note, it's important that the macro definition isn't the only command in the file because it's important that
    // the macro parser finds the end of the macro definition correctly and that it doesn't consume additional lines.

    let expected_macro: ApertureMacro = ApertureMacro {
        name: "THERMAL80".to_string(),
        content: vec![MacroContent::Thermal(ThermalPrimitive {
            center: (MacroDecimal::Value(0.0), MacroDecimal::Value(0.5)),
            outer_diameter: MacroDecimal::Value(0.8),
            inner_diameter: MacroDecimal::Value(0.55),
            gap: MacroDecimal::Value(0.125),
            angle: MacroDecimal::Value(45.0),
        })],
    };

    // when
    parse_and_filter!(reader, commands, filtered_commands, |cmd| matches!(
        cmd,
        Ok(Command::ExtendedCode(ExtendedCode::ApertureMacro(_)))
            | Ok(Command::ExtendedCode(ExtendedCode::ApertureDefinition(_)))
    ));

    // then
    assert_eq_commands!(
        filtered_commands,
        vec![
            Ok(Command::ExtendedCode(ExtendedCode::ApertureMacro(
                expected_macro
            ))),
            Ok(Command::ExtendedCode(ExtendedCode::ApertureDefinition(
                ApertureDefinition::new(19, Aperture::Macro("THERMAL80".to_string(), None))
            ))),
        ]
    );
}

/// Diptrace 4.3 generates operation commands without a leading `0` on the `D0*` commands.
#[test]
fn test_standalone_d_commands() {
    // given
    logging_init();

    let reader = gerber_to_reader(
        r#"
%FSLAX35Y35*%
%MOMM*%
%ADD10C,0.25*%
D10*
G01*

X10000Y10000D2*
D1*
X20000D1*
D3*
Y20000D1*
D3*
X10000D1*
D3*
Y10000D1*
D3*
M02*
"#,
    );

    // when
    parse_and_filter!(reader, commands, filtered_commands, |cmd| matches!(
        cmd,
        Ok(Command::FunctionCode(FunctionCode::DCode(
            DCode::Operation(_)
        )))
    ));

    // then
    let fs = CoordinateFormat::new(ZeroOmission::Leading, CoordinateMode::Absolute, 3, 5);

    assert_eq_commands!(
        filtered_commands,
        vec![
            // Initial move
            Ok(Command::FunctionCode(FunctionCode::DCode(
                DCode::Operation(Operation::Move(
                    coordinates_from_gerber(10000, 10000, fs).unwrap()
                ))
            ))),
            // D1 with no coordinates - uses previous position
            Ok(Command::FunctionCode(FunctionCode::DCode(
                DCode::Operation(Operation::Interpolate(
                    partial_coordinates_from_gerber(None, None, fs).unwrap(),
                    None
                ))
            ))),
            // X20000D1 - keeps previous Y coordinate
            Ok(Command::FunctionCode(FunctionCode::DCode(
                DCode::Operation(Operation::Interpolate(
                    partial_coordinates_from_gerber(Some(20000), None, fs).unwrap(),
                    None
                ))
            ))),
            // D3 with no coordinates - uses previous position
            Ok(Command::FunctionCode(FunctionCode::DCode(
                DCode::Operation(Operation::Flash(
                    partial_coordinates_from_gerber(None, None, fs).unwrap()
                ))
            ))),
            // Y20000D1 - keeps previous X coordinate
            Ok(Command::FunctionCode(FunctionCode::DCode(
                DCode::Operation(Operation::Interpolate(
                    partial_coordinates_from_gerber(None, Some(20000), fs).unwrap(),
                    None
                ))
            ))),
            // D3 with no coordinates - uses previous position
            Ok(Command::FunctionCode(FunctionCode::DCode(
                DCode::Operation(Operation::Flash(
                    partial_coordinates_from_gerber(None, None, fs).unwrap()
                ))
            ))),
            // X10000D1 - keeps previous Y coordinate
            Ok(Command::FunctionCode(FunctionCode::DCode(
                DCode::Operation(Operation::Interpolate(
                    partial_coordinates_from_gerber(Some(10000), None, fs).unwrap(),
                    None
                ))
            ))),
            // D3 with no coordinates - uses previous position
            Ok(Command::FunctionCode(FunctionCode::DCode(
                DCode::Operation(Operation::Flash(
                    partial_coordinates_from_gerber(None, None, fs).unwrap()
                ))
            ))),
            // Y10000D1 - keeps previous X coordinate
            Ok(Command::FunctionCode(FunctionCode::DCode(
                DCode::Operation(Operation::Interpolate(
                    partial_coordinates_from_gerber(None, Some(10000), fs).unwrap(),
                    None
                ))
            ))),
            // D3 with no coordinates - uses previous position
            Ok(Command::FunctionCode(FunctionCode::DCode(
                DCode::Operation(Operation::Flash(
                    partial_coordinates_from_gerber(None, None, fs).unwrap()
                ))
            )))
        ]
    );
}

#[test]
#[allow(non_snake_case)]
fn test_kicad_macro() {
    // given
    logging_init();

    let reader = gerber_to_reader(
        r#"
%TF.GenerationSoftware,KiCad,Pcbnew,8.0.3*%
%TF.CreationDate,2025-04-28T16:25:44+02:00*%
%TF.ProjectId,SPRacingRXN1-RevB-20240507-1510,53505261-6369-46e6-9752-584e312d5265,rev?*%
%TF.SameCoordinates,Original*%
%TF.FileFunction,Copper,L1,Top*%
%TF.FilePolarity,Positive*%
%FSLAX46Y46*%
G04 Gerber Fmt 4.6, Leading zero omitted, Abs format (unit mm)*
G04 Created by KiCad (PCBNEW 8.0.3) date 2025-04-28 16:25:44*
%MOMM*%
%LPD*%
G01*
G04 APERTURE LIST*
G04 Aperture macros list*
%AMRoundRect*
0 Rectangle with rounded corners*
0 $1 Rounding radius*
0 $2 $3 $4 $5 $6 $7 $8 $9 X,Y pos of 4 corners*
0 Add a 4 corners polygon primitive as box body*
4,1,4,$2,$3,$4,$5,$6,$7,$8,$9,$2,$3,0*
0 Add four circle primitives for the rounded corners*
1,1,$1+$1,$2,$3*
1,1,$1+$1,$4,$5*
1,1,$1+$1,$6,$7*
1,1,$1+$1,$8,$9*
0 Add four rect primitives between the rounded corners*
20,1,$1+$1,$2,$3,$4,$5,0*
20,1,$1+$1,$4,$5,$6,$7,0*
20,1,$1+$1,$6,$7,$8,$9,0*
20,1,$1+$1,$8,$9,$2,$3,0*%

G04 Aperture macros list end*
%ADD36RoundRect,0.110250X0.114771X-0.214739X0.114729X0.214761X-0.114771X0.214739X-0.114729X-0.214761X0*%
D36*
X155251142Y-100803551D03*
M02*
    "#,
    );

    // Note the comment "0 Add a 4 corners polygon primitive as box body" is wrong (Kicad 8.0)
    // and should read "0 Add a 4-corner outline primitive as box body" (code 4 = outline)

    // Note the comment "Add four rect primitives between the rounded corners" is wrong (Kicad 8.0)
    // and should read "Add four vector-line primitives between the rounded corners" (code 20 = vector-line)

    // FUTURE raise the above 2 issues with this in the KiCad issue tracker, add a link here.

    // Note, it's important that the macro definition isn't the only command in the file because it's important that
    // the macro parser finds the end of the macro definition correctly and that it doesn't consume additional lines.

    let expected_macro_1: ApertureMacro = ApertureMacro {
        name: "RoundRect".to_string(),
        content: vec![
            MacroContent::Comment("Rectangle with rounded corners".to_string()),
            MacroContent::Comment("$1 Rounding radius".to_string()),
            MacroContent::Comment("$2 $3 $4 $5 $6 $7 $8 $9 X,Y pos of 4 corners".to_string()),
            MacroContent::Comment("Add a 4 corners polygon primitive as box body".to_string()),
            MacroContent::Outline(OutlinePrimitive {
                exposure: MacroBoolean::Value(true),
                points: vec![
                    (MacroDecimal::Variable(2), MacroDecimal::Variable(3)),
                    (MacroDecimal::Variable(4), MacroDecimal::Variable(5)),
                    (MacroDecimal::Variable(6), MacroDecimal::Variable(7)),
                    (MacroDecimal::Variable(8), MacroDecimal::Variable(9)),
                    (MacroDecimal::Variable(2), MacroDecimal::Variable(3)),
                ],
                angle: MacroDecimal::Value(0.0),
            }),
            MacroContent::Comment("Add four circle primitives for the rounded corners".to_string()),
            MacroContent::Circle(CirclePrimitive {
                exposure: MacroBoolean::Value(true),
                diameter: MacroDecimal::Expression("$1+$1".to_string()),
                center: (MacroDecimal::Variable(2), MacroDecimal::Variable(3)),
                angle: None,
            }),
            MacroContent::Circle(CirclePrimitive {
                exposure: MacroBoolean::Value(true),
                diameter: MacroDecimal::Expression("$1+$1".to_string()),
                center: (MacroDecimal::Variable(4), MacroDecimal::Variable(5)),
                angle: None,
            }),
            MacroContent::Circle(CirclePrimitive {
                exposure: MacroBoolean::Value(true),
                diameter: MacroDecimal::Expression("$1+$1".to_string()),
                center: (MacroDecimal::Variable(6), MacroDecimal::Variable(7)),
                angle: None,
            }),
            MacroContent::Circle(CirclePrimitive {
                exposure: MacroBoolean::Value(true),
                diameter: MacroDecimal::Expression("$1+$1".to_string()),
                center: (MacroDecimal::Variable(8), MacroDecimal::Variable(9)),
                angle: None,
            }),
            MacroContent::Comment(
                "Add four rect primitives between the rounded corners".to_string(),
            ),
            MacroContent::VectorLine(VectorLinePrimitive {
                exposure: MacroBoolean::Value(true),
                width: MacroDecimal::Expression("$1+$1".to_string()),
                start: (MacroDecimal::Variable(2), MacroDecimal::Variable(3)),
                end: (MacroDecimal::Variable(4), MacroDecimal::Variable(5)),
                angle: MacroDecimal::Value(0.0),
            }),
            MacroContent::VectorLine(VectorLinePrimitive {
                exposure: MacroBoolean::Value(true),
                width: MacroDecimal::Expression("$1+$1".to_string()),
                start: (MacroDecimal::Variable(4), MacroDecimal::Variable(5)),
                end: (MacroDecimal::Variable(6), MacroDecimal::Variable(7)),
                angle: MacroDecimal::Value(0.0),
            }),
            MacroContent::VectorLine(VectorLinePrimitive {
                exposure: MacroBoolean::Value(true),
                width: MacroDecimal::Expression("$1+$1".to_string()),
                start: (MacroDecimal::Variable(6), MacroDecimal::Variable(7)),
                end: (MacroDecimal::Variable(8), MacroDecimal::Variable(9)),
                angle: MacroDecimal::Value(0.0),
            }),
            MacroContent::VectorLine(VectorLinePrimitive {
                exposure: MacroBoolean::Value(true),
                width: MacroDecimal::Expression("$1+$1".to_string()),
                start: (MacroDecimal::Variable(8), MacroDecimal::Variable(9)),
                end: (MacroDecimal::Variable(2), MacroDecimal::Variable(3)),
                angle: MacroDecimal::Value(0.0),
            }),
        ],
    };

    // when
    parse_and_filter!(reader, commands, filtered_commands, |cmd| matches!(
        cmd,
        Ok(Command::ExtendedCode(ExtendedCode::ApertureMacro(_)))
            | Ok(Command::ExtendedCode(ExtendedCode::ApertureDefinition(_)))
    ));

    // then
    assert_eq_commands!(
        filtered_commands,
        vec![
            Ok(Command::ExtendedCode(ExtendedCode::ApertureMacro(
                expected_macro_1
            ))),
            Ok(Command::ExtendedCode(ExtendedCode::ApertureDefinition(
                ApertureDefinition::new(
                    36,
                    Aperture::Macro(
                        "RoundRect".to_string(),
                        Some(vec![
                            MacroDecimal::Value(0.110250),
                            MacroDecimal::Value(0.114771),
                            MacroDecimal::Value(-0.214739),
                            MacroDecimal::Value(0.114729),
                            MacroDecimal::Value(0.214761),
                            MacroDecimal::Value(-0.114771),
                            MacroDecimal::Value(0.214739),
                            MacroDecimal::Value(-0.114729),
                            MacroDecimal::Value(-0.214761),
                            MacroDecimal::Value(0.0),
                        ])
                    )
                )
            ))),
        ]
    );
}

// See gerber spec 2021-02, section 4.5
#[test]
fn test_macro_with_variable_definition() {
    // given
    let reader = gerber_to_reader(
        r#"
    %FSLAX35Y35*%
    %MOMM*%
    %AMPLUS*
    0 $1 Line Width*
    0 $2 Width/Height*
    0 $3 $4 Center X/Y*
    $1=1.0*
    $2=5.0*
    $3=0.0*
    $4=0.0*
    20,1,$1,$3-$2,$4,$3+$2,$4,0*
    20,1,$1,$3,$4-$2,$3,$4+$2,0*%
    %ADD10PLUS*%
    "#,
    );

    // Note, it's important that the macro definition isn't the only command in the file because it's important that
    // the macro parser finds the end of the macro definition correctly and that it doesn't consume additional lines.

    let expected_macro: ApertureMacro = ApertureMacro {
        name: "PLUS".to_string(),
        content: vec![
            MacroContent::Comment("$1 Line Width".to_string()),
            MacroContent::Comment("$2 Width/Height".to_string()),
            MacroContent::Comment("$3 $4 Center X/Y".to_string()),
            MacroContent::VariableDefinition(VariableDefinition {
                number: 1,
                expression: "1.0".to_string(),
            }),
            MacroContent::VariableDefinition(VariableDefinition {
                number: 2,
                expression: "5.0".to_string(),
            }),
            MacroContent::VariableDefinition(VariableDefinition {
                number: 3,
                expression: "0.0".to_string(),
            }),
            MacroContent::VariableDefinition(VariableDefinition {
                number: 4,
                expression: "0.0".to_string(),
            }),
            MacroContent::VectorLine(VectorLinePrimitive {
                exposure: MacroBoolean::Value(true), // 1 indicates exposure on
                width: MacroDecimal::Variable(1),
                start: (
                    MacroDecimal::Expression("$3-$2".to_string()),
                    MacroDecimal::Variable(4),
                ),
                end: (
                    MacroDecimal::Expression("$3+$2".to_string()),
                    MacroDecimal::Variable(4),
                ),
                angle: MacroDecimal::Value(0.0),
            }),
            MacroContent::VectorLine(VectorLinePrimitive {
                exposure: MacroBoolean::Value(true), // 1 indicates exposure on
                width: MacroDecimal::Variable(1),
                start: (
                    MacroDecimal::Variable(3),
                    MacroDecimal::Expression("$4-$2".to_string()),
                ),
                end: (
                    MacroDecimal::Variable(3),
                    MacroDecimal::Expression("$4+$2".to_string()),
                ),
                angle: MacroDecimal::Value(0.0),
            }),
        ],
    };

    // when
    parse_and_filter!(reader, commands, filtered_commands, |cmd| matches!(
        cmd,
        Ok(Command::ExtendedCode(ExtendedCode::ApertureMacro(_)))
            | Ok(Command::ExtendedCode(ExtendedCode::ApertureDefinition(_)))
    ));

    // then
    assert_eq_commands!(
        filtered_commands,
        vec![
            Ok(Command::ExtendedCode(ExtendedCode::ApertureMacro(
                expected_macro
            ))),
            Ok(Command::ExtendedCode(ExtendedCode::ApertureDefinition(
                ApertureDefinition::new(10, Aperture::Macro("PLUS".to_string(), None))
            ))),
        ]
    );
}

/// This is a real-world example.
#[test]
fn test_jlccam_macro_1_with_empty_line() {
    // given
    let reader = gerber_to_reader(
        r#"
    G04 -- output software:jlccam v1.5.1 *
    G04 -- dbname:*
    G04 -- stepname:set*
    G04 -- layer:ts*
    *
    G04 -- format*
    %FSLAX26Y26*%
    %MOIN*%
    *
    G04 -- apertures*
    G04 A5ts - set.0.from.i274x.OUTLINE37.d133+inc-0.6x0x00*
    %AMA5ts*
    4,1,5,
    0.016703,-0.006161,
    -0.004670,0.015209,
    -0.010643,0.015208,
    -0.016703,0.009147,
    0.007655,-0.015209,
    0.016703,-0.006161,
    0.000*
    %
    %ADD26A5ts*%
    "#,
    );

    // In the above example, note the '*' of the outline primitive appears after the rotation value and that the
    // '%' appears on the next line.

    // Note, it's important that the macro definition isn't the only command in the file because it's important that
    // the macro parser finds the end of the macro definition correctly and that it doesn't consume additional lines.

    let expected_macro: ApertureMacro = ApertureMacro {
        name: "A5ts".to_string(),
        content: vec![MacroContent::Outline(OutlinePrimitive {
            exposure: MacroBoolean::Value(true),
            points: vec![
                (
                    MacroDecimal::Value(0.016703),
                    MacroDecimal::Value(-0.006161),
                ),
                (
                    MacroDecimal::Value(-0.004670),
                    MacroDecimal::Value(0.015209),
                ),
                (
                    MacroDecimal::Value(-0.010643),
                    MacroDecimal::Value(0.015208),
                ),
                (
                    MacroDecimal::Value(-0.016703),
                    MacroDecimal::Value(0.009147),
                ),
                (
                    MacroDecimal::Value(0.007655),
                    MacroDecimal::Value(-0.015209),
                ),
                (
                    MacroDecimal::Value(0.016703),
                    MacroDecimal::Value(-0.006161),
                ),
            ],
            angle: MacroDecimal::Value(0.0),
        })],
    };

    // when
    parse_and_filter!(reader, commands, filtered_commands, |cmd| matches!(
        cmd,
        Ok(Command::ExtendedCode(ExtendedCode::ApertureMacro(_)))
            | Ok(Command::ExtendedCode(ExtendedCode::ApertureDefinition(_)))
    ));

    // then
    assert_eq_commands!(
        filtered_commands,
        vec![
            Ok(Command::ExtendedCode(ExtendedCode::ApertureMacro(
                expected_macro
            ))),
            Ok(Command::ExtendedCode(ExtendedCode::ApertureDefinition(
                ApertureDefinition::new(26, Aperture::Macro("A5ts".to_string(), None))
            ))),
        ]
    );
}

#[test]
fn diptrace_rounded_rectangle_pcb_outline() {
    // NOTE Diptrace 4.3.0.6 still uses deprecated syntax which combines a G03 and a D01, Deprecated since 2015.06.
    //      See 2024.06 Gerber spec Section 8.3.1 "Combining G01/G02/G03 and D01 in a single command".

    // given
    let reader = gerber_to_reader(
        r#"
        %TF.GenerationSoftware,Novarm,DipTrace,4.3.0.6*%
        %TF.CreationDate,2025-06-02T14:41:54+00:00*%
        %FSLAX35Y35*%
        %MOMM*%
        %TF.FileFunction,Profile*%
        %TF.Part,Single*%
        %ADD12C,0.14*%
        G75*
        G01*
        %LPD*%
        X500000Y1500000D2*
        D12*
        G03X0Y1000000I0J-500000D01*
        G01*
        Y500000D1*
        G03X500000Y0I500000J0D01*
        G01*
        X1500000D1*
        G03X2000000Y500000I0J500000D01*
        G01*
        Y1000000D1*
        G03X1500000Y1500000I-500000J0D01*
        G01*
        X500000D1*
        M02*
    "#,
    );

    // when
    parse_and_filter!(reader, commands, filtered_commands, |cmd| matches!(
        cmd,
        Ok(Command::FunctionCode(FunctionCode::GCode(_)))
            | Ok(Command::FunctionCode(FunctionCode::DCode(_)))
    ));

    // then
    let format = CoordinateFormat {
        zero_omission: ZeroOmission::Leading,
        coordinate_mode: CoordinateMode::Absolute,
        integer: 3,
        decimal: 5,
    };

    assert_eq_commands!(
        filtered_commands,
        vec![
            Ok(Command::FunctionCode(FunctionCode::GCode(
                GCode::QuadrantMode(QuadrantMode::Multi)
            ))),
            Ok(Command::FunctionCode(FunctionCode::GCode(
                GCode::InterpolationMode(InterpolationMode::Linear)
            ))),
            Ok(Command::FunctionCode(FunctionCode::DCode(
                DCode::Operation(Operation::Move(Some(Coordinates::new(5, 15, format))))
            ))),
            Ok(Command::FunctionCode(FunctionCode::DCode(
                DCode::SelectAperture(12)
            ))),
            Ok(Command::FunctionCode(FunctionCode::GCode(
                GCode::InterpolationMode(InterpolationMode::CounterclockwiseCircular)
            ))),
            Ok(Command::FunctionCode(FunctionCode::DCode(
                DCode::Operation(Operation::Interpolate(
                    Some(Coordinates::new(0, 10, format)),
                    Some(CoordinateOffset::new(0, -5, format)),
                ))
            ))),
            Ok(Command::FunctionCode(FunctionCode::GCode(
                GCode::InterpolationMode(InterpolationMode::Linear)
            ))),
            Ok(Command::FunctionCode(FunctionCode::DCode(
                DCode::Operation(Operation::Interpolate(
                    Some(Coordinates::new::<Option<i32>, Option<i32>>(
                        None,
                        Some(5),
                        format
                    )),
                    None,
                ))
            ))),
            Ok(Command::FunctionCode(FunctionCode::GCode(
                GCode::InterpolationMode(InterpolationMode::CounterclockwiseCircular)
            ))),
            Ok(Command::FunctionCode(FunctionCode::DCode(
                DCode::Operation(Operation::Interpolate(
                    Some(Coordinates::new(5, 0, format)),
                    Some(CoordinateOffset::new(5, 0, format)),
                ))
            ))),
            Ok(Command::FunctionCode(FunctionCode::GCode(
                GCode::InterpolationMode(InterpolationMode::Linear)
            ))),
            Ok(Command::FunctionCode(FunctionCode::DCode(
                DCode::Operation(Operation::Interpolate(
                    Some(Coordinates::new::<Option<i32>, Option<i32>>(
                        Some(15),
                        None,
                        format
                    )),
                    None,
                ))
            ))),
            Ok(Command::FunctionCode(FunctionCode::GCode(
                GCode::InterpolationMode(InterpolationMode::CounterclockwiseCircular)
            ))),
            Ok(Command::FunctionCode(FunctionCode::DCode(
                DCode::Operation(Operation::Interpolate(
                    Some(Coordinates::new(20, 5, format)),
                    Some(CoordinateOffset::new(0, 5, format)),
                ))
            ))),
            Ok(Command::FunctionCode(FunctionCode::GCode(
                GCode::InterpolationMode(InterpolationMode::Linear)
            ))),
            Ok(Command::FunctionCode(FunctionCode::DCode(
                DCode::Operation(Operation::Interpolate(
                    Some(Coordinates::new::<Option<i32>, Option<i32>>(
                        None,
                        Some(10),
                        format
                    )),
                    None,
                ))
            ))),
            Ok(Command::FunctionCode(FunctionCode::GCode(
                GCode::InterpolationMode(InterpolationMode::CounterclockwiseCircular)
            ))),
            Ok(Command::FunctionCode(FunctionCode::DCode(
                DCode::Operation(Operation::Interpolate(
                    Some(Coordinates::new(15, 15, format)),
                    Some(CoordinateOffset::new(-5, 0, format)),
                ))
            ))),
            Ok(Command::FunctionCode(FunctionCode::GCode(
                GCode::InterpolationMode(InterpolationMode::Linear)
            ))),
            Ok(Command::FunctionCode(FunctionCode::DCode(
                DCode::Operation(Operation::Interpolate(
                    Some(Coordinates::new::<Option<i32>, Option<i32>>(
                        Some(5),
                        None,
                        format
                    )),
                    None,
                ))
            ))),
        ]
    );
}

/// There was an issue with the parser creating MacroDecimal::Value and not MacroDecimal::Expression for expressions
/// that start with a value
#[test]
fn vector_font_macro_1() {
    // given
    let reader = gerber_to_reader(
        r#"
        %FSLAX46Y46*%
        G04 Gerber Fmt 4.6, Leading zero omitted, Abs format (unit mm)*
        %MOMM*%
        %LPD*%
        G01*
        
        
        %AMVECTORFONT_77*
        0 ASCII 'M' 77 (0x4D)*
        0 $1 = width*
        0 $2 = scale*
        0 $3 = rotation*
        20,1,$1,-10x$2,-10x$2,-10x$2,10x$2,$3*
        20,1,$1,$2x-10,$2x10,$2x0,$2x-10,$3*
        20,1,$1,0x$2,-10x$2,10x$2,10x$2,$3*
        20,1,$1,$2x10,$2x-10,$2x10,$2x10,$3*
        %
        %ADD177VECTORFONT_77,0.1X0.49X0*%
        D177*
        X-20000000Y0D03*
        X00000000Y0D03*
        X20000000Y0D03*
    "#,
    );

    // NOTE some of the macro uses expressions that start with numbers, some start with variables, some values are
    //      positive and some negative.
    //      lines 1 and 3 use <value * variable>, lines 2 and 4 use <variable * value>

    let expected_macro: ApertureMacro = ApertureMacro {
        name: "VECTORFONT_77".to_string(),
        content: vec![
            MacroContent::Comment("ASCII 'M' 77 (0x4D)".to_string()),
            MacroContent::Comment("$1 = width".to_string()),
            MacroContent::Comment("$2 = scale".to_string()),
            MacroContent::Comment("$3 = rotation".to_string()),
            // 20,1,$1,-10x$2,-10x$2,-10x$2,10x$2,$3*
            MacroContent::VectorLine(VectorLinePrimitive {
                exposure: MacroBoolean::Value(true),
                width: MacroDecimal::Variable(1),
                start: (
                    MacroDecimal::Expression("-10x$2".to_string()),
                    MacroDecimal::Expression("-10x$2".to_string()),
                ),
                end: (
                    MacroDecimal::Expression("-10x$2".to_string()),
                    MacroDecimal::Expression("10x$2".to_string()),
                ),
                angle: MacroDecimal::Variable(3),
            }),
            // 20,1,$1,$2x-10,$2x10,$2x0,$2x-10,$3*
            MacroContent::VectorLine(VectorLinePrimitive {
                exposure: MacroBoolean::Value(true),
                width: MacroDecimal::Variable(1),
                start: (
                    MacroDecimal::Expression("$2x-10".to_string()),
                    MacroDecimal::Expression("$2x10".to_string()),
                ),
                end: (
                    MacroDecimal::Expression("$2x0".to_string()),
                    MacroDecimal::Expression("$2x-10".to_string()),
                ),
                angle: MacroDecimal::Variable(3),
            }),
            // 20,1,$1,0x$2,-10x$2,10x$2,10x$2,$3*
            MacroContent::VectorLine(VectorLinePrimitive {
                exposure: MacroBoolean::Value(true),
                width: MacroDecimal::Variable(1),
                start: (
                    MacroDecimal::Expression("0x$2".to_string()),
                    MacroDecimal::Expression("-10x$2".to_string()),
                ),
                end: (
                    MacroDecimal::Expression("10x$2".to_string()),
                    MacroDecimal::Expression("10x$2".to_string()),
                ),
                angle: MacroDecimal::Variable(3),
            }),
            // 20,1,$1,$2x10,$2x-10,$2x10,$2x10,$3*
            MacroContent::VectorLine(VectorLinePrimitive {
                exposure: MacroBoolean::Value(true),
                width: MacroDecimal::Variable(1),
                start: (
                    MacroDecimal::Expression("$2x10".to_string()),
                    MacroDecimal::Expression("$2x-10".to_string()),
                ),
                end: (
                    MacroDecimal::Expression("$2x10".to_string()),
                    MacroDecimal::Expression("$2x10".to_string()),
                ),
                angle: MacroDecimal::Variable(3),
            }),
        ],
    };

    // when
    parse_and_filter!(reader, commands, filtered_commands, |cmd| matches!(
        cmd,
        Ok(Command::ExtendedCode(ExtendedCode::ApertureMacro(_)))
            | Ok(Command::ExtendedCode(ExtendedCode::ApertureDefinition(_)))
    ));

    assert_eq_commands!(
        filtered_commands,
        vec![
            Ok(Command::ExtendedCode(ExtendedCode::ApertureMacro(
                expected_macro
            ))),
            Ok(Command::ExtendedCode(ExtendedCode::ApertureDefinition(
                ApertureDefinition::new(
                    177,
                    Aperture::Macro(
                        "VECTORFONT_77".to_string(),
                        Some(vec![
                            MacroDecimal::Value(0.1),
                            MacroDecimal::Value(0.49),
                            MacroDecimal::Value(0.0),
                        ])
                    )
                )
            ))),
        ]
    );
}

// See gerber spec 2021-02, section 4.5
#[test]
fn test_aperture_block() {
    // given
    let reader = gerber_to_reader(
        r#"
        G04 Ucamco copyright*
        %TF.GenerationSoftware,Ucamco,UcamX,2016.04-160425*%
        %TF.CreationDate,2016-04-25T00:00;00+01:00*%
        %TF.Part,Other,Testfile*%
        %FSLAX46Y46*%
        %MOMM*%
        G04 Define standard apertures*
        %ADD10C,7.500000*%
        %ADD11C,15*%
        %ADD12R,20X10*%
        %ADD13R,10X20*%
        G04 Define block aperture 100, consisting of two draws and a round dot*
        %ABD100*%
        D10*
        X65532000Y17605375D02*
        Y65865375D01*
        X-3556000D01*
        D11*
        X-3556000Y17605375D03*
        %AB*%
        G04 Define block aperture 102, consisting of 2x3 flashes of aperture 101
        and 1 flash of D12*
        %ABD102*%
        G04 Define nested block aperture 101, consisting of 2x2 flashes of
        aperture 100*
        %ABD101*%
        D100*
        X0Y0D03*
        X0Y70000000D03*
        X100000000Y0D03*
        X100000000Y70000000D03*
        %AB*%
        D101*
        X0Y0D03*
        X0Y160000000D03*
        X0Y320000000D03*
        X230000000Y0D03*
        X230000000Y160000000D03*
        X230000000Y320000000D03*
        D12*
        X19500000Y-10000000D03*
        %AB*%
        G04 Flash D13 twice outside of blocks*
        D13*
        X-30000000Y10000000D03*
        X143000000Y-30000000D03*
        G04 Flash block 102 3x2 times*
        D102*
        X0Y0D03*
        X0Y520000000D03*
        X500000000Y0D03*
        X500000000Y520000000D03*
        X1000000000Y0D03*
        X1000000000Y520000000D03*
        M02*
    "#,
    );

    // when
    parse_and_filter!(reader, commands, filtered_commands, |cmd| matches!(
        cmd,
        Ok(Command::ExtendedCode(ExtendedCode::ApertureBlock(_)))
    ));

    assert_eq_commands!(
        filtered_commands,
        vec![
            Ok(Command::ExtendedCode(ExtendedCode::ApertureBlock(
                ApertureBlock::Open { code: 100 }
            ))),
            Ok(Command::ExtendedCode(ExtendedCode::ApertureBlock(
                ApertureBlock::Close
            ))),
            Ok(Command::ExtendedCode(ExtendedCode::ApertureBlock(
                ApertureBlock::Open { code: 102 }
            ))),
            Ok(Command::ExtendedCode(ExtendedCode::ApertureBlock(
                ApertureBlock::Open { code: 101 }
            ))),
            Ok(Command::ExtendedCode(ExtendedCode::ApertureBlock(
                ApertureBlock::Close
            ))),
            Ok(Command::ExtendedCode(ExtendedCode::ApertureBlock(
                ApertureBlock::Close
            ))),
        ]
    );
}

/// LibrePCB generates macro definitions on a single-line.
#[test]
fn librepcb_single_line_macro() {
    // given
    logging_init();

    let reader = gerber_to_reader(
        r#"
        %FSLAX66Y66*%
        %MOMM*%
        G04 THIS HAS ONE PRIMITIVE *
        %AMOUTLINE10*4,1,2,-0.746681,-0.498916,-0.829667,-0.34366,-0.746681,-0.498916,100.0*%
        %ADD10OUTLINE10*%

        G04 THIS HAS THREE PRIMITIVES *
        %AMROTATEDOBROUND16*1,1,1.0,-0.433013,-0.25*1,1,1.0,0.433013,0.25*20,1,1.0,-0.433013,-0.25,0.433013,0.25,0*%
        %ADD16ROTATEDOBROUND16*%

        G04 --- BOARD END --- *

        M02*
    "#,
    );

    // when
    parse_and_filter!(reader, commands, filtered_commands, |cmd| matches!(
        cmd,
        Ok(Command::ExtendedCode(ExtendedCode::ApertureMacro(_)))
            | Ok(Command::ExtendedCode(ExtendedCode::ApertureDefinition(_)))
    ));

    // then
    assert_eq_commands!(
        &filtered_commands,
        &vec![
            Ok(Command::ExtendedCode(ExtendedCode::ApertureMacro(
                ApertureMacro {
                    name: "OUTLINE10".to_string(),
                    content: vec![MacroContent::Outline(OutlinePrimitive {
                        exposure: MacroBoolean::Value(true),
                        points: vec![
                            (
                                MacroDecimal::Value(-0.746681),
                                MacroDecimal::Value(-0.498916)
                            ),
                            (
                                MacroDecimal::Value(-0.829667),
                                MacroDecimal::Value(-0.34366)
                            ),
                            (
                                MacroDecimal::Value(-0.746681),
                                MacroDecimal::Value(-0.498916)
                            )
                        ],
                        angle: MacroDecimal::Value(100.0)
                    })]
                }
            ))),
            Ok(Command::ExtendedCode(ExtendedCode::ApertureDefinition(
                ApertureDefinition {
                    code: 10,
                    aperture: Aperture::Macro("OUTLINE10".to_string(), None)
                }
            ))),
            Ok(Command::ExtendedCode(ExtendedCode::ApertureMacro(
                ApertureMacro {
                    name: "ROTATEDOBROUND16".to_string(),
                    content: vec![
                        MacroContent::Circle(CirclePrimitive {
                            exposure: MacroBoolean::Value(true),
                            diameter: MacroDecimal::Value(1.0),
                            center: (MacroDecimal::Value(-0.433013), MacroDecimal::Value(-0.25)),
                            angle: None
                        }),
                        MacroContent::Circle(CirclePrimitive {
                            exposure: MacroBoolean::Value(true),
                            diameter: MacroDecimal::Value(1.0),
                            center: (MacroDecimal::Value(0.433013), MacroDecimal::Value(0.25)),
                            angle: None
                        }),
                        MacroContent::VectorLine(VectorLinePrimitive {
                            exposure: MacroBoolean::Value(true),
                            width: MacroDecimal::Value(1.0),
                            start: (MacroDecimal::Value(-0.433013), MacroDecimal::Value(-0.25)),
                            end: (MacroDecimal::Value(0.433013), MacroDecimal::Value(0.25)),
                            angle: MacroDecimal::Value(0.0)
                        })
                    ]
                }
            ))),
            Ok(Command::ExtendedCode(ExtendedCode::ApertureDefinition(
                ApertureDefinition {
                    code: 16,
                    aperture: Aperture::Macro("ROTATEDOBROUND16".to_string(), None)
                }
            )))
        ]
    );
    assert!(!filtered_commands.is_empty());
}

#[test]
fn image_settings() {
    // given
    logging_init();

    let reader = gerber_to_reader(
        r#"
        %FSLAX66Y66*%
        %MOMM*%

        %ASAXBY*%
        %ASAYBX*%

        %IPPOS*%
        %IPNEG*%

        %IR0*%
        %IR90*%
        %IR180*%
        %IR270*%

        %MI*%
        %MIA0B0*%
        %MIA1*%
        %MIA1B0*%
        %MIB1*%
        %MIA0B1*%
        %MIA1B1*%

        %OF*%
        %OFA0.0001*%
        %OFA999.99999*%
        %OFB0.0001*%
        %OFB999.99999*%
        %OFA0.0001B0.0001*%
        %OFA999.99999B999.99999*%
        
        %SF*%
        %SFA0.0001*%
        %SFA999.99999*%
        %SFB0.0001*%
        %SFB999.99999*%
        %SFA0.0001B0.0001*%
        %SFA999.99999B999.99999*%

        M02*
    "#,
    );

    // when
    parse_and_filter!(reader, commands, filtered_commands, |cmd| matches!(
        cmd,
        Ok(Command::ExtendedCode(ExtendedCode::AxisSelect(_)))
            | Ok(Command::ExtendedCode(ExtendedCode::ImagePolarity(_)))
            | Ok(Command::ExtendedCode(ExtendedCode::RotateImage(_)))
            | Ok(Command::ExtendedCode(ExtendedCode::MirrorImage(_)))
            | Ok(Command::ExtendedCode(ExtendedCode::OffsetImage(_)))
            | Ok(Command::ExtendedCode(ExtendedCode::ScaleImage(_)))
    ));

    // then
    assert_eq_commands!(
        &filtered_commands,
        &vec![
            Ok(Command::ExtendedCode(ExtendedCode::AxisSelect(
                AxisSelect::AXBY
            ))),
            Ok(Command::ExtendedCode(ExtendedCode::AxisSelect(
                AxisSelect::AYBX
            ))),
            Ok(Command::ExtendedCode(ExtendedCode::ImagePolarity(
                ImagePolarity::Positive
            ))),
            Ok(Command::ExtendedCode(ExtendedCode::ImagePolarity(
                ImagePolarity::Negative
            ))),
            Ok(Command::ExtendedCode(ExtendedCode::RotateImage(
                ImageRotation::None
            ))),
            Ok(Command::ExtendedCode(ExtendedCode::RotateImage(
                ImageRotation::CCW_90
            ))),
            Ok(Command::ExtendedCode(ExtendedCode::RotateImage(
                ImageRotation::CCW_180
            ))),
            Ok(Command::ExtendedCode(ExtendedCode::RotateImage(
                ImageRotation::CCW_270
            ))),
            Ok(Command::ExtendedCode(ExtendedCode::MirrorImage(
                ImageMirroring::None
            ))),
            Ok(Command::ExtendedCode(ExtendedCode::MirrorImage(
                ImageMirroring::None
            ))),
            Ok(Command::ExtendedCode(ExtendedCode::MirrorImage(
                ImageMirroring::A
            ))),
            Ok(Command::ExtendedCode(ExtendedCode::MirrorImage(
                ImageMirroring::A
            ))),
            Ok(Command::ExtendedCode(ExtendedCode::MirrorImage(
                ImageMirroring::B
            ))),
            Ok(Command::ExtendedCode(ExtendedCode::MirrorImage(
                ImageMirroring::B
            ))),
            Ok(Command::ExtendedCode(ExtendedCode::MirrorImage(
                ImageMirroring::AB
            ))),
            Ok(Command::ExtendedCode(ExtendedCode::OffsetImage(
                ImageOffset { a: 0.0, b: 0.0 }
            ))),
            Ok(Command::ExtendedCode(ExtendedCode::OffsetImage(
                ImageOffset { a: 0.0001, b: 0.0 }
            ))),
            Ok(Command::ExtendedCode(ExtendedCode::OffsetImage(
                ImageOffset {
                    a: 999.99999,
                    b: 0.0
                }
            ))),
            Ok(Command::ExtendedCode(ExtendedCode::OffsetImage(
                ImageOffset { a: 0.0, b: 0.0001 }
            ))),
            Ok(Command::ExtendedCode(ExtendedCode::OffsetImage(
                ImageOffset {
                    a: 0.0,
                    b: 999.99999
                }
            ))),
            Ok(Command::ExtendedCode(ExtendedCode::OffsetImage(
                ImageOffset {
                    a: 0.0001,
                    b: 0.0001
                }
            ))),
            Ok(Command::ExtendedCode(ExtendedCode::OffsetImage(
                ImageOffset {
                    a: 999.99999,
                    b: 999.99999
                }
            ))),
            Ok(Command::ExtendedCode(ExtendedCode::ScaleImage(
                ImageScaling { a: 1.0, b: 1.0 }
            ))),
            Ok(Command::ExtendedCode(ExtendedCode::ScaleImage(
                ImageScaling { a: 0.0001, b: 1.0 }
            ))),
            Ok(Command::ExtendedCode(ExtendedCode::ScaleImage(
                ImageScaling {
                    a: 999.99999,
                    b: 1.0
                }
            ))),
            Ok(Command::ExtendedCode(ExtendedCode::ScaleImage(
                ImageScaling { a: 1.0, b: 0.0001 }
            ))),
            Ok(Command::ExtendedCode(ExtendedCode::ScaleImage(
                ImageScaling {
                    a: 1.0,
                    b: 999.99999
                }
            ))),
            Ok(Command::ExtendedCode(ExtendedCode::ScaleImage(
                ImageScaling {
                    a: 0.0001,
                    b: 0.0001
                }
            ))),
            Ok(Command::ExtendedCode(ExtendedCode::ScaleImage(
                ImageScaling {
                    a: 999.99999,
                    b: 999.99999
                }
            ))),
        ]
    );
    assert!(!filtered_commands.is_empty());
}

#[test]
#[allow(non_snake_case)]
fn G04_comment_attributes() {
    // given
    logging_init();

    let reader = gerber_to_reader(
        r#"
        %FSLAX66Y66*%
        %MOMM*%

        G04 Normal Comment*
        
        G04 #@! TA.AperFunction,SMDPad,CuDef*
        G04 #@! TF.FileFunction,Profile,NP*
        G04 #@! TO.C,R1*

        G04 #@! TAExample,value1,value2*
        G04 #@! TFExample,value1,value2*
        G04 #@! TOExample,value1,value2*

        M02*
    "#,
    );

    // when
    parse_and_filter!(reader, commands, filtered_commands, |cmd| matches!(
        cmd,
        Ok(Command::FunctionCode(FunctionCode::GCode(GCode::Comment(
            _
        ))))
    ));

    // then
    assert_eq_commands!(
        &filtered_commands,
        &vec![
            Ok(Command::FunctionCode(FunctionCode::GCode(GCode::Comment(
                CommentContent::String("Normal Comment".to_string())
            )))),
            Ok(Command::FunctionCode(FunctionCode::GCode(GCode::Comment(
                CommentContent::Standard(StandardComment::ApertureAttribute(
                    ApertureAttribute::ApertureFunction(ApertureFunction::SmdPad(
                        SmdPadType::CopperDefined
                    ))
                ))
            )))),
            Ok(Command::FunctionCode(FunctionCode::GCode(GCode::Comment(
                CommentContent::Standard(StandardComment::FileAttribute(
                    FileAttribute::FileFunction(FileFunction::Profile(Some(Profile::NonPlated)))
                ))
            )))),
            Ok(Command::FunctionCode(FunctionCode::GCode(GCode::Comment(
                CommentContent::Standard(StandardComment::ObjectAttribute(
                    ObjectAttribute::Component("R1".to_string())
                ))
            )))),
            Ok(Command::FunctionCode(FunctionCode::GCode(GCode::Comment(
                CommentContent::Standard(StandardComment::ApertureAttribute(
                    ApertureAttribute::UserDefined {
                        name: "Example".to_string(),
                        values: vec!["value1".to_string(), "value2".to_string()],
                    }
                ))
            )))),
            Ok(Command::FunctionCode(FunctionCode::GCode(GCode::Comment(
                CommentContent::Standard(StandardComment::FileAttribute(
                    FileAttribute::UserDefined {
                        name: "Example".to_string(),
                        values: vec!["value1".to_string(), "value2".to_string()],
                    }
                ))
            )))),
            Ok(Command::FunctionCode(FunctionCode::GCode(GCode::Comment(
                CommentContent::Standard(StandardComment::ObjectAttribute(
                    ObjectAttribute::UserDefined {
                        name: "Example".to_string(),
                        values: vec!["value1".to_string(), "value2".to_string()],
                    }
                ))
            )))),
        ]
    );
    assert!(!filtered_commands.is_empty());
}

/// Regression test to ensure that malformed aperture definitions don't cause a panic.
#[test]
fn malformed_aperture_definition() {
    // given
    logging_init();

    let reader = gerber_to_reader(
        "
    %FSLAX23Y23*%
    %MOMM*%-


    G04 Too many parameters *
    %ADD10C,1X2X3*%
    G04 Missing parameter *
    %ADD10C*%

    G04 Too many parameters *
    %ADD10R,1X2X3X4*%
    G04 Missing second parameter *
    %ADD10R,1*%
    G04 Missing both parameters *
    %ADD10R*%

    G04 Too many parameters *
    %ADD10O,1X2X3X4*%
    G04 Missing second parameter *
    %ADD10O,1*%
    G04 Missing both parameters *
    %ADD10O*%

    G04 Too many parameters *
    %ADD10P,1X2X3X4X5*%
    G04 Missing second parameter *
    %ADD10P,1*%
    G04 Missing all parameters *
    %ADD10P*%

    G04 Example of an unknown aperture definition code *
    %ADD10T*%
    ",
    );
    let doc = parse(reader).unwrap();

    // then
    let errors = doc.into_errors();

    let (error, errors) = errors.split_first().unwrap();
    println!("{:#?}", error);
    assert!(matches!(error,
        GerberParserErrorWithContext {
            error: ContentError::IncorrectDefinitionArgCount {
                aperture_code,
                aperture_name,
            },
            line: Some((_line_number, content)),
        } if *aperture_code == 10 && aperture_name.eq("C") && content.eq("%ADD10C,1X2X3*%")
    ));

    let (error, errors) = errors.split_first().unwrap();
    println!("{:#?}", error);
    assert!(matches!(error,
        GerberParserErrorWithContext {
            error: ContentError::IncorrectDefinitionArgCount {
                aperture_code,
                aperture_name,
            },
            line: Some((_line_number, content)),
        } if *aperture_code == 10 && aperture_name.eq("C") && content.eq("%ADD10C*%")
    ));

    let (error, errors) = errors.split_first().unwrap();
    println!("{:#?}", error);
    assert!(matches!(error,
        GerberParserErrorWithContext {
            error: ContentError::IncorrectDefinitionArgCount {
                aperture_code,
                aperture_name,
            },
            line: Some((_line_number, content)),
        } if *aperture_code == 10 && aperture_name.eq("R") && content.eq("%ADD10R,1X2X3X4*%")
    ));

    let (error, errors) = errors.split_first().unwrap();
    println!("{:#?}", error);
    assert!(matches!(error,
        GerberParserErrorWithContext {
            error: ContentError::IncorrectDefinitionArgCount {
                aperture_code,
                aperture_name,
            },
            line: Some((_line_number, content)),
        } if *aperture_code == 10 && aperture_name.eq("R") && content.eq("%ADD10R,1*%")
    ));

    let (error, errors) = errors.split_first().unwrap();
    println!("{:#?}", error);
    assert!(matches!(error,
        GerberParserErrorWithContext {
            error: ContentError::IncorrectDefinitionArgCount {
                aperture_code,
                aperture_name,
            },
            line: Some((_line_number, content)),
        } if *aperture_code == 10 && aperture_name.eq("R") && content.eq("%ADD10R*%")
    ));

    let (error, errors) = errors.split_first().unwrap();
    println!("{:#?}", error);
    assert!(matches!(error,
        GerberParserErrorWithContext {
            error: ContentError::IncorrectDefinitionArgCount {
                aperture_code,
                aperture_name,
            },
            line: Some((_line_number, content)),
        } if *aperture_code == 10 && aperture_name.eq("O") && content.eq("%ADD10O,1X2X3X4*%")
    ));

    let (error, errors) = errors.split_first().unwrap();
    println!("{:#?}", error);
    assert!(matches!(error,
        GerberParserErrorWithContext {
            error: ContentError::IncorrectDefinitionArgCount {
                aperture_code,
                aperture_name,
            },
            line: Some((_line_number, content)),
        } if *aperture_code == 10 && aperture_name.eq("O") && content.eq("%ADD10O,1*%")
    ));

    let (error, errors) = errors.split_first().unwrap();
    println!("{:#?}", error);
    assert!(matches!(error,
        GerberParserErrorWithContext {
            error: ContentError::IncorrectDefinitionArgCount {
                aperture_code,
                aperture_name,
            },
            line: Some((_line_number, content)),
        } if *aperture_code == 10 && aperture_name.eq("O") && content.eq("%ADD10O*%")
    ));

    let (error, errors) = errors.split_first().unwrap();
    println!("{:#?}", error);
    assert!(matches!(error,
        GerberParserErrorWithContext {
            error: ContentError::IncorrectDefinitionArgCount {
                aperture_code,
                aperture_name,
            },
            line: Some((_line_number, content)),
        } if *aperture_code == 10 && aperture_name.eq("P") && content.eq("%ADD10P,1X2X3X4X5*%")
    ));

    let (error, errors) = errors.split_first().unwrap();
    println!("{:#?}", error);
    assert!(matches!(error,
        GerberParserErrorWithContext {
            error: ContentError::IncorrectDefinitionArgCount {
                aperture_code,
                aperture_name,
            },
            line: Some((_line_number, content)),
        } if *aperture_code == 10 && aperture_name.eq("P") && content.eq("%ADD10P,1*%")
    ));

    let (error, errors) = errors.split_first().unwrap();
    println!("{:#?}", error);
    assert!(matches!(error,
        GerberParserErrorWithContext {
            error: ContentError::IncorrectDefinitionArgCount {
                aperture_code,
                aperture_name,
            },
            line: Some((_line_number, content)),
        } if *aperture_code == 10 && aperture_name.eq("P") && content.eq("%ADD10P*%")
    ));

    let (error, _) = errors.split_first().unwrap();
    println!("{:#?}", error);
    assert!(matches!(error,
        GerberParserErrorWithContext {
            error: ContentError::UnknownApertureType {
                type_str
            },
            line: Some((_line_number, content)),
        } if type_str.eq("T") && content.eq("%ADD10T*%")
    ));
}
