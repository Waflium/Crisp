use super::*;

pub fn snbt(input: &str) -> String {
    let units: Vec<u16> = input.encode_utf16().collect();
    let mut reader = StringReader::new(&units);
    match parse_as_argument(&mut reader) {
        Ok(tag) => format!("TAG {} @{}", to_string(&tag), reader.cursor),
        Err(e) => error(&e),
    }
}

pub fn error(e: &CommandSyntaxException) -> String {
    let mut out = format!("ERR {}", e.message.get_string());
    if let Some(input) = &e.input
        && e.cursor >= 0
    {
        let input: Vec<u16> = input.encode_utf16().collect();
        let cursor = input.len().min(e.cursor as usize);
        out.push_str(" | CTX ");
        if cursor > 10 {
            out.push_str("...");
        }
        out.push_str(&String::from_utf16_lossy(&input[cursor.saturating_sub(10)..]));
        out.push_str("<--[HERE]");
    }
    out
}

fn check(cases: &[(&str, &str)], run: impl Fn(&str) -> String) {
    let failures: Vec<String> = cases
        .iter()
        .filter_map(|(input, expected)| {
            let actual = run(input);
            (actual != *expected).then(|| format!("{input}\n  vanilla: {expected}\n  crisp:   {actual}"))
        })
        .collect();
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn snbt_matches_vanilla() {
    check(
        &[
            ("99999999999", "ERR Expected literal . | CTX ...9999999999<--[HERE]"),
            ("300b", "ERR Failed to parse number: Value out of range. Value:\"300\" Radix:10 | CTX 300b<--[HERE]"),
            ("300ub", "ERR Failed to parse number: out of range: 300 | CTX 300ub<--[HERE]"),
            ("70000us", "ERR Failed to parse number: out of range: 70000 | CTX 70000us<--[HERE]"),
            ("99999999999ui", "ERR Failed to parse number: String value 99999999999 exceeds range of unsigned int. | CTX ...99999999ui<--[HERE]"),
            ("99999999999999999999ul", "ERR Failed to parse number: String value 99999999999999999999 exceeds range of unsigned long. | CTX ...99999999ul<--[HERE]"),
            ("0x1FFFFFFFF", "ERR Expected literal u|U | CTX ...x1FFFFFFFF<--[HERE]"),
            ("0xFFFFFFFF", "TAG -1 @10"),
            ("-0x10", "ERR Expected literal u|U | CTX -0x10<--[HERE]"),
            ("-0x10sb", "TAG -16b @7"),
            ("0b102", "TAG 2 @4"),
            ("0b", "TAG 0b @2"),
            ("0x", "ERR Expected a hexadecimal number | CTX 0x<--[HERE]"),
            ("01", "ERR Expected literal . | CTX 01<--[HERE]"),
            ("1_000", "TAG 1000 @5"),
            ("1__0", "TAG 10 @4"),
            ("_1", "TAG \"_1\" @2"),
            ("1_", "ERR Expected literal ( | CTX 1_<--[HERE]"),
            ("1e999", "TAG 1 @1"),
            ("1e999f", "TAG 1 @1"),
            ("1.5e3", "TAG 1500.0d @5"),
            (".5", "TAG 0.5d @2"),
            ("5.", "TAG 5.0d @2"),
            ("1f", "TAG 1.0f @2"),
            ("1d", "TAG 1.0d @2"),
            ("1e5", "TAG 100000.0d @3"),
            ("1 b", "TAG 1b @3"),
            ("- 5", "TAG -5 @3"),
            ("+5", "TAG 5 @2"),
            ("1x", "TAG 1 @1"),
            ("\"abc", "ERR Invalid string contents | CTX \"abc<--[HERE]"),
            ("'abc", "ERR Invalid string contents | CTX 'abc<--[HERE]"),
            ("\"a\\qb\"", "ERR Expected literal b | CTX \"a\\qb\"<--[HERE]"),
            ("\"\\x4\"", "ERR Expected a character literal of length 2 | CTX \"\\x4\"<--[HERE]"),
            ("\"a\\ n\"", "TAG \"a\\n\" @6"),
            ("{a:1,a:2}", "TAG {a:2} @9"),
            ("{\"\":1}", "ERR Expected literal . | CTX {\"\":1}<--[HERE]"),
            ("{a:1,}", "TAG {a:1} @6"),
            ("{,}", "ERR Expected literal \" | CTX {,}<--[HERE]"),
            ("[1,2,]", "TAG [1,2] @6"),
            ("[B;1b,2b]", "TAG [B;1B,2B] @9"),
            ("[B;1,2]", "TAG [B;1B,2B] @7"),
            ("[I;1b,2s,3]", "TAG [I;1,2,3] @11"),
            ("[I;1L]", "ERR Invalid array element type | CTX [I;1L]<--[HERE]"),
            ("[L;1,2b]", "TAG [L;1L,2L] @8"),
            ("[B;300]", "ERR Failed to parse number: Value out of range. Value:\"300\" Radix:10 | CTX [B;300]<--[HERE]"),
            ("[B;]", "TAG [B;] @4"),
            ("[B; 1]", "TAG [B;1B] @6"),
            ("[1,\"a\"]", "TAG [1,\"a\"] @7"),
            ("[{a:1},2]", "TAG [{a:1},2] @9"),
            ("bool(1)", "TAG 1b @7"),
            ("bool(\"x\")", "ERR Expected a number or a boolean | CTX bool(\"x\")<--[HERE]"),
            ("bool(0.0)", "TAG 0b @9"),
            ("uuid(\"f81d4fae-7dec-11d0-a765-00a0c91e6bf6\")", "TAG [I;-132296786,2112623056,-1486552928,-920753162] @44"),
            ("uuid(1)", "ERR Expected a string representing a valid UUID | CTX uuid(1)<--[HERE]"),
            ("foo(1)", "ERR No such operation: foo/1 | CTX foo(1)<--[HERE]"),
            ("bool(1", "TAG 1b @4"),
            ("TRUE", "TAG 1b @4"),
            ("false", "TAG 0b @5"),
            ("{a:b c}", "ERR Expected literal ( | CTX {a:b c}<--[HERE]"),
            ("{a b}", "ERR Expected literal : | CTX {a b}<--[HERE]"),
            ("{", "ERR Expected literal \" | CTX {<--[HERE]"),
            ("[", "ERR Expected literal B | CTX [<--[HERE]"),
            ("\"", "ERR Invalid string contents | CTX \"<--[HERE]"),
            ("'", "ERR Invalid string contents | CTX '<--[HERE]"),
            ("1.0E10", "TAG 1.0E10d @6"),
            ("1.0e-5", "TAG 1.0E-5d @6"),
            ("123456789.0", "TAG 1.23456789E8d @11"),
            ("0.001", "TAG 0.001d @5"),
            ("1e7", "TAG 1.0E7d @3"),
            ("3.4028235e38f", "TAG 3.4028235E38f @13"),
            ("1e-50f", "TAG 0.0f @6"),
            ("{a:\"\\\"\",b:'\\'',c:'\"'}", "TAG {a:'\"',b:\"'\",c:'\"'} @21"),
            ("{\"a b\":1, \"true\":2, Abc:3, _x:4, a.b:5}", "TAG {Abc:3,_x:4,\"a b\":1,a.b:5,\"true\":2} @39"),
            ("-0", "TAG 0 @2"),
            ("-0.0", "TAG 0.0d @4"),
            ("0u", "TAG 0 @1"),
            ("0ul", "TAG 0L @3"),
            ("-1ub", "ERR Expected a non-negative number | CTX -1ub<--[HERE]"),
            ("255ub", "TAG -1b @5"),
            ("0xFFub", "TAG -1b @6"),
            ("0x7Fsb", "TAG 127b @6"),
            ("0x80sb", "ERR Failed to parse number: Value out of range. Value:\"80\" Radix:16 | CTX 0x80sb<--[HERE]"),
            ("1sl", "TAG 1L @3"),
            ("1.5b", "TAG 1.5d @3"),
            ("[I;1.5]", "ERR Expected literal u|U | CTX [I;1.5]<--[HERE]"),
            ("{a:[B;-1b]}", "TAG {a:[B;-1B]} @11"),
            ("99999999999i", "ERR Failed to parse number: For input string: \"99999999999\" | CTX ...999999999i<--[HERE]"),
            ("99999999999L", "TAG 99999999999L @12"),
            ("99999999999999999999L", "ERR Failed to parse number: For input string: \"99999999999999999999\" | CTX ...999999999L<--[HERE]"),
            ("0x1FFFFFFFFsi", "ERR Failed to parse number: For input string: \"1FFFFFFFF\" under radix 16 | CTX ...FFFFFFFFsi<--[HERE]"),
            ("0x1FFFFFFFFFFFFFFFFFFui", "ERR Failed to parse number: String value 1FFFFFFFFFFFFFFFFFF exceeds range of unsigned int. | CTX ...FFFFFFFFui<--[HERE]"),
            ("0x1FFFFFFFFui", "ERR Failed to parse number: String value 1FFFFFFFF exceeds range of unsigned int. | CTX ...FFFFFFFFui<--[HERE]"),
            ("123456789012345678901234567890ul", "ERR Failed to parse number: String value 123456789012345678901234567890 exceeds range of unsigned long. | CTX ...34567890ul<--[HERE]"),
            ("0xFFFFFFFFFFFFFFFFFul", "ERR Failed to parse number: String value FFFFFFFFFFFFFFFFF exceeds range of unsigned long. | CTX ...FFFFFFFFul<--[HERE]"),
            ("0xFFFFFFFFFFFFFFFFul", "TAG -1L @20"),
            ("18446744073709551616ul", "ERR Failed to parse number: String value 18446744073709551616 exceeds range of unsigned long. | CTX ...09551616ul<--[HERE]"),
            ("18446744073709551615ul", "TAG -1L @22"),
            ("4294967296ui", "ERR Failed to parse number: String value 4294967296 exceeds range of unsigned int. | CTX ...94967296ui<--[HERE]"),
            ("4294967295ui", "TAG -1 @12"),
            ("0b11111111111111111111111111111111111ui", "ERR Failed to parse number: String value 11111111111111111111111111111111111 exceeds range of unsigned int. | CTX ...11111111ui<--[HERE]"),
            ("0b11111111111111111111111111111111ui", "TAG -1 @36"),
            ("-2147483648", "TAG -2147483648 @11"),
            ("-2147483649", "ERR Expected literal . | CTX ...2147483649<--[HERE]"),
            ("0x80000000si", "ERR Failed to parse number: For input string: \"80000000\" under radix 16 | CTX ...80000000si<--[HERE]"),
            ("-0x80000000si", "TAG -2147483648 @13"),
            ("-0x80sb", "TAG -128b @7"),
            ("0x_1", "ERR Expected literal ( | CTX 0x_1<--[HERE]"),
            ("[B;1_]", "ERR Expected literal + | CTX [B;1_]<--[HERE]"),
            ("1_000_", "ERR Expected literal ( | CTX 1_000_<--[HERE]"),
            ("0b_1", "TAG 0b @2"),
            ("{a:1 b}", "TAG {a:1b} @7"),
            ("{a:1b,c}", "ERR Expected literal : | CTX {a:1b,c}<--[HERE]"),
            ("[B;1,2L]", "ERR Invalid array element type | CTX [B;1,2L]<--[HERE]"),
            ("[L;1ub,-1sb]", "TAG [L;1L,-1L] @12"),
            ("[I;-1ub]", "ERR Expected a non-negative number | CTX [I;-1ub]<--[HERE]"),
            ("{a:'b\\''}", "TAG {a:\"b'\"} @9"),
            ("''", "TAG \"\" @2"),
            ("\"\"", "TAG \"\" @2"),
            ("{\"a\":1}", "TAG {a:1} @7"),
            ("{'a':1}", "TAG {a:1} @7"),
            ("{ a : 1 }", "TAG {a:1} @9"),
            ("[ 1 , 2 ]", "TAG [1,2] @9"),
            ("[B ; 1]", "TAG [B;1B] @7"),
            ("[B;1,]", "TAG [B;1B] @6"),
            ("[;]", "ERR Expected literal B | CTX [;]<--[HERE]"),
            ("[B,1]", "TAG [\"B\",1] @5"),
            ("bool(true)", "TAG 1b @10"),
            ("bool()", "ERR No such operation: bool/0 | CTX bool()<--[HERE]"),
            ("bool(1,2)", "ERR No such operation: bool/2 | CTX bool(1,2)<--[HERE]"),
            ("uuid()", "ERR No such operation: uuid/0 | CTX uuid()<--[HERE]"),
            ("e5", "TAG \"e5\" @2"),
            ("1e", "TAG 1 @1"),
            ("1.e5", "TAG 100000.0d @4"),
            ("1.5e+3f", "TAG 1500.0f @7"),
            ("-.5", "TAG -0.5d @3"),
            ("+.5f", "TAG 0.5f @4"),
            ("1.0.0", "TAG 1.0d @3"),
            ("1..", "TAG 1.0d @2"),
            ("0.0f", "TAG 0.0f @4"),
            ("1.000000001f", "TAG 1.0f @12"),
            ("1E-400", "TAG 0.0d @6"),
            ("1e309d", "TAG 1 @1"),
            ("\"\\x41\\u0042\\U00000043\"", "TAG \"ABC\" @22"),
            ("\"\\ud83d\\ude00\"", "TAG \"\u{1F600}\" @14"),
            ("\"\\U99999999\"", "ERR Invalid Unicode character value: U+99999999 | CTX ...\\U99999999\"<--[HERE]"),
        ],
        snbt,
    );
}

#[test]
fn parse_fully_rejects_trailing_data() {
    assert_eq!(parse_fully(" {a:1} ").unwrap(), Nbt::Compound(vec![("a".into(), Nbt::Int(1))]));
    assert_eq!(error(&parse_fully("{a:1} x").unwrap_err()), "ERR Unexpected trailing data | CTX {a:1} x<--[HERE]");
}
