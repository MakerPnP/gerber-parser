use gerber_types::{GCode, DCode, FunctionCode, Command, Unit, CoordinateFormat, Aperture, ApertureMacro, Circle, Rectangular, Polygon, Operation, ExtendedCode, ApertureAttribute, ApertureFunction, FileAttribute, Part, FileFunction, FilePolarity, StepAndRepeat, Coordinates, MacroContent, PolygonPrimitive, OutlinePrimitive, MacroDecimal};
use::std::collections::HashMap;
use gerber_parser::error::{GerberParserErrorWithContext, };
use gerber_parser::parser::{parse_gerber, coordinates_from_gerber, coordinates_offset_from_gerber};

mod utils;


// #[test]
// fn test_full_gerber() {
//     let gerber_reader = utils::gerber_to_reader(&SAMPLE_GERBER_1);
//     let gbr = parse_gerber(gerber_reader);
//     println!("{}",&gbr);
//     assert_eq!(gbr, GerberDoc::new());
// }

#[test]
fn format_specification() {
    let reader_fs_1 = utils::gerber_to_reader("
    %FSLAX15Y15*%
    %MOMM*%
    M02*        
    ");

    let reader_fs_2 = utils::gerber_to_reader("
    %FSLAX36Y36*%
    %MOIN*%
    G04 Actual apertures and draw commands go here*
    M02*        
    ");

    assert_eq!(parse_gerber(reader_fs_1).format_specification, Some(CoordinateFormat::new(1, 5)));

    assert_eq!(parse_gerber(reader_fs_2).format_specification, Some(CoordinateFormat::new(3, 6)));
}

#[test]
fn units() {
    let reader_mm = utils::gerber_to_reader("
    G04 The next line specifies the precision of the units*
    %FSLAX23Y23*%
    G04 The next line specifies the units (inches or mm)*
    %MOMM*%

    G04 Actual apertures and draw commands go here*
    M02*        
    ");

    let reader_in = utils::gerber_to_reader("
    G04 The next line specifies the precision of the units*
    %FSLAX23Y23*%
    G04 The next line specifies the units (inches or mm)*
    %MOIN*%

    G04 Actual apertures and draw commands go here*
    M02*        
    ");

    assert_eq!(parse_gerber(reader_mm).units , Some(Unit::Millimeters));
    assert_eq!(parse_gerber(reader_in).units , Some(Unit::Inches));
}

#[test]
fn G04_comments() {
    let reader = utils::gerber_to_reader("
    G04 Comment before typical configuration lines*
    %FSLAX23Y23*%
    %MOMM*%
    G04 And now a comment after them*
    M02*        
    ");

    let filter_commands = |cmds:Vec<Result<Command, GerberParserErrorWithContext>>| -> Vec<Result<Command, GerberParserErrorWithContext>> {
        cmds.into_iter().filter(|cmd| match cmd {
                Ok(Command::FunctionCode(FunctionCode::GCode(GCode::Comment(_)))) => true, _ => false}).collect()};

    let test_vec: Vec<Result<Command, GerberParserErrorWithContext>> = vec![
        Ok(Command::FunctionCode(FunctionCode::GCode(GCode::Comment("Comment before typical configuration lines".to_string())))),
        Ok(Command::FunctionCode(FunctionCode::GCode(GCode::Comment("And now a comment after them".to_string()))))
    ];
    
    assert_eq!(filter_commands(parse_gerber(reader).commands), test_vec)
}

#[test]
fn aperture_selection() {
    let reader = utils::gerber_to_reader("
    %FSLAX23Y23*%
    %MOMM*%

    %ADD999C, 0.01*%
    %ADD22R, 0.01X0.15*%

    G04 Select some valid apertures*
    D22*
    D999*
    D22*

    M02*        
    ");

    let filter_commands = |cmds:Vec<Result<Command, GerberParserErrorWithContext>>| -> Vec<Result<Command, GerberParserErrorWithContext>> {
        cmds.into_iter().filter(|cmd| match cmd {
            Ok(Command::FunctionCode(FunctionCode::DCode(DCode::SelectAperture(_)))) => true, _ => false}).collect()};

    assert_eq!(filter_commands(parse_gerber(reader).commands), vec![
        Ok(Command::FunctionCode(FunctionCode::DCode(DCode::SelectAperture(22.into())))),
        Ok(Command::FunctionCode(FunctionCode::DCode(DCode::SelectAperture(999.into())))),
        Ok(Command::FunctionCode(FunctionCode::DCode(DCode::SelectAperture(22.into()))))])
}

#[test]
// Test the D01* statements (linear)
fn D01_interpolation_linear() {
    let reader = utils::gerber_to_reader("
    %FSLAX23Y23*%
    %MOMM*%

    %ADD999C, 0.01*%
    D999*

    X4000Y5000D01*
    X0Y0D01*
    X-1000Y-30000D01*

    M02*        
    ");

    let filter_commands = |cmds:Vec<Result<Command, GerberParserErrorWithContext>>| -> Vec<Result<Command, GerberParserErrorWithContext>> {
        cmds.into_iter().filter(|cmd| match cmd {
            Ok(Command::FunctionCode(FunctionCode::DCode(DCode::Operation(Operation::Interpolate(_, _))))) => true, _ => false}).collect()};

    let fs =  CoordinateFormat::new(2,3);
    assert_eq!(filter_commands(parse_gerber(reader).commands), vec![
        Ok(Command::FunctionCode(FunctionCode::DCode(DCode::Operation(Operation::Interpolate(
            coordinates_from_gerber(4000, 5000, fs), None))))),
        Ok(Command::FunctionCode(FunctionCode::DCode(DCode::Operation(Operation::Interpolate(
            coordinates_from_gerber(0, 0, fs), None))))),
        Ok(Command::FunctionCode(FunctionCode::DCode(DCode::Operation(Operation::Interpolate(
            coordinates_from_gerber(-1000, -30000, fs), None)))))])
}


#[test]
// Test the D01* statements (circular)
fn D01_interpolation_circular() {
    let reader = utils::gerber_to_reader("
    %FSLAX23Y23*%
    %MOMM*%

    %ADD999C, 0.01*%
    D999*

    X0Y0D01*
    X-1000Y-30000I200J-5000D01*

    M02*        
    ");

    let filter_commands = |cmds:Vec<Result<Command, GerberParserErrorWithContext>>| -> Vec<Result<Command, GerberParserErrorWithContext>> {
        cmds.into_iter().filter(|cmd| match cmd {
                Ok(Command::FunctionCode(FunctionCode::DCode(DCode::Operation(Operation::Interpolate(_, _))))) => true, _ => false}).collect()};

    let fs =  CoordinateFormat::new(2,3);
    assert_eq!(filter_commands(parse_gerber(reader).commands), vec![
        Ok(Command::FunctionCode(FunctionCode::DCode(DCode::Operation(Operation::Interpolate(
            coordinates_from_gerber(0, 0, fs), None))))),
        Ok(Command::FunctionCode(FunctionCode::DCode(DCode::Operation(Operation::Interpolate(
            coordinates_from_gerber(-1000, -30000, fs),
            Some(coordinates_offset_from_gerber(200, -5000, fs)))))))])
}

#[test]
// Test the D02* statements 
fn DO2_move_to_command() {
    let reader = utils::gerber_to_reader("
    %FSLAX23Y23*%
    %MOMM*%

    %ADD999C, 0.01*%
    D999*

    X0Y-333D02*
    X300Y300D01*

    X5555Y-12D02*
    X-300Y-300D01*

    M02*        
    ");

    let filter_commands = |cmds:Vec<Result<Command, GerberParserErrorWithContext>>| -> Vec<Result<Command, GerberParserErrorWithContext>> {
        cmds.into_iter().filter(|cmd| match cmd {
                Ok(Command::FunctionCode(FunctionCode::DCode(DCode::Operation(Operation::Move(_))))) => true, _ => false}).collect()};

    let fs =  CoordinateFormat::new(2,3);
    assert_eq!(filter_commands(parse_gerber(reader).commands), vec![
        Ok(Command::FunctionCode(FunctionCode::DCode(DCode::Operation(Operation::Move(
            coordinates_from_gerber(0, -333, fs)))))),
        Ok(Command::FunctionCode(FunctionCode::DCode(DCode::Operation(Operation::Move(
            coordinates_from_gerber(5555, -12, fs))))))])
}

#[test]
// Test the D03* statements 
fn DO3_flash_command() {
    let reader = utils::gerber_to_reader("
    %FSLAX23Y23*%
    %MOMM*%

    %ADD999C, 0.01*%
    D999*

    X4000Y-5000D03*
    X0Y0D03*

    M02*        
    ");

    let filter_commands = |cmds:Vec<Result<Command, GerberParserErrorWithContext>>| -> Vec<Result<Command, GerberParserErrorWithContext>> {
        cmds.into_iter().filter(|cmd| match cmd {
                Ok(Command::FunctionCode(FunctionCode::DCode(DCode::Operation(Operation::Flash(_))))) => true, _ => false}).collect()};

    let fs =  CoordinateFormat::new(2,3);
    assert_eq!(filter_commands(parse_gerber(reader).commands), vec![
        Ok(Command::FunctionCode(FunctionCode::DCode(DCode::Operation(Operation::Flash(
            coordinates_from_gerber(4000, -5000, fs)))))),
        Ok(Command::FunctionCode(FunctionCode::DCode(DCode::Operation(Operation::Flash(
            coordinates_from_gerber(0, 0, fs))))))])
}

#[test]
// Gerber spec allows for ommitted coordinates. This means that 'X100D03*' and 'Y100D03*' are
// valid statements.
fn omitted_coordinate() {
    let reader = utils::gerber_to_reader("
    %FSLAX23Y23*%
    %MOMM*%
    %ADD999C, 0.01*%    
    D999*

    G04 here the last coordinate is (0,0) - by construction*
    Y-3000D03*
    G04 Now we set X=1234, but we keep the last Y coordinate, namely -3000*
    X1234D03*

    M02*        
    ");

    let filter_commands = |cmds:Vec<Result<Command, GerberParserErrorWithContext>>| -> Vec<Result<Command, GerberParserErrorWithContext>> {
        cmds.into_iter().filter(|cmd| match cmd {
                Ok(Command::FunctionCode(FunctionCode::DCode(DCode::Operation(Operation::Flash(_))))) => true, _ => false}).collect()};

    let fs =  CoordinateFormat::new(2,3);
    assert_eq!(filter_commands(parse_gerber(reader).commands), vec![
        Ok(Command::FunctionCode(FunctionCode::DCode(DCode::Operation(Operation::Flash(
            coordinates_from_gerber(0, -3000, fs)))))),
        Ok(Command::FunctionCode(FunctionCode::DCode(DCode::Operation(Operation::Flash(
            coordinates_from_gerber(1234, -3000, fs))))))])
}


#[test]
// Test Step and Repeat command (%SR*%)
fn step_and_repeat() {
    let reader = utils::gerber_to_reader("
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
    ");

    let filter_commands = |cmds:Vec<Result<Command, GerberParserErrorWithContext>>| -> Vec<Result<Command, GerberParserErrorWithContext>> {
        cmds.into_iter().filter(|cmd| match cmd {
                Ok(Command::ExtendedCode(ExtendedCode::StepAndRepeat(_))) => true, _ => false}).collect()};

    let fs =  CoordinateFormat::new(2,3);
    assert_eq!(filter_commands(parse_gerber(reader).commands), vec![
        Ok(Command::ExtendedCode(ExtendedCode::StepAndRepeat(StepAndRepeat::Open{
            repeat_x: 12,
            repeat_y: 6,
            distance_x: 3.33,
            distance_y: 8.12,
        }))),
        Ok(Command::ExtendedCode(ExtendedCode::StepAndRepeat(StepAndRepeat::Close))),
    ])
}


#[test]
fn aperture_definitions() {
    let reader = utils::gerber_to_reader("
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
    ");

    assert_eq!(parse_gerber(reader).apertures,  HashMap::from([
        (999, Aperture::Circle(Circle {diameter: 0.01, hole_diameter: None})),
        (22, Aperture::Rectangle(Rectangular{x: 0.01,y: 0.15,hole_diameter: None})),
        (23, Aperture::Obround(Rectangular{x: 0.01,y: 0.15,hole_diameter: None})),
        (21, Aperture::Polygon(Polygon{diameter: 0.7,vertices: 10,rotation: None, hole_diameter: None})),
        (24, Aperture::Polygon(Polygon{diameter: 0.7,vertices: 10,rotation: Some(16.5),hole_diameter: None})),
        (123, Aperture::Circle(Circle {diameter: 0.01,hole_diameter: Some(0.003)})),
        (124, Aperture::Rectangle(Rectangular {x: 0.1,y: 0.15,hole_diameter: Some(0.00001)})),
        (125, Aperture::Obround(Rectangular {x: 0.1,y: 0.15,hole_diameter: Some(0.019)})),
        (126, Aperture::Polygon(Polygon{diameter: 1.0,vertices: 7,rotation: Some(5.5),hole_diameter: Some(0.7)})),
        ]))
}

#[test]
// TODO: make more exhaustive
fn TA_aperture_attributes() {
    let reader = utils::gerber_to_reader("
    %FSLAX23Y23*%
    %MOMM*%

    %ADD999C, 0.01*%
    %TA.AperFunction, WasherPad*%
    %TA.AperFunction,Profile*%
    %TA.AperFunction,   Other,   teststring*%

    %TA.DrillTolerance, 0.032, 0.022*%

    M02*        
    ");

    let filter_commands = |cmds:Vec<Result<Command, GerberParserErrorWithContext>>| -> Vec<Result<Command, GerberParserErrorWithContext>> {
        cmds.into_iter().filter(|cmd| match cmd {
            Ok(Command::ExtendedCode(ExtendedCode::ApertureAttribute(_))) => true, _ => false}).collect()};

    let fs =  CoordinateFormat::new(2,3);
    assert_eq!(filter_commands(parse_gerber(reader).commands), vec![
        Ok(Command::ExtendedCode(ExtendedCode::ApertureAttribute(ApertureAttribute::ApertureFunction(ApertureFunction::WasherPad)))),
        Ok(Command::ExtendedCode(ExtendedCode::ApertureAttribute(ApertureAttribute::ApertureFunction(ApertureFunction::Profile)))),
        Ok(Command::ExtendedCode(ExtendedCode::ApertureAttribute(ApertureAttribute::ApertureFunction(ApertureFunction::Other("teststring".to_string()))))),
        Ok(Command::ExtendedCode(ExtendedCode::ApertureAttribute(ApertureAttribute::DrillTolerance{ plus: 0.032, minus: 0.022 })))
        ])
}

#[test]
// TODO: make more exhaustive
fn TF_file_attributes() {
    let reader = utils::gerber_to_reader("
    %FSLAX23Y23*%
    %MOMM*%

    %ADD999C, 0.01*%

    %TF.Part, Array*%
    %TF.Part, Other, funnypartname*%
    %TF.FileFunction, test part*%
    %TF.FilePolarity, Negative*%

    M02*        
    ");

    let filter_commands = |cmds:Vec<Result<Command, GerberParserErrorWithContext>>| -> Vec<Result<Command, GerberParserErrorWithContext>> {
        cmds.into_iter().filter(|cmd| match cmd {
            Ok(Command::ExtendedCode(ExtendedCode::FileAttribute(_))) => true, _ => false}).collect()};

    let fs =  CoordinateFormat::new(2,3);
    assert_eq!(filter_commands(parse_gerber(reader).commands), vec![
        Ok(Command::ExtendedCode(ExtendedCode::FileAttribute(FileAttribute::Part(Part::Array)))),
        Ok(Command::ExtendedCode(ExtendedCode::FileAttribute(FileAttribute::Part(Part::Other("funnypartname".to_string()))))),
        Ok(Command::ExtendedCode(ExtendedCode::FileAttribute(FileAttribute::FileFunction(FileFunction::Other("test part".to_string()))))),
        Ok(Command::ExtendedCode(ExtendedCode::FileAttribute(FileAttribute::FilePolarity(FilePolarity::Negative)))),
        ])
}

// #[test]
// // TODO: make more exhaustive
// fn TO_object_attributes() {
//     let reader = utils::gerber_to_reader("
//     %FSLAX23Y23*%
//     %MOMM*%
// 
//     %ADD999C, 0.01*%
// 
//     %TO.DoNotForget, 32, fragile*%
//     %TO.N, 0.22*%
// 
//     M02*        
//     ");
// 
//     let filter_commands = |cmds:Vec<Command>| -> Vec<Command> {
//         cmds.into_iter().filter(|cmd| match cmd {
//                 Command::ExtendedCode(ExtendedCode::ObjectAttribute(_)) => true, _ => false}).collect()};
// 
//     let fs =  CoordinateFormat::new(2,3);
//     assert_eq!(filter_commands(parse_gerber(reader).commands), vec![
//         Command::ExtendedCode(ExtendedCode::ObjectAttribute(ObjectAttribute{
//             attribute_name: "DoNotForget".to_string(),
//             values: vec!["32".to_string(), "fragile".to_string()]
//         })),
//         Command::ExtendedCode(ExtendedCode::ObjectAttribute(ObjectAttribute{
//             attribute_name: "N".to_string(),
//             values: vec!["0.22".to_string()]
//         }))])
// }

#[test]
#[should_panic]
fn conflicting_aperture_codes() {
    let reader = utils::gerber_to_reader("
    %FSLAX23Y23*%
    %MOMM*%        
    %ADD24P, 0.7X10X16.5*%
    %ADD39P, 0.7X10X16.5*%
    G04 We cannot use the same code (24) again in the same document*
    %ADD24P, 1X10X20.0*%

    M02*      
    ");
    let guy = parse_gerber(reader);
    assert!(guy.get_errors().is_empty());
}

#[test]
#[should_panic]
fn missing_eof() {
    let reader = utils::gerber_to_reader("
    %FSLAX23Y23*%
    %MOMM*%-

    G04 We should have a MO2 at the end, but what if we forget it?*      
    ");
    let guy = parse_gerber(reader);
    assert!(guy.get_errors().is_empty());
}

#[test]
#[should_panic]
fn multiple_unit_statements() {
    let reader = utils::gerber_to_reader("
    %FSLAX23Y23*%
    %MOMM*%
    G04 We can only declare the unit type once in a document* 
    %MOIN*%
        
    M02*  
    ");
    let guy = parse_gerber(reader);
    assert!(guy.get_errors().is_empty());
}

#[test]
#[should_panic]
fn multiple_fs_statements() {
    let reader = utils::gerber_to_reader("
    %FSLAX23Y23*%
    G04 We can only declare the format specification once in a document* 
    %FSLAX46Y46*%
    %MOMM*%
        
    M02*  
    ");
    let guy = parse_gerber(reader);
    assert!(guy.get_errors().is_empty());
}

#[test]
#[should_panic]
fn nonexistent_aperture_selection() {
    let reader = utils::gerber_to_reader("
    %FSLAX23Y23*%        
    %MOMM*%

    %ADD100P, 0.7X10X16.5*%

    G04 We should not be able to select apertures that are not defined* 
    D
        
    M02*  
    ");
    let guy = parse_gerber(reader);
    assert!(guy.get_errors().is_empty());
}

#[test]
#[ignore]
#[should_panic]
// This statement should fail as this is not within the format specification (2 integer, 3 decimal)
fn coordinates_not_within_format() {
    let reader = utils::gerber_to_reader("
    %FSLAX23Y23*%
    %MOMM*%

    %ADD999C, 0.01*%
    D999*

    X100000Y0D01*

    M02*        
    ");

    let guy = parse_gerber(reader);
    assert!(guy.get_errors().is_empty());
}


#[test]
// Test the D* statements, diptrace exports gerber files without the leading `0` on the `D0*` commands. 
fn diptrace_Dxx_statements() {
    let reader = utils::gerber_to_reader(r#"
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

    "#);

    // when
    let result = parse_gerber(reader).commands;
    
    // then
    let filter_commands = |cmds:Vec<Result<Command, GerberParserErrorWithContext>>| -> Vec<Result<Command, GerberParserErrorWithContext>> {
        cmds.into_iter().filter(|cmd| match cmd {
            Ok(Command::FunctionCode(FunctionCode::DCode(DCode::Operation(Operation::Move(_))))) => true,
            Ok(Command::FunctionCode(FunctionCode::DCode(DCode::Operation(Operation::Interpolate(_, _))))) => true,
            _ => false}
        ).collect()};
    println!("{:?}", result);
    
    let filtered_commands = filter_commands(result);
    let fs =  CoordinateFormat::new(3,5);

    assert_eq!(filtered_commands, vec![
        Ok(Command::FunctionCode(FunctionCode::DCode(DCode::Operation(Operation::Move(coordinates_from_gerber(2928500, 1670000, fs)))))),
        Ok(Command::FunctionCode(FunctionCode::DCode(DCode::Operation(Operation::Interpolate(coordinates_from_gerber(2998500, 1670000, fs), None))))),
        Ok(Command::FunctionCode(FunctionCode::DCode(DCode::Operation(Operation::Interpolate(coordinates_from_gerber(2998500, 1530000, fs), None))))),
    ]);
}

// See gerber spec 2021-02, section 4.5
#[test]
fn test_outline_macro_definition() {
    // given
    let reader = utils::gerber_to_reader(r#"
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
    "#);

    // and
    let expected_macro: ApertureMacro = ApertureMacro {
        name: "OUTLINE0".to_string(),
        content: vec![MacroContent::Outline(OutlinePrimitive {
            exposure: true,  // 1 indicates exposure on
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
    let result = parse_gerber(reader).commands;

    // then
    let filter_commands = |cmds:Vec<Result<Command, GerberParserErrorWithContext>>| -> Vec<Result<Command, GerberParserErrorWithContext>> {
        cmds.into_iter().filter(|cmd| match cmd {
            Ok(Command::ExtendedCode(ExtendedCode::ApertureMacro(_))) => true,
            _ => false}
        ).collect()};
    println!("{:?}", result);

    let filtered_commands = filter_commands(result);

    assert_eq!(filtered_commands, vec![
        Ok(Command::ExtendedCode(ExtendedCode::ApertureMacro(expected_macro))),
    ]);

}
